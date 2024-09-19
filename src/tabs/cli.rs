use crate::app::{Listenable, Renderable, TabImplementation};
use crate::redis_opt::spawn_redis_opt;
use crate::utils::{bytes_to_string, escape_string, is_clean_text_area};
use anyhow::{Error, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use log::{info, warn};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Max, Min};
use ratatui::layout::{Layout, Position, Rect};
use ratatui::prelude::{Line, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::style::Style;
use ratatui::text::{Span, Text};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;
use redis::{Value, VerbatimFormat};
use std::cmp;
use std::fmt::format;
use std::ops::Neg;
use std::time::{Duration, Instant};
use itertools::Itertools;
use tui_textarea::{CursorMove, Scrolling, TextArea};
use crate::components::console_output::{ConsoleData, OutputKind};
use ratatui_macros::{line};
use redis::Value::ServerError;

pub struct CliTab {
    mode: Mode,
    lock_input: bool,
    lock_at: Option<Instant>,
    history: Vec<String>,
    history_viewpoint: usize,
    input_text_area: TextArea<'static>,
    console_data: ConsoleData<'static>,
    data_sender: Sender<Value>,
    data_receiver: Receiver<Value>,
}

#[derive(Default, PartialEq, Eq, Clone)]
pub enum Mode {
    #[default]
    Insert,
    Normal,
}

impl TabImplementation for CliTab {
    fn palette(&self) -> tailwind::Palette {
        tailwind::GREEN
    }

    fn title(&self) -> Line<'static> {
        "    CLI     "
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }
}

impl Renderable for CliTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()>
    where
        Self: Sized,
    {
        let total_lines = self.console_data.paragraph.line_count(rect.width);
        let session_height = cmp::min(total_lines.saturating_add(1) as u16, rect.height);
        let vertical = Layout::vertical([Length(session_height), Fill(0)]).split(rect);
        let vertical = Layout::vertical([Length(session_height - 1), Length(1)]).split(vertical[0]);
        self.render_output(frame, vertical[0])?;
        self.render_input(frame, vertical[1])?;
        Ok(())
    }
}

impl Listenable for CliTab {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        match self.mode {
            Mode::Normal => {
                if key_event.kind == KeyEventKind::Press {
                    match key_event {
                        KeyEvent { code: KeyCode::Char('i'), modifiers: KeyModifiers::NONE, .. } => {
                            self.mode = Mode::Insert;
                        }
                        KeyEvent { code: KeyCode::Enter, .. } => {
                            let command = self.get_command();
                            if let Some(command) = command {
                                spawn_redis_opt(move |operations| async move {
                                    let x = operations.str_cmd(command).await?;

                                    info!("{:?}", x);
                                    Ok(())
                                })?;
                            }
                        }
                        KeyEvent { code: KeyCode::Home, .. } => {
                            self.scroll_start();
                        }
                        KeyEvent { code: KeyCode::End, .. } => {
                            self.scroll_end();
                        }
                        KeyEvent { code: KeyCode::Up, .. } => {
                            self.scroll_up();
                        }
                        KeyEvent { code: KeyCode::Down, .. } => {
                            self.scroll_down();
                        }
                        KeyEvent { code: KeyCode::PageUp, .. } => {
                            self.scroll_page_up();
                        }
                        KeyEvent { code: KeyCode::PageDown, .. } => {
                            self.scroll_page_down();
                        }
                        _ => {}
                    }
                }
            }
            Mode::Insert => {
                if key_event.kind == KeyEventKind::Press {
                    match key_event {
                        KeyEvent { code: KeyCode::Esc, .. } => {
                            if self.input_text_area.is_selecting() {
                                self.input_text_area.cancel_selection();
                            } else {
                                self.mode = Mode::Normal;
                            }
                        }
                        KeyEvent { code: KeyCode::Enter, .. } => {
                            let command = self.get_command();
                            if let Some(command) = command {
                                self.commit_command(&command);
                            }
                        }
                        KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
                        KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => {
                            self.input_text_area.select_all();
                        }
                        KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                            self.input_text_area.undo();
                        }
                        KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                            self.input_text_area.redo();
                        }
                        KeyEvent { code: KeyCode::BackTab, .. } => {
                            return Ok(false);
                        }
                        KeyEvent { code: KeyCode::Tab, .. } => {
                            return Ok(false);
                        }
                        KeyEvent { code: KeyCode::Up, .. } => {
                            self.history_viewpoint = self.history_viewpoint.saturating_sub(1);
                            if let Some(command) = self.history.get(self.history_viewpoint) {
                                let mut input_text_area = TextArea::default();
                                input_text_area.set_cursor_style(Style::default().rapid_blink().reversed());
                                input_text_area.set_cursor_line_style(Style::default());
                                input_text_area.insert_str(command);
                                self.input_text_area = input_text_area;
                            }
                        }
                        KeyEvent { code: KeyCode::Down, .. } => {
                            if self.history_viewpoint < self.history.len().saturating_sub(1) {
                                self.history_viewpoint = self.history_viewpoint.saturating_add(1);
                                if let Some(command) = self.history.get(self.history_viewpoint) {
                                    let mut input_text_area = TextArea::default();
                                    input_text_area.set_cursor_style(Style::default().rapid_blink().reversed());
                                    input_text_area.set_cursor_line_style(Style::default());
                                    input_text_area.insert_str(command);
                                    self.input_text_area = input_text_area;
                                }
                            } else {
                                self.history_viewpoint = self.history.len();
                                let mut input_text_area = TextArea::default();
                                input_text_area.set_cursor_style(Style::default().rapid_blink().reversed());
                                input_text_area.set_cursor_line_style(Style::default());
                                self.input_text_area = input_text_area;
                            }
                        }
                        input => {
                            if !self.lock_input {
                                self.input_text_area.input(input);
                            }
                        }
                    }
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

impl CliTab {
    pub fn new() -> Self {
        let mut input_text_area = TextArea::default();
        input_text_area.set_cursor_style(Style::default().rapid_blink().reversed());
        input_text_area.set_cursor_line_style(Style::default());

        let (tx, rx) = bounded(1);
        Self {
            mode: Mode::default(),
            lock_input: false,
            lock_at: None,
            history: vec![],
            history_viewpoint: 0,
            input_text_area,
            console_data: ConsoleData::default(),
            data_sender: tx,
            data_receiver: rx,
        }
    }

