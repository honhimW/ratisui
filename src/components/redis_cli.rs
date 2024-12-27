use crate::app::{Listenable, Renderable};
use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::Lazy;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::{Alignment, Layout, Margin, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::symbols::scrollbar::Set;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap};
use ratatui::{symbols, Frame};
use serde_json::Value;
use std::cmp;
use strum::Display;
use tui_textarea::{CursorMove, TextArea};
use crate::theme::get_color;

pub struct RedisCli<'a> {
    max_menu_width: u16,
    max_menu_height: u16,
    min_desc_width: u16,
    max_desc_width: u16,
    max_desc_height: u16,
    single_line_text_area: TextArea<'a>,
    table_state: TableState,
    scroll_state: ScrollbarState,
    show_menu: bool,
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
            min_desc_width: 35,
            max_desc_width: 50,
            max_desc_height: 20,
            single_line_text_area: text_area,
            table_state,
            scroll_state,
            show_menu: false,
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
        if self.show_menu && self.completion_items.len() > 0 {
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

}

impl Listenable for RedisCli<'_> {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<bool> {
        if event.kind == KeyEventKind::Press {
            let accepted = match event {
                KeyEvent { code: KeyCode::Esc, .. } => {
                    if self.single_line_text_area.is_selecting() {
                        self.single_line_text_area.cancel_selection();
                        true
                    } else if self.show_menu {
                        self.hide_menu();
                        true
                    } else {
                        false
                    }
                }
                KeyEvent { code: KeyCode::Char(' '), modifiers: KeyModifiers::CONTROL, .. } => {
                    self.show_menu = true;
                    true
                }
                KeyEvent { code: KeyCode::Char('m') | KeyCode::Enter, modifiers: KeyModifiers::CONTROL, .. } => {
                    false
                }
                KeyEvent { code: KeyCode::Enter, .. } => {
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
                    if self.show_menu {
                        self.previous();
                        true
                    } else {
                        false
                    }
                }
                KeyEvent { code: KeyCode::Down, modifiers: KeyModifiers::NONE, .. } => {
                    if self.show_menu {
                        self.next();
                        true
                    } else {
                        false
                    }
                }
                KeyEvent { code: KeyCode::Tab, .. } => {
                    if !self.completion_items.is_empty() && self.show_menu {
                        if let Some(selected) = self.table_state.selected() {
                            if let Some(item) = self.completion_items.get(selected) {
                                if self.raw_input.is_empty() {
                                    self.single_line_text_area.insert_str(item.insert_text.clone());
                                } else {
                                    let (s, mut e) = item.range;
                                    if e < 0 {
                                        e = self.raw_input.len() as isize;
                                    }
                                    let (cursor_y, _) = self.single_line_text_area.cursor();
                                    self.single_line_text_area.move_cursor(CursorMove::Jump(cursor_y as u16, s as u16));
                                    self.single_line_text_area.start_selection();
                                    for _ in 0..(e - s) {
                                        self.single_line_text_area.move_cursor(CursorMove::Forward);
                                    }
                                    self.single_line_text_area.insert_str(item.insert_text.clone());
                                }
                                self.hide_menu();
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
                        self.show_menu = true;
                        true
                    } else {
                        false
                    }
                }
            };
            let (cursor_y, cursor_x) = self.single_line_text_area.cursor();
            self.raw_input = self.single_line_text_area.lines().get(cursor_y).unwrap().clone();
            let (mut items, segment) = get_items(&self.raw_input, cursor_x);
            sort_commands(&mut items, &segment);
            self.completion_items = items;
            if self.segment != segment {
                self.reset_state();
            }
            self.segment = segment;
            self.scroll_state = self.scroll_state.content_length(self.completion_items.len());
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

    fn hide_menu(&mut self) {
        self.show_menu = false;
        self.reset_state();
    }

    fn reset_state(&mut self) {
        self.table_state.select_first();
        self.scroll_state.first();
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
        let style = Style::default().bg(get_color(|t| &t.tab.cli.menu.info_bg)).italic();
        let info = Line::raw("↑/↓ Tab").style(style);
        let item_count = Line::raw(format!("{}:{}", selected.saturating_add(1), self.completion_items.len()))
            .alignment(Alignment::Right)
            .style(style);
        frame.render_widget(info, horizontal[0]);
        frame.render_widget(item_count, horizontal[1]);
    }

    fn render_desc(&mut self, frame: &mut Frame, menu_area: Rect, doc: Doc) {
        let (max_height, max_width) = self.frame_size;
        let desc_width = self.min_desc_width;
        let Rect {
            x, y, height: menu_height, width: menu_width
        } = menu_area;
        let paragraph = Paragraph::new(highlight_doc(&doc))
            .wrap(Wrap { trim: false })
            .block(Block::default().border_set(symbols::border::EMPTY).borders(Borders::from_bits_retain(0b1010)))
            .style(Style::default().bg(get_color(|t| &t.tab.cli.doc.bg)))
            ;
        let line_count = paragraph.line_count(desc_width - 2); // block width 2
        let desc_height = cmp::min(line_count as u16, self.max_desc_height);

        let mut is_right = true;
        let mut is_bottom = true;
        let desc_width;
        let right_area_width = max_width.saturating_sub(menu_width.saturating_add(x));
        if right_area_width < self.min_desc_width {
            if x < self.min_desc_width {
                return;
            } else {
                is_right = false;
                desc_width = cmp::min(x, self.max_desc_width);
            }
        } else {
            desc_width = cmp::min(right_area_width, self.max_desc_width);
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
        .style(Style::default().bg(get_color(|t| &t.tab.cli.menu.bg)))
        .row_highlight_style(Style::default().bg(get_color(|t| &t.tab.cli.menu.highlight)).bold());
    table
}

fn get_rows(input: impl Into<String>, items: &Vec<CompletionItem>) -> Vec<Row> {
    let input = input.into();
    let mut rows = vec![];
    for item in items {
        let mut prompt = Line::default();
        if let Some(pos) = item.label.label.find(input.clone().as_str()) {
            prompt.push_span(Span::raw(&item.label.label[0..pos]));
            prompt.push_span(Span::raw(input.clone()).style(Style::default().fg(get_color(|t| &t.tab.cli.menu.input))));
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

    // Find current word
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
    // Find command by first word
    for item in COMMANDS.iter() {
        let mut item_clone = item.clone();
        if let Some((idx, _, _, start_pos, end_pos)) = current_word {
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

fn sort_commands(commands: &mut Vec<CompletionItem>, segment: &String) {
    commands.sort_by(|x, x1| {
        let x_starts_with = x.label.label.starts_with(segment);
        let x1_starts_with = x1.label.label.starts_with(segment);

        match (x_starts_with, x1_starts_with) {
            (true, false) => cmp::Ordering::Less,
            (false, true) => cmp::Ordering::Greater,
            _ => x.label.label.cmp(&x1.label.label),
        }
    });
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
    Flag(String, String),      // [CH]
    Enum(Vec<(String, String)>), // [NX | XX]
    Arg {              // [match pattern]
        key: String,   // match
        arg: String,   // pattern
        detail: String,
    },
    Single(String),    // cursor
    Many(String),            // score member [score members...], tail
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

    fn single(s: impl Into<String>) -> Parameter {
        Parameter::Single(s.into())
    }

    fn many(s: impl Into<String>) -> Parameter {
        Parameter::Many(s.into())
    }

    fn to_string(&self) -> String {
        let mut detail = String::new();
        match self {
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
            Parameter::Single(name) => {
                detail.push_str(name);
            }
            Parameter::Many(name) => {
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
    Other,
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

    parts.push((current, None, start, cursor));
    parts
}

/// Redis Commands Completion Items Definition

fn highlight_doc(doc: &Doc) -> Text {
    let mut text = Text::default();
    let attr_color = get_color(|t| &t.tab.cli.doc.attribute);
    text.push_line(Line::raw(doc.syntax.clone()).style(Style::default().fg(get_color(|t| &t.tab.cli.doc.command))));
    text.push_line(Line::raw(""));
    text.push_line(Line::raw(doc.summary.clone()));
    if let Some(since) = &doc.since {
        let mut line = Line::default();
        line.push_span(Span::raw("since     : ").style(Style::default().fg(attr_color)));
        line.push_span(since);
        text.push_line(line);
    }
    if let Some(acl) = &doc.acl {
        let mut line = Line::default();
        line.push_span(Span::raw("acl       : ").style(Style::default().fg(attr_color)));
        line.push_span(acl);
        text.push_line(line);
    }
    if let Some(complexity) = &doc.complexity {
        let mut line = Line::default();
        line.push_span(Span::raw("complexity: ").style(Style::default().fg(attr_color)));
        line.push_span(complexity);
        text.push_line(line);
    }

    text
}

static COMMANDS: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let redis_cmd_json = include_str!("./redis-cmd.json");
    let result = serde_json::from_str::<Vec<Value>>(redis_cmd_json);
    let mut items = vec![];
    let clear_item = CompletionItem {
        kind: CompletionItemKind::Other,
        label: Label {
            label: "CLEAR".to_string(),
            detail: Some("clear the cli screen".to_string()),
            description: None,
        },
        parameters: vec![],
        range: (0, -1),
        insert_text: "clear".to_string(),
    };
    let exit_item = CompletionItem {
        kind: CompletionItemKind::Other,
        label: Label {
            label: "EXIT".to_string(),
            detail: Some("exit the application".to_string()),
            description: None,
        },
        parameters: vec![],
        range: (0, -1),
        insert_text: "exit".to_string(),
    };
    items.push(clear_item);
    items.push(exit_item);
    if let Ok(commands) = result {
        for command in commands.iter() {
            let cmd = command.get("command").expect("command is empty");
            let group = command.get("group").expect("group is empty");
            let syntax = command.get("syntax").expect("syntax is empty");
            let summary = command.get("summary").expect("summary is empty");
            let since = command.get("since").expect("since is empty");
            let complexity = command.get("complexity").expect("complexity is empty");
            let acl = command.get("acl").expect("acl is empty");
            let arguments = command.get("arguments").expect("arguments is empty");

            let mut item = CompletionItem::default(value_to_string(cmd));

            let kind: CompletionItemKind = match group.as_str().expect("group is not string") {
                "generic" => CompletionItemKind::Generic,
                "string" => CompletionItemKind::String,
                "list" => CompletionItemKind::List,
                "set" => CompletionItemKind::Set,
                "sorted-set" => CompletionItemKind::ZSet,
                "hash" => CompletionItemKind::Hash,
                "stream" => CompletionItemKind::Stream,
                "pubsub" => CompletionItemKind::PubSub,
                "server" => CompletionItemKind::Server,
                _ => CompletionItemKind::Other,
            };
            item.kind = kind;
            item = item.description(Doc::default()
                .syntax(value_to_string(syntax))
                .summary(value_to_string(summary))
                .since(value_to_string(since))
                .complexity(value_to_string(complexity))
                .acl(value_to_string(acl))
            );
            let arguments = arguments.as_array().expect("arguments is not an array");
            for argument in arguments.iter() {
                let arg_type = argument.get("type").expect("argument type is empty");
                if let Value::String(type_str) = arg_type {
                    match type_str.as_str() {
                        "flag" => {
                            let value = argument.get("value").expect("flag value is empty");
                            let string = value_to_string(value);
                            item = item.add_param(Parameter::flag(string.clone(), string.clone()));
                        },
                        "enum" => {
                            let values = argument.get("values").expect("enums values is empty");
                            let values = values.as_array().expect("values is not an array");
                            let mut vec = vec![];
                            for value in values {
                                let string = value_to_string(value);
                                vec.push((string.clone(), string.clone()));
                            }
                            item = item.add_param(Parameter::enums(vec));
                        },
                        "arg" => {
                            let key = argument.get("key").expect("arg key is empty");
                            let arg = argument.get("arg").expect("arg arg is empty");
                            let detail = argument.get("detail").expect("arg detail is empty");
                            let key = value_to_string(key);
                            let arg = value_to_string(arg);
                            let detail = value_to_string(detail);
                            item = item.add_param(Parameter::arg(key, arg, detail));
                        },
                        "many" => {
                            let name = argument.get("name").expect("many name is empty");
                            let name = value_to_string(name);
                            item = item.add_param(Parameter::many(name));
                        },
                        "single" => {
                            let name = argument.get("name").expect("many name is empty");
                            let name = value_to_string(name);
                            item = item.add_param(Parameter::single(name));
                        },
                        _ => {}
                    }
                }
            }
            item = item.build_label();
            items.push(item);
        }
    }
    items
});

fn value_to_string(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        String::new()
    }
}