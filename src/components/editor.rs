use std::{ops::DerefMut, pin::Pin};

use crossterm::event::{Event, KeyCode};
use ropey::Rope;

use crate::{components::text_block::TextBlock, core::extended_linked_list::ExtendedLinkedList};

use super::{
    text_block::{Mode, TextBlockEvent},
    TUIComponent,
};

pub struct InfoLine {
    content: Pin<Box<ExtendedLinkedList<Rope>>>,
    text_block: TextBlock,
}

pub struct Editor {
    text_block: TextBlock,
    info_line: InfoLine,
}

impl TUIComponent for Editor {
    fn render(
        &mut self,
        w: &mut std::io::Stdout,
        window_size: crate::chai::TermSize,
    ) -> anyhow::Result<()> {
        self.text_block.render(w, window_size)?;

        let mode = match self.text_block.mode {
            Mode::Normal => "-- NORMAL --",
            Mode::Insert => "-- INSERT --",
            Mode::Command => "-- COMMAND --",
        };

        *self.info_line.content.deref_mut() = ExtendedLinkedList::from([Rope::from(mode)]);
        self.info_line.text_block.render(w, window_size)?;

        Ok(())
    }
}

impl Editor {
    pub fn new(
        content: *mut ExtendedLinkedList<Rope>,
        size: (usize, usize),
        position: (u16, u16),
        cursor: Option<(usize, usize)>,
    ) -> Self {
        let mut info_line_content = Box::pin(ExtendedLinkedList::from([Rope::from("INSERT")]));
        let pointer = Pin::get_mut(info_line_content.as_mut());

        Self {
            info_line: InfoLine {
                text_block: TextBlock::new(
                    pointer,
                    (size.0, 1),
                    (position.0, (size.1) as u16),
                    None,
                ),
                content: info_line_content,
            },
            text_block: TextBlock::new(content, size, position, cursor),
        }
    }

    fn update_command(&mut self, event: &Event) -> anyhow::Result<Option<TextBlockEvent>> {
        Ok(match event {
            Event::Key(event) => match event.code {
                KeyCode::Char(_c) => None,
                KeyCode::Enter => None,
                KeyCode::Esc => {
                    self.text_block.mode = Mode::Normal;
                    None
                }
                KeyCode::Backspace => None,
                _ => None,
            },

            Event::Paste(_data) => None,
            _ => None,
        })
    }

    pub fn update(&mut self, event: &Event) -> anyhow::Result<()> {
        match self.text_block.mode {
            Mode::Command => {
                self.update_command(event)?;
            }
            _ => {
                let event = self.text_block.update(event)?;

                match event {
                    Some(TextBlockEvent::Char(c)) => {
                        self.text_block.add_char(c)?;

                        let cursor_position = self.text_block.get_cursor_pos()?;
                        self.text_block.set_cursor_x(cursor_position.0 + 1)?;
                    }
                    Some(TextBlockEvent::NewLine) => self.text_block.new_line()?,
                    Some(TextBlockEvent::Delete) => self.text_block.delete()?,
                    None => {}
                };
            }
        };

        Ok(())
    }

    pub fn get_cursor_term_pos(&self) -> anyhow::Result<(u16, u16)> {
        self.text_block.get_cursor_term_pos()
    }
}
