use once_cell::sync::Lazy;
use ratatui::style::palette::tailwind;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

type RColor = ratatui::style::Color;

static CURRENT_THEME: Lazy<RwLock<Theme>> = Lazy::new(|| {
    RwLock::new(Theme::dark())
});

static LIGHT_THEME: Lazy<Theme> = Lazy::new(|| Theme::light());

static DARK_THEME: Lazy<Theme> = Lazy::new(|| Theme::dark());

pub fn set_theme(theme: Theme) {
    if let Ok(mut guard) = CURRENT_THEME.write() {
        *guard = theme;
    }
}

pub fn get_color<F: Fn(&Theme) -> &Color>(f: F) -> RColor {
    if let Ok(ref theme) = CURRENT_THEME.read() {
        let color = f(theme);
        if let Some(r_color) = color.to_color() {
            return r_color;
        } else {
            let theme = match theme.kind {
                Kind::Light => &LIGHT_THEME,
                Kind::Dark => &DARK_THEME,
            };
            let color = f(theme);
            if let Some(r_color) = color.to_color() {
                return r_color;
            }
        }
    }
    RColor::default()
}

impl Theme {
    pub fn light() -> Self {
        let mut theme = Self::default();
        theme.name = "ratisui-light".to_string();
        theme.kind = Kind::Light;

        theme.context.bg = Color::Rgb(255, 255, 255);
        theme.context.fps = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C500);
        theme.context.key_bg = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C700);

        theme.server.highlight = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C400);
        theme.server.name = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.server.location = Color::Tailwind(TailwindColor::CYAN, TailwindPalette::C700);
        theme.server.db = Color::Tailwind(TailwindColor::BLUE, TailwindPalette::C700);
        theme.server.username = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.server.tls = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.server.protocol = Color::Tailwind(TailwindColor::EMERALD, TailwindPalette::C600);

        theme.table.bg = Color::Default;
        theme.table.header_bg = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C300);
        theme.table.header = Color::Default;
        theme.table.row = Color::Default;
        theme.table.odd_row_bg = Color::Default;
        theme.table.even_row_bg = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C100);

        theme.raw.string = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.raw.boolean = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.keyword = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.constant = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.null = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.property = Color::Tailwind(TailwindColor::FUCHSIA, TailwindPalette::C700);
        theme.raw.comment = Color::Tailwind(TailwindColor::CYAN, TailwindPalette::C500);
        theme.raw.number = Color::Tailwind(TailwindColor::BLUE, TailwindPalette::C600);

        theme.border.highlight = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C700);
        theme.border.default = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C400);

        theme.editor.editing = Color::Tailwind(TailwindColor::SKY, TailwindPalette::C700);
        theme.editor.warning = Color::Tailwind(TailwindColor::RED, TailwindPalette::C600);

        theme.tab.title = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C200);

        theme.tab.explorer.accent = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C900);
        theme.tab.explorer.highlight = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.explorer.tree.highlight = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C100);
        theme.tab.explorer.tree.highlight_bg = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.explorer.key_type.hash = Color::Tailwind(TailwindColor::BLUE, TailwindPalette::C400);
        theme.tab.explorer.key_type.list = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C400);
        theme.tab.explorer.key_type.set = Color::Tailwind(TailwindColor::ORANGE, TailwindPalette::C400);
        theme.tab.explorer.key_type.zset = Color::Tailwind(TailwindColor::PINK, TailwindPalette::C400);
        theme.tab.explorer.key_type.string = Color::Tailwind(TailwindColor::PURPLE, TailwindPalette::C400);
        theme.tab.explorer.key_type.json = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C400);
        theme.tab.explorer.key_type.stream = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C400);
        theme.tab.explorer.key_type.time_series = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C400);
        theme.tab.explorer.key_type.bloom_filter = Color::Tailwind(TailwindColor::ORANGE, TailwindPalette::C400);
        theme.tab.explorer.key_type.unknown = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C500);

        theme.tab.cli.accent = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C900);
        theme.tab.cli.highlight = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C700);
        theme.tab.cli.console.cmd = Color::Tailwind(TailwindColor::EMERALD, TailwindPalette::C700);
        theme.tab.cli.console.out = Color::Default;
        theme.tab.cli.console.err = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.cli.menu.bg = Color::Tailwind(TailwindColor::NEUTRAL, TailwindPalette::C300);
        theme.tab.cli.menu.highlight = Color::Tailwind(TailwindColor::ZINC, TailwindPalette::C300);
        theme.tab.cli.menu.info_bg = Color::Tailwind(TailwindColor::STONE, TailwindPalette::C300);
        theme.tab.cli.menu.input = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.tab.cli.doc.bg = Color::Tailwind(TailwindColor::NEUTRAL, TailwindPalette::C300);
        theme.tab.cli.doc.command = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C800);
        theme.tab.cli.doc.attribute = Color::Tailwind(TailwindColor::PINK, TailwindPalette::C800);

        theme.tab.logger.accent = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C900);
        theme.tab.logger.highlight = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.tab.logger.level.error = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.logger.level.warn = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.tab.logger.level.info = Color::Tailwind(TailwindColor::CYAN, TailwindPalette::C700);
        theme.tab.logger.level.debug = Color::Tailwind(TailwindColor::EMERALD, TailwindPalette::C700);
        theme.tab.logger.level.trace = Color::Tailwind(TailwindColor::VIOLET, TailwindPalette::C700);

        theme.toast.info = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C500);
        theme.toast.warn = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C500);
        theme.toast.error = Color::Tailwind(TailwindColor::RED, TailwindPalette::C500);

        theme
    }

    pub fn dark() -> Self {
        let mut theme = Self::default();
        theme.name = "ratisui-dark".to_string();
        theme.kind = Kind::Dark;

        theme.context.bg = Color::Rgb(0, 0, 0);
        theme.context.fps = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C500);
        theme.context.key_bg = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C700);

        theme.server.highlight = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C950);
        theme.server.name = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C400);
        theme.server.location = Color::Tailwind(TailwindColor::CYAN, TailwindPalette::C500);
        theme.server.db = Color::Tailwind(TailwindColor::BLUE, TailwindPalette::C600);
        theme.server.username = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C400);
        theme.server.tls = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.server.protocol = Color::Tailwind(TailwindColor::EMERALD, TailwindPalette::C600);

        theme.table.bg = Color::Default;
        theme.table.header_bg = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C900);
        theme.table.header = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C200);
        theme.table.row = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C200);
        theme.table.odd_row_bg = Color::Default;
        theme.table.even_row_bg = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C950);

        theme.raw.string = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C400);
        theme.raw.boolean = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.keyword = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.constant = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.null = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C600);
        theme.raw.property = Color::Tailwind(TailwindColor::FUCHSIA, TailwindPalette::C700);
        theme.raw.comment = Color::Tailwind(TailwindColor::CYAN, TailwindPalette::C500);
        theme.raw.number = Color::Tailwind(TailwindColor::BLUE, TailwindPalette::C600);

        theme.border.highlight = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C300);
        theme.border.default = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C600);

        theme.editor.editing = Color::Tailwind(TailwindColor::SKY, TailwindPalette::C700);
        theme.editor.warning = Color::Tailwind(TailwindColor::RED, TailwindPalette::C700);

        theme.tab.title = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C100);

        theme.tab.explorer.accent = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C900);
        theme.tab.explorer.highlight = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.explorer.tree.highlight = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C100);
        theme.tab.explorer.tree.highlight_bg = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.explorer.key_type.hash = Color::Tailwind(TailwindColor::BLUE, TailwindPalette::C700);
        theme.tab.explorer.key_type.list = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C700);
        theme.tab.explorer.key_type.set = Color::Tailwind(TailwindColor::ORANGE, TailwindPalette::C700);
        theme.tab.explorer.key_type.zset = Color::Tailwind(TailwindColor::PINK, TailwindPalette::C700);
        theme.tab.explorer.key_type.string = Color::Tailwind(TailwindColor::PURPLE, TailwindPalette::C700);
        theme.tab.explorer.key_type.json = Color::Tailwind(TailwindColor::GRAY, TailwindPalette::C700);
        theme.tab.explorer.key_type.stream = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C700);
        theme.tab.explorer.key_type.time_series = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C700);
        theme.tab.explorer.key_type.bloom_filter = Color::Tailwind(TailwindColor::ORANGE, TailwindPalette::C700);
        theme.tab.explorer.key_type.unknown = Color::Tailwind(TailwindColor::SLATE, TailwindPalette::C500);

        theme.tab.cli.accent = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C900);
        theme.tab.cli.highlight = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C700);
        theme.tab.cli.console.cmd = Color::Tailwind(TailwindColor::EMERALD, TailwindPalette::C700);
        theme.tab.cli.console.out = Color::Default;
        theme.tab.cli.console.err = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.cli.menu.bg = Color::Tailwind(TailwindColor::NEUTRAL, TailwindPalette::C800);
        theme.tab.cli.menu.highlight = Color::Tailwind(TailwindColor::ZINC, TailwindPalette::C900);
        theme.tab.cli.menu.info_bg = Color::Tailwind(TailwindColor::STONE, TailwindPalette::C900);
        theme.tab.cli.menu.input = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C500);
        theme.tab.cli.doc.bg = Color::Tailwind(TailwindColor::NEUTRAL, TailwindPalette::C800);
        theme.tab.cli.doc.command = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C400);
        theme.tab.cli.doc.attribute = Color::Tailwind(TailwindColor::PINK, TailwindPalette::C800);

        theme.tab.logger.accent = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C900);
        theme.tab.logger.highlight = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.tab.logger.level.error = Color::Tailwind(TailwindColor::ROSE, TailwindPalette::C700);
        theme.tab.logger.level.warn = Color::Tailwind(TailwindColor::AMBER, TailwindPalette::C700);
        theme.tab.logger.level.info = Color::Tailwind(TailwindColor::CYAN, TailwindPalette::C700);
        theme.tab.logger.level.debug = Color::Tailwind(TailwindColor::EMERALD, TailwindPalette::C700);
        theme.tab.logger.level.trace = Color::Tailwind(TailwindColor::VIOLET, TailwindPalette::C700);

        theme.toast.info = Color::Tailwind(TailwindColor::GREEN, TailwindPalette::C700);
        theme.toast.warn = Color::Tailwind(TailwindColor::YELLOW, TailwindPalette::C700);
        theme.toast.error = Color::Tailwind(TailwindColor::RED, TailwindPalette::C700);

        theme
    }
}

