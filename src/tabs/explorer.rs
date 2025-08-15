use crate::app::{AppEvent, Listenable, Renderable, TabImplementation, centered_rect};
use crate::components::create_key_editor::{Form, KeyType};
use crate::components::ft_search_panel::FtSearchPanel;
use crate::components::hash_table::HashValue;
use crate::components::list_table::ListValue;
use crate::components::popup::Popup;
use crate::components::raw_paragraph::RawParagraph;
use crate::components::set_table::SetValue;
use crate::components::stream_view::SteamView;
use crate::components::time_series_table::TimeSeriesValue;
use crate::components::zset_table::ZSetValue;
use crate::tabs::explorer::CurrentScreen::{KeysTree, ValuesViewer};
use anyhow::{Context, Error, Result, anyhow};
use bitflags::bitflags;
use crossbeam_channel::{Receiver, Sender, unbounded};
use deadpool_redis::redis::{FromRedisValue, Value};
use log::info;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Line, Style, Stylize, Text};
use ratatui::style::{Color, Modifier};
use ratatui::text::Span;
use ratatui::widgets::block::Position;
use ratatui::widgets::{
    Block, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
};
use ratatui::{Frame, symbols};
use ratisui_core::bus::{GlobalEvent, Message, publish_event, publish_msg};
use ratisui_core::marcos::KeyAsserter;
use ratisui_core::redis_opt::{async_redis_opt, redis_operations, spawn_redis_opt};
use ratisui_core::theme::get_color;
use ratisui_core::utils::{ContentType, deserialize_bytes};
use ratisui_core::utils::{bytes_to_string, clean_text_area};
use std::collections::HashMap;
use tokio::join;
use tui_textarea::TextArea;
use tui_tree_widget::{Tree, TreeItem, TreeState};

const PAGE_SIZE: isize = 100;

pub struct ExplorerTab {
    pub current_screen: CurrentScreen,
    show_filter: bool,
    show_create: bool,
    show_rename: bool,
    show_delete_popup: bool,
    show_search_popup: bool,
    filter_mod: FilterMod,
    scan_size: u16,
    try_format: bool,
    filter_text_area: TextArea<'static>,
    create_key_form: Form,
    rename_key_text_area: TextArea<'static>,
    scan_keys_result: Vec<RedisKey>,
    tree_state: TreeState<String>,
    tree_items: Vec<TreeItem<'static, String>>,
    ft_search_panel: FtSearchPanel<'static>,
    redis_separator: String,
    selected_key: Option<RedisKey>,
    selected_raw_value: Option<RawParagraph<'static>>,
    selected_list_value: Option<ListValue>,
    selected_set_value: Option<SetValue>,
    selected_zset_value: Option<ZSetValue>,
    selected_hash_value: Option<HashValue>,
    selected_stream_value: Option<SteamView>,
    selected_time_series_value: Option<TimeSeriesValue>,
    data_sender: Sender<Data>,
    data_receiver: Receiver<Data>,
    offset: isize,

    has_search_module: bool,
}

#[derive(Default, Clone)]
pub struct Data {
    key_name: String,
    data_flags: DataFlags,
    scan_keys_result: Vec<RedisKey>,
    selected_string_value: Option<(String, Option<ContentType>)>,
    selected_list_value: Option<Vec<String>>,
    selected_set_value: Option<Vec<String>>,
    selected_zset_value: Option<Vec<(String, f64)>>,
    selected_hash_value: Option<HashMap<String, String>>,
    selected_stream_value: Option<Vec<(String, Vec<String>)>>,
    selected_time_series_value: Option<Vec<(u64, f64)>>,
    key_type: Option<String>,
    key_size: Option<usize>,
    length: Option<usize>,
    ttl: Option<u64>,
}

impl Data {
    fn add(&mut self, data_flags: DataFlags) {
        self.data_flags |= data_flags;
    }
}

bitflags! {
    #[derive(Clone)]
    struct DataFlags: u16 {
        const NONE = 0b0000_0000_0000_0000;
        const SCAN_KEYS = 0b0000_0000_0000_0001;
        const STRING_VALUE = 0b0000_0000_0000_0010;
        const LIST_VALUE = 0b0000_0000_0000_0100;
        const SET_VALUE = 0b0000_0000_0000_1000;
        const ZSET_VALUE = 0b0000_0000_0001_0000;
        const HASH_VALUE = 0b0000_0000_0010_0000;
        const STREAM_VALUE = 0b0000_0000_0100_0000;
        const KEY_TYPE = 0b0000_0000_1000_0000;
        const KEY_SIZE = 0b0000_0001_0000_0000;
        const LENGTH = 0b0000_0010_0000_0000;
        const TTL = 0b0000_0100_0000_0000;

        const TIME_SERIES_VALUE = 0b0000_1000_0000_0000;
    }
}

