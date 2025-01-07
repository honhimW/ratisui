#![allow(unused)]

use crate::app::{Listenable, Renderable};
use crate::components::completion::{sort_commands, CompletableTextArea, CompletionItem, Doc};
use anyhow::{Error, Result};
use bitflags::bitflags;
use crossbeam_channel::{unbounded, Receiver, Sender};
use futures::FutureExt;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Constraint::{Fill, Length, Percentage};
use ratatui::layout::{Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Clear};
use ratatui::Frame;
use ratisui_core::redis_opt::spawn_redis_opt;
use ratisui_core::theme::get_color;
use redis::{FromRedisValue, RedisResult, Value};
use std::collections::HashMap;

pub struct FtSearchPanel<'a> {
    editing: Editing,
    indexes: Option<Vec<String>>,
    index_info: Option<IndexInfo>,
    indexes_info: HashMap<String, IndexInfo>,
    index_area: CompletableTextArea<'a>,
    search_area: CompletableTextArea<'a>,
    index_block: Block<'a>,
    search_block: Block<'a>,
    data_sender: Sender<Data>,
    data_receiver: Receiver<Data>,
}

enum Editing {
    Index,
    Search,
}

bitflags! {
    #[derive(Default, Clone)]
    struct Flags: u8 {
        const NONE = 0b0000_0000;
        const INDEX = 0b0000_0001;
        const SEARCH = 0b0000_0010;
    }
}

#[derive(Default, Clone)]
struct Data {
    flags: Flags,
    indexes: Option<Vec<String>>,
    index_info: Option<IndexInfo>,
}

#[derive(Default, Clone)]
struct IndexInfo {
    name: String,
    key_type: String,
    prefixes: Vec<String>,
    attributes: Vec<AttributeInfo>,
    num_docs: i64,
    max_doc_id: i64,
    total_index_memory_sz_mb: f64,
}

#[derive(Default, Clone)]
struct AttributeInfo {
    identifier: String,
    attribute: String,
    kind: String,
}

