use crate::app::{Listenable, Renderable};
use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::Lazy;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::palette::tailwind;
use ratatui::style::{Style, Stylize};
use ratatui::symbols::scrollbar::Set;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap};
use ratatui::{symbols, Frame};
use std::cmp;
use strum::Display;
use tui_textarea::{CursorMove, TextArea};

pub struct RedisCli<'a> {
    max_menu_width: u16,
    max_menu_height: u16,
    max_desc_width: u16,
    max_desc_height: u16,
    single_line_text_area: TextArea<'a>,
    table_state: TableState,
    scroll_state: ScrollbarState,
    show_table: bool,
    frame_size: (u16, u16), // max width, max height
    completion_items: Vec<CompletionItem>,
    raw_input: String,
    segment: String,
}

impl RedisCli<'_> {
    pub fn new() -> Self {
        let mut text_area = TextArea::default();
        text_area.set_cursor_style(Style::default().rapid_blink().reversed());
        text_area.set_cursor_line_style(Style::default());
        let mut table_state = TableState::default();
        table_state.select_first();
        let mut scroll_state = ScrollbarState::default();
        scroll_state.first();
        Self {
            max_menu_width: 50,
            max_menu_height: 11,
            max_desc_width: 35,
            max_desc_height: 20,
            single_line_text_area: text_area,
            table_state,
            scroll_state,
            show_table: false,
            frame_size: (0, 0),
            completion_items: vec![],
            raw_input: "".to_string(),
            segment: "".to_string(),
        }
    }
}

impl Renderable for RedisCli<'_> {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let (cursor_y, cursor_x) = self.single_line_text_area.cursor();
        let items = &self.completion_items;
        let segment = self.segment.clone();
        self.scroll_state = self.scroll_state.content_length(items.len());
        let rows = get_rows(&segment, &items);
        let table = get_table(rows);
        let size = items.len() as u16;

        let max_height = self.frame_size.0;
        let max_width = self.frame_size.1;

        if max_width <= self.max_menu_width || max_height <= (self.max_menu_height + 1) {
            return Ok(());
        }

        let input_area = rect;

        let should_scroll = size > self.max_menu_height;

        let mut menu_area = Rect {
            x: input_area.x + cursor_x as u16 + 1,
            y: input_area.y + cursor_y as u16 + 1,
            height: cmp::min(self.max_menu_height, size + 1),
            width: self.max_menu_width,
        };
        if menu_area.x + menu_area.width > max_width {
            let x_offset = menu_area.x + menu_area.width - max_width;
            menu_area.x = menu_area.x.saturating_sub(x_offset);
        }
        if menu_area.y + menu_area.height > max_height {
            menu_area.y = menu_area.y.saturating_sub(menu_area.height).saturating_sub(1);
        }

        frame.render_widget(&self.single_line_text_area, input_area);
        if self.show_table && self.completion_items.len() > 0 {
            frame.render_widget(Clear::default(), menu_area);
            let vertical = Layout::vertical([Fill(1), Length(1)]).split(menu_area);
            frame.render_stateful_widget(table, vertical[0], &mut self.table_state);
            self.render_menu_footer(frame, vertical[1]);
            if should_scroll {
                let scrollbar = Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .symbols(Set {
                        track: " ",
                        thumb: "█",
                        begin: "↑",
                        end: "↓",
                    })
                    .begin_symbol(None)
                    .end_symbol(None);
                frame.render_stateful_widget(scrollbar, vertical[0].inner(Margin {
                    vertical: 0,
                    horizontal: 0,
                }), &mut self.scroll_state);
            }

            if let Some(idx) = self.table_state.selected() {
                if let Some(item) = self.completion_items.get(idx) {
                    if let Some(ref desc) = item.label.description {
                        self.render_desc(frame, menu_area, desc.clone());
                    }
                }
            }
        }
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        todo!()
    }
}