    fn clear_output(&mut self) {
        self.console_data = ConsoleData::default();
    }

    fn commit_command(&mut self, command: &String) {
        self.lock_input = true;
        self.lock_at = Some(Instant::now());
        self.console_data.push(format!(">_ {}", command));
        if command.is_empty() {
            self.lock_input = false;
            self.console_data.push("");
            self.console_data.build_paragraph();
            return;
        }
        if let Some(last_command) = self.history.last() {
            if last_command != command {
                self.history.push(command.clone())
            }
        } else {
            self.history.push(command.clone())
        }
        self.history_viewpoint = self.history.len();
        let mut input_text_area = TextArea::default();
        input_text_area.set_cursor_style(Style::default().rapid_blink().reversed());
        input_text_area.set_cursor_line_style(Style::default());
        self.input_text_area = input_text_area;

        if "clear".eq_ignore_ascii_case(&command) {
            self.clear_output();
            self.lock_input = false;
            return;
        }

        let cmd = command.clone();
        let sender = self.data_sender.clone();
        let result = spawn_redis_opt(move |operations| async move {
            match operations.str_cmd(cmd).await {
                Ok(value) => sender.send(value)?,
                Err(e) => sender.send(Value::SimpleString(format!("#err# {}", e.to_string())))?,
            }
            Ok(())
        });

        if let Err(e) = result {
            let string = format!("{}", e);
            self.console_data.push_err(string);
            self.lock_input = false;
            self.console_data.push("");
            self.console_data.build_paragraph();
        }
    }

    fn scroll_start(&mut self) {
        self.console_data.scroll_start();
    }

    fn scroll_end(&mut self) {
        self.console_data.scroll_end();
    }

    fn scroll_up(&mut self) {
        self.console_data.scroll_up();
    }

    fn scroll_down(&mut self) {
        self.console_data.scroll_down();
    }

    fn scroll_page_up(&mut self) {
        self.console_data.scroll_page_up();
    }

    fn scroll_page_down(&mut self) {
        self.console_data.scroll_page_down();
    }

    fn render_input(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let horizontal = Layout::horizontal([Length(3), Min(10)]).split(rect);
        frame.render_widget(Span::raw(">_ "), horizontal[0]);
        frame.render_widget(&self.input_text_area, horizontal[1]);
        Ok(())
    }

    fn render_output(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        if let Ok(v) = self.data_receiver.try_recv() {
            let lines = value_to_lines(&v, 0);
            self.console_data.extend(lines);
            if let Some(lock_at) = self.lock_at {
                let elapsed = lock_at.elapsed();
                let duration = chronoutil::RelativeDuration::from(elapsed).format_to_iso8601();
                self.console_data.push("---");
                self.console_data.push(format!("cost: {}", duration));
            }
            self.lock_input = false;
            self.console_data.push("");
            self.console_data.build_paragraph();
        }
        self.console_data.update(&rect);
        frame.render_widget(&self.console_data.paragraph, rect);
        Ok(())
    }

