use crate::app::{Listenable, Renderable};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Percentage};
use ratatui::layout::{Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Clear};
use ratisui_core::configuration::{CliOutputFormatKind, Configuration, save_configuration};
use ratisui_core::marcos::KeyAsserter;
use ratisui_core::theme::get_color;
use ratisui_core::utils::clean_text_area;
use std::cmp;
use std::sync::Arc;
use strum::{Display, EnumCount, EnumIter, IntoEnumIterator};
use tui_textarea::TextArea;
use ratisui_core::bus::{publish_event, GlobalEvent};

pub struct Options {
    default_configuration: Configuration,
    editing_style: Style,
    editing: usize,
    fps: TextArea<'static>,
    scan_size: TextArea<'static>,
    try_format: bool,
    theme: TextArea<'static>,
    history_size: TextArea<'static>,
    cli_output_format: OutputFormat,
    console_capacity: TextArea<'static>,
    enable_mouse_capture: bool,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter, Display)]
enum Editing {
    #[default]
    #[strum(serialize = "FPS")]
    Fps,
    #[strum(serialize = "Scan Size")]
    ScanSize,
    #[strum(serialize = "Try Format")]
    TryFormat,
    #[strum(serialize = "Theme")]
    Theme,
    #[strum(serialize = "History Size")]
    HistorySize,
    #[strum(serialize = "Cli Output Format")]
    CliOutputFormat,
    #[strum(serialize = "Console Capacity")]
    ConsoleCapacity,
    #[strum(serialize = "Enable Mouse Capture")]
    EnableMouseCapture,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter, Display)]
pub enum OutputFormat {
    #[default]
    Redis,
    Ron,
}

fn cursor_style() -> Style {
    Style::default().rapid_blink().reversed()
}

impl Default for Options {
    fn default() -> Self {
        let mut option = Self {
            default_configuration: Configuration::default(),
            editing_style: Style::default().fg(get_color(|t| &t.editor.editing)),
            editing: 0,
            fps: TextArea::default(),
            scan_size: TextArea::default(),
            try_format: false,
            theme: TextArea::default(),
            history_size: TextArea::default(),
            cli_output_format: OutputFormat::default(),
            console_capacity: TextArea::default(),
            enable_mouse_capture: false,
        };
        option
            .fps
            .set_placeholder_style(Style::default().fg(get_color(|t| &t.editor.warning)).dim());
        option
    }
}

impl Options {
    pub fn next(&mut self) {
        let tmp = self.editing + 1;
        self.editing = tmp % Editing::COUNT;
        self.change_editing();
    }

    pub fn prev(&mut self) {
        let tmp = self.editing + (Editing::COUNT - 1);
        self.editing = tmp % Editing::COUNT;
        self.change_editing();
    }

    fn current(&self) -> Editing {
        Editing::iter().get(self.editing).unwrap()
    }

