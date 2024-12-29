use std::io::Stdout;

use crate::chai::TermSize;

pub mod pane;
pub mod text_block;

pub use pane::Pane;
pub use text_block::TextBlock;

pub trait TUIComponent {
    fn render(&mut self, w: &mut Stdout, window_size: TermSize) -> anyhow::Result<()>;
}