impl<'a> FtSearchPanel<'a> {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let mut index_area = CompletableTextArea::new();
        index_area.focus();
        let mut search_area = CompletableTextArea::new();
        search_area.blur();
        Self {
            editing: Editing::Index,
            indexes: None,
            index_info: None,
            index_area,
            search_area,
            indexes_info: HashMap::new(),
            index_block: Block::bordered()
                .title("Index ●")
                .border_type(BorderType::Double),
            search_block: Block::bordered()
                .title("Search")
                .border_type(BorderType::Plain),
            data_sender: tx,
            data_receiver: rx,
        }
    }

    pub fn list_indexes(&mut self) -> Result<()> {
        let sender = self.data_sender.clone();
        Ok(spawn_redis_opt(move |operations| async move {
            let indexes = operations.str_cmd("FT._LIST").await?;
            let indexes = Vec::<String>::from_redis_value(&indexes)?;
            sender.send(Data {
                flags: Flags::INDEX,
                indexes: Some(indexes.clone()),
                ..Default::default()
            })?;

            for index in indexes.iter() {
                let sender_clone = sender.clone();
                let opt_clone = operations.clone();
                let index_clone = index.clone();
                tokio::spawn(async move {
                    let v: IndexInfo = opt_clone
                        .str_cmd(format!("FT.INFO {}", index_clone))
                        .await?;
                    sender_clone.send(Data {
                        flags: Flags::SEARCH,
                        index_info: Some(v),
                        ..Default::default()
                    })?;
                    Ok::<(), Error>(())
                });
            }
            Ok::<(), Error>(())
        })?)
    }

    fn update_data(&mut self, data: Data) {
        if data.flags.contains(Flags::INDEX) {
            self.indexes = data.indexes;
        }
        if data.flags.contains(Flags::SEARCH) {
            if let Some(ref index_info) = data.index_info {
                self.indexes_info
                    .insert(index_info.name.clone(), index_info.clone());
            }
        }
    }

    fn next(&mut self) {
        self.editing = match self.editing {
            Editing::Index => {
                self.index_area.blur();
                self.search_area.focus();
                self.index_block = Block::bordered()
                    .title("Index")
                    .border_type(BorderType::Plain);
                self.search_block = Block::bordered()
                    .title("Search ●")
                    .border_type(BorderType::Double);
                let input_index = self.index_area.get_input();
                if let Some(index_info) = self.indexes_info.get(&input_index) {
                    self.index_info = Some(index_info.clone());
                }
                Editing::Search
            }
            Editing::Search => {
                self.index_area.focus();
                self.search_area.blur();
                self.index_block = Block::bordered()
                    .title("Index ●")
                    .border_type(BorderType::Double);
                self.search_block = Block::bordered()
                    .title("Search")
                    .border_type(BorderType::Plain);
                Editing::Index
            }
        };
    }

    fn get_index_items(&self, input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
        if let Some(ref indexes) = self.indexes {
            let items = indexes
                .iter()
                .map(|index_name| {
                    let mut item = CompletionItem::custom(index_name, "Index");
                    if let Some(index_info) = self.indexes_info.get(index_name) {
                        item = item.detail(format!("{:.3}M", index_info.total_index_memory_sz_mb));
                        let mut doc = Doc::default()
                            .syntax(index_info.name.clone())
                            .summary(format!("[{}]", index_info.prefixes.join("] [")))
                            .attribute("type                     ", index_info.key_type.clone())
                            .attribute("num_docs                 ", index_info.num_docs.to_string())
                            .attribute(
                                "max_doc_id               ",
                                index_info.max_doc_id.to_string(),
                            )
                            .attribute(
                                "total_index_memory_sz_mb ",
                                format!("{:.3}M", index_info.total_index_memory_sz_mb),
                            )
                            .attribute(
                                "attributes_count         ",
                                index_info.attributes.len().to_string(),
                            );
                        // for info in index_info.attributes.iter() {
                        //     doc = doc
                        //         .attribute("type                     ", info.kind.clone())
                        //         .attribute("identifier               ", info.identifier.clone())
                        //         .attribute("attribute                ", info.attribute.clone())
                        //     ;
                        // }
                        item = item.description(doc);
                    };
                    item
                })
                .collect::<Vec<CompletionItem>>();
            return (items, input.to_string());
        }
        (vec![], "".to_string())
    }

    fn get_filter_items(&self, input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
        if let Some(ref index_info) = self.index_info {
            let items = index_info
                .attributes
                .iter()
                .map(|info| CompletionItem::custom(info.attribute.clone(), "attribute")
                    .detail(info.kind.clone())
                    .description(Doc::default()
                        .syntax(info.attribute.clone())
                        .summary(info.identifier.clone())
                        .attribute("type", info.kind.clone())
                    ))
                .collect::<Vec<CompletionItem>>();
            return (items, input.to_string());
        }
        (vec![], "".to_string())
    }
}

impl Renderable for FtSearchPanel<'_> {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        while !self.data_receiver.is_empty() {
            let data = self.data_receiver.try_recv();
            if let Ok(data) = data {
                self.update_data(data);
            }
        }

        frame.render_widget(Clear::default(), rect);
        let horizontal = if matches!(self.editing, Editing::Index) {
            Layout::horizontal([Percentage(30), Fill(1)]).split(rect)
        } else {
            Layout::horizontal([Percentage(20), Fill(1)]).split(rect)
        };

        let index_area = self.index_block.inner(horizontal[0]);
        let search_area = self.index_block.inner(horizontal[1]);
        frame.render_widget(&self.index_block, horizontal[0]);
        frame.render_widget(&self.search_block, horizontal[1]);
        let block = Block::bordered();
        let inner_area = block.inner(rect);
        let frame_area = frame.area();
        self.index_area
            .update_frame(frame_area.height, frame_area.width);
        self.search_area
            .update_frame(frame_area.height, frame_area.width);
        self.index_area.render_frame(frame, index_area)?;
        self.search_area.render_frame(frame, search_area)?;
        Ok(())
    }
}

