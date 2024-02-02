mod buffer;
mod chai;

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
    let result = chai::Chai::new().start();

    result
}
