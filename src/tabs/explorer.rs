use crate::app::{AppEvent, Listenable, Renderable, TabImplementation};
use crate::components::highlight_value::{HighlightKind, HighlightProcessor, HighlightText};
use crate::redis_opt::{redis_operations, redis_opt};
use crate::tabs::explorer::CurrentScreen::Keys;
use anyhow::{Error, Result};
use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Line, Style, Stylize, Text};
use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Modifier};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation};
use ratatui::Frame;
use redis::{Cmd, Commands, ConnectionLike, Iter, RedisResult, ScanOptions, Value};
use std::collections::HashMap;
use std::ops::Not;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crossbeam_channel::{unbounded, Receiver, Sender};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use tui_textarea::TextArea;
use tui_tree_widget::{Tree, TreeItem, TreeState};
use uuid::{uuid, Uuid};

pub struct ExplorerTab {
    pub current_screen: CurrentScreen,
    show_filter: bool,
    filter_mod: FilterMod,
    scan_size: u16,
    filter_text_area: TextArea<'static>,
    scan_keys_result: Vec<RedisKey>,
    tree_state: TreeState<String>,
    tree_items: Vec<TreeItem<'static, String>>,
    redis_separator: String,
    selected_key: Option<RedisKey>,
    select_string_value: Option<String>,
    data_sender: Sender<Data>,
    data_receiver: Receiver<Data>,
}

