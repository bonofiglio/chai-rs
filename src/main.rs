mod buffer;
mod chai;
mod components;
mod core;

pub struct Coords {
    pub x: usize,
    pub y: usize,
}

pub struct TermScreenCoords {
    pub x: u16,
    pub y: u16,
}

impl From<(usize, usize)> for Coords {
    fn from((x, y): (usize, usize)) -> Self {
        Self { x, y }
    }
}

impl From<(u16, u16)> for TermScreenCoords {
    fn from((x, y): (u16, u16)) -> Self {
        Self { x, y }
    }
}

fn main() -> anyhow::Result<()> {
    let file_path = std::env::args().nth(1).map(String::into_boxed_str);

    let result = chai::Chai::new(file_path)?.start();

    result
}
