use std::thread::sleep;
use std::time::Duration;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Cell, Clear, Row, Table, Widget};
use ratatui::{Frame, TerminalOptions, Viewport};
use ratatui::buffer::Buffer;
use ratatui::text::{Span, Text};
use tui_textarea::TextArea;
use anyhow::Result;
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

fn draw_picture(frame: &mut Frame, text_area: &TextArea, table: &Table) {
    let rect = frame.area();
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
