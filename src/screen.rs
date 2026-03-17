// double-buffered cell grid for differential rendering

use crossterm::style::Color;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
}

impl Cell {
    pub fn new(ch: char, fg: Color, bg: Color) -> Self {
        Self { ch, fg, bg, bold: false }
    }
    pub fn blank(bg: Color) -> Self {
        Self { ch: ' ', fg: Color::Reset, bg, bold: false }
    }
}

impl Default for Cell {
    fn default() -> Self { Self::blank(Color::Reset) }
}

pub struct Screen {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Vec<Cell>>,
    prev: Vec<Vec<Cell>>,
    force_redraw: bool,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Self {
        let blank_row = || vec![Cell::default(); width];
        let cells: Vec<Vec<Cell>> = (0..height).map(|_| blank_row()).collect();
        let prev: Vec<Vec<Cell>> = (0..height).map(|_| blank_row()).collect();
        Self { width, height, cells, prev, force_redraw: true }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        let blank_row = || vec![Cell::default(); width];
        self.cells = (0..height).map(|_| blank_row()).collect();
        self.prev = (0..height).map(|_| blank_row()).collect();
        self.force_redraw = true;
    }

    pub fn clear(&mut self, bg: Color) {
        for row in &mut self.cells {
            for cell in row.iter_mut() {
                *cell = Cell::blank(bg);
            }
        }
    }

    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        if row < self.height && col < self.width {
            self.cells[row][col] = cell;
        }
    }

    pub fn put_char(&mut self, row: usize, col: usize, ch: char, fg: Color, bg: Color) {
        self.set(row, col, Cell::new(ch, fg, bg));
    }

    pub fn put_char_bold(&mut self, row: usize, col: usize, ch: char, fg: Color, bg: Color) {
        if row < self.height && col < self.width {
            self.cells[row][col] = Cell { ch, fg, bg, bold: true };
        }
    }

    pub fn put_str(&mut self, row: usize, start_col: usize, text: &str, fg: Color, bg: Color) {
        for (i, ch) in text.chars().enumerate() {
            let col = start_col + i;
            if col >= self.width { break; }
            self.put_char(row, col, ch, fg, bg);
        }
    }

    pub fn put_str_bold(&mut self, row: usize, start_col: usize, text: &str, fg: Color, bg: Color) {
        for (i, ch) in text.chars().enumerate() {
            let col = start_col + i;
            if col >= self.width { break; }
            self.put_char_bold(row, col, ch, fg, bg);
        }
    }

    /// flush diff to terminal -- only emits changed cells
    pub fn flush(&mut self) -> io::Result<()> {
        use crossterm::{cursor, queue, style};
        let stdout = &mut io::stdout();
        let mut last_fg = Color::Reset;
        let mut last_bg = Color::Reset;
        let mut last_bold = false;
        let mut need_move = true;
        let mut prev_row: usize = 0;
        let mut prev_col: usize = 0;

        for row in 0..self.height {
            for col in 0..self.width {
                let cell = &self.cells[row][col];
                let prev_cell = &self.prev[row][col];
                if !self.force_redraw && cell == prev_cell { // skip unchanged
                    need_move = true;
                    continue;
                }
                if need_move || row != prev_row || col != prev_col + 1 {
                    queue!(stdout, cursor::MoveTo(col as u16, row as u16))?;
                    need_move = false;
                }
                if cell.fg != last_fg {
                    queue!(stdout, style::SetForegroundColor(cell.fg))?;
                    last_fg = cell.fg;
                }
                if cell.bg != last_bg {
                    queue!(stdout, style::SetBackgroundColor(cell.bg))?;
                    last_bg = cell.bg;
                }
                if cell.bold != last_bold {
                    if cell.bold {
                        queue!(stdout, style::SetAttribute(style::Attribute::Bold))?;
                    } else {
                        queue!(stdout, style::SetAttribute(style::Attribute::NoBold))?;
                    }
                    last_bold = cell.bold;
                }
                queue!(stdout, style::Print(cell.ch))?;
                prev_row = row;
                prev_col = col;
            }
        }
        queue!(stdout, style::ResetColor)?;
        if last_bold {
            queue!(stdout, style::SetAttribute(style::Attribute::NoBold))?;
        }
        stdout.flush()?;

        // swap buffers
        std::mem::swap(&mut self.prev, &mut self.cells);
        // clear current to blank for next frame
        for row in &mut self.cells {
            for cell in row.iter_mut() { *cell = Cell::default(); }
        }
        self.force_redraw = false;
        Ok(())
    }

    pub fn force_redraw(&mut self) { self.force_redraw = true; }
}
