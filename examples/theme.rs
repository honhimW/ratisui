use anyhow::{Context, Result};
use ratisui::theme::{Color, Tab, Theme};

fn main() -> Result<()> {
    let mut theme = Theme::default();
    let mut tab = Tab::default();
    theme.tab = tab;

    let r_color = Color::hex("ffffff").to_color().context("")?;
    assert!(matches!(r_color, ratatui::style::Color::Rgb(255,255,255)));

    Ok(())
}