impl Default for DataFlags {
    fn default() -> Self {
        Self::NONE
    }
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
        let mut item: Option<TreeItem<String>> = None;
        if child.children.is_empty() {
            if let Some(_type) = &child.key_type {
                let name_span = Span::raw(key.clone());
                let mut line = Line::default();
                line.push_span(name_span);
                let text = Text::from(line);
                item = Some(TreeItem::new_leaf(child.id.clone(), text));
            } else {
                item = Some(TreeItem::new_leaf(child.id.clone(), key.clone()));
            }
        } else {
            let vec = build_tree_items(child);
            if let Ok(tree_item) = TreeItem::new(child.id.clone(), key.clone(), vec) {
                item = Some(tree_item);
            }
        }
        if let Some(item) = item {
            items.push(item);
        }
    }
    items.sort_by(|x, x1| x.identifier().cmp(x1.identifier()));
    items
}

fn get_type_color(key_type: &str) -> Color {
    match key_type {
        "Hash" | "hash" => get_color(|t| &t.tab.explorer.key_type.hash),
        "List" | "list" => get_color(|t| &t.tab.explorer.key_type.list),
        "Set" | "set" => get_color(|t| &t.tab.explorer.key_type.set),
        "ZSet" | "zset" => get_color(|t| &t.tab.explorer.key_type.zset),
        "String" | "string" => get_color(|t| &t.tab.explorer.key_type.string),
        "JSON" | "json" | "ReJSON-RL" | "ReJSON" => get_color(|t| &t.tab.explorer.key_type.json),
        "Stream" | "stream" => get_color(|t| &t.tab.explorer.key_type.stream),
        "TSDB-TYPE" | "tsdb-type" => get_color(|t| &t.tab.explorer.key_type.time_series),
        "MBbloom--" | "mbbloom--" => get_color(|t| &t.tab.explorer.key_type.bloom_filter),
        "unknown" => get_color(|t| &t.tab.explorer.key_type.unknown),
        _ => Color::default(),
    }
}

fn get_type_style(key_type: &str) -> Style {
    Style::default()
        .bold()
        .bg(get_type_color(key_type))
        .fg(Color::default())
}

impl RedisKey {
    fn new<N: Into<String>, T: Into<String>>(name: N, key_type: T) -> Self {
        Self {
            name: name.into(),
            key_type: key_type.into(),
            ..Self::default()
        }
    }
}

