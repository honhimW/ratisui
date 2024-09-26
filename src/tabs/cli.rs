use crate::app::{Listenable, Renderable, TabImplementation};
use crate::components::console_output::{ConsoleData, OutputKind};
use crate::components::redis_cli::RedisCli;
use crate::redis_opt::{spawn_redis_opt, Disposable};
use crate::utils::{bytes_to_string, escape_string, split_args};
use anyhow::{Error, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use itertools::Itertools;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::{Layout, Rect};
use ratatui::prelude::{Line, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::Frame;
use redis::{Value, VerbatimFormat};
use std::cmp;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use strum::Display;
use throbber_widgets_tui::{Throbber, ThrobberState};

pub struct CliTab {
    mode: Mode,
    lock_input: bool,
    lock_at: Option<Instant>,
    history: Vec<String>,
    history_viewpoint: usize,
    redis_cli: RedisCli<'static>,
    // input_text_area: TextArea<'static>,
    console_data: ConsoleData<'static>,
    data_sender: Sender<Value>,
    data_receiver: Receiver<Value>,
    disposable: Arc<Mutex<Option<Box<dyn Disposable>>>>,
    listen_state: (ThrobberState, Instant),
    input_throbber_state: ThrobberState,
}

#[derive(Default, PartialEq, Eq, Clone, Display)]
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
        let input_height = 1u16;
        let session_height = cmp::min(input_height.saturating_add(total_lines as u16), rect.height);
        let vertical = Layout::vertical([Length(session_height), Fill(0), Length(1)]).split(rect);
        let session_vertical = Layout::vertical([Length(session_height - input_height), Length(input_height)]).split(vertical[0]);
        self.render_output(frame, session_vertical[0])?;
        self.render_input(frame, session_vertical[1])?;
        let throbber = Throbber::default()
            .throbber_set(throbber_widgets_tui::BRAILLE_EIGHT_DOUBLE);
        let horizontal = Layout::horizontal([Length(2), Fill(0)]).split(vertical[2]);
        frame.render_stateful_widget(throbber, horizontal[0], &mut self.input_throbber_state);
        frame.render_widget(Span::raw(format!("- {} -", self.mode)), horizontal[1]);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        if self.is_listening() {
            elements.push(("Esc", "Stop"));
        } else {
            match self.mode {
                Mode::Insert => {
                    elements.push(("Esc", "Normal"));
                    elements.push(("^Space", "Suggest"));
                },
                Mode::Normal => elements.push(("i", "Insert")),
            }
        }
        elements
    }
}

