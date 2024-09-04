use ratatui::buffer::Buffer;
use ratatui::layout::{Direction, Layout, Rect};
use ratatui::layout::Constraint::{Length, Max, Min};
use ratatui::style::{Style, Styled};
use ratatui::symbols;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};
use crate::components::raw_value::raw_value_to_highlight_text;
use anyhow::Result;

pub struct ListBuilder {
    pub selected_style: Style,
    pub unselected_style: Style,
    pub max_height: u16,
    cell_width: Vec<u16>,
    rows: Vec<ListRow>,
}

impl ListBuilder {
    pub fn new(cell_width: Vec<u16>) -> Self {
        Self {
            cell_width,
            selected_style: Style::default(),
            unselected_style: Style::default(),
            max_height: 3,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, data: Vec<String>) -> Result<()> {
        if self.cell_width.len() != data.len() {
            return Err(anyhow::anyhow!("cell_width and data should be same length"));
        }
        if let Some(last) = self.rows.last_mut() {
            last.is_last = false;
        }
        let mut row = ListRow {
            index: 0,
            cell_width: self.cell_width.clone(),
            cells: data
                .into_iter()
                .enumerate()
                .map(|(i, d)| RowCell {
                    index: i,
                    data: Some(d),
                    style: Style::default(),
                })
                .collect(),
            style: self.unselected_style.clone(),
            expand: false,
            max_height: self.max_height,
            is_last: true,
        };
        let i = self.rows.len();
        row.index = i;
        self.rows.push(row);

        Ok(())
    }
}

pub struct ListRow {
    pub index: usize,
    pub cell_width: Vec<u16>,
    pub cells: Vec<RowCell>,
    pub style: Style,
    pub expand: bool,
    pub max_height: u16,
    pub is_last: bool,
}

impl ListRow {
    pub fn new(cell_width: Vec<u16>, data: Vec<String>) -> Result<Self> {
        if cell_width.len() != data.len() {
            return Err(anyhow::anyhow!("cell_width and data should be same length"));
        }
        Ok(Self {
            index: 0,
            cell_width: Vec::new(),
            cells: data
                .into_iter()
                .enumerate()
                .map(|(i, d)| RowCell {
                    index: i,
                    data: Some(d),
                    style: Style::default(),
                })
                .collect(),
            style: Style::default(),
            expand: false,
            max_height: 3,
            is_last: false,
        })
    }

}

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

impl Widget for ListRow {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        let content_constraint = if self.expand {
            Max(self.max_height)
        } else {
            Length(1)
        };
        let [sep, content] = Layout::vertical([Length(1), content_constraint]).areas(area);

        let line_separator = Block::default()
            .borders(Borders::TOP)
            .border_set(symbols::border::PLAIN);
        line_separator.render(sep, buf);
        let mut row_constraints = vec![];
        for width in self.cell_width {
            row_constraints.push(Length(1));
            row_constraints.push(Max(width));
        }
        row_constraints.push(Length(1));
        let rc = Layout::horizontal(row_constraints).split(content);
        let cell_separator = &Block::default()
            .borders(Borders::LEFT)
            .border_set(symbols::border::PLAIN);
        let mut i = 0;
        for cell in self.cells {
            let position = i * 2;
            cell_separator.render(rc[position], buf);
            cell.render(rc[position + 1], buf);
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
            text.render(area, buf);
        }
    }
}