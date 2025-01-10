use ratatui::prelude::Color;
use ratisui_core::theme::get_color;

pub mod fps;
pub mod popup;

pub struct TableColors {
    // table background
    pub bg: Color,
    // header background
    pub header_bg: Color,
    // header foreground
    pub header_fg: Color,
    // row foreground
    pub row_fg: Color,
    // odd-numbered row
    pub normal_row: Color,
    // even-numbered row
    pub alt_row: Color,
}

impl TableColors {
    pub fn new() -> Self {
        Self {
            bg: get_color(|t| &t.table.bg),
            header_bg: get_color(|t| &t.table.header_bg),
            header_fg: get_color(|t| &t.table.header),
            row_fg: get_color(|t| &t.table.row),
            normal_row: get_color(|t| &t.table.odd_row_bg),
            alt_row: get_color(|t| &t.table.even_row_bg),
        }
    }
}
