use std::path::Path;

use ropey::Rope;

use crate::core::extended_linked_list::ExtendedLinkedList;

pub struct Document {
    file_path: Option<String>,
    dirty: bool,
    pub content: ExtendedLinkedList<Rope>,
}

impl Document {
    pub async fn new(file_path: Option<String>) -> std::io::Result<Self> {
        let content = match file_path.as_deref() {
            Some(path) => ExtendedLinkedList::from_vec(
                tokio::fs::read_to_string(Path::new(path))
                    .await?
                    .lines()
                    .map(Rope::from)
                    .collect::<Vec<_>>(),
            ),
            None => ExtendedLinkedList::from([Rope::new()]),
        };

        Ok(Self {
            file_path,
            content,
            dirty: false,
        })
    }

    pub fn get_content(&self) -> &ExtendedLinkedList<Rope> {
        &self.content
    }
}
