// Window management for split panes

use crate::cursor::Cursor;
use crate::viewport::Viewport;

#[derive(Debug)]
pub struct Window {
    pub buffer_index: usize,
    pub cursor: Cursor,
    pub viewport: Viewport,
}

impl Window {
    pub fn new(buffer_index: usize, width: usize, height: usize) -> Self {
        Self {
            buffer_index,
            cursor: Cursor::default(),
            viewport: Viewport::new(width, height),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Layout {
    Leaf(usize),             // Window index
    Horizontal(Vec<Layout>), // Split vertically (side-by-side)
    Vertical(Vec<Layout>),   // Split horizontally (top-bottom)
}

impl Layout {
    pub fn new_leaf(window_index: usize) -> Self {
        Layout::Leaf(window_index)
    }
}
