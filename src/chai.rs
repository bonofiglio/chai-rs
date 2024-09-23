use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    terminal::{
        disable_raw_mode, enable_raw_mode, window_size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use futures_core::Stream;
use futures_util::StreamExt;
use ropey::Rope;

use crate::{
    components::{editor::Editor, TUIComponent},
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
    pub windows: Vec<Editor>,
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
                content,
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

    pub async fn start<S>(mut self, read_stream: &mut S) -> anyhow::Result<()>
    where
        S: Stream<Item = std::io::Result<crossterm::event::Event>> + Unpin,
    {
        self.setup_terminal()?;

        let size = window_size()?;

        self.window_size = TermSize {
            width: (size.columns as usize).saturating_sub(1),
            height: size.rows as usize,
        };

        let buffer = self.get_current_buffer_mut()?;
        let content: *mut ExtendedLinkedList<_> = &mut buffer.content;

        self.windows.push(Editor::new(
            content,
            (self.window_size.width / 2, self.window_size.height - 1),
            (0, 0),
            None,
        ));
        self.windows.push(Editor::new(
            content,
            (self.window_size.width / 2, self.window_size.height - 1),
            ((self.window_size.width / 2) as u16, 0),
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

        let result = self.run_loop(read_stream).await;

        self.restore_terminal()?;

        result
    }

    async fn run_loop<S>(&mut self, read_stream: &mut S) -> anyhow::Result<()>
    where
        S: Stream<Item = std::io::Result<crossterm::event::Event>> + Unpin,
    {
        while !self.should_close() {
            let Some(event) = read_stream.next().await else {
                continue;
            };

            let event = event?;

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

    fn get_current_buffer_mut(&mut self) -> anyhow::Result<&mut Buffer> {
        self.buffers
            .get_mut(self.current_buffer_index)
            .ok_or(anyhow::anyhow!("No buffer found"))
    }

    fn get_active_window(&self) -> anyhow::Result<&Editor> {
        self.windows
            .get(self.active_window_index)
            .ok_or(anyhow::anyhow!("No window found"))
    }

    fn get_active_window_mut(&mut self) -> anyhow::Result<&mut Editor> {
        self.windows
            .get_mut(self.active_window_index)
            .ok_or(anyhow::anyhow!("No window found"))
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

        self.get_active_window_mut()?.update(&event)
    }

    fn handle_key(&mut self, event: KeyEvent) -> anyhow::Result<()> {
        if let (KeyModifiers::CONTROL, KeyCode::Char('c')) = (event.modifiers, event.code) {
            self.restore_terminal()?;
            exit(0);
        };

        Ok(())
    }

    fn should_close(&self) -> bool {
        self.windows.is_empty()
    }
}