impl Listenable for CliTab {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if self.is_listening() {
            if key_event.kind == KeyEventKind::Press {
                match key_event {
                    KeyEvent { code: KeyCode::Esc, .. } => {
                        self.do_dispose();
                    }
                    KeyEvent { code: KeyCode::Home, .. } => {
                        self.scroll_start();
                    }
                    KeyEvent { code: KeyCode::End, .. } => {
                        self.scroll_end();
                    }
                    KeyEvent { code: KeyCode::Up | KeyCode::Char('k'), .. } => {
                        self.scroll_up();
                    }
                    KeyEvent { code: KeyCode::Down | KeyCode::Char('j'), .. } => {
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
            return Ok(true);
        }
        match self.mode {
            Mode::Normal => {
                if key_event.kind == KeyEventKind::Press {
                    match key_event {
                        KeyEvent { code: KeyCode::Char('i'), modifiers: KeyModifiers::NONE, .. } => {
                            self.mode = Mode::Insert;
                        }
                        KeyEvent { code: KeyCode::Home, .. } => {
                            self.scroll_start();
                        }
                        KeyEvent { code: KeyCode::End, .. } => {
                            self.scroll_end();
                        }
                        KeyEvent { code: KeyCode::Up | KeyCode::Char('k'), .. } => {
                            self.scroll_up();
                        }
                        KeyEvent { code: KeyCode::Down | KeyCode::Char('j'), .. } => {
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
                    if !self.lock_input {
                        self.input_throbber_state.calc_next();
                        let handled = self.redis_cli.handle_key_event(key_event)?;
                        if handled {
                            return Ok(true);
                        }
                    }
                    match key_event {
                        KeyEvent { code: KeyCode::Esc, .. } => {
                            self.mode = Mode::Normal;
                        }
                        KeyEvent { code: KeyCode::Enter, .. } => {
                            let command = self.get_command();
                            if let Some(command) = command {
                                self.commit_command(&command);
                            }
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
                                self.redis_cli = RedisCli::new();
                                self.redis_cli.insert_str(command);
                            }
                        }
                        KeyEvent { code: KeyCode::Down, .. } => {
                            if self.history_viewpoint < self.history.len().saturating_sub(1) {
                                self.history_viewpoint = self.history_viewpoint.saturating_add(1);
                                if let Some(command) = self.history.get(self.history_viewpoint) {
                                    self.redis_cli = RedisCli::new();
                                    self.redis_cli.insert_str(command);
                                }
                            } else {
                                self.history_viewpoint = self.history.len();
                                self.redis_cli = RedisCli::new();
                            }
                        }
                        KeyEvent { code: KeyCode::Home, modifiers: KeyModifiers::CONTROL, .. } => {
                            self.scroll_start();
                        }
                        KeyEvent { code: KeyCode::End, modifiers: KeyModifiers::CONTROL, .. } => {
                            self.scroll_end();
                        }
                        KeyEvent { code: KeyCode::Char('k'), modifiers: KeyModifiers::CONTROL, .. } => {
                            self.scroll_up();
                        }
                        KeyEvent { code: KeyCode::Char('j'), modifiers: KeyModifiers::CONTROL, .. } => {
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
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

impl CliTab {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            mode: Mode::default(),
            lock_input: false,
            lock_at: None,
            history: vec![],
            history_viewpoint: 0,
            redis_cli: RedisCli::new(),
            console_data: ConsoleData::default(),
            data_sender: tx,
            data_receiver: rx,
            disposable: Arc::new(Mutex::new(None)),
            listen_state: (ThrobberState::default(), Instant::now()),
            input_throbber_state: ThrobberState::default(),
        }
    }

    fn clear_output(&mut self) {
        self.console_data = ConsoleData::default();
    }

    fn do_dispose(&mut self) {
        let arc = Arc::clone(&self.disposable);
        let result = arc.lock();
        if let Ok(mut disposable) = result {
            if let Some(instance) = disposable.as_mut() {
                let _ = instance.disposable();
            }
            *disposable = None;
        }
    }

    fn commit_command(&mut self, command: &String) {
        self.lock_input = true;
        self.lock_at = Some(Instant::now());
        self.console_data.push(OutputKind::CMD, format!(">_ {}", command));
        self.console_data.build_paragraph();
        if command.is_empty() {
            self.lock_input = false;
            self.console_data.push_std("");
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
        let command = command.trim().to_string();
        self.history_viewpoint = self.history.len();
        self.redis_cli = RedisCli::new();

        if "clear".eq_ignore_ascii_case(&command) {
            self.clear_output();
            self.lock_input = false;
            return;
        }

        let args = split_args(&command);
        let result = if args.len() == 1 && "monitor".eq_ignore_ascii_case(&args[0]) {
            self.do_dispose();
            let arc = Arc::clone(&self.disposable);
            let sender_clone = self.data_sender.clone();
            spawn_redis_opt(move |operations| async move {
                let x = operations.monitor(sender_clone).await?;
                if let Ok(mut monitor) = arc.lock() {
                    *monitor = Some(Box::new(x));
                }
                Ok::<(), Error>(())
            })
        } else if args.len() == 2 && "subscribe".eq_ignore_ascii_case(&args[0]) {
            self.do_dispose();
            let arc = Arc::clone(&self.disposable);
            let sender_clone = self.data_sender.clone();
            spawn_redis_opt(move |operations| async move {
                let x = operations.subscribe(args[1].clone(), sender_clone).await?;
                if let Ok(mut subscriber) = arc.lock() {
                    *subscriber = Some(Box::new(x));
                }
                Ok::<(), Error>(())
            })
        } else if args.len() == 2 && "psubscribe".eq_ignore_ascii_case(&args[0]) {
            self.do_dispose();
            let arc = Arc::clone(&self.disposable);
            let sender_clone = self.data_sender.clone();
            spawn_redis_opt(move |operations| async move {
                let x = operations.psubscribe(args[1].clone(), sender_clone).await?;
                if let Ok(mut p_subscriber) = arc.lock() {
                    *p_subscriber = Some(Box::new(x));
                }
                Ok::<(), Error>(())
            })
        } else {
            let cmd = command.clone();
            let sender = self.data_sender.clone();
            spawn_redis_opt(move |operations| async move {
                match operations.str_cmd(cmd).await {
                    Ok(value) => sender.send(value)?,
                    Err(e) => {
                        sender.send(Value::VerbatimString {
                            format: VerbatimFormat::Unknown(String::from("ERROR")),
                            text: format!("{:?}", e),
                        })?;
                    },
                }
                Ok(())
            })
        };

        if let Err(e) = result {
            let string = format!("{}", e);
            self.console_data.push_err(string);
            self.lock_input = false;
            self.console_data.push_std("");
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

    fn is_listening(&self) -> bool {
        let arc = Arc::clone(&self.disposable);
        let read_result = arc.lock();
        if let Ok(disposable) = read_result {
            disposable.is_some()
        } else {
            false
        }
    }

    fn render_input(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        if self.is_listening() {
            if self.listen_state.1.elapsed() >= Duration::from_millis(150) {
                self.listen_state.0.calc_next();
                self.listen_state.1 = Instant::now();
            }
            let throbber = Throbber::default()
                .throbber_set(throbber_widgets_tui::BRAILLE_EIGHT_DOUBLE)
                .label(Span::raw("Listening...").style(Style::default().dim()));
            frame.render_stateful_widget(throbber, rect, &mut self.listen_state.0);
        } else {
            let vertical = Layout::vertical([Length(1), Length(1)]).split(rect);
            let horizontal = Layout::horizontal([Length(3), Min(10)]).split(vertical[0]);
            frame.render_widget(Span::raw(">_ "), horizontal[0]);
            let frame_area = frame.area();
            self.redis_cli.update_frame(frame_area.height, frame_area.width);
            self.redis_cli.render_frame(frame, horizontal[1])?;
        }
        Ok(())
    }

    fn render_output(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        if self.is_listening() {
            loop {
                match self.data_receiver.try_recv() {
                    Ok(v) => {
                        let lines = value_to_lines(&v, 0);
                        self.console_data.extend(lines);
                        self.console_data.build_paragraph();
                    }
                    Err(..) => {
                        break
                    }
                }
            }
            // self.scroll_end();
        } else {
            loop {
                match self.data_receiver.try_recv() {
                    Ok(v) => {
                        let lines = value_to_lines(&v, 0);
                        self.console_data.extend(lines);
                        if let Some(lock_at) = self.lock_at {
                            let elapsed = lock_at.elapsed();
                            let duration = chronoutil::RelativeDuration::from(elapsed).format_to_iso8601();
                            self.console_data.push(OutputKind::Else(Style::default().dim()), "---");
                            self.console_data.push(OutputKind::Else(Style::default().dim()), format!("Elapsed: {}", duration));
                        }
                        self.lock_input = false;
                        self.console_data.push_std("");
                        self.console_data.build_paragraph();
                    }
                    Err(..) => { break; }
                }
            }
        }
        self.console_data.update(&rect);
        frame.render_widget(&self.console_data.paragraph, rect);
        Ok(())
    }

    fn get_command(&self) -> Option<String> {
        Some(self.redis_cli.get_input())
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
            vec![format("(Nil)".as_ref())]
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
                                _ => {}
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
            let string = escape_string(string);
            let lines = string.lines();
            lines.map(|line| format(line.as_ref())).collect_vec()
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
                            _ => {}
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
                            _ => {}
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
        Value::Attribute { .. } => {
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
                            _ => {}
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
                    if s == "ERROR" {
                        text.lines().map(|line| format_err(line.as_ref())).collect_vec()
                    } else if s == "PROMPT" {
                        text.lines().map(|line| {
                            (OutputKind::Else(Style::default().dim()), format!("{prepend}{line}"))
                        }).collect_vec()
                    } else {
                        vec![format(format!("\"{}\"", escape_string(s)).as_ref())]
                    }
                }
                _ => {
                    text.lines().map(|line| format(line.as_ref())).collect_vec()
                }
            }
        }
        Value::BigNumber(big_number) => {
            vec![format(&big_number.to_string())]
        }
        Value::Push { data, .. } => {
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