    fn span(&self, editing: Editing) -> Span<'static> {
        let title = editing.to_string();
        let style = if editing == self.current() {
            self.editing_style
        } else {
            Style::default()
        };
        Span::raw(title).style(style)
    }

    fn change_editing(&mut self) {
        self.fps.set_cursor_style(Style::default());
        self.scan_size.set_cursor_style(Style::default());
        self.theme.set_cursor_style(Style::default());
        self.history_size.set_cursor_style(Style::default());
        self.console_capacity.set_cursor_style(Style::default());
        self.fps.cancel_selection();
        self.scan_size.cancel_selection();
        self.theme.cancel_selection();
        self.history_size.cancel_selection();
        self.console_capacity.cancel_selection();

        match self.current() {
            Editing::Fps => self.fps.set_cursor_style(cursor_style()),
            Editing::ScanSize => self.scan_size.set_cursor_style(cursor_style()),
            Editing::Theme => self.theme.set_cursor_style(cursor_style()),
            Editing::HistorySize => self.history_size.set_cursor_style(cursor_style()),
            Editing::ConsoleCapacity => self.console_capacity.set_cursor_style(cursor_style()),
            _ => {}
        }
    }

    fn render_fps(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Fps);
        self.fps.set_style(key.style);
        let value = &self.fps;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_scan_size(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::ScanSize);
        self.scan_size.set_style(key.style);
        let value = &self.scan_size;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_theme(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Theme);
        self.theme.set_style(key.style);
        let value = &self.theme;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_try_format(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::TryFormat);
        let value = Span::raw(if self.try_format {
            "◄ Yes ►"
        } else {
            "◄ No ►"
        })
        .style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_history_size(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::HistorySize);
        self.history_size.set_style(key.style);
        let value = &self.history_size;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_console_capacity(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::ConsoleCapacity);
        self.console_capacity.set_style(key.style);
        let value = &self.console_capacity;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_cli_output_format(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::CliOutputFormat);
        let value =
            Span::raw(format!("◄ {} ►", self.cli_output_format.to_string())).style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn render_enable_mouse_capture(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(22), Fill(0), Length(16)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::EnableMouseCapture);
        let value = Span::raw(if self.enable_mouse_capture {
            "◄ Yes ►"
        } else {
            "◄ No ►"
        })
        .style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[2]);
    }

    fn get_fps(&self) -> u8 {
        let first_line = self.fps.lines().get(0).clone().cloned();
        if let Some(first_line) = first_line {
            return first_line.parse::<u8>().unwrap_or(0);
        }
        self.default_configuration.fps
    }

    fn get_scan_size(&self) -> u16 {
        let first_line = self.scan_size.lines().get(0).clone().cloned();
        if let Some(first_line) = first_line {
            return first_line.parse::<u16>().unwrap_or(0);
        }
        self.default_configuration.scan_size
    }

    fn get_theme(&self) -> Option<String> {
        let first_line = self.theme.lines().get(0).clone().cloned();
        if let Some(first_line) = first_line {
            if first_line.is_empty() {
                return None;
            }
            return first_line.parse::<String>().ok();
        }
        self.default_configuration.theme.clone()
    }

    fn get_history_size(&self) -> u32 {
        let first_line = self.history_size.lines().get(0).clone().cloned();
        if let Some(first_line) = first_line {
            return first_line.parse::<u32>().unwrap_or(0);
        }
        self.default_configuration.history_size
    }

    fn get_console_capacity(&self) -> usize {
        let first_line = self.console_capacity.lines().get(0).clone().cloned();
        if let Some(first_line) = first_line {
            return first_line.parse::<usize>().unwrap_or(0);
        }
        self.default_configuration.console_capacity
    }

    pub fn init_values(&mut self, config: Arc<Configuration>) {
        self.fps.insert_str(config.fps.to_string());
        self.scan_size.insert_str(config.scan_size.to_string());
        self.try_format = config.try_format;
        self.theme
            .insert_str(config.theme.clone().unwrap_or_default());
        self.history_size
            .insert_str(config.history_size.to_string());
        self.cli_output_format = match config.cli_output_format {
            CliOutputFormatKind::Redis => OutputFormat::Redis,
            CliOutputFormatKind::Ron => OutputFormat::Ron,
        };
        self.console_capacity
            .insert_str(config.console_capacity.to_string());
        self.enable_mouse_capture = config.enable_mouse_capture;
    }
}

impl Renderable for Options {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
        let height = 10;
        let blank_length = (rect.height - height) / 2;
        let area = Layout::vertical([Length(blank_length), Length(height), Length(blank_length)])
            .split(rect)[1];
        let area =
            Layout::horizontal([Percentage(30), Percentage(40), Percentage(30)]).split(area)[1];
        // let area = centered_rect(50, 70, rect);
        frame.render_widget(Clear::default(), area);
        let block = Block::bordered()
            .title("Options(*restart)")
            .border_type(BorderType::Rounded);
        let block_inner_area = block.inner(area);
        let block_inner_area =
            Layout::horizontal([Length(1), Fill(0), Length(1)]).split(block_inner_area)[1];
        let vertical = Layout::vertical([
            Length(1), // fps
            Length(1), // scan_size
            Length(1), // try_format
            Length(1), // theme
            Length(1), // history_size
            Length(1), // console_capacity
            Length(1), // cli_output_format
            Length(1), // enable_mouse_capture
        ]);
        let rc = vertical.split(block_inner_area);
        self.render_fps(frame, rc[0]);
        self.render_scan_size(frame, rc[1]);
        self.render_try_format(frame, rc[2]);
        self.render_theme(frame, rc[3]);
        self.render_history_size(frame, rc[4]);
        self.render_console_capacity(frame, rc[5]);
        self.render_cli_output_format(frame, rc[6]);
        self.render_enable_mouse_capture(frame, rc[7]);
        frame.render_widget(block, area);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("Tab", "Next"));
        elements.push(("BackTab", "Prev"));
        elements.push(("Enter", "Confirm"));
        elements.push(("Esc", "Close"));

        let editing = self.current();

        match editing {
            Editing::Fps | Editing::ScanSize | Editing::HistorySize | Editing::ConsoleCapacity => {
                elements.push(("↑/j", "+10"));
                elements.push(("↓/k", "-10"));
                elements.push(("→/l", "+1"));
                elements.push(("←/h", "-1"));
            }
            _ => {}
        }

        elements
    }
}

impl Listenable for Options {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> anyhow::Result<bool> {
        let editing = self.current();
        match key_event.code {
            KeyCode::Tab => {
                self.next();
                return Ok(true);
            }
            KeyCode::BackTab => {
                self.prev();
                return Ok(true);
            }
            _ => {}
        }
        if key_event.is_c_c() {
            return Ok(false);
        }

