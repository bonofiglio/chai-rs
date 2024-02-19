use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    terminal::{
        disable_raw_mode, enable_raw_mode, window_size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ropey::Rope;

use crate::{
    components::{
        text_block::{TextBlock, TextBlockEvent},
        TUIComponent,
    },
    core::{buffer::Buffer, extended_linked_list::ExtendedLinkedList},
};
use std::{
    io::{self, Write},
    process::exit,
};

#[derive(Copy, Clone)]
pub struct TermSize {
    pub width: usize,
    pub height: usize,
}

pub struct Chai {
    pub writer: io::Stdout,
    pub buffers: Vec<Buffer>,
    pub windows: Vec<TextBlock>,
    pub current_buffer_index: usize,
    pub active_window_index: usize,
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
            Some(ref path) => ExtendedLinkedList::from_vec(
                String::from_utf8(std::fs::read(path.as_ref())?)?
                    .lines()
                    .map(Rope::from)
                    .collect::<Vec<_>>(),
            ),
            None => ExtendedLinkedList::from([Rope::new()]),
        };

        Ok(Chai {
            writer: io::stdout(),
            buffers: vec![Buffer {
                file_path,
                dirty: false,
                content: content.into(),
            }],
            current_buffer_index: 0,
            active_window_index: 0,
            window_size: TermSize {
                width: 0,
                height: 0,
            },
            windows: Vec::new(),
        })
    }

    pub fn start(mut self) -> anyhow::Result<()> {
        self.setup_terminal()?;

        let size = window_size()?;

        self.window_size = TermSize {
            width: (size.columns as usize).saturating_sub(1),
            height: size.rows as usize,
        };

        let buffer = self.get_current_buffer_mut()?;

        let content: *const ExtendedLinkedList<Rope> = &buffer.content;

        self.windows.push(TextBlock::new(
            content,
            (self.window_size.width, self.window_size.height),
            (0, 0),
            None,
        ));

        self.clear()?;
        self.render()?;

        let term_cursor_pos = self.get_active_window()?.get_cursor_term_pos()?;

        queue!(
            self.writer,
            cursor::MoveTo(term_cursor_pos.0, term_cursor_pos.1)
        )?;
        self.writer.flush()?;

        let result = self.run_loop();

        self.restore_terminal()?;

        result
    }

    fn run_loop(&mut self) -> anyhow::Result<()> {
        while let Ok(event) = read() {
            self.clear()?;
            self.handle_event(event)?;

            self.render()?;

            let term_cursor_pos = self.get_active_window()?.get_cursor_term_pos()?;

            queue!(
                self.writer,
                cursor::MoveTo(term_cursor_pos.0, term_cursor_pos.1)
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

    fn get_active_window(&self) -> anyhow::Result<&TextBlock> {
        self.windows
            .get(self.active_window_index)
            .ok_or(anyhow::anyhow!("No window found"))
    }

    fn get_active_window_mut(&mut self) -> anyhow::Result<&mut TextBlock> {
        self.windows
            .get_mut(self.active_window_index)
            .ok_or(anyhow::anyhow!("No window found"))
    }

    fn get_cursor_pos(&self) -> anyhow::Result<(usize, usize)> {
        let window = self.get_active_window()?;
        let raw_pos = &window.cursor;

        let line = self.get_line_at(raw_pos.y)?;
        let x = raw_pos.x.min(line.len_chars());

        Ok((x, raw_pos.y))
    }

    fn clear(&mut self) -> io::Result<()> {
        queue!(self.writer, Clear(ClearType::All))
    }

    fn render(&mut self) -> anyhow::Result<()> {
        for window in self.windows.iter_mut() {
            window.render(&mut self.writer, self.window_size)?;
        }

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
            Event::Paste(ref _data) => {}
            Event::Resize(_width, _height) => {}
        };

        let event = self.get_active_window_mut()?.update(&event)?;

        match event {
            Some(TextBlockEvent::Char(c)) => {
                self.add_char(c)?;

                let cursor_position = self.get_cursor_pos()?;
                self.set_cursor_x(cursor_position.0 + 1)?;
            }
            Some(TextBlockEvent::NewLine) => self.new_line()?,
            Some(TextBlockEvent::Delete) => self.delete()?,
            None => {}
        };

        Ok(())
    }

    fn handle_key(&mut self, event: KeyEvent) -> anyhow::Result<()> {
        match (event.modifiers, event.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                self.restore_terminal()?;
                exit(0);
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
        let window = self.get_active_window_mut()?;
        window.cursor.y = y;

        Ok(())
    }

    fn set_cursor_x(&mut self, x: usize) -> anyhow::Result<()> {
        let window = self.get_active_window_mut()?;

        window.cursor.x = x;

        Ok(())
    }

    fn new_line(&mut self) -> anyhow::Result<()> {
        let (cursor_x, cursor_y) = self.get_cursor_pos()?;
        let line = self.get_line_at_mut(cursor_y)?;

        let new_line = if cursor_x < line.len_chars() {
            line.try_split_off(cursor_x)?
        } else {
            Rope::new()
        };

        let window = self.get_active_window_mut()?;

        window.cursor.y += 1;
        window.cursor.x = 0;

        let buffer = self.get_current_buffer_mut()?;
        buffer.content.push_at(cursor_y + 1, new_line);

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
                cursor_position.0 = self
                    .get_line_len(y.saturating_sub(1))
                    .unwrap_or(0)
                    .saturating_sub(1);
                cursor_position.1 = y.saturating_sub(1);
            }
            (x, 0) => {
                cursor_position.0 = x.saturating_sub(1);
                cursor_position.1 = 0;
            }
            (x, y) => {
                cursor_position.0 = x.saturating_sub(1);
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
        let window = self.get_active_window()?;
        let cursor_y = window.cursor.y;

        let buffer = self.get_current_buffer_mut()?;

        if cursor_y == 0 {
            return Ok(());
        }

        let removed_line = buffer.content.remove_at(cursor_y).ok_or(anyhow::anyhow!(
            "Could not remove line at index {}",
            cursor_y
        ))?;
        let prev_line = self.get_line_at_mut(cursor_y.saturating_sub(1))?;

        prev_line.append(removed_line);

        Ok(())
    }
}
