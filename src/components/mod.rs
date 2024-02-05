use std::io::Stdout;

use crate::chai::TermSize;

pub mod text_block;

pub trait TUIComponent {
    fn render(&mut self, w: &mut Stdout, window_size: TermSize) -> anyhow::Result<()>;
}
