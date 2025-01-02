use crate::app::{centered_rect, AppEvent, Listenable, Renderable, TabImplementation};
use ratisui_core::bus::{Kind, Message};
use crate::components::servers::ServerList;
use ratisui_core::configuration::Databases;
use crate::tabs::cli::CliTab;
use crate::tabs::explorer::ExplorerTab;
use crate::tabs::logger::LoggerTab;
use ratisui_core::utils::none_match;
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Max, Min};
use ratatui::layout::{Alignment, Layout, Rect};
use ratatui::prelude::{Color, Span, Style, Stylize, Text};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap};
use ratatui::{symbols, Frame};
use std::time::Instant;
use strum::{EnumCount, EnumIter, IntoEnumIterator};
use ratisui_core::theme::get_color;

pub struct Context {
    show_server_switcher: bool,
    current_tab: CurrentTab,
    current_tab_index: usize,
    explorer_tab: ExplorerTab,
    cli_tab: CliTab,
    logger_tab: LoggerTab,
    server_list: ServerList,
    pub toast: Option<Message>,
    pub fps: f32,
}

#[derive(Eq, PartialEq, EnumCount, EnumIter)]
enum CurrentTab {
    Explorer,
    Cli,
    Logger,
}

impl Context {
    pub fn new(databases: Databases) -> Self {
        Self {
            show_server_switcher: false,
            current_tab: CurrentTab::Explorer,
            current_tab_index: 0,
            explorer_tab: ExplorerTab::new(),
            cli_tab: CliTab::new(),
            logger_tab: LoggerTab::new(),
            server_list: ServerList::new(&databases),
            toast: None,
            fps: 0.0,
        }
    }

    pub fn get_current_tab(&self) -> &dyn TabImplementation {
        match self.current_tab {
            CurrentTab::Explorer => &self.explorer_tab,
            CurrentTab::Cli => &self.cli_tab,
            CurrentTab::Logger => &self.logger_tab,
        }
    }

    pub fn get_current_tab_as_mut(&mut self) -> &mut dyn TabImplementation {
        match self.current_tab {
            CurrentTab::Explorer => &mut self.explorer_tab,
            CurrentTab::Cli => &mut self.cli_tab,
            CurrentTab::Logger => &mut self.logger_tab,
        }
    }

    pub fn get_all_tabs(&self) -> Vec<&dyn TabImplementation> {
        vec![
            &self.explorer_tab,
            &self.cli_tab,
            &self.logger_tab,
        ]
    }

    fn next_tab(&mut self) {
        let tmp_index = self.current_tab_index + 1;
        self.current_tab_index = tmp_index % CurrentTab::COUNT;
        self.current_tab = CurrentTab::iter().get(self.current_tab_index).unwrap_or(CurrentTab::Explorer);
    }

    fn prev_tab(&mut self) {
        let tmp_index = self.current_tab_index + (CurrentTab::COUNT - 1);
        self.current_tab_index = tmp_index % CurrentTab::COUNT;
        self.current_tab = CurrentTab::iter().get(self.current_tab_index).unwrap_or(CurrentTab::Explorer);
    }

    fn render_bg(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let block = Block::default().style(Style::default().bg(get_color(|t| &t.context.bg)));
        frame.render_widget(block, area);
        Ok(())
    }

    fn render_title(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        frame.render_widget("Redis TUI".bold(), area);
        Ok(())
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let titles = self.get_all_tabs().iter().map(|tab| tab.title()).collect::<Vec<_>>();
        let current_tab = self.get_current_tab();
        let highlight_style = Style::default().fg(get_color(|t| &t.tab.title))
            .bg(current_tab.highlight());
        frame.render_widget(Tabs::new(titles)
                                .highlight_style(highlight_style)
                                .select(self.current_tab_index)
                                .padding("", "")
                                .divider("|"), area);

        Ok(())
    }

    fn render_fps(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        frame.render_widget(Text::from(format!("{:.1}", self.fps))
                                .style(Style::default().fg(get_color(|t| &t.context.fps))), area);
        Ok(())
    }

    fn render_separator(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let current_tab = self.get_current_tab();
        let block = Block::default()
            .borders(Borders::TOP)
            .border_set(symbols::border::PROPORTIONAL_TALL)
            .border_style(current_tab.highlight());
        frame.render_widget(block, area);

        Ok(())
    }

