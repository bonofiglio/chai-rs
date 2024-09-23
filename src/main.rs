use crossterm::event::EventStream;

mod chai;
mod components;
mod core;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file_path = std::env::args().nth(1).map(String::into_boxed_str);

    chai::Chai::new(file_path)?
        .start(&mut EventStream::new())
        .await
}