#[derive(Default, Clone)]
struct Data {
    key_name: String,
    scan_keys_result: (bool, Vec<RedisKey>),
    select_string_value: (bool, Option<String>),
    key_type: (bool, Option<String>),
    key_size: (bool, Option<usize>),
    length: (bool, Option<usize>),
    ttl: (bool, Option<u64>),
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CurrentScreen {
    #[default]
    Keys,
    Values,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterMod {
    #[default]
    Fuzzy,
    Pattern,
}

#[derive(Default, Debug, Clone)]
struct RedisKey {
    name: String,
    key_type: String,
    key_size: Option<usize>,
    length: Option<usize>,
    ttl: Option<u64>,
}

#[derive(Debug)]
struct TreeNode {
    children: HashMap<String, TreeNode>,
    key_type: Option<String>,
    id: String,
}

impl TreeNode {
    fn new() -> Self {
        TreeNode {
            children: HashMap::new(),
            key_type: None,
            id: "".to_string(),
        }
    }

    fn insert(&mut self, path: &[&str], key_type: &str, prefix: &str, separator: String) {
        if let Some((first, rest)) = path.split_first() {
            let child = self.children.entry(first.to_string()).or_insert_with(|| {
                let mut node = TreeNode::new();
                if !prefix.is_empty() {
                    node.id = format!("{}{}{}", prefix, separator, first);
                } else {
                    node.id = first.to_string();
                }
                node
            });

            if rest.is_empty() {
                child.key_type = Some(key_type.to_string());
            } else {
                child.insert(rest, key_type, &child.id.clone(), separator);
            }
        }
    }
}

fn build_tree_items(node: &TreeNode) -> Vec<TreeItem<'static, String>> {
    let mut items = Vec::new();
    let id = node.id.clone();
    for (key, child) in &node.children {
        let item;
        if child.children.is_empty() {
            if let Some(_type) = &child.key_type {
                let name_span = Span::raw(key.clone());
                // let type_span = Span::styled(format!("[{}]", _type.clone()), get_type_color(_type));
                let mut line = Line::default();
                line.push_span(name_span);
                // line.push_span(Span::raw(" "));
                // line.push_span(type_span);
                let text = Text::from(line);
                item = TreeItem::new_leaf(child.id.clone(), text);
            } else {
                item = TreeItem::new_leaf(child.id.clone(), key.clone());
            }
        } else {
            let vec = build_tree_items(child);
            let msg = format!("{:#?}", &vec);
            item = TreeItem::new(child.id.clone(), key.clone(), vec)
                .expect(&msg);
        }
        items.push(item);
    }
    items.sort_by(|x, x1| x.identifier().cmp(x1.identifier()));
    items
}

fn get_type_color(key_type: &str) -> Color {
    match key_type {
        "Hash" | "hash" => { tailwind::BLUE.c700 }
        "List" | "list" => { tailwind::GREEN.c700 }
        "Set" | "set" => { tailwind::ORANGE.c700 }
        "ZSet" | "zset" => { tailwind::PINK.c700 }
        "String" | "string" => { tailwind::PURPLE.c700 }
        "JSON" | "json" => { tailwind::GRAY.c700 }
        "Stream" | "stream" => { tailwind::YELLOW.c700 }
        "unknown" => { tailwind::SLATE.c500 }
        _ => { Color::default() }
    }
}

fn get_type_style(key_type: &str) -> Style {
    Style::default()
        .bold()
        .bg(get_type_color(key_type))
        .fg(Color::default())
}

impl RedisKey {
    fn new(name: &str, key_type: &str) -> Self {
        Self { name: name.to_string(), key_type: key_type.to_string(), ..Self::default() }
    }
}

impl ExplorerTab {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            current_screen: Keys,
            show_filter: false,
            filter_mod: FilterMod::Fuzzy,
            scan_size: 2_000,
            filter_text_area: TextArea::default(),
            scan_keys_result: vec![],
            tree_state: Default::default(),
            tree_items: vec![],
            redis_separator: ":".to_string(),
            selected_key: None,
            select_string_value: None,
            data_sender: tx,
            data_receiver: rx,
        }
    }

    fn update_data(&mut self, data: Data) {
        if let Some(redis_key) = &mut self.selected_key {
            if redis_key.name == data.key_name {
                if data.scan_keys_result.0 {
                    self.scan_keys_result = data.scan_keys_result.1;
                }
                if data.key_type.0 {
                    redis_key.key_type = data.key_type.1.unwrap_or("unknown".to_string());
                }
                if data.key_size.0 {
                    redis_key.key_size = data.key_size.1;
                }
                if data.length.0 {
                    redis_key.length = data.length.1;
                }
                if data.ttl.0 {
                    redis_key.ttl = data.ttl.1;
                }
                if data.select_string_value.0 {
                    self.select_string_value = data.select_string_value.1;
                }
            }
        }
    }

    fn border_color(&self, screen: CurrentScreen) -> Color {
        let mut color;
        if screen == self.current_screen {
            color = tailwind::GRAY.c300
        } else {
            color = tailwind::GRAY.c600;
        }
        color
    }

    fn render_keys_block(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.render_tree(frame, area)?;
        if self.show_filter {
            let vertical = Layout::vertical([Min(0), Length(3), Length(1)]).split(area);
            let horizontal = Layout::horizontal([Length(1), Min(0), Length(1)]).split(vertical[1]);
            self.render_filter_input(frame, horizontal[1])?;
        }

        Ok(())
    }

    fn render_values_block(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let vertical = Layout::vertical([Length(5), Min(0)]).split(area);
        self.render_key_information(frame, vertical[0])?;
        self.render_value_view(frame, vertical[1])?;

        Ok(())
    }

    fn render_key_information(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let values_block = Block::default().title("Info")
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .border_style(self.border_color(CurrentScreen::Values));

        if let Some(redis_key) = &self.selected_key {
            let key_name = redis_key.name.clone();
            let key_type = redis_key.key_type.clone();
            let x = redis_operations();
            let type_style = get_type_style(&key_type);

            let mut lines = Text::default();

            let key_type_span = Span::styled(format!(" {} ", &key_type), type_style);
            let break_span = Span::raw(" ");
            let key_value_span = Span::styled(&key_name, Style::default().bold());

            let mut line = Line::default();
            line.push_span(key_type_span);
            line.push_span(break_span);
            line.push_span(key_value_span);

            lines.push_line(line);


            let key_size_span = Span::raw(format!("Key Size: {:?} B", redis_key.key_size.unwrap_or(0)));
            let length_span = Span::raw(format!("  Length: {:?}", redis_key.length.unwrap_or(0)));
            let ttl_span = if redis_key.ttl.is_some() {
                let duration = chronoutil::RelativeDuration::seconds(redis_key.ttl.unwrap_or(0) as i64).format_to_iso8601();
                Span::raw(format!("  TTL: {}", duration))
            } else {
                Span::raw("  TTL: No Limit".to_string())
            };

            let mut line = Line::default();
            line.push_span(key_size_span);
            line.push_span(length_span);
            line.push_span(ttl_span);
            lines.push_line(line);

            let text = Text::from(lines);
            let paragraph = Paragraph::new(text)
                .block(values_block);
            frame.render_widget(paragraph, area);
            return Ok(());
        }
        // let values_text = Paragraph::new(
        //     format!("selected id: {}", selected_id)
        // ).block(values_block);
        // frame.render_widget(values_text, area);
        // return;
        let values_text = Paragraph::new(
            "N/A"
        ).block(values_block);
        frame.render_widget(values_text, area);

        Ok(())
    }

    fn render_value_view(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let values_block = Block::default().title("Values")
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .border_style(self.border_color(CurrentScreen::Values));
        if let Ok(text) = self.get_value() {
            let values_text = Paragraph::new(text)
                .block(values_block);
            frame.render_widget(values_text, area);
        }
        Ok(())
    }

    fn render_filter_input(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.filter_text_area.set_cursor_line_style(Style::default());
        match self.filter_mod {
            FilterMod::Fuzzy => { self.filter_text_area.set_placeholder_text(" Fuzzy "); }
            FilterMod::Pattern => { self.filter_text_area.set_placeholder_text(" Pattern "); }
        }
        self.filter_text_area.set_block(
            Block::bordered()
                .border_style(self.border_color(CurrentScreen::Keys))
                .title(format!("Scan Keys ({})", self.scan_keys_result.len()))
        );
        frame.render_widget(&self.filter_text_area.clone(), area);
        Ok(())
    }

    fn render_tree(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let tree = Tree::new(&self.tree_items)
            .expect("")
            .block(
                Block::bordered()
                    .title("Keys")
                    .title_bottom(""),
            )
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None),
            ))
            .highlight_style(
                Style::new()
                    .fg(Color::default())
                    .bg(self.palette().c700)
                    .add_modifier(Modifier::BOLD),
            )
            .node_no_children_symbol("- ")
            .highlight_symbol("");
        tokio::task::block_in_place(|| {
            frame.render_stateful_widget(tree, area, &mut self.tree_state);
        });
        Ok(())
    }

    fn toggle_screen(&mut self, screen: CurrentScreen) {
        self.current_screen = screen;
    }

    fn do_scan(&mut self, pattern: String, count: u16) -> Result<()> {
        let x = redis_operations();

        if let Some(c) = x {
            let mut con = c.get_connection()?;
            let iter: Iter<String> = con.scan_options(ScanOptions::default().with_pattern(pattern.clone()).with_count(count as usize))?;
            let mut results: Vec<RedisKey> = vec![];
            for key in iter {
                let redis_key;
                let key_type = "unknown".to_string();
                redis_key = RedisKey::new(&key, key_type.as_str());
                results.push(redis_key);
            }
            self.scan_keys_result = results;
        } else {
            self.scan_keys_result = vec![
                RedisKey::new("spring:security:token:abc", "JSON"),
                RedisKey::new("spring:security:token:def", "JSON"),
                RedisKey::new("spring:security:session:123", "List"),
                RedisKey::new("spring:security:session:456", "List"),
                RedisKey::new("spring:security:session:743812974890321749807590817498321749", "String"),
                RedisKey::new("spring:security:version", "String"),
                RedisKey::new("spring:cache:user:anonymous", "List"),
                RedisKey::new("spring:cache:user:root", "Stream"),
                RedisKey::new("spring:cache:user:guest", "ZSet"),
                RedisKey::new("spring:cache:version", "Set"),
                RedisKey::new("properties", "Hash"),
            ];
        }
        Ok(())
    }

    fn build_tree_items(&mut self, filter_text: &String) -> Result<()> {
        match self.filter_mod {
            FilterMod::Fuzzy => {
                let mut matcher = filter_text.clone();
                if matcher.is_empty() {
                    matcher = "*".to_string();
                } else {
                    matcher = format!("*{}*", matcher);
                }
                self.do_scan(matcher, 2000)?;
            }
            FilterMod::Pattern => {
                self.do_scan(filter_text.clone(), 2000)?;
            }
        }
        if filter_text.is_empty().not() {
            self.scan_keys_result.retain(|redis_key| {
                let contains;
                match self.filter_mod {
                    FilterMod::Fuzzy => { contains = redis_key.name.contains(filter_text); }
                    FilterMod::Pattern => {
                        // TODO scan key
                        let fuzzy_start = filter_text.starts_with("*");
                        let fuzzy_end = filter_text.ends_with("*");
                        let mut _filter_text = filter_text.clone();
                        if fuzzy_start {
                            _filter_text = _filter_text[1..].to_string();
                        }
                        if fuzzy_end {
                            _filter_text = _filter_text[.._filter_text.len() - 1].to_string();
                        }
                        if fuzzy_start && fuzzy_end {
                            contains = redis_key.name.contains(&_filter_text);
                        } else if fuzzy_start && !fuzzy_end {
                            contains = redis_key.name.ends_with(&_filter_text);
                        } else if !fuzzy_start && fuzzy_end {
                            contains = redis_key.name.starts_with(&_filter_text);
                        } else {
                            contains = redis_key.name.eq(&_filter_text);
                        }
                    }
                }

                contains
            });
            for redis_key in self.scan_keys_result.iter() {
                if let Some(filter_position) = redis_key.name.rfind(filter_text) {
                    let split = redis_key.name.split(":");
                    let vec: Vec<String> = split.map(|x| x.to_string()).collect();
                    let mut i = 0;
                    let last_index = filter_position + filter_text.len();
                    let mut open_vec: Vec<String> = vec![];
                    for segment in vec {
                        i += &segment.len();
                        i += 1; // ':'
                        if i < last_index {
                            open_vec.push(segment);
                            let mut open = vec![];
                            for j in 1..open_vec.len() + 1 {
                                let string = open_vec[0..j].join(":");
                                open.push(string);
                            }
                            self.tree_state.open(open);
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        let mut root = TreeNode::new();
        for data in &self.scan_keys_result {
            let parts: Vec<&str> = data.name.split(self.redis_separator.as_str()).collect();
            root.insert(&parts, &data.key_type, "", self.redis_separator.clone());
        }
        self.tree_items = build_tree_items(&root);
        Ok(())
    }

    fn get_filter_text(&self) -> Option<String> {
        self.filter_text_area.lines().get(0).cloned()
    }

    fn handle_filter_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind != KeyEventKind::Press {
            return Ok(true);
        }
        match key_event {
            KeyEvent { code: KeyCode::Esc, .. } => {
                self.show_filter = false;
            }
            KeyEvent { code: KeyCode::Enter, .. } => {
                // TODO perform scan keys, or scan after input changed, enter will refresh keys
                if let Some(first_line) = self.get_filter_text() {
                    self.build_tree_items(&first_line)?;
                }
            }
            KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
            KeyEvent { code: KeyCode::Char('/'), modifiers: KeyModifiers::CONTROL, .. } => {
                match self.filter_mod {
                    FilterMod::Fuzzy => { self.filter_mod = FilterMod::Pattern; }
                    FilterMod::Pattern => { self.filter_mod = FilterMod::Fuzzy; }
                }
            }
            input => {
                self.filter_text_area.input(input);
            }
        }
        Ok(true)
    }

    fn get_value(&self) -> Result<Text> {
        let mut value = "".to_string();
        if let Some(s) = &self.select_string_value {
            value = s.clone().to_string();
        }
        let mut processor = HighlightProcessor::new(value.clone());
        // processor.disable_formatting();
        let result = processor.process();
        let fragments = match result {
            Ok(_) => { processor.get_fragments().clone() }
            Err(_) => {
                vec![HighlightText {
                    text: value.clone(),
                    kind: HighlightKind::String,
                }]
            }
        };
        let mut text = Text::default();
        for highlight_text in fragments {
            let fragment = highlight_text.text.clone();
            let mut style = Style::default();
            match highlight_text.kind {
                HighlightKind::String => style = Style::default().fg(tailwind::AMBER.c400),
                HighlightKind::Boolean |
                HighlightKind::Keyword |
                HighlightKind::Constant |
                HighlightKind::Null => style = Style::default().fg(tailwind::ROSE.c600),
                HighlightKind::Property => style = Style::default().fg(tailwind::FUCHSIA.c700),
                HighlightKind::Comment => style = Style::default().fg(tailwind::CYAN.c500),
                _ => {}
            }

            let lines: Vec<&str> = fragment.lines().collect();
            if lines.len() > 1 {
                for (i, &l) in lines.iter().enumerate() {
                    if i == 0 {
                        let span = Span::styled(l.to_string(), style);
                        text.push_span(span);
                    } else {
                        let line = Line::styled(l.to_string(), style);
                        text.push_line(line);
                    }
                }
            } else {
                for l in lines {
                    let span = Span::styled(l.to_string(), style);
                    text.push_span(span);
                }
            }
        }
        Ok(text)
    }

    fn handle_tree_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.modifiers == KeyModifiers::NONE {
            let current_selected_key = self.selected_key.clone().map(|current| { current.name });
            let accepted = match key_event.code {
                KeyCode::Left | KeyCode::Char('h') => self.tree_state.key_left(),
                KeyCode::Right | KeyCode::Char('l') => {
                    if self.selected_key.is_some() {
                        self.toggle_screen(CurrentScreen::Values);
                        true
                    } else {
                        self.tree_state.key_right()
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.tree_state.key_down(),
                KeyCode::Up | KeyCode::Char('k') => self.tree_state.key_up(),
                KeyCode::Esc => self.tree_state.select(Vec::new()),
                KeyCode::Home => self.tree_state.select_first(),
                KeyCode::End => self.tree_state.select_last(),
                KeyCode::PageDown => self.tree_state.scroll_down(3),
                KeyCode::PageUp => self.tree_state.scroll_up(3),
                _ => false,
            };

            if accepted {
                let vec = self.tree_state.selected().to_vec();
                let changed_selected_key = vec.last().cloned();
                if changed_selected_key != current_selected_key {
                    if let Some(id) = changed_selected_key {
                        let option = self.scan_keys_result.iter().find(|redis_key| {
                            id.eq(&redis_key.name)
                        }).cloned();
                        self.selected_key = option;
                        self.select_string_value = None;
                        if self.selected_key.is_some() {
                            let sender = self.data_sender.clone();
                            tokio::spawn(async move {
                                let data = Self::do_get_key_info(id.clone()).await?;
                                sender.send(data.clone())?;
                                if let Some(key_type) = data.key_type.1 {
                                    let data = Self::do_get_value(id.clone(), key_type).await?;
                                    sender.send(data)?;
                                }
                                Ok::<(), Error>(())
                            });
                        }
                    }
                }
            }

            return Ok(accepted);
        }
        Ok(false)
    }

    async fn do_get_key_info(key_name: String) -> Result<Data> {
        let mut data = Data::default();
        data.key_name = key_name.clone();
        let key_type = redis_opt(|op| {
            let mut con = op.get_connection()?;
            let key_type: String = con.key_type(&key_name)?;
            Ok(key_type)
        });
        if let Ok(key_type) = key_type {
            data.key_type = (true, Some(key_type));
        }
        let key_size = redis_opt(|op| {
            let mut con = op.get_connection()?;
            let value = con.req_command(&Cmd::new().arg("MEMORY").arg("USAGE").arg(&key_name).arg("SAMPLES").arg("0"))?;
            if let Value::Int(int) = value {
                let key_size = int as usize;
                Ok(key_size)
            } else {
                Ok(0)
            }
        });
        if let Ok(key_size) = key_size {
            data.key_size = (true, Some(key_size));
        }
        let length = redis_opt(|op| {
            let mut con = op.get_connection()?;
            let length: usize = con.strlen(&key_name)?;
            Ok(length)
        });
        if let Ok(length) = length {
            data.length = (true, Some(length));
        }
        let ttl = redis_opt(|op| {
            let mut con = op.get_connection()?;
            let ttl: i64 = con.ttl(&key_name)?;
            Ok(ttl)
        });
        if let Ok(ttl) = ttl {
            if ttl.is_positive() {
                data.ttl = (true, Some(ttl as u64));
            } else {
                data.ttl = (true, None);
            }
        }
        Ok(data)
    }

    async fn do_get_value(key_name: String, key_type: String) -> Result<Data> {
        // tokio::time::sleep(Duration::from_millis(300)).await;
        let mut data = Data::default();
        data.key_name = key_name.clone();
        let value = redis_opt(|op| {
            let mut con = op.get_connection()?;
            return match key_type.to_ascii_lowercase().as_str() {
                "string" => {
                    let bytes: Vec<u8> = con.get(&key_name)?;
                    return if let Ok(string) = String::from_utf8(bytes.clone()) {
                        Ok(string)
                    } else {
                        Ok(bytes.iter().map(|&b| {
                            if b.is_ascii() {
                                (b as char).to_string()
                            } else {
                                format!("\\x{:02x}", b)
                            }
                        }).collect::<String>())
                    };
                }
                _ => {Ok("".to_string())}
            }

        });
        if let Ok(s) = value {
            data.select_string_value = (true, Some(s));
        } else {
            data.select_string_value = (true, None);
        }
        Ok(data)
    }

    // fn update_selected_key(&mut self, id_list: Vec<String>) -> Result<()> {
    //     let key_id = id_list.last();
    //     if let Some(id) = key_id {
    //         let option = self.scan_keys_result.iter().find(|redis_key| {
    //             id.eq(&redis_key.name)
    //         }).cloned();
    //         if let Some(mut redis_key) = option {
    //             let key_type = redis_opt(|op| {
    //                 let mut con = op.get_connection()?;
    //                 let key_type: String = con.key_type(&redis_key.name)?;
    //                 Ok(key_type)
    //             });
    //             if let Ok(key_type) = key_type {
    //                 redis_key.key_type = key_type;
    //             }
    //             let key_size = redis_opt(|op| {
    //                 let mut con = op.get_connection()?;
    //                 let value = con.req_command(&Cmd::new().arg("MEMORY").arg("USAGE").arg(&redis_key.name).arg("SAMPLES").arg("0"))?;
    //                 if let Value::Int(int) = value {
    //                     let key_size = int as usize;
    //                     Ok(key_size)
    //                 } else {
    //                     Ok(0)
    //                 }
    //             });
    //             if let Ok(key_size) = key_size {
    //                 redis_key.key_size = Some(key_size);
    //             }
    //             let length = redis_opt(|op| {
    //                 let mut con = op.get_connection()?;
    //                 let length: usize = con.strlen(&redis_key.name)?;
    //                 Ok(length)
    //             });
    //             if let Ok(length) = length {
    //                 redis_key.length = Some(length);
    //             }
    //             let ttl = redis_opt(|op| {
    //                 let mut con = op.get_connection()?;
    //                 let ttl: i64 = con.ttl(&redis_key.name)?;
    //                 Ok(ttl)
    //             });
    //             if let Ok(ttl) = ttl {
    //                 if ttl.is_positive() {
    //                     redis_key.ttl = Some(ttl as u64);
    //                 } else {
    //                     redis_key.ttl = None;
    //                 }
    //             }
    //             self.selected_key = Some(redis_key);
    //         } else {
    //             self.selected_key = None;
    //         }
    //     }
    //     Ok(())
    // }
}

impl TabImplementation for ExplorerTab {
    fn palette(&self) -> tailwind::Palette {
        tailwind::ROSE
    }

    fn title(&self) -> Line<'static> {
        "  Explorer  ".to_string()
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }
}

impl Renderable for ExplorerTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        while self.data_receiver.is_empty().not() {
            let data = self.data_receiver.try_recv()?;
            self.update_data(data);
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(rect);
        self.render_keys_block(frame, chunks[0])?;
        self.render_values_block(frame, chunks[1])?;

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        if self.show_filter {
            let quit_desc = match self.filter_mod {
                FilterMod::Fuzzy => "Pattern",
                FilterMod::Pattern => "Fuzzy",
            };
            elements.push(("^/", quit_desc));
            elements.push(("Esc", "Close"));
            elements.push(("Enter", "Do Scan"));
        } else {
            if self.current_screen == Keys {
                elements.push(("/", "Scan"));
                elements.push(("c", "Connection"));
                elements.push(("d", "Delete"));
                elements.push(("r", "Rename"));
                elements.push(("↑/j", "Up"));
                elements.push(("↓/k", "Down"));
                elements.push(("←/h", "Close"));
                elements.push(("→/l", "Open"));
            }
        }
        elements
    }
}

impl Listenable for ExplorerTab {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if self.show_filter {
            return self.handle_filter_key_event(key_event);
        }

        if CurrentScreen::Values == self.current_screen {
            if KeyCode::Left == key_event.code {
                self.toggle_screen(Keys);
                return Ok(true);
            }
        }

        if Keys == self.current_screen {
            if key_event.modifiers == KeyModifiers::NONE {
                if self.handle_tree_key_event(key_event)? {
                    return Ok(true);
                };
                match key_event.code {
                    KeyCode::Char('/') => {
                        self.show_filter = true;
                        return Ok(true);
                    }
                    _ => {}
                }
            }
        }

        match key_event.code {
            KeyCode::Char('`') => {
                if CurrentScreen::Keys == self.current_screen {
                    self.toggle_screen(CurrentScreen::Values);
                } else {
                    self.toggle_screen(CurrentScreen::Keys);
                }
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn on_app_event(&mut self, _app_event: AppEvent) -> Result<()> {
        if _app_event == AppEvent::Init {
            self.build_tree_items(&String::new())?;
        }
        Ok(())
    }
}
