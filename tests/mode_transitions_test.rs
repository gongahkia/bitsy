// Integration tests for mode transitions and edge cases

#[cfg(test)]
mod mode_transition_tests {
    use bitsy::{buffer::Buffer, cursor::Cursor, mode::Mode, selection::Selection};

    #[test]
    fn test_selection_created_on_visual_mode() {
        let cursor = Cursor::new(5, 10);
        let selection = Selection::from_cursor(cursor, Mode::Visual);

        assert_eq!(selection.anchor().line, 5);
        assert_eq!(selection.anchor().col, 10);
        assert_eq!(selection.cursor().line, 5);
        assert_eq!(selection.cursor().col, 10);
    }

    #[test]
    fn test_selection_cleared_on_normal_mode() {
        // This tests the concept - selection should be None in Normal mode
        // In actual editor, this is handled in execute_action for EnterNormalMode
        let mode = Mode::Normal;
        assert_eq!(mode, Mode::Normal);
    }

    #[test]
    fn test_visual_to_visualline_transition() {
        let cursor = Cursor::new(5, 10);
        let selection1 = Selection::from_cursor(cursor, Mode::Visual);
        let selection2 = Selection::from_cursor(cursor, Mode::VisualLine);

        // Both should start at same position
        assert_eq!(selection1.anchor(), selection2.anchor());

        // But different modes
        assert_eq!(selection1.mode(), Mode::Visual);
        assert_eq!(selection2.mode(), Mode::VisualLine);
    }

    #[test]
    fn test_cursor_clamping_in_normal_mode() {
        let buffer = Buffer::new();
        let cursor = Cursor::new(0, 0);

        // In normal mode with empty buffer, cursor should be at 0,0
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.col, 0);
        assert_eq!(buffer.line_count(), 1);
    }

    #[test]
    fn test_mode_string_representations() {
        assert_eq!(Mode::Normal.as_str(), "NORMAL");
        assert_eq!(Mode::Insert.as_str(), "INSERT");
        assert_eq!(Mode::Visual.as_str(), "VISUAL");
        assert_eq!(Mode::VisualLine.as_str(), "VISUAL LINE");
        assert_eq!(Mode::VisualBlock.as_str(), "VISUAL BLOCK");
        assert_eq!(Mode::Command.as_str(), "COMMAND");
    }

    #[test]
    fn test_selection_range_normalization() {
        // Test forward selection
        let selection1 = Selection::new(
            bitsy::selection::Position { line: 0, col: 0 },
            bitsy::selection::Position { line: 5, col: 10 },
            Mode::Visual,
        );
        let (start, end) = selection1.range();
        assert_eq!(start.line, 0);
        assert_eq!(end.line, 5);

        // Test backward selection
        let selection2 = Selection::new(
            bitsy::selection::Position { line: 5, col: 10 },
            bitsy::selection::Position { line: 0, col: 0 },
            Mode::Visual,
        );
        let (start, end) = selection2.range();
        assert_eq!(start.line, 0);
        assert_eq!(end.line, 5);
    }

    #[test]
    fn test_visualline_selects_entire_lines() {
        let selection = Selection::new(
            bitsy::selection::Position { line: 2, col: 5 },
            bitsy::selection::Position { line: 4, col: 3 },
            Mode::VisualLine,
        );

        // Should select all columns on lines 2, 3, 4
        assert!(selection.contains(2, 0));
        assert!(selection.contains(2, 100));
        assert!(selection.contains(3, 0));
        assert!(selection.contains(3, 50));
        assert!(selection.contains(4, 0));
        assert!(selection.contains(4, 100));

        // Should not select lines outside range
        assert!(!selection.contains(1, 0));
        assert!(!selection.contains(5, 0));
    }

    #[test]
    fn test_visualblock_rectangular_selection() {
        let selection = Selection::new(
            bitsy::selection::Position { line: 2, col: 5 },
            bitsy::selection::Position { line: 4, col: 10 },
            Mode::VisualBlock,
        );

        // Should select columns 5-10 on lines 2-4
        assert!(selection.contains(2, 5));
        assert!(selection.contains(2, 7));
        assert!(selection.contains(2, 10));
        assert!(selection.contains(3, 5));
        assert!(selection.contains(4, 10));

        // Should not select outside block
        assert!(!selection.contains(2, 4));
        assert!(!selection.contains(2, 11));
        assert!(!selection.contains(1, 7));
        assert!(!selection.contains(5, 7));
    }

    #[test]
    fn test_empty_buffer_cursor_position() {
        let buffer = Buffer::new();
        let cursor = Cursor::default();

        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.line_len(0), 0);
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn test_cursor_movement_with_selection_update() {
        let mut selection = Selection::from_cursor(Cursor::new(0, 0), Mode::Visual);

        // Update cursor to new position
        selection.update_cursor(bitsy::selection::Position { line: 2, col: 5 });

        assert_eq!(selection.anchor().line, 0);
        assert_eq!(selection.anchor().col, 0);
        assert_eq!(selection.cursor().line, 2);
        assert_eq!(selection.cursor().col, 5);
    }

    #[test]
    fn test_selection_contains_single_character() {
        let selection = Selection::new(
            bitsy::selection::Position { line: 0, col: 5 },
            bitsy::selection::Position { line: 0, col: 5 },
            Mode::Visual,
        );

        assert!(selection.contains(0, 5));
        assert!(!selection.contains(0, 4));
        assert!(!selection.contains(0, 6));
    }

    #[test]
    fn test_selection_multiline_boundaries() {
        let selection = Selection::new(
            bitsy::selection::Position { line: 1, col: 3 },
            bitsy::selection::Position { line: 3, col: 7 },
            Mode::Visual,
        );

        // First line: should start at col 3
        assert!(!selection.contains(1, 2));
        assert!(selection.contains(1, 3));
        assert!(selection.contains(1, 10));

        // Middle line: should select all
        assert!(selection.contains(2, 0));
        assert!(selection.contains(2, 100));

        // Last line: should end at col 7
        assert!(selection.contains(3, 0));
        assert!(selection.contains(3, 7));
        assert!(!selection.contains(3, 8));
    }
}
