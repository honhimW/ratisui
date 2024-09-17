use crate::app::{Listenable, Renderable, TabImplementation};
use crate::redis_opt::spawn_redis_opt;
use anyhow::{Error, Result};
use log::info;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::layout::{Layout, Rect};
use ratatui::prelude::{Line, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row, Table};
use ratatui::Frame;
use redis::Value;
use tui_textarea::TextArea;

pub struct CliTab {
    mode: Mode,
    input_text_area: TextArea<'static>,
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
        let vertical = Layout::vertical([Length(1), Min(10)]).split(rect);
        self.render_input(frame, vertical[0])?;
        self.render_output(frame, vertical[1])?;
        Ok(())
    }
}

impl Listenable for CliTab {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        match self.mode {
            Mode::Normal => {
                if key_event.kind == KeyEventKind::Press {
                    match key_event {
                        KeyEvent { code: KeyCode::Esc, .. } => {
                            if self.input_text_area.is_selecting() {
                                self.input_text_area.cancel_selection();
                            }
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
                        KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
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
                            }
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
                        input => {
                            self.input_text_area.input(input);
                        }
                    }
                }
            }
        }

        Ok(false)
    }
}

impl CliTab {

    pub fn new() -> Self {
        let mut text_area = TextArea::default();
        text_area.set_cursor_style(Style::default().rapid_blink().reversed());
        text_area.set_cursor_line_style(Style::default());
        Self {
            mode: Mode::Normal,
            input_text_area: text_area,
        }
    }

    fn render_input(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let horizontal = Layout::horizontal([Length(2), Min(10)]).split(rect);
        frame.render_widget(Span::raw("> "), horizontal[0]);
        frame.render_widget(&self.input_text_area, horizontal[1]);
        Ok(())
    }

    fn render_output(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let mut rows = vec![];
        let mut cells = vec![];
        cells.push(Cell::new(Span::raw("")));
        rows.push(Row::new(cells));
        Table::new(rows, [Fill(0)]);
        Ok(())
    }

    fn get_command(&self) -> Option<String> {
        self.input_text_area.lines().get(0).cloned()
    }

}