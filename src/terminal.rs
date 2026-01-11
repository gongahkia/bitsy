// Terminal initialization and rendering

use crossterm::{
    cursor,
    event::{self, Event},
    execute,
    style::{self, Color},
    terminal::{self, ClearType},
};
use std::io::{self, Write};

use crate::error::Result;

pub struct Terminal {
    width: u16,
    height: u16,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen)?;
        execute!(io::stdout(), cursor::Hide)?;

        let (width, height) = terminal::size()?;

        Ok(Self { width, height })
    }

    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub fn clear_screen(&self) -> Result<()> {
        execute!(io::stdout(), terminal::Clear(ClearType::All))?;
        Ok(())
    }

    pub fn clear_line(&self) -> Result<()> {
        execute!(io::stdout(), terminal::Clear(ClearType::CurrentLine))?;
        Ok(())
    }

    pub fn move_cursor(&self, x: u16, y: u16) -> Result<()> {
        execute!(io::stdout(), cursor::MoveTo(x, y))?;
        Ok(())
    }

    pub fn show_cursor(&self) -> Result<()> {
        execute!(io::stdout(), cursor::Show)?;
        Ok(())
    }

    pub fn hide_cursor(&self) -> Result<()> {
        execute!(io::stdout(), cursor::Hide)?;
        Ok(())
    }

    pub fn flush(&self) -> Result<()> {
        io::stdout().flush()?;
        Ok(())
    }

    pub fn print(&self, text: &str) -> Result<()> {
        print!("{}", text);
        Ok(())
    }

    pub fn print_colored(&self, text: &str, color: Color) -> Result<()> {
        execute!(io::stdout(), style::SetForegroundColor(color))?;
        print!("{}", text);
        execute!(io::stdout(), style::ResetColor)?;
        Ok(())
    }

    pub fn read_event(&self) -> Result<Option<Event>> {
        if event::poll(std::time::Duration::from_millis(100))? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    pub fn update_size(&mut self) -> Result<()> {
        let (width, height) = terminal::size()?;
        self.width = width;
        self.height = height;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show);
        let _ = execute!(io::stdout(), terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
