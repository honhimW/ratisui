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
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
}

impl TableColors {
    fn new() -> Self {
        Self {
            buffer_bg: get_color(|t| &t.table.buffer_bg),
            header_bg: get_color(|t| &t.table.header_bg),
            header_fg: get_color(|t| &t.table.header_fg),
            row_fg: get_color(|t| &t.table.row_fg),
            normal_row_color: get_color(|t| &t.table.normal_row_color),
            alt_row_color: get_color(|t| &t.table.alt_row_color),
        }
    }
}