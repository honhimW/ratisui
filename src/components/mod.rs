use ratatui::prelude::Color;
use crate::theme::get_color;

pub mod highlight_value;
pub mod list_table;
pub mod set_table;
pub mod zset_table;
pub mod hash_table;
pub mod raw_value;
pub mod fps;
pub mod popup;
pub mod servers;
mod database_editor;
pub mod create_key_editor;
pub mod console_output;
pub mod redis_cli;
pub mod raw_paragraph;
pub mod stream_view;

struct TableColors {
    // table background
    bg: Color,
    // header background
    header_bg: Color,
    // header foreground
    header_fg: Color,
    // row foreground
    row_fg: Color,
    // odd-numbered row
    normal_row: Color,
    // even-numbered row
    alt_row: Color,
}

impl TableColors {
    fn new() -> Self {
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