#![allow(unused)]

use anyhow::{Error, Result};
use bitflags::bitflags;
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::info;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Layout, Rect};
use ratatui::layout::Constraint::{Fill, Length};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Clear};
use redis::FromRedisValue;
use ratisui_core::redis_opt::spawn_redis_opt;
use ratisui_core::theme::get_color;
use crate::app::{Listenable, Renderable};
use crate::components::completion::{sort_commands, CompletableTextArea, CompletionItem};

pub struct FtSearchPanel<'a> {
    editing: Editing,
    indexes: Option<Vec<String>>,
    index_info: Option<IndexInfo>,
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
        const FILTER = 0b0000_0010;
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
    attributes: Vec<AttributeInfo>,
    num_docs: u64,
    max_doc_id: u64,
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
        let search_area = CompletableTextArea::new();
        Self {
            editing: Editing::Index,
            indexes: None,
            index_info: None,
            index_area,
            search_area,
            index_block: Block::bordered().title("Index").border_type(BorderType::Double),
            search_block: Block::bordered().title("Search").border_type(BorderType::Plain),
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
                indexes: Some(indexes),
                ..Default::default()
            })?;
            Ok::<(), Error>(())
        })?)
    }

    fn update_data(&mut self, data: Data) {
        if data.flags.contains(Flags::INDEX) {
            self.indexes = data.indexes;
        }
        if data.flags.contains(Flags::FILTER) {
            self.index_info = data.index_info;
        }
    }

    fn next(&mut self) {
        self.editing = match self.editing {
            Editing::Index => {
                self.index_area.blur();
                self.search_area.focus();
                self.index_block = self.index_block.clone().border_type(BorderType::Plain);
                self.search_block = self.search_block.clone().border_type(BorderType::Double);
                Editing::Search
            },
            Editing::Search => {
                self.index_area.focus();
                self.search_area.blur();
                self.index_block = self.index_block.clone().title_style(Style).border_type(BorderType::Double);
                self.search_block = self.search_block.clone().border_type(BorderType::Plain);
                Editing::Index
            }
        };
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
        let horizontal = Layout::horizontal([Length(35), Fill(1)]).split(rect);

        let index_area = self.index_block.inner(horizontal[0]);
        let search_area = self.index_block.inner(horizontal[1]);
        frame.render_widget(&self.index_block, horizontal[0]);
        frame.render_widget(&self.search_block, horizontal[1]);
        let block = Block::bordered();
        let inner_area = block.inner(rect);
        let frame_area = frame.area();
        self.index_area.update_frame(frame_area.height, frame_area.width);
        self.search_area.update_frame(frame_area.height, frame_area.width);
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
                    self.next();
                    return Ok(true);
                },
                _ => {}
            }
            match self.editing {
                Editing::Index => {
                    let accepted = self.index_area.handle_key_event(event)?;
                    if accepted {
                        info!("{}", self.index_area.get_input());
                    }
                    let (_, cursor_x) = self.index_area.get_cursor();
                    let raw_input = self.index_area.get_input();
                    let (mut items, segment) = get_index_items(&raw_input, cursor_x);
                    sort_commands(&mut items, &segment);
                    self.index_area.update_completion_items(items, segment);
                    Ok(accepted)
                }
                Editing::Search => {
                    let accepted = self.search_area.handle_key_event(event)?;
                    let (_, cursor_x) = self.search_area.get_cursor();
                    let raw_input = self.search_area.get_input();
                    let (mut items, segment) = get_filter_items(&raw_input, cursor_x);
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

fn get_index_items(input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
    (vec![], "".to_string())
}

fn get_filter_items(input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
    (vec![], "".to_string())
}
