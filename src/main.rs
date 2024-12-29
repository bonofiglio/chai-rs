use chai::TermSize;
use crossterm::{event::EventStream, terminal::window_size};

mod chai;
mod components;
mod core;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file_path = std::env::args().nth(1);
    let size = window_size()?;

    let size = TermSize {
        width: size.width,
        height: size.height,
    };

    chai::Chai::new(file_path, size)
        .await?
        .start(&mut EventStream::new())
        .await
}
