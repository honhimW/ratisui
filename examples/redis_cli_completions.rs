#![allow(unused)]

use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::Lazy;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::palette::tailwind;
use ratatui::style::{Style, Stylize};
use ratatui::symbols::scrollbar::Set;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
    TableState,
};
use ratatui::Frame;
use std::cmp;
use std::time::Duration;
use strum::Display;
use tui_textarea::{CursorMove, TextArea};

fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    let mut text_area = TextArea::default();
    text_area.set_cursor_style(Style::default().rapid_blink().reversed());
    text_area.set_cursor_line_style(Style::default());
    text_area.set_block(Block::bordered().border_type(BorderType::Thick));
    let mut table_state = TableState::default();
    let mut scroll_state = ScrollbarState::default();
    table_state.select_first();
    scroll_state.first();
    let mut show_table = false;
    loop {
        let (cursor_y, cursor_x) = text_area.cursor();
        let input = text_area.lines().get(cursor_y).unwrap().clone();
        let (items, cmd) = get_items(&input, cursor_x);
        scroll_state = scroll_state.content_length(items.len());
        let rows = get_rows(&cmd, &items);
        let table = get_table(rows);
        let size = items.len() as u16;
        terminal.draw(|frame: &mut Frame| {
            let rect = frame.area();
            let Rect {
                height: max_height,
                width: max_width,
                ..
            } = rect;
            let menu_width = 50;
            if max_width <= menu_width || max_height <= 11 {
                return;
            }
            let rect = centered_rect(100, 10, rect);

            let area = Rect {
                height: rect.height - 1,
                ..rect
            };

            let max_menu_height = 10;
            let should_scroll = size > max_menu_height;

            let mut menu_area = Rect {
                x: area.x + cursor_x as u16 + 1,
                y: area.y + cursor_y as u16 + 2,
                height: cmp::min(max_menu_height, size),
                width: menu_width,
            };
            if menu_area.x + menu_area.width > max_width {
                let x_offset = menu_area.x + menu_area.width - max_width;
                menu_area.x = menu_area.x.saturating_sub(x_offset);
            }
            if menu_area.y + menu_area.height > max_height {
                menu_area.y = menu_area
                    .y
                    .saturating_sub(menu_area.height)
                    .saturating_sub(1);
            }

            frame.render_widget(&text_area, area);
            if show_table {
                frame.render_widget(Clear::default(), menu_area);
                frame.render_stateful_widget(table, menu_area, &mut table_state);
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
                        menu_area.inner(Margin {
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
                        KeyEvent {
                            modifiers: KeyModifiers::CONTROL,
                            code: KeyCode::Char('c'),
                            ..
                        } => {
                            break;
                        }
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        } => {
                            if text_area.is_selecting() {
                                text_area.cancel_selection();
                            } else if show_table {
                                show_table = false;
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char(' '),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            show_table = true;
                        }
                        KeyEvent {
                            code: KeyCode::Char('m'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {}
                        KeyEvent {
                            code: KeyCode::Char('a'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            text_area.select_all();
                        }
                        KeyEvent {
                            code: KeyCode::Char('z'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            text_area.undo();
                        }
                        KeyEvent {
                            code: KeyCode::Char('y'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            text_area.redo();
                        }
                        KeyEvent {
                            code: KeyCode::Up,
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            table_state.select_previous();
                            scroll_state.prev();
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            table_state.select_next();
                            scroll_state.next();
                        }
                        KeyEvent {
                            code: KeyCode::Tab | KeyCode::Enter,
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            if !items.is_empty() && show_table {
                                if let Some(selected) = table_state.selected() {
                                    if let Some(item) = items.get(selected) {
                                        show_table = false;
                                        if input.is_empty() {
                                            text_area.insert_str(item.insert_text.clone());
                                        } else {
                                            let (s, mut e) = item.range;
                                            if e < 0 {
                                                e = input.len() as isize;
                                            }
                                            text_area.move_cursor(CursorMove::Jump(
                                                cursor_y as u16,
                                                s as u16,
                                            ));
                                            text_area.start_selection();
                                            for _ in 0..(e - s) {
                                                text_area.move_cursor(CursorMove::Forward);
                                            }
                                            text_area.insert_str(item.insert_text.clone());
                                        }
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
    let table = Table::new(rows, [Min(1), Length(7), Length(0)])
        .style(Style::default().bg(tailwind::NEUTRAL.c800))
        .row_highlight_style(Style::default().bg(tailwind::ZINC.c900).bold());
    table
}

fn get_rows(input: impl Into<String>, items: &Vec<CompletionItem>) -> Vec<Row> {
    let input = input.into();
    let mut rows = vec![];
    for item in items {
        let mut prompt = Line::default();
        if let Some(pos) = item.label.label.find(input.clone().as_str()) {
            prompt.push_span(Span::raw(&item.label.label[0..pos]));
            prompt.push_span(
                Span::raw(input.clone()).style(Style::default().fg(tailwind::AMBER.c500)),
            );
            prompt.push_span(Span::raw(
                &item.label.label[pos + input.len()..item.label.label.len()],
            ));
        }
        if let Some(ref detail) = item.label.detail {
            prompt.push_span(Span::raw(" "));
            prompt.push_span(Span::raw(detail).style(Style::default().dim()));
        }
        let prompt = Cell::new(prompt);
        let kind = Cell::new(
            Line::raw(item.kind.to_string())
                .alignment(Alignment::Right)
                .style(Style::default().dim()),
        );
        let row = Row::new(vec![prompt, kind]);
        rows.push(row);
    }
    rows
}

fn get_items(input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
    let args = split_args(input);

    // Find current word
    let mut current_word: Option<(usize, String, Option<char>, usize, usize)> = None;
    let mut segment = String::new();
    for (idx, (arg, quote, start_pos, end_pos)) in args.iter().enumerate() {
        if start_pos <= &cursor_x && &cursor_x <= end_pos {
            current_word = Some((
                idx,
                arg.clone(),
                quote.clone(),
                start_pos.clone(),
                end_pos.clone(),
            ));
            segment = (&input[*start_pos..cursor_x]).to_uppercase();
            break;
        }
    }

    let mut commands = vec![];
    // Find command by first word
    for item in TOAST_CHANNEL.iter() {
        let mut item_clone = item.clone();
        if let Some((idx, ref _cmd, _, start_pos, end_pos)) = current_word {
            if idx == 0 {
                if item.label.label.contains(&segment) {
                    item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                    commands.push(item_clone);
                }
            } else {
                if let Some((cmd, _, start_pos, end_pos)) = args.first() {
                    if &item.label.label == &cmd.to_uppercase() {
                        item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                        commands.push(item_clone);
                        break;
                    }
                }
            }
        } else {
            if let Some((cmd, _, start_pos, end_pos)) = args.first() {
                if &item.label.label == &cmd.to_uppercase() {
                    item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                    commands.push(item.clone());
                    break;
                }
            } else {
                commands.push(item.clone());
            }
        }
    }

    if let Some((idx, _, _, _, _)) = current_word {
        if idx == 0 {
            return (commands, segment);
        }
    }

    if !commands.is_empty() {
        let mut parameters = vec![];
        let (start, end) = if let Some((_, _, _, start_pos, end_pos)) = current_word {
            (start_pos as isize, end_pos as isize)
        } else {
            (0, -1)
        };
        for item in commands.iter() {
            for param in item.parameters.iter() {
                match param {
                    Parameter::Flag(flag, detail) => {
                        if flag.contains(&segment) {
                            parameters.push(
                                CompletionItem::option(flag)
                                    .detail(detail)
                                    .range(start, end),
                            );
                        }
                    }
                    Parameter::Enum(es) => {
                        for (e, detail) in es {
                            if e.contains(&segment) {
                                parameters.push(
                                    CompletionItem::option(e).detail(detail).range(start, end),
                                );
                            }
                        }
                    }
                    Parameter::Arg { key, detail, .. } => {
                        if key.contains(&segment) {
                            parameters
                                .push(CompletionItem::option(key).detail(detail).range(start, end));
                        }
                    }
                    _ => {}
                }
            }
        }
        commands = parameters;
    }

    (commands, segment)
}

static TOAST_CHANNEL: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let mut vec = vec![];
    vec.push(
        CompletionItem::default("SCAN")
            .add_param(Parameter::single("CURSOR", "* cursor"))
            .add_param(Parameter::arg("MATCH", "pattern", "pattern"))
            .add_param(Parameter::arg("COUNT", "count", "count"))
            .add_param(Parameter::arg("TYPE", "type", "type"))
            .build_label(),
    );
    vec.push(CompletionItem::default("TTL"));
    vec.push(CompletionItem::default("EXPIRE"));
    vec.push(CompletionItem::default("MGET"));
    vec.push(CompletionItem::default("PING").server());
    vec.push(CompletionItem::default("INFO").server());
    vec.push(CompletionItem::default("MONITOR").server());
    vec.push(CompletionItem::default("SET").string());
    vec.push(CompletionItem::default("GET").string());
    vec.push(CompletionItem::default("STRLEN").string());
    vec.push(CompletionItem::default("HGETALL").hash());
    vec.push(CompletionItem::default("HGET").hash());
    vec.push(CompletionItem::default("SADD").set());
    vec.push(CompletionItem::default("ZADD").z_set());
    vec
});

#[derive(Debug, Clone)]
struct CompletionItem {
    kind: CompletionItemKind,
    label: Label,
    parameters: Vec<Parameter>,
    range: (isize, isize),
    insert_text: String,
}

#[derive(Clone, Debug)]
enum Parameter {
    None,                        // monitor
    Flag(String, String),        // [CH]
    Enum(Vec<(String, String)>), // [NX | XX]
    Arg {
        // [match pattern]
        key: String, // match
        arg: String, // pattern
        detail: String,
    },
    Single(String, String), // cursor
    Many(String, String),   // score member [score members...], tail
}

impl Parameter {
    fn flag(s: impl Into<String>, detail: impl Into<String>) -> Parameter {
        Parameter::Flag(s.into(), detail.into())
    }

    fn enums(vec: Vec<(impl Into<String>, impl Into<String>)>) -> Parameter {
        Parameter::Enum(
            vec.into_iter()
                .map(|(s, detail)| (s.into(), detail.into()))
                .collect(),
        )
    }

    fn arg(key: impl Into<String>, arg: impl Into<String>, detail: impl Into<String>) -> Parameter {
        Parameter::Arg {
            key: key.into(),
            arg: arg.into(),
            detail: detail.into(),
        }
    }

    fn single(s: impl Into<String>, detail: impl Into<String>) -> Parameter {
        Parameter::Single(s.into(), detail.into())
    }

    fn many(s: impl Into<String>, detail: impl Into<String>) -> Parameter {
        Parameter::Many(s.into(), detail.into())
    }

    fn to_string(&self) -> String {
        let mut detail = String::new();
        match self {
            Parameter::None => {}
            Parameter::Flag(flag, _) => {
                detail.push('[');
                detail.push_str(flag);
                detail.push(']');
            }
            Parameter::Enum(es) => {
                detail.push('[');
                detail.push_str(es.iter().map(|(e, _)| e).join(" | ").as_str());
                detail.push(']');
            }
            Parameter::Arg { key, arg, .. } => {
                detail.push('[');
                detail.push_str(key);
                detail.push(' ');
                detail.push_str(arg);
                detail.push(']');
            }
            Parameter::Single(name, _) => {
                detail.push_str(name);
            }
            Parameter::Many(name, _) => {
                detail.push_str(name);
                detail.push_str(" [");
                detail.push_str(name);
                detail.push_str("...]");
            }
        }
        detail
    }
}

impl CompletionItem {
    fn empty() -> CompletionItem {
        Self::new("", CompletionItemKind::Generic)
    }

    fn default(s: impl Into<String>) -> CompletionItem {
        Self::new(s, CompletionItemKind::Generic)
    }

    fn option(s: impl Into<String>) -> CompletionItem {
        Self::new(s, CompletionItemKind::Option)
    }

    fn new(s: impl Into<String>, kind: CompletionItemKind) -> CompletionItem {
        let s = s.into();
        Self {
            kind,
            label: Label {
                label: s.clone(),
                detail: Some(s.clone()),
                description: Some(s.clone()),
            },
            parameters: vec![],
            range: (0, -1),
            insert_text: s,
        }
    }

    fn string(mut self) -> Self {
        self.kind = CompletionItemKind::String;
        self
    }

    fn list(mut self) -> Self {
        self.kind = CompletionItemKind::List;
        self
    }

    fn set(mut self) -> Self {
        self.kind = CompletionItemKind::Set;
        self
    }

    fn z_set(mut self) -> Self {
        self.kind = CompletionItemKind::ZSet;
        self
    }

    fn hash(mut self) -> Self {
        self.kind = CompletionItemKind::Hash;
        self
    }

    fn stream(mut self) -> Self {
        self.kind = CompletionItemKind::Stream;
        self
    }

    fn pub_sub(mut self) -> Self {
        self.kind = CompletionItemKind::PubSub;
        self
    }

    fn server(mut self) -> Self {
        self.kind = CompletionItemKind::Server;
        self
    }

    fn add_param(mut self, p: Parameter) -> Self {
        self.parameters.push(p);
        self
    }

    fn detail(mut self, s: impl Into<String>) -> Self {
        self.label.detail = Some(s.into());
        self
    }

    fn description(mut self, s: impl Into<String>) -> Self {
        self.label.description = Some(s.into());
        self
    }

    fn range(mut self, start: isize, end: isize) -> Self {
        self.range = (start.into(), end.into());
        self
    }

    fn build_label(mut self) -> Self {
        if !self.parameters.is_empty() {
            let mut detail = String::new();
            for parameter in self.parameters.iter() {
                let detail_part = parameter.to_string();
                detail.push_str(detail_part.as_str());
                detail.push(' ');
            }
            self.label.detail = Some(detail);
        }
        self
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
    Generic,
    Option,
    String,
    List,
    Set,
    ZSet,
    Hash,
    Stream,
    PubSub,
    Server,
}

pub fn split_args(cmd: impl Into<String>) -> Vec<(String, Option<char>, usize, usize)> {
    let cmd = cmd.into();

    let mut parts: Vec<(String, Option<char>, usize, usize)> = vec![];
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';

    let mut cursor: usize = 0;
    let mut start: usize = 0;
    for c in cmd.chars() {
        if in_quotes {
            if c == quote_char {
                in_quotes = false;
                parts.push((current.clone(), Some(quote_char), start, cursor));
                current.clear();
            } else {
                current.push(c);
            }
        } else {
            if c.is_whitespace() {
                if !current.is_empty() {
                    parts.push((current.clone(), None, start, cursor));
                    current.clear();
                }
                start = cursor + 1;
            } else if c == '\'' || c == '"' {
                in_quotes = true;
                quote_char = c;
                start = cursor + 1;
            } else {
                current.push(c);
            }
        }
        cursor += 1;
    }

    // if !current.is_empty() {
    //     parts.push((current, None, start, cursor));
    // }
    parts.push((current, None, start, cursor));
    parts
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

pub fn find_word_start_backward(line: &str, start_col: usize) -> usize {
    let idx = line
        .char_indices()
        .nth(start_col)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let mut it = line[..idx].chars().rev().enumerate();
    let mut cur = if let Some(next) = it.next() {
        CharKind::new(next.1)
    } else {
        CharKind::Space
    };
    if cur == CharKind::Space {
        return start_col;
    }
    for (i, c) in it {
        let next = CharKind::new(c);
        if cur != CharKind::Space && next != cur {
            return start_col - i;
        }
        cur = next;
    }
    if cur != CharKind::Space {
        0
    } else {
        start_col
    }
}
