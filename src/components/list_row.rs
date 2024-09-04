use std::cmp;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::layout::Constraint::{Length, Max, Min};
use ratatui::style::{Style, Styled};
use ratatui::symbols;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};
use crate::components::raw_value::raw_value_to_highlight_text;
use anyhow::Result;
use ratatui::symbols::border::Set;
use ratatui::symbols::line;
use ratatui::text::Text;

pub struct RowsBuilder {
    pub selected_style: Style,
    pub unselected_style: Style,
    pub max_height: u16,
    cell_width: Vec<Constraint>,
    rows: Vec<ListRow>,
}

impl RowsBuilder {
    pub fn new(cell_width: Vec<Constraint>) -> Self {
        Self {
            cell_width,
            selected_style: Style::default(),
            unselected_style: Style::default(),
            max_height: 3,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, data: Vec<String>) {
        if self.cell_width.len() != data.len() {
            return;
        }
        if let Some(last_row) = self.rows.last_mut() {
            (*last_row).is_last = false;
        }
        let mut max_height: u16 = self.max_height;
        let cells = data
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                max_height = cmp::min(max_height, d.lines().count() as u16);
                RowCell {
                    index: i,
                    data: Some(d.clone()),
                    style: Style::default(),
                }
            })
            .collect();
        let mut row = ListRow {
            index: 0,
            cell_width: self.cell_width.clone(),
            cells,
            style: self.unselected_style.clone(),
            expand: false,
            max_height,
            is_first: self.rows.is_empty(),
            is_last: true,
        };
        let i = self.rows.len();
        row.index = i;
        self.rows.push(row);
    }

    pub fn get_rows(self) -> Vec<ListRow> {
        self.rows
    }
}

#[derive(Clone)]
pub struct ListRow {
    pub index: usize,
    pub cell_width: Vec<Constraint>,
    pub cells: Vec<RowCell>,
    style: Style,
    pub expand: bool,
    pub max_height: u16,
    pub is_first: bool,
    pub is_last: bool,
}

#[derive(Clone)]
pub struct RowCell {
    pub index: usize,
    pub data: Option<String>,
    pub style: Style,
}

impl Styled for ListRow {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(mut self, style: S) -> Self::Item {
        self.style = style.into();
        for mut x in &mut self.cells {
            x.style = self.style.clone()
        }
        self
    }
}

impl Styled for RowCell {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(mut self, style: S) -> Self::Item {
        self.style = style.into();
        self
    }
}

pub const PLAIN_START_VERTICAL: Set = Set {
    top_left: line::NORMAL.top_left,
    top_right: line::NORMAL.top_right,
    bottom_left: line::NORMAL.vertical_right,
    bottom_right: line::NORMAL.vertical_left,
    vertical_left: line::NORMAL.vertical,
    vertical_right: line::NORMAL.vertical,
    horizontal_top: line::NORMAL.horizontal,
    horizontal_bottom: line::NORMAL.horizontal,
};

pub const PLAIN_VERTICAL: Set = Set {
    top_left: line::NORMAL.vertical_right,
    top_right: line::NORMAL.vertical_left,
    bottom_left: line::NORMAL.vertical_right,
    bottom_right: line::NORMAL.vertical_left,
    vertical_left: line::NORMAL.vertical,
    vertical_right: line::NORMAL.vertical,
    horizontal_top: line::NORMAL.horizontal,
    horizontal_bottom: line::NORMAL.horizontal,
};

pub const PLAIN_END_VERTICAL: Set = Set {
    top_left: line::NORMAL.vertical_right,
    top_right: line::NORMAL.vertical_left,
    bottom_left: line::NORMAL.bottom_left,
    bottom_right: line::NORMAL.bottom_right,
    vertical_left: line::NORMAL.vertical,
    vertical_right: line::NORMAL.vertical,
    horizontal_top: line::NORMAL.horizontal,
    horizontal_bottom: line::NORMAL.horizontal,
};

impl Widget for ListRow {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        let separator_block;
        if self.is_first && self.is_last {
            separator_block = Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::PLAIN);
        } else if self.is_first {
            separator_block = Block::default()
                .borders(Borders::ALL)
                .border_set(PLAIN_START_VERTICAL);
        } else if self.is_last {
            separator_block = Block::default()
                .borders(Borders::from_bits_retain(0b1110))
                .border_set(PLAIN_END_VERTICAL);
        } else {
            separator_block = Block::default()
                .borders(Borders::from_bits_retain(0b1110))
                .border_set(PLAIN_VERTICAL);
        }
        let block_inner_area = separator_block.inner(area);
        separator_block.render(area, buf);
        let mut row_constraints = vec![];
        let column_size = self.cell_width.len();
        let mut i = 0;
        for width in self.cell_width {
            i = i + 1;
            row_constraints.push(width);
            if i < column_size {
                row_constraints.push(Length(1));
            }
        }
        row_constraints.push(Length(1));
        let rc = Layout::horizontal(row_constraints).split(block_inner_area);
        let cell_separator = &Block::default()
            .borders(Borders::LEFT)
            .border_set(symbols::border::PLAIN);
        let mut i = 0;
        for cell in self.cells {
            let position = i * 2;
            i = i + 1;
            cell.render(rc[position], buf);
            if i < column_size {
                cell_separator.render(rc[position + 1], buf);
            }
        }
    }
}

impl Widget for RowCell {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        if let Some(data) = self.data {
            let text = raw_value_to_highlight_text(&data, false);
            text.style(self.style).render(area, buf);
        }
    }
}