        if let KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } = key_event {
            let fps = self.get_fps();
            let scan_size = self.get_scan_size();
            let history_size = self.get_history_size();
            let console_capacity = self.get_console_capacity();
            let try_format = self.try_format;
            let theme = self.get_theme();
            let enable_mouse_capture = self.enable_mouse_capture;
            let current_configuration = Configuration {
                fps,
                scan_size,
                try_format,
                history_size,
                theme,
                cli_output_format: match self.cli_output_format {
                    OutputFormat::Redis => CliOutputFormatKind::Redis,
                    OutputFormat::Ron => CliOutputFormatKind::Ron,
                },
                console_capacity,
                enable_mouse_capture,
            };
            save_configuration(&current_configuration)?;
            publish_event(GlobalEvent::Restart)?;
            return Ok(true);
        }

        let editor = match editing {
            Editing::Fps => Some(&mut self.fps),
            Editing::ScanSize => Some(&mut self.scan_size),
            Editing::Theme => Some(&mut self.scan_size),
            Editing::HistorySize => Some(&mut self.history_size),
            Editing::ConsoleCapacity => Some(&mut self.console_capacity),
            _ => None,
        };
        if let Some(text_area) = editor {
            match key_event {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    return if text_area.is_selecting() {
                        text_area.cancel_selection();
                        Ok(true)
                    } else {
                        Ok(false)
                    };
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {}
                KeyEvent {
                    code: KeyCode::Char('m'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {}
                KeyEvent {
                    code: KeyCode::Char('a'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => text_area.select_all(),
                KeyEvent {
                    code: KeyCode::Char('z'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    text_area.undo();
                }
                KeyEvent {
                    code: KeyCode::Char('y'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    text_area.redo();
                }
                input => match editing {
                    Editing::Fps
                    | Editing::ScanSize
                    | Editing::HistorySize
                    | Editing::ConsoleCapacity => {
                        if input.code == KeyCode::Backspace {
                            text_area.input(input);
                        } else {
                            let first_line = text_area.lines().get(0).clone().cloned();
                            if let Some(first_line) = first_line {
                                if let KeyCode::Char(c) = input.code {
                                    if c.is_numeric() {
                                        text_area.input(input);
                                        let first_line = text_area.lines().get(0).clone().cloned();
                                        if let Some(first_line) = first_line {
                                            if let Ok(num) = first_line.parse::<u64>() {
                                                let new_value = cmp::min(num, u64::MAX).to_string();
                                                clean_text_area(text_area);
                                                text_area.insert_str(new_value);
                                            }
                                        }
                                    }
                                } else {
                                    let num = first_line.parse::<u64>().unwrap_or(0);
                                    let new_value = match input.code {
                                        KeyCode::Char('h') | KeyCode::Left => {
                                            num.saturating_sub(1).to_string()
                                        }
                                        KeyCode::Char('l') | KeyCode::Right => {
                                            num.saturating_add(1).to_string()
                                        }
                                        KeyCode::Char('j') | KeyCode::Down => {
                                            num.saturating_sub(10).to_string()
                                        }
                                        KeyCode::Char('k') | KeyCode::Up => {
                                            num.saturating_add(10).to_string()
                                        }
                                        _ => first_line,
                                    };
                                    clean_text_area(text_area);
                                    text_area.insert_str(new_value);
                                }
                            }
                        }
                    }
                    _ => {
                        text_area.input(input);
                    }
                },
            }
            Ok(true)
        } else {
            match key_event.code {
                KeyCode::Esc => {
                    return Ok(false);
                }
                KeyCode::Char('h') | KeyCode::Left => match editing {
                    Editing::TryFormat => self.try_format = !self.try_format,
                    Editing::CliOutputFormat => match self.cli_output_format {
                        OutputFormat::Redis => self.cli_output_format = OutputFormat::Ron,
                        OutputFormat::Ron => self.cli_output_format = OutputFormat::Redis,
                    },
                    Editing::EnableMouseCapture => {
                        self.enable_mouse_capture = !self.enable_mouse_capture
                    }
                    _ => {}
                },
                KeyCode::Char('l') | KeyCode::Right => match editing {
                    Editing::TryFormat => self.try_format = !self.try_format,
                    Editing::CliOutputFormat => match self.cli_output_format {
                        OutputFormat::Redis => self.cli_output_format = OutputFormat::Ron,
                        OutputFormat::Ron => self.cli_output_format = OutputFormat::Redis,
                    },
                    Editing::EnableMouseCapture => {
                        self.enable_mouse_capture = !self.enable_mouse_capture
                    }
                    _ => {}
                },
                _ => {}
            }
            Ok(true)
        }
    }

}
