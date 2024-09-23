use std::cmp;
use std::thread::sleep;
use std::time::Duration;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Widget};
use ratatui::{Frame, TerminalOptions, Viewport};
use ratatui::buffer::Buffer;
use ratatui::text::{Line, Span, Text};
use tui_textarea::{CursorMove, Input, TextArea};
use anyhow::Result;
use once_cell::sync::Lazy;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::style::{Style, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::symbols::scrollbar::Set;
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
    let mut table_state = TableState::default();
    let mut scroll_state = ScrollbarState::default();
    table_state.select_first();
    scroll_state.first();
    let mut show_table = false;
    loop {
        let mut input = text_area.lines().get(0).unwrap().clone();
        let (y, x) = text_area.cursor();
        if let Some(start_pos) = find_word_start_backward(&input, x) {
            input = (&input[start_pos..x]).to_string();
        }
        let items = get_items(&input);
        scroll_state = scroll_state.content_length(items.len());
        let rows = get_rows(&input, &items);
        let table = get_table(rows);
        let size = items.len() as u16;
        terminal
            .draw(|frame: &mut Frame| {
                let rect = frame.area();
                let Rect {
                    height: max_height,
                    width: max_width,
                    ..
                } = rect;
                let rect = centered_rect(50, 50, rect);

                let area = Rect {
                    height: rect.height - 1,
                    ..rect
                };

                let max_height = 4;
                let should_scroll = size > max_height;
                let width = if should_scroll {
                    41
                } else {
                    40
                };
                let scrollbar_area = Rect {
                    x: area.x + x as u16 + 1,
                    y: area.y + y as u16 + 2,
                    height: cmp::min(max_height, size),
                    width,
                };
                let menu_area = Rect {
                    x: area.x + x as u16 + 1,
                    y: area.y + y as u16 + 2,
                    height: cmp::min(max_height, size),
                    width: 40,
                };

                frame.render_widget(&text_area, area);
                if show_table {
                    frame.render_widget(Clear::default(), scrollbar_area);
                    frame.render_stateful_widget(table, scrollbar_area, &mut table_state);
                    if should_scroll {
                        frame.render_stateful_widget(
                            Scrollbar::default()
                                .orientation(ScrollbarOrientation::VerticalRight)
                                .symbols(Set {
                                    track: " ",
                                    thumb: "█",
                                    begin: "↑",
                                    end: "↓",
                                })
                                .begin_symbol(None)
                                .end_symbol(None),
                            scrollbar_area.inner(Margin {
                                vertical: 0,
                                horizontal: 0,
                            }),
                            &mut scroll_state,
                        );
                    }
                }
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
                            } else if show_table {
                                show_table = false;
                            }
                        }
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
                        KeyEvent { code: KeyCode::Up, modifiers: KeyModifiers::NONE, .. } => {
                            table_state.select_previous();
                            scroll_state.prev();
                        }
                        KeyEvent { code: KeyCode::Down, modifiers: KeyModifiers::NONE, .. } => {
                            table_state.select_next();
                            scroll_state.next();
                        }
                        KeyEvent { code: KeyCode::Tab | KeyCode::Enter, modifiers: KeyModifiers::NONE, .. } => {
                            if !items.is_empty() && show_table {
                                if let Some(selected) = table_state.selected() {
                                    if let Some(item) = items.get(selected) {
                                        text_area.start_selection();
                                        text_area.move_cursor(CursorMove::WordBack);
                                        // text_area.input(tui_textarea::Input {
                                        //     key: tui_textarea::Key::Backspace,
                                        //     ..tui_textarea::Input::default()
                                        // });
                                        text_area.insert_str(item.insert_text.clone());
                                        show_table = false;
                                    }
                                }
                            }
                        }
                        input => {
                            if text_area.input(input) {
                                show_table = true;
                            };
                        }
                    }
                }
            }
        };
    }
    ratatui::restore();
    Ok(())
}

