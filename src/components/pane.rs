use ropey::Rope;

use crate::{
    chai::TermSize,
    components::text_block::TextBlock,
    core::{ExtendedLinkedList, TermScreenCoords},
};

pub struct Pane {
    views: Vec<TextBlock>,
    active_view_index: usize,
}

impl Pane {
    pub fn render(
        &mut self,
        w: &mut std::io::Stdout,
        window_size: crate::chai::TermSize,
    ) -> anyhow::Result<()> {
        for view in &mut self.views {
            view.render(w, window_size)?;
        }

        Ok(())
    }

    pub fn new(
        content: ExtendedLinkedList<Rope>,
        size: TermSize,
        position: TermScreenCoords,
        cursor: Option<(usize, usize)>,
    ) -> Self {
        Self {
            views: vec![TextBlock::new(content, size, position, cursor)],
            active_view_index: 0,
        }
    }

    pub fn get_active_view(&self) -> anyhow::Result<&TextBlock> {
        self.views
            .get(self.active_view_index)
            .ok_or(anyhow::anyhow!("No view found"))
    }

    pub fn get_current_view_mut(&mut self) -> anyhow::Result<&mut TextBlock> {
        self.views
            .get_mut(self.active_view_index)
            .ok_or(anyhow::anyhow!("No view found"))
    }
}
