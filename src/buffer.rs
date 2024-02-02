use ropey::Rope;

use crate::Cursor;

pub struct Buffer {
    pub file_path: Option<Box<str>>,
    pub cursor: Cursor,
    pub dirty: bool,
    pub content: Vec<Rope>,
    pub offset: (usize, usize),
}