impl Listenable for RedisCli<'_> {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<bool> {
        if event.kind == KeyEventKind::Press {
            let accepted = match event {
                KeyEvent { code: KeyCode::Esc, .. } => {
                    if self.single_line_text_area.is_selecting() {
                        self.single_line_text_area.cancel_selection();
                        true
                    } else if self.show_table {
                        self.show_table = false;
                        true
                    } else {
                        false
                    }
                }
                KeyEvent { code: KeyCode::Char(' '), modifiers: KeyModifiers::CONTROL, .. } => {
                    self.show_table = true;
                    true
                }
                KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {
                    false
                }
                KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => {
                    self.single_line_text_area.select_all();
                    true
                }
                KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                    self.single_line_text_area.undo();
                    true
                }
                KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                    self.single_line_text_area.redo();
                    true
                }
                KeyEvent { code: KeyCode::Up, modifiers: KeyModifiers::NONE, .. } => {
                    if self.show_table {
                        self.previous();
                        true
                    } else {
                        false
                    }
                }
                KeyEvent { code: KeyCode::Down, modifiers: KeyModifiers::NONE, .. } => {
                    if self.show_table {
                        self.next();
                        true
                    } else {
                        false
                    }
                }
                KeyEvent { code: KeyCode::Tab | KeyCode::Enter, .. } => {
                    if !self.completion_items.is_empty() && self.show_table {
                        if let Some(selected) = self.table_state.selected() {
                            if let Some(item) = self.completion_items.get(selected) {
                                self.show_table = false;
                                if self.raw_input.is_empty() {
                                    self.single_line_text_area.insert_str(item.insert_text.clone());
                                } else {
                                    let (s, mut e) = item.range;
                                    if e < 0 {
                                        e = self.raw_input.len() as isize;
                                    }
                                    let (cursor_y, cursor_x) = self.single_line_text_area.cursor();
                                    self.single_line_text_area.move_cursor(CursorMove::Jump(cursor_y as u16, s as u16));
                                    self.single_line_text_area.start_selection();
                                    for _ in 0..(e - s) {
                                        self.single_line_text_area.move_cursor(CursorMove::Forward);
                                    }
                                    self.single_line_text_area.insert_str(item.insert_text.clone());
                                }
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                input => {
                    if self.single_line_text_area.input(input) {
                        self.show_table = true;
                        true
                    } else {
                        false
                    }
                }
            };
            if accepted {
                let (cursor_y, cursor_x) = self.single_line_text_area.cursor();
                self.raw_input = self.single_line_text_area.lines().get(cursor_y).unwrap().clone();
                let (items, segment) = get_items(&self.raw_input, cursor_x);
                self.completion_items = items;
                self.segment = segment;
                self.scroll_state = self.scroll_state.content_length(self.completion_items.len());
            }
            Ok(accepted)
        } else {
            Ok(false)
        }
    }
}

impl RedisCli<'_> {
    pub fn insert_str(&mut self, s: impl Into<String>) {
        self.single_line_text_area.insert_str(s.into());
    }

    pub fn get_input(&self) -> String {
        let (cursor_y, _) = self.single_line_text_area.cursor();
        self.single_line_text_area.lines().get(cursor_y).unwrap().clone()
    }

    pub fn update_frame(&mut self, frame_height: u16, frame_width: u16) {
        self.frame_size = (frame_height, frame_width);
    }

    fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.completion_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i);
    }

    fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.completion_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i);
    }

    fn render_menu_footer(&mut self, frame: &mut Frame, area: Rect) {
        let horizontal = Layout::horizontal([Fill(1), Length(7)]).split(area);
        let selected = self.table_state.selected().unwrap_or(0);
        let style = Style::default().bg(tailwind::STONE.c900).italic();
        let info = Line::raw("↑/↓ Tab/Enter").style(style);
        let item_count = Line::raw(format!("{}:{}", selected.saturating_add(1), self.completion_items.len()))
            .alignment(Alignment::Right)
            .style(style);
        frame.render_widget(info, horizontal[0]);
        frame.render_widget(item_count, horizontal[1]);
    }

    fn render_desc(&mut self, frame: &mut Frame, menu_area: Rect, doc: Doc) {
        let (max_height, max_width) = self.frame_size;
        let desc_width = self.max_desc_width;
        let Rect {
            x, y, height: menu_height, width: menu_width
        } = menu_area;
        let paragraph = Paragraph::new(highlight_doc(&doc))
            .wrap(Wrap { trim: false })
            .block(Block::default().border_set(symbols::border::EMPTY).borders(Borders::from_bits_retain(0b1010)))
            .style(Style::default().bg(tailwind::NEUTRAL.c800))
            ;
        let line_count = paragraph.line_count(desc_width - 2); // block width 2
        let desc_height = cmp::min(line_count as u16, self.max_desc_height);

        let mut is_right = true;
        let mut is_bottom = true;
        let desc_width = self.max_desc_width;
        let right_area_width = max_width.saturating_sub(menu_width.saturating_add(x));
        if right_area_width < self.max_desc_width {
            if x < self.max_desc_width {
                return;
            } else {
                is_right = false;
            }
        }

        let bottom_area_height = max_height.saturating_sub(y);
        if bottom_area_height < self.max_desc_height {
            if y.saturating_sub(2) < self.max_desc_height {
                return;
            } else {
                is_bottom = false;
            }
        }

        let desc_area = Rect {
            x: if is_right { x.saturating_add(menu_width) } else { x.saturating_sub(desc_width) },
            y: if is_bottom { y } else { y.saturating_add(menu_height).saturating_sub(desc_height) },
            width: desc_width,
            height: desc_height,
        };
        frame.render_widget(Clear::default(), desc_area);
        frame.render_widget(paragraph, desc_area);
    }
}