impl Listenable for FtSearchPanel<'_> {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<bool> {
        if event.kind == KeyEventKind::Press {
            match event.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    let accepted = match self.editing {
                        Editing::Index => self.index_area.handle_key_event(event)?,
                        Editing::Search => self.search_area.handle_key_event(event)?,
                    };
                    if !accepted {
                        self.next();
                    }
                    return Ok(true);
                }
                _ => {}
            }
            match self.editing {
                Editing::Index => {
                    let accepted = self.index_area.handle_key_event(event)?;
                    let (_, cursor_x) = self.index_area.get_cursor();
                    let raw_input = self.index_area.get_input();
                    let (mut items, segment) = self.get_index_items(&raw_input, cursor_x);
                    sort_commands(&mut items, &segment);
                    self.index_area.update_completion_items(items, segment);
                    Ok(accepted)
                }
                Editing::Search => {
                    let accepted = self.search_area.handle_key_event(event)?;
                    let (_, cursor_x) = self.search_area.get_cursor();
                    let raw_input = self.search_area.get_input();
                    let (mut items, segment) = self.get_filter_items(&raw_input, cursor_x);
                    sort_commands(&mut items, &segment);
                    self.search_area.update_completion_items(items, segment);
                    Ok(accepted)
                }
            }
        } else {
            Ok(false)
        }
    }
}

impl FromRedisValue for IndexInfo {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let mut this = Self::default();
        if let Value::Map(ref map) = v {
            for (key, value) in map {
                if let Value::SimpleString(key) = key {
                    match key.as_ref() {
                        "index_name" => {
                            if let Value::SimpleString(value) = value {
                                this.name = value.clone();
                            }
                        }
                        "index_definition" => {
                            if let Value::Map(value) = value {
                                for (key, value) in value {
                                    if let Value::SimpleString(key) = key {
                                        if key == "key_type" {
                                            if let Value::SimpleString(value) = value {
                                                this.key_type = value.clone();
                                            }
                                        }
                                        if key == "prefixes" {
                                            if let Value::Array(value) = value {
                                                for v in value.iter() {
                                                    if let Value::SimpleString(s) = v {
                                                        this.prefixes.push(s.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "attributes" => {
                            if let Value::Array(value) = value {
                                for attribute in value {
                                    if let Value::Map(attribute) = attribute {
                                        let mut attribute_info = AttributeInfo {
                                            identifier: "".to_string(),
                                            attribute: "".to_string(),
                                            kind: "".to_string(),
                                        };
                                        for (key, value) in attribute {
                                            if let Value::SimpleString(key) = key {
                                                match key.as_ref() {
                                                    "identifier" => {
                                                        if let Value::SimpleString(identifier) =
                                                            value
                                                        {
                                                            attribute_info.identifier =
                                                                identifier.clone();
                                                        }
                                                    }
                                                    "attribute" => {
                                                        if let Value::SimpleString(attribute) =
                                                            value
                                                        {
                                                            attribute_info.attribute =
                                                                attribute.clone();
                                                        }
                                                    }
                                                    "type" => {
                                                        if let Value::SimpleString(kind) = value {
                                                            attribute_info.kind = kind.clone();
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        this.attributes.push(attribute_info);
                                    }
                                }
                            }
                        }
                        "num_docs" => {
                            if let Value::Int(value) = value {
                                this.num_docs = value.clone();
                            }
                        }
                        "max_doc_id" => {
                            if let Value::Int(value) = value {
                                this.max_doc_id = value.clone();
                            }
                        }
                        "total_index_memory_sz_mb" => {
                            if let Value::Double(value) = value {
                                this.total_index_memory_sz_mb = value.clone();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(this)
    }

    fn from_owned_redis_value(v: Value) -> RedisResult<Self> {
        Self::from_redis_value(&v)
    }
}
