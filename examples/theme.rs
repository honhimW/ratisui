use anyhow::Result;
use ron::ser::PrettyConfig;
use ratisui::theme::{Color, Tab, TailwindColor, TailwindPalette, Theme};
use ratisui::theme::TailwindPalette::C100;

fn main() -> Result<()> {
    let mut theme = Theme::default();
    let mut tab = Tab::default();
    tab.explorer = Color::Tailwind(TailwindColor::RED, C100);
    tab.cli = Color::Hex("ffffff".to_string());
    tab.logger = Color::Default;
    theme.tab = tab;

    let result = ron::ser::to_string_pretty(&theme, PrettyConfig::default())?;
    println!("{}", result);

    Ok(())
}