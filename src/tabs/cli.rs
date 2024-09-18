use crate::app::{Listenable, Renderable, TabImplementation};
use crate::redis_opt::spawn_redis_opt;
use crate::utils::{bytes_to_string, is_clean_text_area};
use anyhow::{Error, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use log::info;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Max, Min};
use ratatui::layout::{Layout, Rect};
use ratatui::prelude::{Line, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Cell, Row, Table};
use ratatui::Frame;
use redis::{Value, VerbatimFormat};
use std::cmp;
use std::fmt::format;
use tui_textarea::{CursorMove, Scrolling, TextArea};

pub struct CliTab {
    mode: Mode,
    lock_input: bool,
    history: Vec<String>,
    history_viewpoint: usize,
    input_text_area: TextArea<'static>,
    output_text_area: TextArea<'static>,
    data_sender: Sender<Value>,
    data_receiver: Receiver<Value>,
}

#[derive(PartialEq, Eq, Clone)]
pub enum Mode {
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
        let total_lines = self.output_text_area.lines().len();
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
                        KeyEvent { code: KeyCode::Up, .. } => {
                            self.output_text_area.scroll(Scrolling::HalfPageUp);

                        }
                        KeyEvent { code: KeyCode::Down, .. } => {
                            self.output_text_area.scroll(Scrolling::HalfPageDown);
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

        let mut output_text_area = TextArea::default();
        // output_text_area.set_cursor_style(Style::default());
        // output_text_area.set_cursor_line_style(Style::default());
        output_text_area.set_block(Block::bordered());

        let (tx, rx) = bounded(1);
        Self {
            mode: Mode::Normal,
            lock_input: false,
            history: vec![],
            history_viewpoint: 0,
            input_text_area,
            output_text_area,
            data_sender: tx,
            data_receiver: rx,
        }
    }

    fn clear_output(&mut self) {
        let mut output_text_area = TextArea::default();
        output_text_area.set_cursor_style(Style::default());
        output_text_area.set_cursor_line_style(Style::default());
        output_text_area.set_block(Block::bordered());
        self.output_text_area = output_text_area;
    }

    fn commit_command(&mut self, command: &String) {
        self.lock_input = true;
        self.output_text_area.insert_str(format!("> {}", command));
        self.output_text_area.insert_newline();
        if command.is_empty() {
            self.lock_input = false;
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
            let x = operations.str_cmd(cmd).await?;
            sender.send(x)?;
            Ok(())
        });

        if let Err(e) = result {
            let string = format!("{}", e);
            self.output_text_area.insert_str(string);
            self.output_text_area.insert_newline();
            self.lock_input = false;
        }

    }

    // fn append_output(&mut self, content: String) {
    //     self.output_text_area.move_cursor(CursorMove::End)
    //     self.output_text_area.scroll(Scrolling::Delta {rows: })
    // }

    fn render_input(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let horizontal = Layout::horizontal([Length(3), Min(10)]).split(rect);
        frame.render_widget(Span::raw(">_ "), horizontal[0]);
        frame.render_widget(&self.input_text_area, horizontal[1]);
        Ok(())
    }

    fn render_output(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        if let Ok(v) = self.data_receiver.try_recv() {
            self.output_text_area.insert_str(format_value(v));
            self.output_text_area.insert_newline();
            self.lock_input = false;
        }
        frame.render_widget(&self.output_text_area, rect);
        Ok(())
    }

    fn get_command(&self) -> Option<String> {
        self.input_text_area.lines().get(0).cloned()
    }

}

fn format_value(value: Value) -> String {
    match value {
        Value::Nil => {
            String::from("(empty)")
        }
        Value::Int(int) => {
            int.to_string()
        }
        Value::BulkString(bulk_string) => {
            bytes_to_string(bulk_string).unwrap_or_else(|e| e.to_string())
        }
        Value::Array(array) => {
            let mut string = String::new();
            let mut i = 1;
            for value in array {
                let v = format_value(value);
                string.push_str(&format!("{})", i));
                string.push_str(&v);
                string.push('\n');
                i = i + 1;
            }
            string
        }
        Value::SimpleString(string) => {
            string
        }
        Value::Okay => {
            String::from("OK")
        }
        Value::Map(map) => {
            let mut string = String::new();
            let mut i = 1;
            for (key, value) in map {
                let k = format_value(key);
                let v = format_value(value);
                string.push_str(&format!("{})", i));
                string.push_str(&k);
                string.push('\n');
                i = i + 1;
                string.push_str(&format!("{})", i));
                string.push_str(&v);
                string.push('\n');
                i = i + 1;
            }
            string
        }
        Value::Attribute { attributes, data,.. } => {
            String::from("Attribute, not supported yet")
        }
        Value::Set(set) => {
            let mut string = String::new();
            let mut i = 1;
            for value in set {
                let v = format_value(value);
                string.push_str(&format!("{})", i));
                string.push_str(&format!("{}", v));
                string.push('\n');
                i = i + 1;
            }
            string
        }
        Value::Double(double) => {
            double.to_string()
        }
        Value::Boolean(boolean) => {
            boolean.to_string()
        }
        Value::VerbatimString { format, text,.. } => {
            text
        }
        Value::BigNumber(big_number) => {
            big_number.to_string()
        }
        Value::Push { .. } => {
            String::new()
        }
        Value::ServerError(e) => {
            format!("{:?}", e)
        }
    }
}