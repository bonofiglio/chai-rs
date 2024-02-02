use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{
        disable_raw_mode, enable_raw_mode, window_size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ropey::Rope;

use crate::{buffer::Buffer, Cursor};
use std::{
    io::{self, Write},
    process::exit,
};

pub struct TermSize {
    width: usize,
    height: usize,
}

pub struct Chai {
    pub writer: io::Stdout,
    pub buffers: Vec<Buffer>,
    pub current_buffer_index: usize,
    pub window_size: TermSize,
}

impl Drop for Chai {
    fn drop(&mut self) {
        self.restore_terminal().unwrap();
    }
}

impl Chai {
    pub fn new(file_path: Option<Box<str>>) -> anyhow::Result<Self> {
        let content = match file_path {
            Some(ref path) => String::from_utf8(std::fs::read(path.as_ref())?)?
                .lines()
                .map(Rope::from)
                .collect(),
            None => vec![Rope::new()],
        };

        Ok(Chai {
            writer: io::stdout(),
            buffers: vec![Buffer {
                file_path,
                cursor: Cursor { x: 0, y: 0 },
                dirty: false,
                content,
                offset: (0, 0),
            }],
            current_buffer_index: 0,
            window_size: TermSize {
                width: 0,
                height: 0,
            },
        })
    }

    pub fn start(mut self) -> anyhow::Result<()> {
        self.setup_terminal()?;

        let size = window_size()?;

        self.window_size = TermSize {
            width: (size.columns as usize).saturating_sub(1),
            height: size.rows as usize,
        };

        self.clear()?;
        self.render()?;
        queue!(self.writer, cursor::MoveTo(0, 0))?;
        self.writer.flush()?;

        let result = self.run_loop();

        self.restore_terminal()?;

        result
    }

    fn run_loop(&mut self) -> anyhow::Result<()> {
        while let Ok(event) = read() {
            self.clear()?;
            self.handle_event(event)?;

            self.scroll()?;

            self.render()?;

            let term_cursor_pos = self.get_term_cursor_pos()?;

            queue!(
                self.writer,
                cursor::MoveTo(term_cursor_pos.0 as u16, term_cursor_pos.1 as u16)
            )?;

            self.writer.flush()?;
        }

        Ok(())
    }

    fn get_current_buffer(&self) -> anyhow::Result<&Buffer> {
        self.buffers
            .get(self.current_buffer_index)
            .ok_or(anyhow::anyhow!("No buffer found"))
    }

    fn get_current_buffer_mut(&mut self) -> anyhow::Result<&mut Buffer> {
        self.buffers
            .get_mut(self.current_buffer_index)
            .ok_or(anyhow::anyhow!("No buffer found"))
    }

    fn get_line_at(&self, index: usize) -> anyhow::Result<&Rope> {
        let buffer = self.get_current_buffer()?;
        let line = buffer
            .content
            .get(index)
            .ok_or(anyhow::anyhow!("No line at index {}", index))?;

        Ok(line)
    }

    fn get_line_at_mut(&mut self, index: usize) -> anyhow::Result<&mut Rope> {
        let buffer = self.get_current_buffer_mut()?;
        let line = buffer
            .content
            .get_mut(index)
            .ok_or(anyhow::anyhow!("No line at index {}", index))?;

        Ok(line)
    }

    fn get_cursor_pos(&self) -> anyhow::Result<(usize, usize)> {
        let buffer = self.get_current_buffer()?;
        let raw_pos = buffer.cursor.get_pos();

        let line = self.get_line_at(raw_pos.1)?;

        let x = raw_pos.0.min(line.len_chars());

        Ok((x, raw_pos.1))
    }

    fn get_term_cursor_pos(&self) -> anyhow::Result<(usize, usize)> {
        let (x, y) = self.get_cursor_pos()?;
        let buffer = self.get_current_buffer()?;

        let x = x.saturating_sub(buffer.offset.0);
        let y = y.saturating_sub(buffer.offset.1);

        Ok((x, y))
    }

    fn clear(&mut self) -> io::Result<()> {
        queue!(self.writer, Clear(ClearType::All))
    }

    fn render(&mut self) -> anyhow::Result<()> {
        queue!(self.writer, cursor::MoveTo(0, 0))?;
        let Some(buffer) = self.buffers.get(self.current_buffer_index) else {
            return Ok(());
        };

        let slices = buffer.content.iter().skip(buffer.offset.1).map(|l| {
            l.get_slice(
                buffer.offset.0..(buffer.offset.0 + self.window_size.width).min(l.len_chars()),
            )
            .map(|s| s.as_str().unwrap_or(""))
        });

        let len = slices.len();

        for (i, slice) in slices.enumerate() {
            queue!(&mut self.writer, Print(slice.unwrap_or("")))?;
            if i < len - 1 {
                queue!(&mut self.writer, Print("\n\r"))?;
            }
        }

        Ok(())
    }

    fn scroll(&mut self) -> anyhow::Result<()> {
        let (cursor_x, cursor_y) = self.get_cursor_pos()?;
        let (window_width, window_height) = (self.window_size.width, self.window_size.height);
        let buffer = self.get_current_buffer_mut()?;

        buffer.offset.0 = cursor_x.saturating_sub(window_width);
        buffer.offset.1 = cursor_y.saturating_sub(window_height);

        Ok(())
    }

    fn setup_terminal(&mut self) -> io::Result<()> {
        execute!(self.writer, EnterAlternateScreen)?;
        enable_raw_mode()?;

        execute!(self.writer, Clear(ClearType::All))?;

        Ok(())
    }

    pub fn restore_terminal(&mut self) -> io::Result<()> {
        execute!(self.writer, LeaveAlternateScreen)?;
        disable_raw_mode()?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Key(event) => self.handle_key(event)?,
            Event::Mouse(_event) => {}
            Event::Paste(_data) => {}
            Event::Resize(_width, _height) => {}
        };

        Ok(())
    }

    fn handle_key(&mut self, event: KeyEvent) -> anyhow::Result<()> {
        match (event.modifiers, event.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                self.restore_terminal()?;
                exit(0);
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.add_char(c)?;

                let cursor_position = self.get_cursor_pos()?;
                self.set_cursor_x(cursor_position.0 + 1)?;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.new_line()?;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.delete()?;
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                let cursor_position = self.get_cursor_pos()?;

                self.set_cursor_x(cursor_position.0.saturating_sub(1))?;
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                let cursor_position = self.get_cursor_pos()?;
                let line = self.get_line_at(cursor_position.1)?;

                self.set_cursor_x((cursor_position.0 + 1).min(line.len_chars()))?;
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                let cursor_position = self.get_cursor_pos()?;
                let new_cursor_y = cursor_position.1.saturating_sub(1);

                self.set_cursor_y(new_cursor_y)?;
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                let cursor_position = self.get_cursor_pos()?;
                let lines_len = self.get_current_buffer()?.content.len();
                let new_cursor_y = (cursor_position.1 + 1).min(lines_len - 1);

                self.set_cursor_y(new_cursor_y)?;
            }
            _ => {}
        };

        Ok(())
    }

    fn add_char(&mut self, c: char) -> anyhow::Result<()> {
        let (cursor_index, line_index) = self.get_cursor_pos()?;

        let last_line = self.get_line_at_mut(line_index)?;

        last_line.try_insert_char(cursor_index, c)?;

        Ok(())
    }

    fn set_cursor_y(&mut self, y: usize) -> anyhow::Result<()> {
        let buffer = self.get_current_buffer_mut()?;
        buffer.cursor.y = y;

        Ok(())
    }

    fn set_cursor_x(&mut self, x: usize) -> anyhow::Result<()> {
        let buffer = self.get_current_buffer_mut()?;

        buffer.cursor.x = x;

        Ok(())
    }

    fn new_line(&mut self) -> anyhow::Result<()> {
        let buffer = self.get_current_buffer_mut()?;

        buffer.cursor.y += 1;
        buffer.cursor.x = 0;

        buffer.content.push(Rope::new());

        Ok(())
    }

    fn delete(&mut self) -> anyhow::Result<()> {
        let (cursor_index, _) = self.get_cursor_pos()?;
        let (cursor_x, cursor_y) = self.get_cursor_pos()?;

        let (mut new_cursor_x, new_cursor_y) = self.subtract_cursor_pos()?;

        if cursor_x > 0 {
            let last_line = self.get_line_at_mut(cursor_y)?;
            last_line.try_remove(cursor_index.saturating_sub(1)..cursor_index)?;
        };

        if new_cursor_y < cursor_y {
            new_cursor_x = self.get_line_at(new_cursor_y)?.len_chars();
            self.append_to_prev_line()?;
        }

        self.set_cursor_x(new_cursor_x)?;
        self.set_cursor_y(new_cursor_y)?;
        Ok(())
    }

    fn subtract_cursor_pos(&self) -> anyhow::Result<(usize, usize)> {
        let mut cursor_position = self.get_cursor_pos()?;

        match cursor_position {
            (0, 0) => {}
            (0, y) => {
                cursor_position.0 = self.get_line_len(y - 1).unwrap_or(0).saturating_sub(1);
                cursor_position.1 = y - 1;
            }
            (x, 0) => {
                cursor_position.0 = x - 1;
                cursor_position.1 = 0;
            }
            (x, y) => {
                cursor_position.0 = x - 1;
                cursor_position.1 = y;
            }
        };

        Ok(cursor_position)
    }

    fn get_line_len(&self, index: usize) -> anyhow::Result<usize> {
        let line = self.get_line_at(index)?;

        Ok(line.len_chars())
    }

    fn append_to_prev_line(&mut self) -> anyhow::Result<()> {
        let buffer = self.get_current_buffer_mut()?;
        let cursor_y = buffer.cursor.y;

        if cursor_y == 0 {
            return Ok(());
        }

        let removed_line = buffer.content.remove(cursor_y);
        let prev_line = self.get_line_at_mut(cursor_y - 1)?;

        prev_line.append(removed_line);

        Ok(())
    }
}