    fn get_command(&self) -> Option<String> {
        self.input_text_area.lines().get(0).cloned()
    }
}

fn value_to_lines(value: &Value, pad: u16) -> Vec<(OutputKind, String)> {
    let prepend = " ".repeat(pad as usize);
    let format = |str: &str| {
        (OutputKind::STD, format!("{prepend}{str}"))
    };
    let format_err = |str: &str| {
        (OutputKind::ERR, format!("{prepend}{str}"))
    };
    match value {
        Value::Nil => {
            vec![format("(empty)".as_ref())]
        }
        Value::Int(int) => {
            vec![format(int.to_string().as_ref())]
        }
        Value::BulkString(bulk_string) => {
            let bulk_string = bytes_to_string(bulk_string.clone()).unwrap_or_else(|e| e.to_string());
            let bulk_string = bulk_string.replace("\t", "\\t");
            // let bulk_string = format!("\"{}\"", bulk_string);
            let lines = bulk_string.lines();
            lines.map(|line| format(line)).collect_vec()
        }
        Value::Array(array) => {
            let mut lines = vec![];
            let mut i = 1;
            for value in array {
                let sub_lines = value_to_lines(value, pad + 2);
                if sub_lines.len() == 1 {
                    if let Some((kind, first_line)) = sub_lines.get(0) {
                        if first_line.len() > 2 {
                            let x = &first_line[2..];
                            match kind {
                                OutputKind::STD => lines.push(format(&format!("{i}) {x}"))),
                                OutputKind::ERR => lines.push(format_err(&format!("{i}) {x}"))),
                            }
                        } else {
                            lines.push(format(&format!("{i}) ")));
                        }
                    }
                } else {
                    lines.push(format(&format!("{i}) ")));
                    lines.extend(sub_lines);
                }
                i = i + 1;
            }
            lines
        }
        Value::SimpleString(string) => {
            let is_error = string.starts_with("#err#");
            let string = escape_string(string);
            let lines = string.lines();
            lines.map(|line| {
                if is_error {
                    format_err(line[5..].as_ref())
                } else {
                    format(line.as_ref())
                }
            }).collect_vec()
        }
        Value::Okay => {
            vec![format("Okay")]
        }
        Value::Map(map) => {
            let mut lines = vec![];
            let mut i = 1;
            for (key, value) in map {
                let k_lines = value_to_lines(key, pad + 2);
                let v_lines = value_to_lines(value, pad + 2);
                if k_lines.len() == 1 {
                    if let Some((kind, first_line)) = k_lines.get(0) {
                        let x = &first_line[2..];
                        match kind {
                            OutputKind::STD => lines.push(format(&format!("{i}) {x}"))),
                            OutputKind::ERR => lines.push(format_err(&format!("{i}) {x}"))),
                        }
                    }
                } else {
                    lines.push(format(&format!("{i}) ")));
                    lines.extend(k_lines);
                }
                i = i + 1;
                if v_lines.len() == 1 {
                    if let Some((kind, first_line)) = v_lines.get(0) {
                        let x = &first_line[2..];
                        match kind {
                            OutputKind::STD => lines.push(format(&format!("{i}) {x}"))),
                            OutputKind::ERR => lines.push(format_err(&format!("{i}) {x}"))),
                        }
                    }
                } else {
                    lines.push(format(&format!("{i}) ")));
                    lines.extend(v_lines);
                }
                i = i + 1;
            }
            lines
        }
        Value::Attribute { attributes, data, .. } => {
            vec![format_err("Attribute, not supported yet")]
        }
        Value::Set(set) => {
            let mut lines = vec![];
            let mut i = 1;
            for value in set {
                let sub_lines = value_to_lines(value, pad + 2);
                if sub_lines.len() == 1 {
                    if let Some((kind, first_line)) = sub_lines.get(0) {
                        let x = &first_line[2..];
                        match kind {
                            OutputKind::STD => lines.push(format(&format!("{i}) {x}"))),
                            OutputKind::ERR => lines.push(format_err(&format!("{i}) {x}"))),
                        }
                    }
                } else {
                    lines.push(format(&format!("{i}) ")));
                    lines.extend(sub_lines);
                }
                i = i + 1;
            }
            lines
        }
        Value::Double(double) => {
            vec![format(&double.to_string())]
        }
        Value::Boolean(boolean) => {
            vec![format(&boolean.to_string())]
        }
        Value::VerbatimString { format: _format, text, .. } => {
            match _format {
                VerbatimFormat::Unknown(s) => {
                    vec![format(format!("\"{}\"", escape_string(s)).as_ref())]
                }
                _ => {
                    text.lines().map(|line| format(line.as_ref())).collect_vec()
                }
            }
        }
        Value::BigNumber(big_number) => {
            vec![format(&big_number.to_string())]
        }
        Value::Push { kind, data, .. } => {
            let mut lines = vec![];
            for value in data {
                let sub_lines = value_to_lines(value, pad + 2);
                lines.extend(sub_lines);
            }
            lines
        }
        Value::ServerError(e) => {
            vec![format_err(&format!("{:?}", e))]
        }
    }
}