impl ExplorerTab {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let mut filter_text_area = TextArea::default();
        let mut rename_key_text_area = TextArea::default();
        filter_text_area.set_cursor_line_style(Style::default());
        rename_key_text_area.set_cursor_line_style(Style::default());
        rename_key_text_area.set_block(Block::bordered().title("Rename Key"));
        Self {
            current_screen: KeysTree,
            show_filter: false,
            show_create: false,
            show_rename: false,
            show_delete_popup: false,
            show_search_popup: false,
            filter_mod: FilterMod::Fuzzy,
            scan_size: 2_000,
            try_format: false,
            filter_text_area,
            create_key_form: Form::default().title("Create Key"),
            rename_key_text_area,
            scan_keys_result: vec![],
            tree_state: Default::default(),
            tree_items: vec![],
            ft_search_panel: FtSearchPanel::new(),
            redis_separator: ":".to_string(),
            selected_key: None,
            selected_raw_value: None,
            selected_list_value: None,
            selected_set_value: None,
            selected_zset_value: None,
            selected_hash_value: None,
            selected_stream_value: None,
            selected_time_series_value: None,
            data_sender: tx,
            data_receiver: rx,
            offset: 0,
            has_search_module: false,
        }
    }

    fn update_data(&mut self, data: Data) {
        let flags = data.data_flags;
        if flags.contains(DataFlags::SCAN_KEYS) {
            self.scan_keys_result = data.scan_keys_result;
            let _ = self.build_tree_items();
        }
        if let Some(redis_key) = &mut self.selected_key
            && redis_key.name == data.key_name
        {
            if flags.contains(DataFlags::KEY_TYPE) {
                redis_key.key_type = data.key_type.unwrap_or("unknown".to_string());
            }
            if flags.contains(DataFlags::KEY_SIZE) {
                redis_key.key_size = data.key_size;
            }
            if flags.contains(DataFlags::LENGTH) {
                redis_key.length = data.length;
            }
            if flags.contains(DataFlags::TTL) {
                redis_key.ttl = data.ttl;
            }
            if flags.contains(DataFlags::STRING_VALUE) {
                let raw = data.selected_string_value.unwrap_or_default();
                self.selected_raw_value = Some(RawParagraph::new(raw.0, raw.1, self.try_format));
            }
            if flags.contains(DataFlags::LIST_VALUE) {
                self.selected_list_value = Some(ListValue::new(
                    data.selected_list_value.unwrap_or_default(),
                    self.offset as usize,
                ));
            }
            if flags.contains(DataFlags::SET_VALUE) {
                self.selected_set_value = Some(SetValue::new(
                    data.selected_set_value.unwrap_or_default(),
                    self.offset as usize,
                ));
            }
            if flags.contains(DataFlags::ZSET_VALUE) {
                self.selected_zset_value = Some(ZSetValue::new(
                    data.selected_zset_value.unwrap_or_default(),
                    self.offset as usize,
                ));
            }
            if flags.contains(DataFlags::HASH_VALUE) {
                self.selected_hash_value = Some(HashValue::new(
                    data.selected_hash_value.unwrap_or_default(),
                    self.offset as usize,
                ));
            }
            if flags.contains(DataFlags::STREAM_VALUE) {
                self.selected_stream_value = Some(SteamView::new(
                    data.selected_stream_value.unwrap_or_default(),
                ));
            }
            if flags.contains(DataFlags::TIME_SERIES_VALUE) {
                self.selected_time_series_value = Some(TimeSeriesValue::new(
                    data.selected_time_series_value.unwrap_or_default(),
                    self.offset as usize,
                ));
            }
        }
    }

    fn border_color(&self, screen: CurrentScreen) -> Color {
        if screen == self.current_screen {
            get_color(|t| &t.border.highlight)
        } else {
            get_color(|t| &t.border.default)
        }
    }

    fn render_keys_block(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        if self.show_filter || self.show_rename {
            let vertical = Layout::vertical([Min(0), Length(3)]).split(area);
            self.render_tree(frame, vertical[0])?;
            if self.show_filter {
                self.render_filter_input(frame, vertical[1])?;
            }
            if self.show_rename {
                self.render_rename_key_input(frame, vertical[1])?;
            }
        } else if self.show_search_popup {
            let vertical = Layout::vertical([Min(0), Length(6)]).split(area);
            self.render_tree(frame, vertical[0])?;
            self.ft_search_panel.render_frame(frame, vertical[1])?;
        } else {
            self.render_tree(frame, area)?;
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
        let values_block = Block::default()
            .title("Info")
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

            let key_size_span =
                Span::raw(format!("Key Size: {:?} B", redis_key.key_size.unwrap_or(0)));
            let length_span = Span::raw(format!("  Length: {:?}", redis_key.length.unwrap_or(0)));
            let ttl_span = if redis_key.ttl.is_some() {
                let duration =
                    chronoutil::RelativeDuration::seconds(redis_key.ttl.unwrap_or(0) as i64)
                        .format_to_iso8601();
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
            let paragraph = Paragraph::new(text).block(values_block);
            frame.render_widget(paragraph, area);
            return Ok(());
        }
        let values_text = Paragraph::new("N/A").block(values_block);
        frame.render_widget(values_text, area);
        Ok(())
    }

    fn render_value_view(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let values_block = Block::default()
            .title("Values")
            .padding(Padding::horizontal(1))
            .borders(Borders::ALL)
            .border_style(self.border_color(ValuesViewer));
        let block_inner_area = values_block.inner(area);
        frame.render_widget(values_block, area);
        if let Some(ref mut raw_value) = self.selected_raw_value {
            raw_value.render(frame, block_inner_area)?;
        } else if let Some(ref mut list_value) = self.selected_list_value {
            list_value.render_frame(frame, block_inner_area)?;
        } else if let Some(ref mut set_value) = self.selected_set_value {
            set_value.render_frame(frame, block_inner_area)?;
        } else if let Some(ref mut set_value) = self.selected_zset_value {
            set_value.render_frame(frame, block_inner_area)?;
        } else if let Some(ref mut hash_value) = self.selected_hash_value {
            hash_value.render_frame(frame, block_inner_area)?;
        } else if let Some(ref mut stream_view) = self.selected_stream_value {
            stream_view.render_frame(frame, block_inner_area)?;
        } else if let Some(ref mut time_series_view) = self.selected_time_series_value {
            time_series_view.render_frame(frame, block_inner_area)?;
        } else {
            let values_text = Paragraph::new("N/A");
            frame.render_widget(values_text, block_inner_area);
        }
        Ok(())
    }

    fn render_filter_input(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        match self.filter_mod {
            FilterMod::Fuzzy => self.filter_text_area.set_placeholder_text(" Fuzzy "),
            FilterMod::Pattern => self.filter_text_area.set_placeholder_text(" Pattern "),
        }
        self.filter_text_area.set_block(
            Block::bordered()
                .border_style(self.border_color(KeysTree))
                .title(format!("Scan Keys ({})", self.scan_keys_result.len())),
        );
        frame.render_widget(Clear::default(), area);
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
            text.push_line(
                Line::raw(redis_key.name.clone())
                    .alignment(Alignment::Center)
                    .underlined(),
            );
            text.push_line(Line::default());
            text.push_line(
                Line::raw("Will be deleted. Are you sure?")
                    .alignment(Alignment::Center)
                    .bold(),
            );
            let paragraph = Paragraph::new(text).alignment(Alignment::Center);
            let delete_popup = Popup::new(paragraph)
                .title(String::from(" [Enter] Confirm | [Esc] Cancel "))
                .title_position(Position::Bottom)
                .borders(Borders::ALL)
                .border_set(symbols::border::DOUBLE)
                .style(Style::default().bg(get_color(|t| &t.tab.explorer.accent)));
            frame.render_widget(delete_popup, popup_area);
        }
    }

    fn render_tree(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let tree = Tree::new(&self.tree_items)?
            .block(
                Block::bordered()
                    .title("Keys")
                    .title_bottom("")
                    .border_style(self.border_color(KeysTree)),
            )
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None),
            ))
            .highlight_style(
                Style::new()
                    .fg(get_color(|t| &t.tab.explorer.tree.highlight))
                    .bg(get_color(|t| &t.tab.explorer.tree.highlight_bg))
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
            FilterMod::Pattern => filter_text.clone(),
        };

        let sender = self.data_sender.clone();
        let size_clone = self.scan_size.clone();
        spawn_redis_opt(move |operations| async move {
            let mut data = Data::default();
            let keys = operations.scan(pattern, size_clone as usize).await;
            if let Ok(keys) = keys {
                let vec = keys
                    .iter()
                    .map(|s| RedisKey::new(s, "unknown"))
                    .collect::<Vec<RedisKey>>();
                data.add(DataFlags::SCAN_KEYS);
                data.scan_keys_result = vec;
            }
            sender.send(data.clone())?;
            Ok(())
        })?;
        Ok(())
    }

    fn do_ft_search(&mut self) -> Result<()> {
        let (index, query) = self.ft_search_panel.get_input()?;
        if index.is_empty() || query.is_empty() {
            publish_msg(Message::warning(
                "Neither 'Index' nor 'Query' can be empty.",
            ))?;
            return Ok(());
        }
        let sender = self.data_sender.clone();
        spawn_redis_opt(move |operations| async move {
            let result: Result<Value> = operations
                .str_cmd(format!("FT.SEARCH {index} {query}"))
                .await;

            match result {
                Ok(v) => {
                    if let Value::Map(entries) = v {
                        let mut data = Data::default();
                        let mut vec: Vec<RedisKey> = vec![];
                        entries
                            .iter()
                            .filter_map(|(key, value)| {
                                if let Value::SimpleString(key) = key
                                    && key == "results"
                                    && let Value::Array(array) = value
                                {
                                    return Some(array);
                                }
                                None
                            })
                            .flat_map(|array| array.iter())
                            .filter_map(|result| {
                                if let Value::Map(meta_data) = result {
                                    return Some(meta_data);
                                }
                                None
                            })
                            .flat_map(|meta_data| meta_data.iter())
                            .filter_map(|(k, v)| {
                                if let Value::SimpleString(k) = k
                                    && k == "id"
                                    && let Ok(id) = String::from_redis_value(v)
                                {
                                    return Some(RedisKey::new(id, "unknown"));
                                }
                                None
                            })
                            .for_each(|redis_key| vec.push(redis_key));
                        data.add(DataFlags::SCAN_KEYS);
                        data.scan_keys_result = vec;
                        sender.send(data.clone())?;
                    }
                }
                Err(e) => publish_msg(Message::error(format!("{:?}", e)))?,
            }
            Ok::<(), Error>(())
        })?;
        Ok(())
    }

    fn build_tree_items(&mut self) -> Result<()> {
        if let Some(first_line) = self.get_filter_text()
            && !first_line.is_empty()
        {
            let filter_text = &first_line.clone();
            self.scan_keys_result.retain(|redis_key| {
                let contains;
                match self.filter_mod {
                    FilterMod::Fuzzy => contains = redis_key.name.contains(filter_text),
                    FilterMod::Pattern => {
                        if filter_text == "*" {
                            return true;
                        }
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
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                if self.filter_text_area.is_selecting() {
                    self.filter_text_area.cancel_selection();
                } else {
                    self.show_filter = false;
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if let Some(first_line) = self.get_filter_text() {
                    self.do_scan(first_line)?;
                }
            }
            KeyEvent {
                code: KeyCode::Char('m'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {}
            KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => match self.filter_mod {
                FilterMod::Fuzzy => self.filter_mod = FilterMod::Pattern,
                FilterMod::Pattern => self.filter_mod = FilterMod::Fuzzy,
            },
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.filter_text_area.select_all(),
            KeyEvent {
                code: KeyCode::Char('z'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.filter_text_area.undo();
            }
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
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
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                if self.rename_key_text_area.is_selecting() {
                    self.rename_key_text_area.cancel_selection();
                } else {
                    self.show_rename = false;
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if let Some(first_line) = self.rename_key_text_area.lines().get(0).cloned()
                    && let Some(redis_key) = &mut self.selected_key
                {
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
                            FilterMod::Pattern => filter_text.clone(),
                        };
                        let sender = self.data_sender.clone();
                        let pattern_clone = pattern.clone();
                        let size_clone = self.scan_size.clone();
                        spawn_redis_opt(move |operations| async move {
                            operations.rename_nx(key_name, new_key_name).await?;
                            let mut data = Data::default();
                            let keys = operations.scan(pattern_clone, size_clone as usize).await?;
                            let vec = keys
                                .iter()
                                .map(|s| RedisKey::new(s, "unknown"))
                                .collect::<Vec<RedisKey>>();
                            data.add(DataFlags::SCAN_KEYS);
                            data.scan_keys_result = vec;
                            sender.send(data.clone())?;
                            Ok(())
                        })?;
                    }
                    self.selected_key = None;
                }
                self.show_rename = false;
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
            } => self.rename_key_text_area.select_all(),
            KeyEvent {
                code: KeyCode::Char('z'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.rename_key_text_area.undo();
            }
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
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
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.rename_key_text_area.is_selecting() {
                    self.rename_key_text_area.cancel_selection();
                } else {
                    self.show_create = false;
                }
                Ok(true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
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
                        FilterMod::Pattern => filter_text.clone(),
                    };
                    let sender = self.data_sender.clone();
                    let size_clone = self.scan_size.clone();
                    spawn_redis_opt(move |operations| async move {
                        match key_type {
                            KeyType::String => operations.set_nx(key_name.clone(), "").await?,
                            KeyType::Hash => operations.hset_nx(key_name.clone(), "", "").await?,
                            KeyType::List => operations.lpush(key_name.clone(), "").await?,
                            KeyType::Set => operations.sadd(key_name.clone(), "").await?,
                            KeyType::SortedSet => operations.zadd(key_name.clone(), "", 0.0).await?,
                            KeyType::Stream => operations.xadd(key_name.clone(), "", "").await?,
                        }

                        if let Some(ttl) = ttl {
                            operations.expire(key_name.clone(), ttl as i64).await?;
                        }

                        let mut data = Data::default();
                        let keys = operations.scan(pattern, size_clone as usize).await?;
                        let vec = keys
                            .iter()
                            .map(|s| RedisKey::new(s, "unknown"))
                            .collect::<Vec<RedisKey>>();
                        data.add(DataFlags::SCAN_KEYS);
                        data.scan_keys_result = vec;
                        sender.send(data.clone())?;

                        Ok(())
                    })?;
                }
                self.show_create = false;
                Ok(true)
            }
            input => self.create_key_form.handle_key_event(input),
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
                    self.tree_items
                        .retain_mut(|item| item.identifier().ne(key_name));
                }
                self.show_delete_popup = false;
            }
            KeyCode::Esc => self.show_delete_popup = false,
            _ => {}
        }

        Ok(true)
    }

    fn handle_search_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        let accepted = self.ft_search_panel.handle_key_event(key_event)?;
        if !accepted {
            if key_event.kind != KeyEventKind::Press || key_event.modifiers != KeyModifiers::NONE {
                return Ok(true);
            }
            match key_event.code {
                KeyCode::Enter => {
                    self.do_ft_search()?;
                    // self.show_search_popup = false;
                    return Ok(true);
                }
                KeyCode::Esc => {
                    self.show_search_popup = false;
                    return Ok(true);
                }
                _ => {}
            }
        }
        Ok(accepted)
    }

    fn handle_tree_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.modifiers == KeyModifiers::NONE {
            let current_selected_key = self.selected_key.clone().map(|current| current.name);
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
                if changed_selected_key != current_selected_key && let Some(id) = changed_selected_key {
                    let option = self
                        .scan_keys_result
                        .iter()
                        .find(|redis_key| id.eq(&redis_key.name))
                        .cloned();
                    self.selected_key = option;
                    self.selected_raw_value = None;
                    self.selected_list_value = None;
                    self.selected_set_value = None;
                    self.selected_zset_value = None;
                    self.selected_hash_value = None;
                    self.selected_stream_value = None;
                    self.selected_time_series_value = None;
                    if self.selected_key.is_some() {
                        let sender = self.data_sender.clone();
                        tokio::spawn(async move {
                            let data = Self::do_get_key_info(id.clone()).await?;
                            sender.send(data.clone())?;
                            if let Some(key_type) = data.key_type {
                                let data = Self::do_get_value(id.clone(), key_type, 0).await?;
                                sender.send(data)?;
                            }
                            Ok::<(), Error>(())
                        });
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
        let key_type = async_redis_opt(|op| async move { Ok(op.key_type(key_name_clone).await?) });
        let key_name_clone = key_name.clone();
        let key_size = async_redis_opt(|op| async move { Ok(op.mem_usage(key_name_clone).await?) });
        let key_name_clone = key_name.clone();
        let ttl = async_redis_opt(|op| async move { Ok(op.ttl(key_name_clone).await?) });
        let (key_type, key_size, ttl) = join!(key_type, key_size, ttl);
        if let Ok(key_type) = key_type {
            data.add(DataFlags::KEY_TYPE);
            data.key_type = Some(key_type.clone());
            let key_name_clone = key_name.clone();
            let key_type_clone = key_type.clone();
            let length = async_redis_opt(|op| async move {
                match key_type_clone.to_lowercase().as_str() {
                    "string" => Ok(op.strlen(key_name_clone).await?),
                    "list" => Ok(op.llen(key_name_clone).await?),
                    "hash" => Ok(op.hlen(key_name_clone).await?),
                    "set" => Ok(op.scard(key_name_clone).await?),
                    "zset" => Ok(op.zcard(key_name_clone).await?),
                    "stream" => Ok(op.xlen(key_name_clone).await?),
                    "rejson-rl" => {
                        let json_type = op.json_type(&key_name_clone).await?;
                        let len = match json_type.to_lowercase().as_str() {
                            "object" => op.json_objlen(key_name_clone).await?,
                            "array" => op.json_arrlen(key_name_clone).await?,
                            "string" => op.json_strlen(key_name_clone).await?,
                            _ => 0,
                        };
                        Ok(len)
                    }
                    "mbbloom--" => Ok(op.bf_item(key_name_clone).await?),
                    "tsdb-type" => Ok(op.ts_total(key_name_clone).await?),
                    _ => Ok(0),
                }
            })
            .await;
            if let Ok(length) = length {
                data.add(DataFlags::LENGTH);
                data.length = Some(length);
            }
        }
        if let Ok(key_size) = key_size {
            data.add(DataFlags::KEY_SIZE);
            data.key_size = Some(key_size as usize);
        }

        if let Ok(ttl) = ttl {
            data.add(DataFlags::TTL);
            if ttl.is_positive() {
                data.ttl = Some(ttl as u64);
            } else {
                data.ttl = None;
            }
        }
        Ok(data)
    }

    async fn do_get_value(key_name: String, key_type: String, offset: isize) -> Result<Data> {
        let mut data = Data::default();
        data.key_name = key_name.clone();
        let key_name_clone = key_name.clone();
        let start = offset;
        let stop = start + (PAGE_SIZE - 1);
        async_redis_opt(|op| async move {
            match key_type.to_lowercase().as_str() {
                "string" => {
                    let bytes: Vec<u8> = op.get(key_name_clone).await?;
                    let result =
                        deserialize_bytes(bytes).context("Failed to deserialize string")?;
                    data.add(DataFlags::STRING_VALUE);
                    data.selected_string_value = Some((result.0, result.1));
                }
                "list" => {
                    let values: Vec<Vec<u8>> = op.get_list(key_name_clone, start, stop).await?;
                    let strings: Vec<String> = values
                        .iter()
                        .map(|item| bytes_to_string(item.clone()).unwrap_or_else(|_| String::new()))
                        .collect();
                    data.add(DataFlags::LIST_VALUE);
                    data.selected_list_value = Some(strings);
                }
                "set" => {
                    let values: Vec<Value> = op
                        .sscan(key_name_clone, start as usize, PAGE_SIZE as usize)
                        .await?;
                    let values = values.get(1).cloned().unwrap_or(Value::Nil);
                    if let Value::Array(arr) = values {
                        let mut strings: Vec<String> = vec![];
                        for v in arr.iter() {
                            if let Value::BulkString(bytes) = v {
                                let string = bytes_to_string(bytes.clone()).unwrap_or_default();
                                strings.push(string);
                            }
                        }
                        data.add(DataFlags::SET_VALUE);
                        data.selected_set_value = Some(strings);
                    }
                }
                "zset" => {
                    let values: Vec<(Vec<u8>, f64)> =
                        op.get_zset(key_name_clone, start, stop).await?;
                    let tuples: Vec<(String, f64)> = values
                        .iter()
                        .map(|(item, score)| match bytes_to_string(item.clone()) {
                            Ok(s) => (s, score.clone()),
                            Err(_) => (String::new(), score.clone()),
                        })
                        .collect();
                    data.add(DataFlags::ZSET_VALUE);
                    data.selected_zset_value = Some(tuples);
                }
                "hash" => {
                    let values: Vec<Value> = op
                        .hscan(key_name_clone, start as usize, PAGE_SIZE as usize)
                        .await?;
                    let values = values.get(1).cloned().unwrap_or(Value::Nil);
                    if let Value::Array(arr) = values {
                        let hash_value: HashMap<String, String> = arr
                            .chunks_exact(2)
                            .map(|chunk| {
                                let key = match chunk.get(0) {
                                    Some(v) => match v {
                                        Value::BulkString(bytes) => {
                                            bytes_to_string(bytes.clone()).unwrap_or_default()
                                        }
                                        _ => String::new(),
                                    },
                                    None => String::new(),
                                };
                                let value = match chunk.get(1) {
                                    Some(v) => match v {
                                        Value::BulkString(bytes) => {
                                            bytes_to_string(bytes.clone()).unwrap_or_default()
                                        }
                                        _ => String::new(),
                                    },
                                    None => String::new(),
                                };
                                (key, value)
                            })
                            .collect();
                        data.add(DataFlags::HASH_VALUE);
                        data.selected_hash_value = Some(hash_value);
                    }
                }
                "stream" => {
                    let values: Vec<(Vec<u8>, Vec<Vec<u8>>)> =
                        op.xrange(key_name_clone, "-", PAGE_SIZE as usize).await?;
                    let hash_value: Vec<(String, Vec<String>)> = values
                        .iter()
                        .map(|(key, value)| {
                            let key_str: String = bytes_to_string(key.clone()).unwrap_or_default();
                            let values: Vec<String> = value
                                .iter()
                                .map(|item| bytes_to_string(item.clone()).unwrap_or_default())
                                .collect();
                            (key_str, values)
                        })
                        .collect();
                    data.add(DataFlags::STREAM_VALUE);
                    data.selected_stream_value = Some(hash_value);
                }
                "rejson-rl" => {
                    let json_string: String = op.json_get(key_name_clone).await?;
                    data.add(DataFlags::STRING_VALUE);
                    data.selected_string_value = Some((json_string, Some(ContentType::Json)));
                }
                "mbbloom--" => {}
                "tsdb-type" => {
                    let values: Vec<Vec<Value>> =
                        op.ts_range(key_name_clone, PAGE_SIZE as usize).await?;
                    let mut tuples: Vec<(u64, f64)> = vec![];
                    for value in values.iter() {
                        let mut ts: u64 = 0;
                        let mut v: f64 = 0.0;
                        if let Some(Value::Int(timestamp)) = value.first() {
                            ts = *timestamp as u64;
                        }
                        if let Some(Value::Double(val)) = value.get(1) {
                            v = *val;
                        }
                        tuples.push((ts, v));
                    }
                    data.add(DataFlags::TIME_SERIES_VALUE);
                    data.selected_time_series_value = Some(tuples);
                }
                _ => {}
            }
            Ok(data)
        })
        .await
    }

    fn buffer_turning(&mut self, key_event: &KeyEvent) -> bool {
        if let Some(ref redis_key) = self.selected_key {
            let key_name = redis_key.name.clone();
            let key_type = redis_key.key_type.clone();
            let length = redis_key.length.unwrap_or_default() as isize;
            if length < PAGE_SIZE {
                return false;
            }
            if key_event.is_c_n() {
                let sender = self.data_sender.clone();
                let mut offset = self.offset.saturating_add(PAGE_SIZE);
                if offset >= length {
                    offset = 0;
                }
                self.offset = offset;
                tokio::spawn(async move {
                    if let Ok(data) = Self::do_get_value(key_name, key_type, offset).await {
                        let _ = sender.send(data);
                    }
                });
                return true;
            } else if key_event.is_c_p() {
                let sender = self.data_sender.clone();
                let mut offset = self.offset.saturating_sub(PAGE_SIZE);
                if offset.is_negative() {
                    offset = length.saturating_add(offset);
                }
                self.offset = offset;
                tokio::spawn(async move {
                    if let Ok(data) = Self::do_get_value(key_name, key_type, offset).await {
                        let _ = sender.send(data);
                    }
                });
                return true;
            }
        }
        false
    }
}

impl TabImplementation for ExplorerTab {
    fn highlight(&self) -> Color {
        get_color(|t| &t.tab.explorer.highlight)
    }

    fn title(&self) -> Line<'static> {
        "  Explorer  "
            .fg(get_color(|t| &t.tab.title))
            .bg(get_color(|t| &t.tab.explorer.accent))
            .into()
    }
}

impl Renderable for ExplorerTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let chunks = match self.current_screen {
            KeysTree => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(37), Constraint::Fill(1)])
                .split(rect),
            ValuesViewer => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(20), Constraint::Fill(1)])
                .split(rect),
        };
        self.render_values_block(frame, chunks[1])?;
        self.render_keys_block(frame, chunks[0])?;

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
        } else if self.show_search_popup {
            elements = self.ft_search_panel.footer_elements();
            elements.push(("Enter", "Apply"));
            elements.push(("Esc", "Close"));
        } else {
            if self.current_screen == KeysTree {
                elements.push(("/", "Scan"));
                if self.has_search_module {
                    elements.push(("^k", "FT.Search"));
                }
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
                    elements.push(("^n", "Next Page"));
                    elements.push(("^p", "Prev Page"));
                }
                if let Some(ref set_value) = self.selected_set_value {
                    set_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                    elements.push(("^n", "Next Page"));
                    elements.push(("^p", "Prev Page"));
                }
                if let Some(ref zset_value) = self.selected_zset_value {
                    zset_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                    elements.push(("^n", "Next Page"));
                    elements.push(("^p", "Prev Page"));
                }
                if let Some(ref hash_value) = self.selected_hash_value {
                    hash_value.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                    elements.push(("^n", "Next Page"));
                    elements.push(("^p", "Prev Page"));
                }
                if let Some(ref stream_view) = self.selected_stream_value {
                    stream_view.footer_elements().iter().for_each(|(k, v)| {
                        elements.push((k, v));
                    });
                    elements.push(("←/h", "Close"));
                }
                if let Some(ref time_series_value) = self.selected_time_series_value {
                    time_series_value
                        .footer_elements()
                        .iter()
                        .for_each(|(k, v)| {
                            elements.push((k, v));
                        });
                    elements.push(("←/h", "Close"));
                    elements.push(("^n", "Next Page"));
                    elements.push(("^p", "Prev Page"));
                }
            }
        }
        elements
    }

    fn handle_data(&mut self) -> Result<bool> {
        let mut needed = false;
        while !self.data_receiver.is_empty() {
            let data = self.data_receiver.try_recv();
            if let Ok(data) = data {
                self.update_data(data);
                needed = true;
            }
        }
        let ft_search_panel_needed = self.ft_search_panel.handle_data()?;
        Ok(needed || ft_search_panel_needed)
    }
}

