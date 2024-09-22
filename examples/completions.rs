use std::thread::sleep;
use std::time::Duration;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Row, Table, Widget};
use ratatui::{Frame, TerminalOptions, Viewport};
use ratatui::buffer::Buffer;
use ratatui::text::{Line, Span, Text};
use tui_textarea::TextArea;
use anyhow::Result;
use once_cell::sync::Lazy;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::style::{Style, Stylize};
use strum::Display;

fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    let mut text_area = TextArea::default();
    text_area.set_block(Block::bordered().border_type(BorderType::Thick));
    let mut rows = vec![];
    {
        let mut cells = vec![];
        cells.push(Cell::new(Span::raw("info server")));
        cells.push(Cell::new(Span::raw("raw")));
        rows.push(Row::new(cells));
    }
    {
        let mut cells = vec![];
        cells.push(Cell::new(Span::raw("get [key]")));
        cells.push(Cell::new(Span::raw("value")));
        rows.push(Row::new(cells));
    }
    loop {
        let input = text_area.lines().get(0).unwrap();
        let table = get_table(input);
        terminal
            .draw(|frame: &mut Frame| {
                draw_picture(frame, &text_area, &table);
            })?;
        if event::poll(Duration::from_millis(20))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key {
                        KeyEvent { modifiers: KeyModifiers::CONTROL, code: KeyCode::Char('c'),.. } => {
                            break;
                        }
                        KeyEvent { code: KeyCode::Esc, .. } => {
                            if text_area.is_selecting() {
                                text_area.cancel_selection();
                            }
                        }
                        KeyEvent { code: KeyCode::Enter, .. } => {}
                        KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
                        KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => {
                            text_area.select_all();
                        }
                        KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                            text_area.undo();
                        }
                        KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                            text_area.redo();
                        }
                        input => {
                            text_area.input(input);
                        }
                    }
                }
            }
        };
    }
    ratatui::restore();
    Ok(())
}

fn get_table(input: &str) -> Table {
    let table = Table::new(get_rows(input), [Min(1), Length(8)]).block(Block::bordered()
        .border_type(BorderType::Rounded));
    table
}

fn get_rows(input: &str) -> Vec<Row> {
    let mut rows = vec![];
    for item in TOAST_CHANNEL.iter() {
        if item.label.label.starts_with(input) {
            let mut prompt = Line::default();
            prompt.push_span(Span::raw(&item.label.label));
            if let Some(ref detail) = item.label.detail {
                prompt.push_span(Span::raw(" "));
                prompt.push_span(Span::raw(detail).style(Style::default().dim()));
            }
            let prompt = Cell::new(prompt);
            let kind = Cell::new(Line::raw(item.kind.to_string()).alignment(Alignment::Right));
            let row = Row::new(vec![prompt, kind]);
            rows.push(row);
        }
    }
    rows
}

static TOAST_CHANNEL: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let mut vec = vec![];
    vec.push(CompletionItem::string("set"));
    vec.push(CompletionItem::string("get"));
    vec.push(CompletionItem::string("strlen"));
    vec.push(CompletionItem::hash("hgetall"));
    vec
});

#[derive(Debug, Clone)]
struct CompletionItem {
    kind: CompletionItemKind,
    label: Label,
    range: (isize, isize),
    insert_text: String,
}

impl CompletionItem {
    fn default() -> CompletionItem {
        Self {
            kind: CompletionItemKind::String,
            label: Label {
                label: "ping".to_string(),
                detail: None,
                description: None,
            },
            range: (0, -1),
            insert_text: "ping".to_string(),
        }
    }

    fn string(s: impl Into<String>) -> CompletionItem {
        let s = s.into();
        Self {
            kind: CompletionItemKind::String,
            label: Label {
                label: s.clone(),
                detail: Some(s.clone()),
                description: Some(s.clone()),
            },
            range: (0, -1),
            insert_text: s,
        }
    }

    fn hash(s: impl Into<String>) -> CompletionItem {
        let s = s.into();
        Self {
            kind: CompletionItemKind::Hash,
            label: Label {
                label: s.clone(),
                detail: Some(s.clone()),
                description: Some(s.clone()),
            },
            range: (0, -1),
            insert_text: s,
        }
    }

}

#[derive(Debug, Clone)]
struct Label {
    label: String,
    detail: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Display)]
enum CompletionItemKind {
    String,
    List,
    Set,
    SortedSet,
    Hash,
    Json,
}

fn draw_picture(frame: &mut Frame, text_area: &TextArea, table: &Table) {
    let rect = frame.area();
    let rect = centered_rect(50, 50, rect);
    let (y, x) = text_area.cursor();

    let area = Rect {
        height: rect.height - 1,
        ..rect
    };

    let menu_area = Rect {
        x: area.x + x as u16 + 1,
        y: area.y + y as u16 + 2,
        height: 7,
        width: 40,
    };

    frame.render_widget(table, menu_area);
    frame.render_widget(text_area, area);
}

pub fn centered_rect(percentage_x: u16, percentage_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percentage_y) / 2),
            Constraint::Percentage(percentage_y),
            Constraint::Percentage((100 - percentage_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percentage_x) / 2),
            Constraint::Percentage(percentage_x),
            Constraint::Percentage((100 - percentage_x) / 2),
        ])
        .split(popup_layout[1])[1]
}