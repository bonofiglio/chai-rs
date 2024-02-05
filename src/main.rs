mod chai;
mod components;
mod core;

fn main() -> anyhow::Result<()> {
    let file_path = std::env::args().nth(1).map(String::into_boxed_str);

    let result = chai::Chai::new(file_path)?.start();

    result
}