fn get_table(rows: Vec<Row>) -> Table {
    let table = Table::new(rows, [Min(1), Length(8), Length(0)])
        // .block(Block::bordered().border_type(BorderType::Rounded))
        .style(Style::default().bg(tailwind::NEUTRAL.c800))
        .highlight_style(Style::default().bg(tailwind::ZINC.c900).bold());
    table
}

fn get_rows(input: impl Into<String>, items: &Vec<CompletionItem>) -> Vec<Row> {
    let input = input.into();
    let mut rows = vec![];
    for item in items {
        let mut prompt = Line::default();
        if let Some(pos) = item.label.label.find(input.clone().as_str()) {
            prompt.push_span(Span::raw(&item.label.label[0..pos]));
            prompt.push_span(Span::raw(input.clone()).style(Style::default().fg(tailwind::AMBER.c500)));
            prompt.push_span(Span::raw(&item.label.label[pos + input.len()..item.label.label.len()]));
        }
        // if let Some(strip) = item.label.label.strip_prefix(input) {
        //     prompt.push_span(Span::raw(input).style(Style::default().fg(tailwind::AMBER.c500)));
        //     prompt.push_span(Span::raw(strip));
        // }
        if let Some(ref detail) = item.label.detail {
            prompt.push_span(Span::raw(" "));
            prompt.push_span(Span::raw(detail).style(Style::default().dim()));
        }
        let prompt = Cell::new(prompt);
        let kind = Cell::new(Line::raw(item.kind.to_string())
            .alignment(Alignment::Right)
            .style(Style::default().dim())
        );
        let row = Row::new(vec![prompt, kind]);
        rows.push(row);
    }
    rows
}

fn get_items(input: &str) -> Vec<CompletionItem> {
    let mut rows = vec![];
    for item in TOAST_CHANNEL.iter() {
        if item.label.label.contains(input) {
            rows.push(item.clone())
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
    vec.push(CompletionItem::hash("scan"));
    vec.push(CompletionItem::hash("ping"));
    vec.push(CompletionItem::hash("monitor"));
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

#[derive(PartialEq, Eq, Clone, Copy)]
enum CharKind {
    Space,
    Punct,
    Other,
}

impl CharKind {
    fn new(c: char) -> Self {
        if c.is_whitespace() {
            Self::Space
        } else if c.is_ascii_punctuation() {
            Self::Punct
        } else {
            Self::Other
        }
    }
}

pub fn find_word_start_forward(line: &str, start_col: usize) -> Option<usize> {
    let mut it = line.chars().enumerate().skip(start_col);
    let mut prev = CharKind::new(it.next()?.1);
    for (col, c) in it {
        let cur = CharKind::new(c);
        if cur != CharKind::Space && prev != cur {
            return Some(col);
        }
        prev = cur;
    }
    None
}

pub fn find_word_exclusive_end_forward(line: &str, start_col: usize) -> Option<usize> {
    let mut it = line.chars().enumerate().skip(start_col);
    let mut prev = CharKind::new(it.next()?.1);
    for (col, c) in it {
        let cur = CharKind::new(c);
        if prev != CharKind::Space && prev != cur {
            return Some(col);
        }
        prev = cur;
    }
    None
}

pub fn find_word_inclusive_end_forward(line: &str, start_col: usize) -> Option<usize> {
    let mut it = line.chars().enumerate().skip(start_col);
    let (mut last_col, c) = it.next()?;
    let mut prev = CharKind::new(c);
    for (col, c) in it {
        let cur = CharKind::new(c);
        if prev != CharKind::Space && cur != prev {
            return Some(col.saturating_sub(1));
        }
        prev = cur;
        last_col = col;
    }
    if prev != CharKind::Space {
        Some(last_col)
    } else {
        None
    }
}

pub fn find_word_start_backward(line: &str, start_col: usize) -> Option<usize> {
    let idx = line
        .char_indices()
        .nth(start_col)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let mut it = line[..idx].chars().rev().enumerate();
    let mut cur = CharKind::new(it.next()?.1);
    for (i, c) in it {
        let next = CharKind::new(c);
        if cur != CharKind::Space && next != cur {
            return Some(start_col - i);
        }
        cur = next;
    }
    (cur != CharKind::Space).then(|| 0)
}
