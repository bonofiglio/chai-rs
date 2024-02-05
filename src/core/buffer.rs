use ropey::Rope;

use crate::core::extended_linked_list::ExtendedLinkedList;

pub struct Buffer {
    pub file_path: Option<Box<str>>,
    pub dirty: bool,
    pub content: ExtendedLinkedList<Rope>,
}
