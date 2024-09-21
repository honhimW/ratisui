use std::thread::sleep;
use std::time::Duration;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Row, Table, Widget};
use ratatui::{Frame, TerminalOptions, Viewport};
use ratatui::buffer::Buffer;
use ratatui::text::{Span, Text};
use tui_textarea::TextArea;
use anyhow::Result;
use once_cell::sync::Lazy;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};

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
    let table = Table::new(rows, [Min(1), Length(8)]).block(Block::bordered().border_type(BorderType::Rounded));
    loop {
        let input = text_area.lines().get(0).unwrap();
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

    rows
}

static TOAST_CHANNEL: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    vec![]
});

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
}

struct Label {
    label: String,
    detail: Option<String>,
    description: Option<String>,
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

enum CompletionItemKind {
    String,
    List,
    Set,
    SortedSet,
    Hash,
    Json,
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