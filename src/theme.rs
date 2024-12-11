use ratatui::style::palette::tailwind;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Theme {
    pub name: String,
    pub tab: Tab,
    pub toast: Toast,
}

fn a() {
    let i: u8 = 255;
}
#[derive(Serialize, Deserialize)]
pub enum TailwindColor {
    SLATE,
    GRAY,
    ZINC,
    NEUTRAL,
    STONE,
    RED,
    ORANGE,
    AMBER,
    YELLOW,
    LIME,
    GREEN,
    EMERALD,
    TEAL,
    CYAN,
    SKY,
    BLUE,
    INDIGO,
    VIOLET,
    PURPLE,
    FUCHSIA,
    PINK,
    BLACK,
    WHITE,
}

#[derive(Serialize, Deserialize, Default)]
pub enum TailwindPalette {
    C50,
    C100,
    C200,
    C300,
    C400,
    #[default]
    C500,
    C600,
    C700,
    C800,
    C900,
    C950,
}

#[derive(Serialize, Deserialize, Default)]
pub enum Color {
    Tailwind(TailwindColor, TailwindPalette),
    Hex(String),
    Rgb(u8, u8, u8),
    #[default]
    Default,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Tab {
    pub explorer: Color,
    pub cli: Color,
    pub logger: Color,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Toast {
    pub info: Color,
    pub warn: Color,
    pub error: Color,
}