    fn render_selected_tab(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        match self.current_tab {
            CurrentTab::Explorer => {
                self.explorer_tab.render_frame(frame, area)
            }
            CurrentTab::Cli => {
                self.cli_tab.render_frame(frame, area)
            }
            CurrentTab::Logger => {
                self.logger_tab.render_frame(frame, area)
            }
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let mut command_text = Text::default();
        let footers = self.footer_elements();
        for (icon, desc) in footers {
            let command_icon = Span::styled(format!(" {} ", icon), Style::default().bold().fg(Color::default()).bg(get_color(|t| &t.context.key_bg)));
            let command_desc = Span::styled(format!(" {} ", desc), Style::default().bold());
            command_text.push_span(command_icon);
            command_text.push_span(command_desc);
        }
        let paragraph = Paragraph::new(command_text)
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_server_switcher(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        if self.show_server_switcher {
            let popup_area = centered_rect(74, 30, area);
            self.server_list.render_frame(frame, popup_area)?;
        }
        Ok(())
    }

    fn render_toast(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        if let Some(ref toast) = self.toast {
            frame.render_widget(Clear::default(), area);
            let bg_color = match toast.kind {
                Kind::Error => get_color(|t| &t.toast.error),
                Kind::Warn => get_color(|t| &t.toast.warn),
                Kind::Info => get_color(|t| &t.toast.info),
            };
            let mut text = Text::default();
            text.push_line(Line::raw(format!("  {}", toast.msg.clone())));
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(Block::bordered()
                    .borders(Borders::from_bits_retain(0b1011))
                    .border_set(symbols::border::EMPTY)
                    .title_style(Style::default().bold())
                    .title(toast.title.clone().unwrap_or(toast.kind.to_string())))
                .bg(bg_color);
            frame.render_widget(paragraph, area);
        }
        Ok(())
    }
}

impl Renderable for Context {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()>
    where
        Self: Sized,
    {
        let vertical = Layout::vertical([Length(1), Length(1), Min(0)]);
        let [header_area, separator_area, rest_area] = vertical.areas(rect);

        let vertical = Layout::vertical([Fill(1), Max(1)]);
        let [inner_area, footer_area] = vertical.areas(rest_area);

        let horizontal = Layout::horizontal([Min(0), Length(15), Length(5)]);
        let [tabs_area, title_area, fps_area] = horizontal.areas(header_area);

        self.render_bg(frame, frame.area())?;
        self.render_tabs(frame, tabs_area)?;
        self.render_title(frame, title_area)?;
        self.render_fps(frame, fps_area)?;
        self.render_separator(frame, separator_area)?;
        self.render_selected_tab(frame, inner_area)?;
        self.render_footer(frame, footer_area)?;
        self.render_server_switcher(frame, rect)?;
        if let Some(ref toast) = self.toast {
            if toast.expired_at < Instant::now() {
                self.toast = None;
            } else {
                let top = Layout::vertical([Length(4), Fill(0)]).split(rect)[0];
                let top_right_area = Layout::horizontal([Fill(0), Length(35)]).split(top)[1];
                self.render_toast(frame, top_right_area)?;
            }
        }
        
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements;
        if self.show_server_switcher {
            elements = self.server_list.footer_elements();
        } else {
            elements = match self.current_tab {
                CurrentTab::Explorer => {
                    self.explorer_tab.footer_elements()
                }
                CurrentTab::Cli => {
                    self.cli_tab.footer_elements()
                }
                CurrentTab::Logger => {
                    self.logger_tab.footer_elements()
                }
            };
            elements.push(("s", "Server"));
        }
        elements.push(("^F5", "Reload"));
        elements.push(("^c", "Quit"));
        elements.push(("^h", "Help"));
        elements
    }
}

impl Listenable for Context {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if self.show_server_switcher {
            if self.server_list.handle_key_event(key_event)? {
                return Ok(true);
            }
            if none_match(&key_event, KeyCode::Esc) {
                self.show_server_switcher = false;
                return Ok(true);
            }
        }

        if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::Char('h') {
            // TODO show help
        }

        let current_tab = self.get_current_tab_as_mut();
        if current_tab.handle_key_event(key_event)? {
            return Ok(true);
        }

        match key_event.code {
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            _ => {}
        }
        if none_match(&key_event, KeyCode::Char('s')) {
            self.show_server_switcher = true;
            return Ok(true);
        }
        Ok(false)
    }

    fn on_app_event(&mut self, app_event: AppEvent) -> Result<()> {
        self.explorer_tab.on_app_event(app_event.clone())?;
        self.cli_tab.on_app_event(app_event.clone())?;
        self.logger_tab.on_app_event(app_event.clone())?;
        self.server_list.on_app_event(app_event.clone())?;
        Ok(())
    }
}