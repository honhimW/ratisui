use clap::{Parser, arg};

#[derive(Default, Clone, Debug, Parser)]
#[command(name = "ratisui")]
#[command(version, about = "Redis TUI build with Ratatui.", long_about = None)]
pub struct AppArguments {
    #[arg(
        short = 't',
        long = "target",
        value_name = "TARGET",
        help = "Named redis target in ~/.config/ratisui/databases.ron"
    )]
    pub target: Option<String>,

    #[arg(
        short = 'T',
        long = "theme",
        value_name = "THEME",
        help = "Theme configuration in ~/.config/ratisui/theme/<THEME>.ron"
    )]
    pub theme: Option<String>,

    #[arg(long = "once", help = "Will not load | save databases")]
    pub once: bool,
}