impl Listenable for ExplorerTab {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.is_c_c() {
            publish_event(GlobalEvent::Exit)?;
            return Ok(true);
        }
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
        if self.show_search_popup {
            return self.handle_search_key_event(key_event);
        }

        if ValuesViewer == self.current_screen {
            if let Some(ref mut raw_value) = self.selected_raw_value && key_event.modifiers == KeyModifiers::NONE {
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

            if self.selected_list_value.is_some()
                || self.selected_set_value.is_some()
                || self.selected_zset_value.is_some()
                || self.selected_hash_value.is_some()
            {
                if self.buffer_turning(&key_event) {
                    return Ok(true);
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
            if let Some(ref mut stream_view) = self.selected_stream_value {
                let accepted = stream_view.handle_key_event(key_event)?;
                if accepted {
                    return Ok(true);
                }
            }
            if let Some(ref mut time_series_value) = self.selected_time_series_value {
                let accepted = time_series_value.handle_key_event(key_event)?;
                if accepted {
                    return Ok(true);
                }
            }
            if KeyCode::Left == key_event.code || KeyCode::Char('h') == key_event.code {
                self.toggle_screen(KeysTree);
                return Ok(true);
            }
        }

        if KeysTree == self.current_screen {
            if key_event.is_c_k() {
                self.show_search_popup = true;
                self.ft_search_panel.list_indexes()?;
                return Ok(true);
            }
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

    fn on_app_event(&mut self, app_event: AppEvent) -> Result<()> {
        match app_event {
            AppEvent::InitConfig(configuration, _) => {
                self.scan_size = configuration.scan_size;
                self.try_format = configuration.try_format;
            }
            AppEvent::Reset => {
                self.show_delete_popup = false;
                self.show_filter = false;
                self.show_search_popup = false;
                self.filter_text_area = TextArea::default();
                if let Some(first_line) = self.get_filter_text() {
                    self.do_scan(first_line)?;
                }
            }
            AppEvent::Bus(global_event) => match global_event {
                GlobalEvent::ClientChanged => {
                    if let Some(first_line) = self.get_filter_text() {
                        self.do_scan(first_line)?;
                        if let Some(ref redis_opt) = redis_operations() {
                            self.has_search_module = redis_opt.has_module("search")?;
                            info!("has search module: {}", self.has_search_module);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
