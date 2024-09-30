use crate::app::{centered_rect, AppEvent, Listenable, Renderable, TabImplementation};
use crate::components::create_key_editor::{Form, KeyType};
use crate::components::hash_table::HashValue;
use crate::components::list_table::ListValue;
use crate::components::popup::Popup;
use crate::components::raw_paragraph::RawParagraph;
use crate::components::set_table::SetValue;
use crate::components::zset_table::ZSetValue;
use crate::redis_opt::{async_redis_opt, spawn_redis_opt};
use crate::tabs::explorer::CurrentScreen::{KeysTree, ValuesViewer};
use crate::utils::{bytes_to_string, clean_text_area};
use crate::utils::{deserialize_bytes, ContentType};
use anyhow::{anyhow, Context, Error, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Line, Style, Stylize, Text};
use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Modifier};
use ratatui::text::Span;
use ratatui::widgets::block::Position;
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation};
use ratatui::{symbols, Frame};
use std::collections::HashMap;
use std::ops::Not;
use tokio::join;
use tui_textarea::TextArea;
use tui_tree_widget::{Tree, TreeItem, TreeState};

pub struct ExplorerTab {
    pub current_screen: CurrentScreen,
    show_filter: bool,
    show_create: bool,
    show_rename: bool,
    show_delete_popup: bool,
    filter_mod: FilterMod,
    scan_size: u16,
    filter_text_area: TextArea<'static>,
    create_key_form: Form,
    rename_key_text_area: TextArea<'static>,
    scan_keys_result: Vec<RedisKey>,
    tree_state: TreeState<String>,
    tree_items: Vec<TreeItem<'static, String>>,
    redis_separator: String,
    selected_key: Option<RedisKey>,
    selected_raw_value: Option<RawParagraph<'static>>,
    selected_list_value: Option<ListValue>,
    selected_set_value: Option<SetValue>,
    selected_zset_value: Option<ZSetValue>,
    selected_hash_value: Option<HashValue>,
    data_sender: Sender<Data>,
    data_receiver: Receiver<Data>,
}

#[derive(Default, Clone)]
struct Data {
    key_name: String,
    scan_keys_result: (bool, Vec<RedisKey>),
    selected_string_value: (bool, Option<(String, Option<ContentType>)>),
    selected_list_value: (bool, Option<Vec<String>>),
    selected_set_value: (bool, Option<Vec<String>>),
    selected_zset_value: (bool, Option<Vec<(String, f64)>>),
    selected_hash_value: (bool, Option<HashMap<String, String>>),
    key_type: (bool, Option<String>),
    key_size: (bool, Option<usize>),
    length: (bool, Option<usize>),
    ttl: (bool, Option<u64>),
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CurrentScreen {
    #[default]
    KeysTree,
    ValuesViewer,
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
        let mut filter_text_area = TextArea::default();
        let mut rename_key_text_area = TextArea::default();
        filter_text_area.set_cursor_line_style(Style::default());
        rename_key_text_area.set_cursor_line_style(Style::default());
        rename_key_text_area.set_block(
            Block::bordered()
                .title("Rename Key")
        );
        Self {
            current_screen: KeysTree,
            show_filter: false,
            show_create: false,
            show_rename: false,
            show_delete_popup: false,
            filter_mod: FilterMod::Fuzzy,
            scan_size: 2_000,
            filter_text_area,
            create_key_form: Form::default().title("Create Key"),
            rename_key_text_area,
            scan_keys_result: vec![],
            tree_state: Default::default(),
            tree_items: vec![],
            redis_separator: ":".to_string(),
            selected_key: None,
            selected_raw_value: None,
            selected_list_value: None,
            selected_set_value: None,
            selected_zset_value: None,
            selected_hash_value: None,
            data_sender: tx,
            data_receiver: rx,
        }
    }

    fn update_data(&mut self, data: Data) {
        if data.scan_keys_result.0 {
            self.scan_keys_result = data.scan_keys_result.1;
            let _ = self.build_tree_items();
        }
        if let Some(redis_key) = &mut self.selected_key {
            if redis_key.name == data.key_name {
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
                if data.selected_string_value.0 {
                    let raw = data.selected_string_value.1.unwrap_or_default();
                    self.selected_raw_value = Some(RawParagraph::new(raw.0, raw.1));
                }
                if data.selected_list_value.0 {
                    self.selected_list_value = Some(ListValue::new(data.selected_list_value.1.unwrap_or_default()));
                }
                if data.selected_set_value.0 {
                    self.selected_set_value = Some(SetValue::new(data.selected_set_value.1.unwrap_or_default()));
                }
                if data.selected_zset_value.0 {
                    self.selected_zset_value = Some(ZSetValue::new(data.selected_zset_value.1.unwrap_or_default()));
                }
                if data.selected_hash_value.0 {
                    self.selected_hash_value = Some(HashValue::new(data.selected_hash_value.1.unwrap_or_default()));
                }
            }
        }
    }