fn get_table(rows: Vec<Row>) -> Table {
    let table = Table::new(rows, [Min(1), Length(7), Length(0)])
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

fn get_items(input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
    let args = split_args(input);

    /// Find current word
    let mut current_word: Option<(usize, String, Option<char>, usize, usize)> = None;
    let mut segment = String::new();
    for (idx, (arg, quote, start_pos, end_pos)) in args.iter().enumerate() {
        if start_pos <= &cursor_x && &cursor_x <= end_pos {
            current_word = Some((idx, arg.clone(), quote.clone(), start_pos.clone(), end_pos.clone()));
            segment = (&input[*start_pos..cursor_x]).to_ascii_uppercase();
            break;
        }
    }

    let mut commands = vec![];
    /// Find command by first word
    for item in COMMANDS.iter() {
        let mut item_clone = item.clone();
        if let Some((idx, ref cmd, _, start_pos, end_pos)) = current_word {
            if idx == 0 {
                if item.label.label.contains(&segment) {
                    item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                    commands.push(item_clone);
                }
            } else {
                if let Some((cmd, _, start_pos, end_pos)) = args.first() {
                    if &item.label.label == &cmd.to_ascii_uppercase() {
                        item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                        commands.push(item_clone);
                        break;
                    }
                }
            }
        } else {
            if let Some((cmd, _, start_pos, end_pos)) = args.first() {
                if &item.label.label == &cmd.to_ascii_uppercase() {
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
                            parameters.push(CompletionItem::option(flag).detail(detail).range(start, end));
                        }
                    }
                    Parameter::Enum(es) => {
                        for (e, detail) in es {
                            if e.contains(&segment) {
                                parameters.push(CompletionItem::option(e).detail(detail).range(start, end));
                            }
                        }
                    }
                    Parameter::Arg { key, detail, .. } => {
                        if key.contains(&segment) {
                            parameters.push(CompletionItem::option(key).detail(detail).range(start, end));
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
    None,              // monitor
    Flag(String, String),      // [CH]
    Enum(Vec<(String, String)>), // [NX | XX]
    Arg {              // [match pattern]
        key: String,   // match
        arg: String,   // pattern
        detail: String,
    },
    Single(String, String),    // cursor
    Many(String, String),            // score member [score members...], tail
}

impl Parameter {
    fn flag(s: impl Into<String>, detail: impl Into<String>) -> Parameter {
        Parameter::Flag(s.into(), detail.into())
    }

    fn enums(vec: Vec<(impl Into<String>, impl Into<String>)>) -> Parameter {
        Parameter::Enum(vec.into_iter().map(|(s, detail)| (s.into(), detail.into())).collect())
    }

    fn arg(key: impl Into<String>, arg: impl Into<String>, detail: impl Into<String>) -> Parameter {
        Parameter::Arg { key: key.into(), arg: arg.into(), detail: detail.into() }
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
                detail.push_str(es.iter().map(|(e, _)| { e }).join(" | ").as_str());
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
                detail: None,
                description: None,
            },
            parameters: vec![],
            range: (0, -1),
            insert_text: s,
        }
    }

    fn generic(mut self) -> Self {
        self.kind = CompletionItemKind::Generic;
        self
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

    fn description(mut self, doc: Doc) -> Self {
        self.label.description = Some(doc);
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
    description: Option<Doc>,
}

#[derive(Debug, Clone)]
struct Doc {
    syntax: String,
    summary: String,
    since: Option<String>,
    acl: Option<String>,
    complexity: Option<String>,
}

impl Doc {
    fn default() -> Self {
        Self {
            syntax: String::new(),
            summary: String::new(),
            since: None,
            acl: None,
            complexity: None,
        }
    }

    fn syntax(mut self, s: impl Into<String>) -> Self {
        self.syntax = s.into();
        self
    }

    fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = s.into();
        self
    }

    fn since(mut self, s: impl Into<String>) -> Self {
        self.since = Some(s.into());
        self
    }

    fn acl(mut self, s: impl Into<String>) -> Self {
        self.acl = Some(s.into());
        self
    }

    fn complexity(mut self, s: impl Into<String>) -> Self {
        self.complexity = Some(s.into());
        self
    }
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

fn split_args(cmd: impl Into<String>) -> Vec<(String, Option<char>, usize, usize)> {
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

fn centered_rect(percentage_x: u16, percentage_y: u16, area: Rect) -> Rect {
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

fn find_word_start_forward(line: &str, start_col: usize) -> Option<usize> {
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

fn find_word_exclusive_end_forward(line: &str, start_col: usize) -> Option<usize> {
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

fn find_word_inclusive_end_forward(line: &str, start_col: usize) -> Option<usize> {
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

fn find_word_start_backward(line: &str, start_col: usize) -> usize {
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


/// Redis Commands Completion Items Definition

fn highlight_doc(doc: &Doc) -> Text {
    let mut text = Text::default();
    text.push_line(Line::raw(doc.syntax.clone()).style(Style::default().fg(tailwind::AMBER.c400)));
    text.push_line(Line::raw(""));
    text.push_line(Line::raw(doc.summary.clone()));
    if let Some(since) = &doc.since {
        let mut line = Line::default();
        line.push_span(Span::raw("since     : ").style(Style::default().fg(tailwind::PINK.c800)));
        line.push_span(since);
        text.push_line(line);
    }
    if let Some(acl) = &doc.acl {
        let mut line = Line::default();
        line.push_span(Span::raw("acl       : ").style(Style::default().fg(tailwind::PINK.c800)));
        line.push_span(acl);
        text.push_line(line);
    }
    if let Some(complexity) = &doc.complexity {
        let mut line = Line::default();
        line.push_span(Span::raw("complexity: ").style(Style::default().fg(tailwind::PINK.c800)));
        line.push_span(complexity);
        text.push_line(line);
    }

    text
}

static COMMANDS: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let mut vec = vec![];
    vec.push(CompletionItem::default("COPY").generic()
        .add_param(Parameter::single("source", ""))
        .add_param(Parameter::single("destination", ""))
        .add_param(Parameter::arg("DB", "destination-db", "database index"))
        .add_param(Parameter::flag("REPLACE", "removes the destination key"))
        .description(Doc::default()
            .syntax("COPY source destination [DB destination-db] [REPLACE]")
            .summary("This command copies the value stored at the source key to the destination key.")
            .since("6.2.0")
            .complexity("O(N) worst case for collections, where N is the number of nested items. O(1) for string values.")
            .acl("@keyspace, @write, @slow")
        )
        .build_label());

    vec.push(CompletionItem::default("DEL").generic()
        .add_param(Parameter::many("key", ""))
        .description(Doc::default()
            .syntax("DEL key [key ...]")
            .summary("Removes the specified keys. A key is ignored if it does not exist.")
            .since("1.0.0")
            .complexity("O(N) where N is the number of keys that will be removed. When a key to remove holds a value other than a string, the individual complexity for this key is O(M) where M is the number of elements in the list, set, sorted set or hash. Removing a single key that holds a string value is O(1).")
            .acl("@keyspace, @write, @slow")
        )
        .build_label());

    vec.push(CompletionItem::default("DUMP").generic()
        .add_param(Parameter::many("key", ""))
        .description(Doc::default()
            .syntax("DEL key [key ...]")
            .summary("Removes the specified keys. A key is ignored if it does not exist.")
            .since("1.0.0")
            .complexity("O(N) where N is the number of keys that will be removed. When a key to remove holds a value other than a string, the individual complexity for this key is O(M) where M is the number of elements in the list, set, sorted set or hash. Removing a single key that holds a string value is O(1).")
            .acl("@keyspace, @write, @slow")
        )
        .build_label());

    vec.push(CompletionItem::default("SCAN").generic()
        .add_param(Parameter::single("cursor", "* cursor"))
        .add_param(Parameter::arg("MATCH", "pattern", "pattern"))
        .add_param(Parameter::arg("COUNT", "count", "count"))
        .add_param(Parameter::arg("TYPE", "type", "type"))
        .description(Doc::default()
            .syntax("SCAN cursor [MATCH pattern] [COUNT count] [TYPE type]")
            .summary("iterates the set of keys in the currently selected Redis database.")
            .since("2.8.0")
            .acl("@keyspace, @read, @slow")
            .complexity("O(1) for every call. O(N) for a complete iteration, including enough command calls for the cursor to return back to 0. N is the number of elements inside the collection.")
        )
        .build_label());

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