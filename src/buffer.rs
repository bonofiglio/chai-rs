use ropey::Rope;

use crate::{core::extended_linked_list::ExtendedLinkedList, Cursor};

pub struct Buffer {
    pub file_path: Option<Box<str>>,
    pub cursor: Cursor,
    pub dirty: bool,
    pub content: ExtendedLinkedList<Rope>,
    pub offset: (usize, usize),
}
