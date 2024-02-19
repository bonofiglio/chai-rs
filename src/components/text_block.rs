use std::io::Stdout;

use crossterm::{
    cursor::{self},
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
};
use once_cell::sync::Lazy;
use ropey::Rope;

use crate::{
    chai::TermSize,
    core::{
        coords::{Coords, TermScreenCoords},
        extended_linked_list::ExtendedLinkedList,
    },
};

use super::TUIComponent;

static WORD_REGEX: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\w+|[^\w\s]+").unwrap());

pub enum Mode {
    Normal,
    Insert,
}

pub enum TextBlockEvent {
    Char(char),
    NewLine,
    Delete,
}

pub struct TextBlock {
    pub position: TermScreenCoords,
    pub content: *const ExtendedLinkedList<Rope>,
    pub offset: Coords,
    pub size: Coords,
    pub cursor: Coords,
    pub mode: Mode,
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
        position: (u16, u16),
        cursor: Option<(usize, usize)>,
    ) -> TextBlock {
        TextBlock {
            content,
            size: size.into(),
            offset: (0, 0).into(),
            position: position.into(),
            cursor: cursor.unwrap_or((0, 0)).into(),
            mode: Mode::Normal,
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

    pub fn update(&mut self, event: &Event) -> anyhow::Result<Option<TextBlockEvent>> {
        Ok(match event {
            Event::Key(event) => self.handle_key(&event)?,
            Event::Paste(_data) => None,
            _ => None,
        })
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

    fn get_prev_word_start(&self) -> Option<usize> {
        let line = self
            .get_line_at(self.cursor.y)
            .ok()?
            .get_slice(0..self.cursor.x)?
            .as_str()?;

        let capture = WORD_REGEX.captures_iter(line).last()?.get(0)?;

        Some(capture.start())
    }

    fn get_next_word_end(&self) -> Option<usize> {
        let line = self
            .get_line_at(self.cursor.y)
            .ok()?
            .get_slice(0..)?
            .as_str()?;

        let captures = WORD_REGEX.captures_iter(line);
        let mut captures = captures.skip_while(|c| match c.get(0) {
            Some(capture) => capture.end() <= self.cursor.x,
            None => false,
        });

        let capture = captures.next()?.get(0)?;

        let word_end = capture.end() - 1;

        if word_end == self.cursor.x {
            return Some(captures.next()?.get(0)?.end() - 1);
        }

        Some(word_end)
    }

    fn get_first_word_end_at_line(&self, line_number: usize) -> Option<usize> {
        let line = self
            .get_line_at(line_number)
            .ok()?
            .get_slice(0..)?
            .as_str()?;

        let mut captures = WORD_REGEX.captures_iter(line);
        let capture = captures.next()?.get(0)?;

        let word_end = capture.end() - 1;

        Some(word_end)
    }

    fn get_last_word_start_at_line(&self, line_number: usize) -> Option<usize> {
        let line = self
            .get_line_at(line_number)
            .ok()?
            .get_slice(0..)?
            .as_str()?;

        let capture = WORD_REGEX.captures_iter(line).last()?.get(0)?;

        Some(capture.start())
    }

    fn handle_key(&mut self, event: &KeyEvent) -> anyhow::Result<Option<TextBlockEvent>> {
        Ok(match (event.modifiers, event.code, &self.mode) {
            // Global movement
            (KeyModifiers::NONE, KeyCode::Left, _)
            | (KeyModifiers::NONE, KeyCode::Char('h'), Mode::Normal) => {
                let line = self.get_line_at(self.cursor.y)?;
                self.cursor.x = self
                    .cursor
                    .x
                    .saturating_sub(1)
                    .min(line.len_chars().saturating_sub(1));
                None
            }
            (KeyModifiers::NONE, KeyCode::Right, _)
            | (KeyModifiers::NONE, KeyCode::Char('l'), Mode::Normal) => {
                let line = self.get_line_at(self.cursor.y)?;

                let prev_cursor_x = self.cursor.x;

                self.cursor.x = (self.cursor.x + 1).min(line.len_chars());
                self.cursor.x = self.cursor.x.max(prev_cursor_x);
                None
            }
            (KeyModifiers::NONE, KeyCode::Up, _)
            | (KeyModifiers::NONE, KeyCode::Char('k'), Mode::Normal) => {
                self.goto_line(self.cursor.y.saturating_sub(1));
                None
            }
            (KeyModifiers::NONE, KeyCode::Down, _)
            | (KeyModifiers::NONE, KeyCode::Char('j'), Mode::Normal) => {
                self.goto_line(self.cursor.y + 1);
                None
            }

            // Normal mode
            (KeyModifiers::NONE, KeyCode::Char('i'), Mode::Normal) => {
                self.mode = Mode::Insert;
                None
            }
            (KeyModifiers::NONE, KeyCode::Char('b'), Mode::Normal) => {
                let prev_start = self.get_prev_word_start();

                match prev_start {
                    Some(prev_start) => {
                        self.cursor.x = prev_start;
                    }
                    None => {
                        if self.cursor.y == 0 {
                            return Ok(None);
                        }

                        let Some(prev_line_start) =
                            self.get_last_word_start_at_line(self.cursor.y - 1)
                        else {
                            return Ok(None);
                        };

                        self.cursor.x = prev_line_start;
                        self.cursor.y = self.cursor.y - 1;
                    }
                };

                None
            }
            (KeyModifiers::NONE, KeyCode::Char('e'), Mode::Normal) => {
                match self.get_next_word_end() {
                    Some(word_end) => {
                        self.cursor.x = word_end;
                    }
                    None => {
                        let Some(next_word_start) =
                            self.get_first_word_end_at_line(self.cursor.y + 1)
                        else {
                            return Ok(None);
                        };

                        self.cursor.x = next_word_start;
                        self.cursor.y = self.cursor.y + 1;
                    }
                }

                None
            }

            // Insert mode
            (KeyModifiers::NONE, KeyCode::Char(c), Mode::Insert)
            | (KeyModifiers::SHIFT, KeyCode::Char(c), Mode::Insert) => {
                Some(TextBlockEvent::Char(c))
            }
            (KeyModifiers::NONE, KeyCode::Esc, Mode::Insert) => {
                self.mode = Mode::Normal;
                None
            }
            (KeyModifiers::NONE, KeyCode::Enter, Mode::Insert) => Some(TextBlockEvent::NewLine),
            (KeyModifiers::NONE, KeyCode::Backspace, Mode::Insert) => Some(TextBlockEvent::Delete),
            _ => None,
        })
    }
}
