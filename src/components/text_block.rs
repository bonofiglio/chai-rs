use std::{cmp::min, io::Stdout};

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

static WORD_REGEX: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\w+|[^\w\s]+").unwrap());

pub enum Mode {
    Normal,
    Insert,
    Command,
}

pub struct TextBlock {
    position: TermScreenCoords,
    content: ExtendedLinkedList<Rope>,
    offset: Coords,
    size: TermSize,
    cursor: Coords,
    mode: Mode,
}

impl TextBlock {
    pub fn render(&mut self, w: &mut Stdout, window_size: TermSize) -> anyhow::Result<()> {
        self.scroll(window_size)?;

        let size = self.get_effective_size(window_size)?;

        queue!(w, cursor::MoveTo(self.position.x, self.position.y))?;

        let slices = self
            .content
            .iter()
            .skip(self.offset.y)
            .map(|l| {
                l.get_slice(
                    self.offset.x
                        ..(self.offset.x + self.size.width as usize)
                            .min(l.len_chars())
                            .min(self.offset.x + size.width as usize),
                )
                .map(|s| s.as_str().unwrap_or(""))
            })
            .take(size.height as usize);

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

    pub fn new(
        content: ExtendedLinkedList<Rope>,
        size: TermSize,
        position: TermScreenCoords,
        cursor: Option<(usize, usize)>,
    ) -> TextBlock {
        TextBlock {
            content,
            size,
            offset: (0, 0).into(),
            position,
            cursor: cursor.unwrap_or((0, 0)).into(),
            mode: Mode::Normal,
        }
    }

    pub fn get_cursor_term_pos(&self) -> anyhow::Result<TermScreenCoords> {
        let x = self.cursor.x.saturating_sub(self.offset.x) + self.position.x as usize;
        let y = self.cursor.y.saturating_sub(self.offset.y) + self.position.y as usize;

        let line_len = self
            .get_line_len(self.cursor.y)?
            .saturating_sub(self.offset.x)
            + self.position.x as usize;

        let x = x.min(line_len);

        Ok(TermScreenCoords {
            x: x.try_into()?,
            y: y.try_into()?,
        })
    }

    pub fn scroll(&mut self, window_size: TermSize) -> anyhow::Result<()> {
        let size = self.get_effective_size(window_size)?;

        // When the cursor_y - offset_y is greater than window_height - 1, add one to the offset_y
        if self.cursor.y.saturating_sub(self.offset.y) > size.height.saturating_sub(1) as usize {
            self.offset.y += 1;
        }
        // When the cursor_y - offset_y is less than 0 (meaning, it hit the top of the window),
        // subtract one from the offset_y
        if (self.cursor.y + 1).saturating_sub(self.offset.y) == 0 {
            self.offset.y = self.offset.y.saturating_sub(1);
        }

        let current_line = self.get_line_at(self.cursor.y)?;

        let offset_cursor = self.cursor.x.saturating_sub(size.width as usize);
        let offset_current_line = current_line.len_chars().saturating_sub(size.width as usize);

        // When the cursor x is greater than the current line length, scroll the TextBlock back to
        // the minimum to show as most of the current line as possible
        self.offset.x = min(offset_cursor, offset_current_line);

        Ok(())
    }

    pub fn get_effective_size(&self, window_size: TermSize) -> anyhow::Result<TermSize> {
        let effective_width = (self.size.width as u32 + self.position.x as u32)
            .min(window_size.width as u32)
            .saturating_sub(self.position.x as u32);
        let effective_height = (self.size.height as u32 + self.position.y as u32)
            .min(window_size.height as u32)
            .saturating_sub(self.position.y as u32);

        Ok(TermSize {
            width: effective_width.try_into()?,
            height: effective_height.try_into()?,
        })
    }

    pub fn update(&mut self, event: &Event) -> anyhow::Result<ExtendedLinkedList<Rope>> {
        match event {
            Event::Key(event) => self.handle_key(event)?,
            Event::Paste(_data) => (),
            _ => (),
        };

        Ok(self.content.clone())
    }

    fn get_line_at(&self, index: usize) -> anyhow::Result<&Rope> {
        let line = self
            .content
            .get(index)
            .ok_or(anyhow::anyhow!("No line at index {}", index))?;

        Ok(line)
    }

    fn get_line_at_mut(&mut self, index: usize) -> anyhow::Result<&mut Rope> {
        let line = self
            .content
            .get_mut(index)
            .ok_or(anyhow::anyhow!("No line at index {}", index))?;

        Ok(line)
    }

    fn get_line_len(&self, index: usize) -> anyhow::Result<usize> {
        let line = self.get_line_at(index)?;

        Ok(line.len_chars())
    }

    fn goto_line(&mut self, line_number: usize) {
        let lines_len = self.content.len();

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

    fn get_next_word_start(&self) -> Option<usize> {
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

        captures.next();

        let capture = captures.next()?.get(0)?;
        let word_end = capture.start();

        if word_end == self.cursor.x {
            return Some(captures.next()?.get(0)?.start());
        }

        Some(word_end)
    }

    fn get_first_word_start_at_line(&self, line_number: usize) -> Option<usize> {
        let line = self
            .get_line_at(line_number)
            .ok()?
            .get_slice(0..)?
            .as_str()?;

        let mut captures = WORD_REGEX.captures_iter(line);

        match captures.next() {
            Some(capture) => match capture.get(0) {
                Some(r#match) => Some(r#match.start()),
                None => Some(0),
            },
            None => Some(0),
        }
    }

    fn get_last_word_start_at_line(&self, line_number: usize) -> Option<usize> {
        let line = self
            .get_line_at(line_number)
            .ok()?
            .get_slice(0..)?
            .as_str()?;

        let last = WORD_REGEX.captures_iter(line).last();

        match last {
            Some(capture) => match capture.get(0) {
                Some(r#match) => Some(r#match.start()),
                None => Some(0),
            },
            None => Some(0),
        }
    }

    fn handle_key(&mut self, event: &KeyEvent) -> anyhow::Result<()> {
        match (event.modifiers, event.code, &self.mode) {
            // Global movement
            (KeyModifiers::NONE, KeyCode::Left, _)
            | (KeyModifiers::NONE, KeyCode::Char('h'), Mode::Normal) => {
                let line = self.get_line_at(self.cursor.y)?;
                self.cursor.x = self
                    .cursor
                    .x
                    .saturating_sub(1)
                    .min(line.len_chars().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Right, _)
            | (KeyModifiers::NONE, KeyCode::Char('l'), Mode::Normal) => {
                let line = self.get_line_at(self.cursor.y)?;

                let prev_cursor_x = self.cursor.x;

                self.cursor.x = (self.cursor.x + 1).min(line.len_chars());
                self.cursor.x = self.cursor.x.max(prev_cursor_x);
            }
            (KeyModifiers::NONE, KeyCode::Up, _)
            | (KeyModifiers::NONE, KeyCode::Char('k'), Mode::Normal) => {
                self.goto_line(self.cursor.y.saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Down, _)
            | (KeyModifiers::NONE, KeyCode::Char('j'), Mode::Normal) => {
                self.goto_line(self.cursor.y + 1);
            }
            (_, KeyCode::Char(':'), Mode::Normal) => {
                self.mode = Mode::Command;
            }

            // Normal mode
            (KeyModifiers::NONE, KeyCode::Char('i'), Mode::Normal) => {
                self.mode = Mode::Insert;
            }
            (KeyModifiers::NONE, KeyCode::Char('b'), Mode::Normal) => {
                let prev_start = self.get_prev_word_start();

                match prev_start {
                    Some(prev_start) => {
                        self.cursor.x = prev_start;
                    }
                    None => {
                        if self.cursor.y == 0 {
                            return Ok(());
                        }

                        let Some(prev_line_start) =
                            self.get_last_word_start_at_line(self.cursor.y - 1)
                        else {
                            return Ok(());
                        };

                        self.cursor.x = prev_line_start;
                        self.cursor.y -= 1;
                    }
                };
            }
            (KeyModifiers::NONE, KeyCode::Char('e'), Mode::Normal) => {
                match self.get_next_word_end() {
                    Some(word_end) => {
                        self.cursor.x = word_end;
                    }
                    None => {
                        let Some(next_word_end) =
                            self.get_first_word_end_at_line(self.cursor.y + 1)
                        else {
                            return Ok(());
                        };

                        self.cursor.x = next_word_end;
                        self.cursor.y += 1;
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('w'), Mode::Normal) => {
                match self.get_next_word_start() {
                    Some(word_start) => {
                        self.cursor.x = word_start;
                    }
                    None => match self.get_first_word_start_at_line(self.cursor.y + 1) {
                        Some(next_word_start) => {
                            self.get_first_word_start_at_line(self.cursor.y + 1);
                            self.cursor.x = next_word_start;
                            self.cursor.y += 1;
                        }
                        None => {
                            let Some(current_word_end) = self.get_next_word_end() else {
                                return Ok(());
                            };

                            self.cursor.x = current_word_end;
                        }
                    },
                }
            }

            // Insert mode
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c), Mode::Insert) => {
                self.add_char(c)?;
            }
            (KeyModifiers::NONE, KeyCode::Esc, Mode::Insert | Mode::Command) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter, Mode::Insert) => {
                self.new_line()?;
            }
            (KeyModifiers::NONE, KeyCode::Backspace, Mode::Insert) => {
                self.delete()?;
            }
            _ => (),
        };

        Ok(())
    }

    pub fn new_line(&mut self) -> anyhow::Result<()> {
        let (cursor_x, cursor_y) = self.get_cursor_pos()?;
        let line = self.get_line_at_mut(cursor_y)?;

        let new_line = if cursor_x < line.len_chars() {
            line.try_split_off(cursor_x)?
        } else {
            Rope::new()
        };

        self.cursor.y += 1;
        self.cursor.x = 0;

        self.content.push_at(cursor_y + 1, new_line);

        Ok(())
    }

    pub fn delete(&mut self) -> anyhow::Result<()> {
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

    fn set_cursor_y(&mut self, y: usize) -> anyhow::Result<()> {
        self.cursor.y = y;

        Ok(())
    }

    pub fn set_cursor_x(&mut self, x: usize) -> anyhow::Result<()> {
        self.cursor.x = x;

        Ok(())
    }

    fn append_to_prev_line(&mut self) -> anyhow::Result<()> {
        let cursor_y = self.cursor.y;

        if cursor_y == 0 {
            return Ok(());
        }

        let removed_line = self.content.remove_at(cursor_y).ok_or(anyhow::anyhow!(
            "Could not remove line at index {}",
            cursor_y
        ))?;
        let prev_line = self.get_line_at_mut(cursor_y.saturating_sub(1))?;

        prev_line.append(removed_line);

        Ok(())
    }

    pub fn get_cursor_pos(&self) -> anyhow::Result<(usize, usize)> {
        let raw_pos = &self.cursor;

        let line = self.get_line_at(raw_pos.y)?;
        let x = raw_pos.x.min(line.len_chars());

        Ok((x, raw_pos.y))
    }

    pub fn add_char(&mut self, c: char) -> anyhow::Result<()> {
        let (cursor_index, line_index) = self.get_cursor_pos()?;

        let last_line = self.get_line_at_mut(line_index)?;

        last_line.try_insert_char(cursor_index, c)?;

        self.set_cursor_x(cursor_index + 1)?;

        Ok(())
    }
}
