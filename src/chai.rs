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

use crate::{
    components::Pane,
    core::{document::Document, TermScreenCoords},
};
use std::{
    io::{self, Write},
    process::exit,
};

#[derive(Copy, Clone)]
pub struct TermSize {
    pub width: u16,
    pub height: u16,
}

pub struct Chai {
    pub writer: io::Stdout,
    pub panes: Vec<Pane>,
    pub active_pane_index: usize,
    pub window_size: TermSize,
    pub documents: Vec<Document>,
}

impl Drop for Chai {
    fn drop(&mut self) {
        self.restore_terminal().unwrap();
    }
}

impl Chai {
    pub async fn new(file_path: Option<String>, window_size: TermSize) -> anyhow::Result<Self> {
        let default_doc = Document::new(file_path).await?;
        let default_content = default_doc.get_content().clone();
        let documents = vec![default_doc];

        let editor = Chai {
            writer: io::stdout(),
            active_pane_index: 0,
            window_size,
            panes: vec![Pane::new(
                default_content,
                TermSize {
                    width: window_size.width,
                    height: window_size.height - 1,
                },
                TermScreenCoords { x: 0, y: 0 },
                None,
            )],
            documents,
        };

        Ok(editor)
    }

    pub async fn start<S>(mut self, read_stream: &mut S) -> anyhow::Result<()>
    where
        S: Stream<Item = std::io::Result<crossterm::event::Event>> + Unpin,
    {
        self.setup_terminal()?;

        let size = window_size()?;

        self.window_size = TermSize {
            width: size.columns.saturating_sub(1),
            height: size.rows,
        };

        self.clear()?;
        self.render()?;

        let term_cursor_pos = self
            .get_active_pane()?
            .get_active_view()?
            .get_cursor_term_pos()?;

        queue!(
            self.writer,
            cursor::MoveTo(term_cursor_pos.x, term_cursor_pos.y)
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

            let term_cursor_pos = self
                .get_active_pane()?
                .get_active_view()?
                .get_cursor_term_pos()?;

            queue!(
                self.writer,
                cursor::MoveTo(term_cursor_pos.x, term_cursor_pos.y)
            )?;

            self.writer.flush()?;
        }

        Ok(())
    }

    fn get_active_pane(&self) -> anyhow::Result<&Pane> {
        self.panes
            .get(self.active_pane_index)
            .ok_or(anyhow::anyhow!("No pane found"))
    }

    fn get_current_pane_mut(&mut self) -> anyhow::Result<&mut Pane> {
        self.panes
            .get_mut(self.active_pane_index)
            .ok_or(anyhow::anyhow!("No pane found"))
    }

    fn clear(&mut self) -> io::Result<()> {
        queue!(self.writer, Clear(ClearType::All))
    }

    fn render(&mut self) -> anyhow::Result<()> {
        for window in self.panes.iter_mut() {
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

        let view = self.get_current_pane_mut()?.get_current_view_mut()?;

        self.documents.first_mut().unwrap().content = view.update(&event)?;

        Ok(())
    }

    fn handle_key(&mut self, event: KeyEvent) -> anyhow::Result<()> {
        if let (KeyModifiers::CONTROL, KeyCode::Char('c')) = (event.modifiers, event.code) {
            self.restore_terminal()?;
            exit(0);
        };

        Ok(())
    }

    fn should_close(&self) -> bool {
        self.panes.is_empty()
    }
}
