use anyhow::{Context, Result};
use ron::ser::PrettyConfig;
use ratisui_core::theme::{Color, Tab, Theme};

fn main() -> Result<()> {
    println!("{}", ron::ser::to_string_pretty(&Theme::dark(), PrettyConfig::default())?);

    let r_color = Color::hex("ffffff").to_color().context("")?;
    assert!(matches!(r_color, ratatui::style::Color::Rgb(255,255,255)));

    Ok(())
}