/// Theme configuration
/// `~/.config/ratisui/theme/{name}.ron`
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub kind: Kind,
    pub context: Context,
    pub server: Server,
    pub table: Table,
    pub raw: Raw,
    pub border: Border,
    pub editor: Editor,
    pub tab: Tab,
    pub toast: Toast,
}

/// Base theme kind, used for fallback
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum Kind {
    Light,
    #[default]
    Dark,
}

/// Colors
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum Color {
    Tailwind(TailwindColor, TailwindPalette),
    Hex(String),
    Rgb(u8, u8, u8),
    Default,
    #[default]
    Fallback,

    /// ANSI Color: Black. Foreground: 30, Background: 40
    Black,
    /// ANSI Color: Red. Foreground: 31, Background: 41
    Red,
    /// ANSI Color: Green. Foreground: 32, Background: 42
    Green,
    /// ANSI Color: Yellow. Foreground: 33, Background: 43
    Yellow,
    /// ANSI Color: Blue. Foreground: 34, Background: 44
    Blue,
    /// ANSI Color: Magenta. Foreground: 35, Background: 45
    Magenta,
    /// ANSI Color: Cyan. Foreground: 36, Background: 46
    Cyan,
    /// ANSI Color: White. Foreground: 37, Background: 47
    ///
    /// Note that this is sometimes called `silver` or `white` but we use `white` for bright white
    Gray,
    /// ANSI Color: Bright Black. Foreground: 90, Background: 100
    ///
    /// Note that this is sometimes called `light black` or `bright black` but we use `dark gray`
    DarkGray,
    /// ANSI Color: Bright Red. Foreground: 91, Background: 101
    LightRed,
    /// ANSI Color: Bright Green. Foreground: 92, Background: 102
    LightGreen,
    /// ANSI Color: Bright Yellow. Foreground: 93, Background: 103
    LightYellow,
    /// ANSI Color: Bright Blue. Foreground: 94, Background: 104
    LightBlue,
    /// ANSI Color: Bright Magenta. Foreground: 95, Background: 105
    LightMagenta,
    /// ANSI Color: Bright Cyan. Foreground: 96, Background: 106
    LightCyan,
    /// ANSI Color: Bright White. Foreground: 97, Background: 107
    /// Sometimes called `bright white` or `light white` in some terminals
    White,
}