    fn border_color(&self, screen: CurrentScreen) -> Color {
        if screen == self.current_screen {
            tailwind::GRAY.c300
        } else {
            tailwind::GRAY.c600
        }
    }

    fn render_keys_block(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.render_tree(frame, area)?;
        if self.show_filter {
            let vertical = Layout::vertical([Min(0), Length(3), Length(1)]).split(area);
            let horizontal = Layout::horizontal([Length(1), Min(0), Length(1)]).split(vertical[1]);
            self.render_filter_input(frame, horizontal[1])?;
        }

        if self.show_rename {
            let vertical = Layout::vertical([Min(0), Length(3), Length(1)]).split(area);
            let horizontal = Layout::horizontal([Length(1), Min(0), Length(1)]).split(vertical[1]);
            self.render_rename_key_input(frame, horizontal[1])?;
        }

        Ok(())
    }

    fn render_values_block(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let vertical = Layout::vertical([Length(5), Min(0)]).split(area);
        self.render_key_information(frame, vertical[0])?;
        self.render_value_view(frame, vertical[1])?;

        Ok(())
    }

    fn render_key_information(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let values_block = Block::default().title("Info")
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .border_style(self.border_color(ValuesViewer));

        if let Some(redis_key) = &self.selected_key {
            let key_name = redis_key.name.clone();
            let key_type = redis_key.key_type.clone();
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
                Span::raw("  TTL: No Limit")
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

    fn render_value_view(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let values_block = Block::default().title("Values")
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .border_style(self.border_color(ValuesViewer));
        let block_inner_area = values_block.inner(area);
        frame.render_widget(values_block, area);
        if self.selected_raw_value.is_some() {
            if let Some(ref mut raw_value) = self.selected_raw_value {
                raw_value.render(frame, block_inner_area)?;
            }
        } else if self.selected_list_value.is_some() {
            if let Some(ref mut list_value) = self.selected_list_value {
                list_value.render_frame(frame, block_inner_area)?;
            }
        } else if self.selected_set_value.is_some() {
            if let Some(ref mut set_value) = self.selected_set_value {
                set_value.render_frame(frame, block_inner_area)?;
            }
        } else if self.selected_zset_value.is_some() {
            if let Some(ref mut set_value) = self.selected_zset_value {
                set_value.render_frame(frame, block_inner_area)?;
            }
        } else if self.selected_hash_value.is_some() {
            if let Some(ref mut hash_value) = self.selected_hash_value {
                hash_value.render_frame(frame, block_inner_area)?;
            }
        } else {
            let values_text = Paragraph::new("N/A");
            frame.render_widget(values_text, block_inner_area);
        }
        Ok(())
    }

    fn render_filter_input(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        match self.filter_mod {
            FilterMod::Fuzzy => { self.filter_text_area.set_placeholder_text(" Fuzzy "); }
            FilterMod::Pattern => { self.filter_text_area.set_placeholder_text(" Pattern "); }
        }
        self.filter_text_area.set_block(
            Block::bordered()
                .border_style(self.border_color(KeysTree))
                .title(format!("Scan Keys ({})", self.scan_keys_result.len()))
        );
        frame.render_widget(&self.filter_text_area, area);
        Ok(())
    }

    fn render_create_key_form(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.create_key_form.render_frame(frame, area)
    }

    fn render_rename_key_input(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        frame.render_widget(&self.rename_key_text_area, area);
        Ok(())
    }

    fn render_delete_popup(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(redis_key) = &self.selected_key {
            let popup_area = centered_rect(30, 15, area);
            let mut text = Text::default();
            text.push_line(Line::raw(redis_key.name.clone())
                .alignment(Alignment::Center)
                .underlined());
            text.push_line(Line::default());
            text.push_line(Line::raw("Will be deleted. Are you sure?")
                .alignment(Alignment::Center)
                .bold());
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center);
            let delete_popup = Popup::new(paragraph)
                .title(String::from(" [Enter] Confirm | [Esc] Cancel "))
                .title_position(Position::Bottom)
                .borders(Borders::ALL)
                .border_set(symbols::border::DOUBLE)
                .style(Style::default()
                    .bg(self.palette().c900));
            frame.render_widget(delete_popup, popup_area);
        }
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

    fn do_scan(&mut self, filter_text: String) -> Result<()> {
        let pattern = match self.filter_mod {
            FilterMod::Fuzzy => {
                let mut matcher = filter_text.clone();
                if matcher.is_empty() {
                    matcher = "*".to_string();
                } else {
                    matcher = format!("*{}*", matcher);
                }
                matcher
            }
            FilterMod::Pattern => {
                filter_text.clone()
            }
        };

        let sender = self.data_sender.clone();
        let pattern_clone = pattern.clone();
        let size_clone = self.scan_size.clone();
        spawn_redis_opt(move |operations| async move {
            let mut data = Data::default();
            let keys = operations.scan(pattern_clone, size_clone as usize).await;
            if let Ok(keys) = keys {
                let vec = keys.iter()
                    .map(|s| RedisKey::new(s, "unknown"))
                    .collect::<Vec<RedisKey>>();
                data.scan_keys_result = (true, vec);
            }
            sender.send(data.clone())?;
            Ok(())
        })?;
        Ok(())
    }

    fn build_tree_items(&mut self) -> Result<()> {
        if let Some(first_line) = self.get_filter_text() {
            if !first_line.is_empty() {
                let filter_text = &first_line.clone();
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
                if self.filter_text_area.is_selecting() {
                    self.filter_text_area.cancel_selection();
                } else {
                    self.show_filter = false;
                }
            }
            KeyEvent { code: KeyCode::Enter, .. } => {
                if let Some(first_line) = self.get_filter_text() {
                    self.do_scan(first_line)?;
                }
            }
            KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
            KeyEvent { code: KeyCode::Char('/'), modifiers: KeyModifiers::CONTROL, .. } => {
                match self.filter_mod {
                    FilterMod::Fuzzy => { self.filter_mod = FilterMod::Pattern; }
                    FilterMod::Pattern => { self.filter_mod = FilterMod::Fuzzy; }
                }
            }
            KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.filter_text_area.select_all();
            }
            KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.filter_text_area.undo();
            }
            KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.filter_text_area.redo();
            }
            input => {
                self.filter_text_area.input(input);
            }
        }
        Ok(true)
    }

    fn handle_rename_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind != KeyEventKind::Press {
            return Ok(true);
        }
        match key_event {
            KeyEvent { code: KeyCode::Esc, .. } => {
                if self.rename_key_text_area.is_selecting() {
                    self.rename_key_text_area.cancel_selection();
                } else {
                    self.show_rename = false;
                }
            }
            KeyEvent { code: KeyCode::Enter, .. } => {
                if let Some(first_line) = self.rename_key_text_area.lines().get(0).cloned() {
                    if let Some(redis_key) = &mut self.selected_key {
                        let key_name = redis_key.name.clone();
                        let new_key_name = first_line.trim().to_string();
                        if let Some(filter_text) = self.get_filter_text() {
                            let pattern = match self.filter_mod {
                                FilterMod::Fuzzy => {
                                    let mut matcher = filter_text.clone();
                                    if matcher.is_empty() {
                                        matcher = "*".to_string();
                                    } else {
                                        matcher = format!("*{}*", matcher);
                                    }
                                    matcher
                                }
                                FilterMod::Pattern => {
                                    filter_text.clone()
                                }
                            };
                            let sender = self.data_sender.clone();
                            let pattern_clone = pattern.clone();
                            let size_clone = self.scan_size.clone();
                            spawn_redis_opt(move |operations| async move {
                                operations.rename_nx(key_name, new_key_name).await?;
                                let mut data = Data::default();
                                let keys = operations.scan(pattern_clone, size_clone as usize).await?;
                                let vec = keys.iter()
                                    .map(|s| RedisKey::new(s, "unknown"))
                                    .collect::<Vec<RedisKey>>();
                                data.scan_keys_result = (true, vec);
                                sender.send(data.clone())?;
                                Ok(())
                            })?;
                        }
                        self.selected_key = None;
                    }
                }
                self.show_rename = false;
            }
            KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
            KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.rename_key_text_area.select_all();
            }
            KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.rename_key_text_area.undo();
            }
            KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.rename_key_text_area.redo();
            }
            input => {
                self.rename_key_text_area.input(input);
            }
        }
        Ok(true)
    }

    fn handle_create_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        match key_event {
            KeyEvent { code: KeyCode::Esc, modifiers: KeyModifiers::NONE, .. } => {
                if self.rename_key_text_area.is_selecting() {
                    self.rename_key_text_area.cancel_selection();
                } else {
                    self.show_create = false;
                }
                Ok(true)
            }
            KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::NONE, .. } => {
                let key_type = self.create_key_form.get_type();
                let key_name = self.create_key_form.get_name();
                let ttl = self.create_key_form.get_ttl();
                if key_name.is_empty() {
                    return Err(anyhow!("Key name must not be blank!"));
                }

                if let Some(filter_text) = self.get_filter_text() {
                    let pattern = match self.filter_mod {
                        FilterMod::Fuzzy => {
                            let mut matcher = filter_text.clone();
                            if matcher.is_empty() {
                                matcher = "*".to_string();
                            } else {
                                matcher = format!("*{}*", matcher);
                            }
                            matcher
                        }
                        FilterMod::Pattern => {
                            filter_text.clone()
                        }
                    };
                    let sender = self.data_sender.clone();
                    let pattern_clone = pattern.clone();
                    let size_clone = self.scan_size.clone();
                    spawn_redis_opt(move |operations| async move {
                        match key_type {
                            KeyType::String => {
                                operations.set_nx(key_name.clone(), "").await?;
                            }
                            KeyType::Hash => {
                                operations.hset_nx(key_name.clone(), "", "").await?;
                            }
                            KeyType::List => {
                                operations.lpush(key_name.clone(), "").await?;
                            }
                            KeyType::Set => {
                                operations.sadd(key_name.clone(), "").await?;
                            }
                            KeyType::SortedSet => {
                                operations.zadd(key_name.clone(), "", 0.0).await?;
                            }
                            KeyType::Stream => {
                                operations.xadd(key_name.clone(), "", "").await?;
                            }
                        }

                        if let Some(ttl) = ttl {
                            operations.expire(key_name.clone(), ttl as i64).await?;
                        }

                        let mut data = Data::default();
                        let keys = operations.scan(pattern_clone, size_clone as usize).await?;
                        let vec = keys.iter()
                            .map(|s| RedisKey::new(s, "unknown"))
                            .collect::<Vec<RedisKey>>();
                        data.scan_keys_result = (true, vec);
                        sender.send(data.clone())?;

                        Ok(())
                    })?;
                }
                self.show_create = false;
                Ok(true)
            }
            input => {
                self.create_key_form.handle_key_event(input)
            }
        }
    }

    fn handle_delete_popup_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind != KeyEventKind::Press || key_event.modifiers != KeyModifiers::NONE {
            return Ok(true);
        }

        match key_event.code {
            KeyCode::Enter => {
                if let Some(redis_key) = &self.selected_key {
                    let key_name = redis_key.name.clone();
                    spawn_redis_opt(move |operations| async move {
                        operations.del(key_name).await?;
                        Ok(())
                    })?;
                    let key_name = redis_key.name.as_str();
                    self.tree_items.retain_mut(|item| item.identifier().ne(key_name));
                }
                self.show_delete_popup = false;
            }
            KeyCode::Esc => {
                self.show_delete_popup = false;
            }
            _ => {}
        }

        Ok(true)
    }

    fn handle_tree_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.modifiers == KeyModifiers::NONE {
            let current_selected_key = self.selected_key.clone().map(|current| { current.name });
            let accepted = match key_event.code {
                KeyCode::Left | KeyCode::Char('h') => self.tree_state.key_left(),
                KeyCode::Right | KeyCode::Char('l') => {
                    if self.selected_key.is_some() {
                        self.toggle_screen(ValuesViewer);
                        true
                    } else {
                        self.tree_state.key_right()
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let key_down = self.tree_state.key_down();
                    if key_down {
                        key_down
                    } else {
                        self.tree_state.select_first()
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let key_up = self.tree_state.key_up();
                    if key_up {
                        key_up
                    } else {
                        self.tree_state.select_last()
                    }
                }
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
                        self.selected_raw_value = None;
                        self.selected_list_value = None;
                        self.selected_set_value = None;
                        self.selected_zset_value = None;
                        self.selected_hash_value = None;
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
        let key_name_clone = key_name.clone();
        let key_type = async_redis_opt(|op| async move {
            Ok(op.key_type(key_name_clone).await?)
        });
        let key_name_clone = key_name.clone();
        let key_size = async_redis_opt(|op| async move {
            Ok(op.mem_usage(key_name_clone).await?)
        });
        let key_name_clone = key_name.clone();
        let ttl = async_redis_opt(|op| async move {
            Ok(op.ttl(key_name_clone).await?)
        });
        let (key_type, key_size, ttl) = join!(key_type, key_size, ttl);
        if let Ok(key_type) = key_type {
            data.key_type = (true, Some(key_type.clone()));
            let key_name_clone = key_name.clone();
            let key_type_clone = key_type.clone();
            let length = async_redis_opt(|op| async move {
                match key_type_clone.to_ascii_lowercase().as_str() {
                    "string" => Ok(op.strlen(key_name_clone).await?),
                    "list" => Ok(op.llen(key_name_clone).await?),
                    "hash" => Ok(op.hlen(key_name_clone).await?),
                    "set" => Ok(op.scard(key_name_clone).await?),
                    "zset" => Ok(op.zcard(key_name_clone).await?),
                    "stream" => Ok(op.xlen(key_name_clone).await?),
                    _ => Ok(0)
                }
            }).await;
            if let Ok(length) = length {
                data.length = (true, Some(length));
            }
        }
        if let Ok(key_size) = key_size {
            data.key_size = (true, Some(key_size as usize));
        }

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
        let mut data = Data::default();
        data.key_name = key_name.clone();
        let key_name_clone = key_name.clone();
        async_redis_opt(|op| async move {
            match key_type.to_ascii_lowercase().as_str() {
                "string" => {
                    let bytes: Vec<u8> = op.get(key_name_clone).await?;
                    let result = deserialize_bytes(bytes).context("Failed to deserialize string")?;
                    data.selected_string_value = (true, Some((result.0, result.1)));
                }
                "list" => {
                    let values: Vec<Vec<u8>> = op.get_list(key_name_clone).await?;
                    let strings: Vec<String> = values.iter().map(|item| {
                        match bytes_to_string(item.clone()) {
                            Ok(s) => { s }
                            Err(_) => { String::new() }
                        }
                    }).collect();
                    data.selected_list_value = (true, Some(strings));
                }
                "set" => {
                    let values: Vec<Vec<u8>> = op.get_set(key_name_clone).await?;
                    let strings: Vec<String> = values.iter().map(|item| {
                        match bytes_to_string(item.clone()) {
                            Ok(s) => { s }
                            Err(_) => { String::new() }
                        }
                    }).collect();
                    data.selected_set_value = (true, Some(strings));
                }
                "zset" => {
                    let values: Vec<(Vec<u8>, f64)> = op.get_zset(key_name_clone).await?;
                    let tuples: Vec<(String, f64)> = values.iter().map(|(item, score)| {
                        match bytes_to_string(item.clone()) {
                            Ok(s) => { (s, score.clone()) }
                            Err(_) => { (String::new(), score.clone()) }
                        }
                    }).collect();
                    data.selected_zset_value = (true, Some(tuples));
                }
                "hash" => {
                    let values: HashMap<Vec<u8>, Vec<u8>> = op.get_hash(key_name_clone).await?;
                    let hash_value: HashMap<String, String> = values.iter().map(|(key, value)| {
                        let key_str: String = bytes_to_string(key.clone()).unwrap_or_else(|_| { String::new() });
                        let value_str = bytes_to_string(value.clone()).unwrap_or_else(|_| { String::new() });
                        (key_str, value_str)
                    }).collect();
                    data.selected_hash_value = (true, Some(hash_value));
                }
                _ => {}
            }
            Ok(data)
        }).await
    }
}

impl TabImplementation for ExplorerTab {
    fn palette(&self) -> tailwind::Palette {
        tailwind::ROSE
    }

    fn title(&self) -> Line<'static> {
        "  Explorer  "
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }
}

impl Renderable for ExplorerTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        while self.data_receiver.is_empty().not() {
            let data = self.data_receiver.try_recv();
            if let Ok(data) = data {
                self.update_data(data);
            }
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(rect);
        self.render_keys_block(frame, chunks[0])?;
        self.render_values_block(frame, chunks[1])?;

        if self.show_delete_popup {
            self.render_delete_popup(frame, rect);
        }
        if self.show_create {
            self.render_create_key_form(frame, frame.area())?;
        }

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
            elements.push(("Enter", "Do Scan"));
            elements.push(("Esc", "Close"));
        } else if self.show_create {
            elements = self.create_key_form.footer_elements();
            elements.push(("Enter", "Create"));
            elements.push(("Esc", "Close"));
        } else {
            if self.current_screen == KeysTree {
                elements.push(("/", "Scan"));
                elements.push(("c", "Create"));
                elements.push(("d/Del", "Delete"));
                elements.push(("r", "Rename"));
                elements.push(("↑/j", "Up"));
                elements.push(("↓/k", "Down"));
                elements.push(("←/h", "Close"));
                elements.push(("→/l", "Open"));
            } else if self.current_screen == ValuesViewer {
                if let Some(ref list_value) = self.selected_list_value {
                    list_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                }
                if let Some(ref set_value) = self.selected_set_value {
                    set_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                }
                if let Some(ref zset_value) = self.selected_zset_value {
                    zset_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                }
                if let Some(ref hash_value) = self.selected_hash_value {
                    hash_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                }
            }
        }
        elements
    }
}

