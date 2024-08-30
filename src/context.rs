use crate::app::{Listenable, Renderable, TabImplementation};
use crate::tabs::explorer::ExplorerTab;
use crate::tabs::profiler::ProfilerTab;
use anyhow::Result;
use async_trait::async_trait;
use ratatui::crossterm::event;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint::{Length, Min};
use ratatui::layout::{Alignment, Layout, Rect};
use ratatui::prelude::{Color, Span, Style, Stylize, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::{symbols, Frame};
use ratatui::style::palette::tailwind;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

pub struct Context {
    current_tab: CurrentTab,
    current_tab_index: usize,
    explorer_tab: ExplorerTab,
    profiler_tab: ProfilerTab,
}

#[derive(Eq, PartialEq, EnumCount, EnumIter)]
enum CurrentTab {
    Explorer,
    Profiler,
}

impl Context {
    pub fn new() -> Self {
        Self {
            current_tab: CurrentTab::Explorer,
            current_tab_index: 0,
            explorer_tab: ExplorerTab::new(),
            profiler_tab: ProfilerTab::default(),
        }
    }

    pub fn start(&self) -> Result<()> {
        Ok(())
    }

    pub fn get_current_tab(&self) -> &dyn TabImplementation {
        match self.current_tab {
            CurrentTab::Explorer => &self.explorer_tab,
            CurrentTab::Profiler => &self.profiler_tab,
        }
    }

    pub fn get_current_tab_as_mut(&mut self) -> &mut dyn TabImplementation {
        match self.current_tab {
            CurrentTab::Explorer => &mut self.explorer_tab,
            CurrentTab::Profiler => &mut self.profiler_tab,
        }
    }

    pub fn get_all_tabs(&self) -> Vec<&dyn TabImplementation> {
        vec![
            &self.explorer_tab,
            &self.profiler_tab,
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

    fn render_title(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        frame.render_widget("Redis TUI".bold(), area);
        Ok(())
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let titles = self.get_all_tabs().iter().map(|tab| tab.title()).collect::<Vec<_>>();
        let current_tab = self.get_current_tab();
        let highlight_style = (Color::default(), current_tab.palette().c700);
        frame.render_widget(Tabs::new(titles)
                                .highlight_style(highlight_style)
                                .select(self.current_tab_index)
                                .padding("", "")
                                .divider("|"), area);

        Ok(())
    }

    fn render_separator(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let current_tab = self.get_current_tab();
        let block = Block::default()
            .borders(Borders::TOP)
            .border_set(symbols::border::PROPORTIONAL_TALL)
            .border_style(current_tab.palette().c700);
        frame.render_widget(block, area);

        Ok(())
    }

    fn render_selected_tab(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        match self.current_tab {
            CurrentTab::Explorer => {
                self.explorer_tab.render_frame(frame, area)
            }
            CurrentTab::Profiler => {
                self.profiler_tab.render_frame(frame, area)
            }
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let mut command_text = Text::default();
        let footers = self.footer_elements();
        for (icon, desc) in footers {
            let command_icon = Span::styled(format!(" {} ", icon), Style::default().bold().fg(Color::default()).bg(tailwind::YELLOW.c700));
            let command_desc = Span::styled(format!(" {} ", desc), Style::default().bold());
            command_text.push_span(command_icon);
            command_text.push_span(command_desc);
        }
        let paragraph = Paragraph::new(command_text).alignment(Alignment::Right);
        frame.render_widget(paragraph, area);
        Ok(())
    }
}

impl Renderable for Context {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()>
    where
        Self: Sized,
    {
        let vertical = Layout::vertical([Length(1), Length(1), Min(0), Length(1)]);
        let [header_area, separator_area, inner_area, footer_area] = vertical.areas(rect);

        let horizontal = Layout::horizontal([Min(0), Length(20)]);
        let [tabs_area, title_area] = horizontal.areas(header_area);

        self.render_title(frame, title_area)?;
        self.render_tabs(frame, tabs_area)?;
        self.render_separator(frame, separator_area)?;
        self.render_selected_tab(frame, inner_area)?;
        self.render_footer(frame, footer_area)?;

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut footer_elements = match self.current_tab {
            CurrentTab::Explorer => {
                self.explorer_tab.footer_elements()
            }
            CurrentTab::Profiler => {
                self.profiler_tab.footer_elements()
            }
        };
        footer_elements.push(("^c", "Quit"));
        footer_elements.push(("^h", "Help"));
        footer_elements
    }
}

#[async_trait]
impl Listenable for Context {

    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        let mut current_tab = self.get_current_tab_as_mut();
        if current_tab.handle_key_event(key_event).await? {
            return Ok(true)
        }

        match key_event.code {
            event::KeyCode::Tab => self.next_tab(),
            event::KeyCode::BackTab => self.prev_tab(),
            _ => {}
        }
        Ok(true)
    }
}