/// Tailwind colors
#[derive(Serialize, Deserialize, Clone, Debug)]
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
    ROSE,
}

/// Tailwind palette
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum TailwindPalette {
    C50,
    C100,
    C200,
    C300,
    C400,
    C500,
    C600,
    #[default]
    C700,
    C800,
    C900,
    C950,
}

/// Context colors
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Context {
    pub bg: Color,
    pub fps: Color,
    pub key_bg: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Server {
    pub highlight: Color,
    pub name: Color,
    pub location: Color,
    pub db: Color,
    pub username: Color,
    pub tls: Color,
    pub protocol: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Table {
    pub bg: Color,
    pub header_bg: Color,
    pub header: Color,
    pub row: Color,
    pub odd_row_bg: Color,
    pub even_row_bg: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Raw {
    pub string: Color,
    pub boolean: Color,
    pub keyword: Color,
    pub constant: Color,
    pub null: Color,
    pub property: Color,
    pub comment: Color,
    pub number: Color,
}

/// Border colors
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Border {
    pub highlight: Color,
    pub default: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Editor {
    pub editing: Color,
    pub warning: Color,
}

/// Tab colors
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Tab {
    pub title: Color,
    pub explorer: Explorer,
    pub cli: Cli,
    pub logger: Logger,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Explorer {
    pub accent: Color,
    pub highlight: Color,
    pub tree: Tree,
    pub key_type: KeyType,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Tree {
    pub highlight: Color,
    pub highlight_bg: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct KeyType {
    pub hash: Color,
    pub list: Color,
    pub set: Color,
    pub zset: Color,
    pub string: Color,
    pub json: Color,
    pub stream: Color,
    pub time_series: Color,
    pub bloom_filter: Color,
    pub unknown: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Cli {
    pub accent: Color,
    pub highlight: Color,
    pub console: Console,
    pub menu: Menu,
    pub doc: Doc,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Console {
    pub cmd: Color,
    pub out: Color,
    pub err: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Menu {
    pub bg: Color,
    pub highlight: Color,
    pub info_bg: Color,
    pub input: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Doc {
    pub bg: Color,
    pub command: Color,
    pub attribute: Color,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Logger {
    pub accent: Color,
    pub highlight: Color,
    pub level: Level,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Level {
    pub error: Color,
    pub warn: Color,
    pub info: Color,
    pub debug: Color,
    pub trace: Color,
}

/// Toast colors
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Toast {
    pub info: Color,
    pub warn: Color,
    pub error: Color,
}

impl Color {
    #[allow(unused)]
    pub fn hex<T: Into<String>>(s: T) -> Self {
        Self::Hex(s.into())
    }

    pub fn to_color(&self) -> Option<RColor> {
        match self {
            Color::Tailwind(c, p) => {
                let color = match c {
                    TailwindColor::SLATE => tailwind::SLATE,
                    TailwindColor::GRAY => tailwind::GRAY,
                    TailwindColor::ZINC => tailwind::ZINC,
                    TailwindColor::NEUTRAL => tailwind::NEUTRAL,
                    TailwindColor::STONE => tailwind::STONE,
                    TailwindColor::RED => tailwind::RED,
                    TailwindColor::ORANGE => tailwind::ORANGE,
                    TailwindColor::AMBER => tailwind::AMBER,
                    TailwindColor::YELLOW => tailwind::YELLOW,
                    TailwindColor::LIME => tailwind::LIME,
                    TailwindColor::GREEN => tailwind::GREEN,
                    TailwindColor::EMERALD => tailwind::EMERALD,
                    TailwindColor::TEAL => tailwind::TEAL,
                    TailwindColor::CYAN => tailwind::CYAN,
                    TailwindColor::SKY => tailwind::SKY,
                    TailwindColor::BLUE => tailwind::BLUE,
                    TailwindColor::INDIGO => tailwind::INDIGO,
                    TailwindColor::VIOLET => tailwind::VIOLET,
                    TailwindColor::PURPLE => tailwind::PURPLE,
                    TailwindColor::FUCHSIA => tailwind::FUCHSIA,
                    TailwindColor::PINK => tailwind::PINK,
                    TailwindColor::ROSE => tailwind::ROSE,
                };
                Some(match p {
                    TailwindPalette::C50 => color.c50,
                    TailwindPalette::C100 => color.c100,
                    TailwindPalette::C200 => color.c200,
                    TailwindPalette::C300 => color.c300,
                    TailwindPalette::C400 => color.c400,
                    TailwindPalette::C500 => color.c500,
                    TailwindPalette::C600 => color.c600,
                    TailwindPalette::C700 => color.c700,
                    TailwindPalette::C800 => color.c800,
                    TailwindPalette::C900 => color.c900,
                    TailwindPalette::C950 => color.c950,
                })
            }
            Color::Hex(hex) => if let Ok(in_u32) = u32::from_str_radix(&hex, 16) {
                Some(RColor::from_u32(in_u32))
            } else {
                None
            },
            Color::Rgb(r, g, b) => Some(RColor::Rgb(*r, *g, *b)),
            Color::Default => Some(RColor::default()),
            Color::Fallback => None,
            Color::Black => Some(RColor::Black),
            Color::Red => Some(RColor::Red),
            Color::Green => Some(RColor::Green),
            Color::Yellow => Some(RColor::Yellow),
            Color::Blue => Some(RColor::Blue),
            Color::Magenta => Some(RColor::Magenta),
            Color::Cyan => Some(RColor::Cyan),
            Color::Gray => Some(RColor::Gray),
            Color::DarkGray => Some(RColor::DarkGray),
            Color::LightRed => Some(RColor::LightRed),
            Color::LightGreen => Some(RColor::LightGreen),
            Color::LightYellow => Some(RColor::LightYellow),
            Color::LightBlue => Some(RColor::LightBlue),
            Color::LightMagenta => Some(RColor::LightMagenta),
            Color::LightCyan => Some(RColor::LightCyan),
            Color::White => Some(RColor::White),
        }
    }
}