impl Listenable for ExplorerTab {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if self.show_delete_popup {
            return self.handle_delete_popup_key_event(key_event);
        }
        if self.show_filter {
            return self.handle_filter_key_event(key_event);
        }
        if self.show_rename {
            return self.handle_rename_key_event(key_event);
        }
        if self.show_create {
            return self.handle_create_key_event(key_event);
        }

        if ValuesViewer == self.current_screen {
            if KeyCode::Left == key_event.code || KeyCode::Char('h') == key_event.code {
                self.toggle_screen(KeysTree);
                return Ok(true);
            }
            if let Some(ref mut raw_value) = self.selected_raw_value {
                if key_event.modifiers == KeyModifiers::NONE {
                    match key_event.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            raw_value.scroll_down();
                            return Ok(true);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            raw_value.scroll_up();
                            return Ok(true);
                        }
                        KeyCode::PageDown => {
                            raw_value.scroll_page_down();
                            return Ok(true);
                        }
                        KeyCode::PageUp => {
                            raw_value.scroll_page_up();
                            return Ok(true);
                        }
                        KeyCode::End => {
                            raw_value.scroll_end();
                            return Ok(true);
                        }
                        KeyCode::Home => {
                            raw_value.scroll_start();
                            return Ok(true);
                        }
                        _ => {}
                    }
                }
            }
            if let Some(ref mut list_value) = self.selected_list_value {
                let accepted = list_value.handle_key_event(key_event)?;
                if accepted {
                    return Ok(true);
                }
            }
            if let Some(ref mut set_value) = self.selected_set_value {
                let accepted = set_value.handle_key_event(key_event)?;
                if accepted {
                    return Ok(true);
                }
            }
            if let Some(ref mut zset_value) = self.selected_zset_value {
                let accepted = zset_value.handle_key_event(key_event)?;
                if accepted {
                    return Ok(true);
                }
            }
            if let Some(ref mut hash_value) = self.selected_hash_value {
                let accepted = hash_value.handle_key_event(key_event)?;
                if accepted {
                    return Ok(true);
                }
            }
        }

        if KeysTree == self.current_screen {
            if key_event.modifiers == KeyModifiers::NONE {
                if self.handle_tree_key_event(key_event)? {
                    return Ok(true);
                };
                match key_event.code {
                    KeyCode::Char('/') => {
                        self.show_filter = true;
                        return Ok(true);
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        if self.selected_key.is_some() {
                            self.show_delete_popup = true;
                        }
                        return Ok(true);
                    }
                    KeyCode::Char('c') => {
                        self.show_create = true;
                        return Ok(true);
                    }
                    KeyCode::Char('r') => {
                        if let Some(redis_key) = &self.selected_key {
                            let key_name = redis_key.name.clone();
                            clean_text_area(&mut self.rename_key_text_area);
                            self.rename_key_text_area.insert_str(key_name);
                            self.show_rename = true;
                        }
                        return Ok(true);
                    }
                    _ => {}
                }
            }
        }

        match key_event.code {
            KeyCode::Char('`') => {
                if KeysTree == self.current_screen {
                    self.toggle_screen(ValuesViewer);
                } else {
                    self.toggle_screen(KeysTree);
                }
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn on_app_event(&mut self, _app_event: AppEvent) -> Result<()> {
        if _app_event == AppEvent::Init {
            if let Some(first_line) = self.get_filter_text() {
                self.do_scan(first_line)?;
            }
        }
        if _app_event == AppEvent::Reset {
            self.show_delete_popup = false;
            self.show_filter = false;
            self.filter_text_area = TextArea::default();
            if let Some(first_line) = self.get_filter_text() {
                self.do_scan(first_line)?;
            }
        }
        Ok(())
    }
}
