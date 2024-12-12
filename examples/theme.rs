use anyhow::Result;
use ratatui::style::palette::tailwind;
use ron::ser::PrettyConfig;
use ratisui::theme::{get_color, Color, Tab, TailwindColor, TailwindPalette, Theme};
use ratisui::theme::TailwindPalette::C100;

fn main() -> Result<()> {
    let mut theme = Theme::default();
    let mut tab = Tab::default();
    theme.tab = tab;

    let result = ron::ser::to_string_pretty(&theme, PrettyConfig::default())?;
    println!("{}", result);

    let color = get_color(|t| &t.tab.explorer.accent);
    println!("{:?}", color);

    println!("{:?}", Color::hex("ffffff").to_color());

    Ok(())
}