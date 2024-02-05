use std::io::Stdout;

use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
};
use ropey::Rope;

use crate::{
    chai::TermSize, core::extended_linked_list::ExtendedLinkedList, Coords, TermScreenCoords,
};

use super::TUIComponent;

pub struct TextBlock {
    pub position: TermScreenCoords,
    pub content: *const ExtendedLinkedList<Rope>,
    pub offset: Coords,
    pub size: Coords,
    pub cursor: Coords,
}

impl TUIComponent for TextBlock {
    fn render(&mut self, w: &mut Stdout, window_size: TermSize) -> anyhow::Result<()> {
        self.scroll(window_size)?;

        let (effective_width, effective_height) = self.get_effective_size(window_size)?;

        queue!(w, cursor::MoveTo(self.position.x, self.position.y))?;

        let content = unsafe { &*self.content };

        let slices = content
            .iter()
            .skip(self.offset.y)
            .map(|l| {
                l.get_slice(
                    self.offset.x
                        ..(self.offset.x + self.size.x)
                            .min(l.len_chars())
                            .min(self.offset.x + effective_width as usize),
                )
                .map(|s| s.as_str().unwrap_or(""))
            })
            .take(effective_height as usize);

        let len = slices.len();

        for (i, slice) in slices.enumerate() {
            queue!(w, Print(slice.unwrap_or("")))?;

            if i < len.saturating_sub(1) {
                queue!(w, Print("\n\r"))?;

                if self.position.x > 0 {
                    queue!(w, cursor::MoveRight(self.position.x))?;
                }
            }
        }

        Ok(())
    }
}

impl TextBlock {
    pub fn new(
        content: *const ExtendedLinkedList<Rope>,
        size: (usize, usize),
        offset: (usize, usize),
        position: (u16, u16),
        cursor: Option<(usize, usize)>,
    ) -> TextBlock {
        TextBlock {
            content,
            size: size.into(),
            offset: offset.into(),
            position: position.into(),
            cursor: cursor.unwrap_or((0, 0)).into(),
        }
    }

    pub fn get_cursor_term_pos(&self) -> anyhow::Result<(u16, u16)> {
        let x = self.cursor.x.saturating_sub(self.offset.x) + self.position.x as usize;
        let y = self.cursor.y.saturating_sub(self.offset.y) + self.position.y as usize;

        let line_len = self.get_line_len(self.cursor.y)? + self.position.x as usize;

        let x = x.min(line_len);

        Ok((x.try_into()?, y.try_into()?))
    }

    pub fn scroll(&mut self, window_size: TermSize) -> anyhow::Result<()> {
        let (window_width, window_height) = self.get_effective_size(window_size)?;

        // When the cursor_y - offset_y is greater than window_height - 1, add one to the offset_y
        if self.cursor.y.saturating_sub(self.offset.y) > window_height.saturating_sub(1) as usize {
            self.offset.y += 1;
        }
        // When the cursor_y - offset_y is less than 0 (meaning, it hit the top of the window),
        // subtract one from the offset_y
        if (self.cursor.y as usize + 1).saturating_sub(self.offset.y) == 0 {
            self.offset.y = self.offset.y.saturating_sub(1);
        }

        self.offset.x = self.cursor.x.saturating_sub(window_width as usize);

        Ok(())
    }

    pub fn get_effective_size(&self, window_size: TermSize) -> anyhow::Result<(u16, u16)> {
        let effective_width = (self.size.x + self.position.x as usize)
            .min(window_size.width as usize)
            .saturating_sub(self.position.x as usize);
        let effective_height = (self.size.y + self.position.y as usize)
            .min(window_size.height as usize)
            .saturating_sub(self.position.y as usize);

        Ok((effective_width.try_into()?, effective_height.try_into()?))
    }

    pub fn handle_event(&mut self, event: &Event) -> anyhow::Result<()> {
        match event {
            Event::Key(event) => self.handle_key(&event)?,
            Event::Paste(_data) => {}
            _ => {}
        };

        Ok(())
    }

    fn get_content(&self) -> &ExtendedLinkedList<Rope> {
        unsafe { &*self.content }
    }

    fn get_line_at(&self, index: usize) -> anyhow::Result<&Rope> {
        let line = self
            .get_content()
            .get(index)
            .ok_or(anyhow::anyhow!("No line at index {}", index))?;

        Ok(line)
    }

    fn get_line_len(&self, index: usize) -> anyhow::Result<usize> {
        let line = self.get_line_at(index)?;

        Ok(line.len_chars())
    }

    fn goto_line(&mut self, line_number: usize) {
        let lines_len = self.get_content().len();

        self.cursor.y = line_number.min(lines_len.saturating_sub(1));
    }

    fn handle_key(&mut self, event: &KeyEvent) -> anyhow::Result<()> {
        match (event.modifiers, event.code) {
            (KeyModifiers::NONE, KeyCode::Left) => {
                let line = self.get_line_at(self.cursor.y)?;
                self.cursor.x = self
                    .cursor
                    .x
                    .saturating_sub(1)
                    .min(line.len_chars().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                let line = self.get_line_at(self.cursor.y)?;

                let prev_cursor_x = self.cursor.x;

                self.cursor.x = (self.cursor.x + 1).min(line.len_chars());
                self.cursor.x = self.cursor.x.max(prev_cursor_x);
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.goto_line(self.cursor.y.saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.goto_line(self.cursor.y + 1);
            }
            _ => {}
        };

        Ok(())
    }
}
