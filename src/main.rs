mod buffer;
mod chai;
mod core;

struct Cursor {
    pub x: usize,
    pub y: usize,
}

impl Cursor {
    pub fn get_pos(&self) -> (usize, usize) {
        (self.x, self.y)
    }
}

fn main() -> anyhow::Result<()> {
    let file_path = std::env::args().nth(1).map(String::into_boxed_str);

    let result = chai::Chai::new(file_path)?.start();

    result
}
