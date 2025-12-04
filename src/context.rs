use std::sync::Arc;
use crate::app::{centered_rect, AppEvent, Listenable, Renderable, TabImplementation};
use crate::components::cmd_viewer::CmdViewer;
use crate::components::fps::FpsCalculator;
use crate::components::servers::ServerList;
use crate::tabs::cli::CliTab;
use crate::tabs::explorer::ExplorerTab;
use crate::tabs::logger::LoggerTab;
use anyhow::{anyhow, Result};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Constraint::{Fill, Length, Max, Min};
use ratatui::layout::{Alignment, Layout, Rect};
use ratatui::prelude::{Color, Span, Style, Stylize, Text};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Tabs, WidgetRef, Wrap};
use ratatui::{symbols, Frame};
use ratisui_core::bus::{publish_msg, GlobalEvent, Kind, Message};
use ratisui_core::configuration::{load_database_configuration, load_theme_configuration, Configuration, Databases};
use ratisui_core::marcos::KeyAsserter;
use ratisui_core::redis_opt::{redis_operations, switch_client};
use ratisui_core::theme::get_color;
use ratisui_core::utils::{clear_frame, none_match};
use std::time::Instant;
use log::{info, warn};
use strum::{EnumCount, EnumIter, IntoEnumIterator};
use ratisui_core::cli::AppArguments;
use ratisui_core::mouse::MouseEventHelper;
use ratisui_core::theme;
use crate::components::app_configuration_editor::Options;

pub struct Context {
    show_server_switcher: bool,
    show_app_options: bool,
    current_tab: CurrentTab,
    current_tab_index: usize,
    explorer_tab: ExplorerTab,
    cli_tab: CliTab,
    logger_tab: LoggerTab,
    server_list: ServerList,
    app_options: Options,
    title: String,
    show_cmd_viewer: bool,
    initial_configuration: Arc<Configuration>,
    pub toast: Option<Message>,
    pub fps_calculator: FpsCalculator,

    tab_area: Rect,
}

#[derive(Eq, PartialEq, EnumCount, EnumIter)]
enum CurrentTab {
    Explorer,
    Cli,
    Logger,
}

impl Context {
    pub fn new() -> Self {
        Self {
            show_server_switcher: false,
            show_app_options: false,
            current_tab: CurrentTab::Explorer,
            current_tab_index: 0,
            explorer_tab: ExplorerTab::new(),
            cli_tab: CliTab::new(),
            logger_tab: LoggerTab::new(),
            // server_list: ServerList::new(&databases),
            server_list: ServerList::new(&Databases::empty()),
            app_options: Options::default(),
            title: "redis ver: ?.?.?".to_string(),
            show_cmd_viewer: false,
            initial_configuration: Arc::new(Configuration::default()),
            toast: None,
            fps_calculator: FpsCalculator::default(),
            tab_area: Rect::default(),
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
        vec![&self.explorer_tab, &self.cli_tab, &self.logger_tab]
    }

    fn next_tab(&mut self) {
        let tmp_index = self.current_tab_index + 1;
        self.current_tab_index = tmp_index % CurrentTab::COUNT;
        self.current_tab = CurrentTab::iter()
            .get(self.current_tab_index)
            .unwrap_or(CurrentTab::Explorer);
    }

    fn prev_tab(&mut self) {
        let tmp_index = self.current_tab_index + (CurrentTab::COUNT - 1);
        self.current_tab_index = tmp_index % CurrentTab::COUNT;
        self.current_tab = CurrentTab::iter()
            .get(self.current_tab_index)
            .unwrap_or(CurrentTab::Explorer);
    }

    fn render_bg(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let block = Block::default().style(Style::default().bg(get_color(|t| &t.context.bg)));
        frame.render_widget(block, area);
        Ok(())
    }

    fn render_title(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        frame.render_widget(self.title.clone().bold(), area);
        Ok(())
    }

    fn render_tabs(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let titles = self
            .get_all_tabs()
            .iter()
            .map(|tab| tab.title())
            .collect::<Vec<_>>();
        let current_tab = self.get_current_tab();
        let highlight_style = Style::default()
            .fg(get_color(|t| &t.tab.title))
            .bg(current_tab.highlight());

        let left_padding = "";
        let right_padding = "";
        let divider = "|";
        let titles_width: usize = titles.iter().map(|x| x.width()).sum();
        let width = left_padding.len() + titles_width + right_padding.len() + (titles.len().saturating_sub(1)) * divider.len();
        self.tab_area = Rect {
            width: width as u16,
            ..area
        };

        let tabs = Tabs::new(titles)
            .highlight_style(highlight_style)
            .select(self.current_tab_index)
            .padding(left_padding, right_padding)
            .divider(divider);
        frame.render_widget(tabs,area);
        Ok(())
    }

    fn render_fps(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let fps = self.fps_calculator.fps.unwrap_or(0.0);
        frame.render_widget(
            Text::from(format!("{fps:.1}"))
                .style(Style::default().fg(get_color(|t| &t.context.fps))),
            area,
        );
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
            CurrentTab::Explorer => self.explorer_tab.render_frame(frame, area),
            CurrentTab::Cli => self.cli_tab.render_frame(frame, area),
            CurrentTab::Logger => self.logger_tab.render_frame(frame, area),
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let mut command_text = Text::default();
        let footers = self.footer_elements();
        for (icon, desc) in footers {
            let command_icon = Span::styled(
                format!(" {} ", icon),
                Style::default()
                    .bold()
                    .fg(Color::default())
                    .bg(get_color(|t| &t.context.key_bg)),
            );
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

    fn render_app_options(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        if self.show_app_options {
            let popup_area = centered_rect(74, 30, area);
            self.app_options.render_frame(frame, popup_area)?;
        }
        Ok(())
    }

    fn render_toast(&mut self, frame: &mut Frame) -> Result<()> {
        if let Some(toast) = &self.toast && toast.expired_at > Instant::now() {
            let rect = frame.area();
            let bg_color = match toast.kind {
                Kind::Error => get_color(|t| &t.toast.error),
                Kind::Warn => get_color(|t| &t.toast.warn),
                Kind::Info => get_color(|t| &t.toast.info),
            };
            let mut text = Text::default();
            text.push_line(Line::raw(format!("  {}", toast.msg.clone())));
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(
                    Block::bordered()
                        .borders(Borders::from_bits_retain(0b1011))
                        .border_set(symbols::border::EMPTY)
                        .title_style(Style::default().bold())
                        .title(toast.title.clone().unwrap_or(toast.kind.to_string())),
                )
                .bg(bg_color);
            let line_count = paragraph.line_count(35);

            let top = Layout::vertical([Length(line_count as u16), Fill(0)]).split(rect)[0];
            let top_right_area = Layout::horizontal([Fill(0), Length(35)]).split(top)[1];

            clear_frame(frame, top_right_area);
            frame.render_widget(paragraph, top_right_area);
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

        let horizontal = Layout::horizontal([Min(0), Length(20), Length(5)]);
        let [tabs_area, title_area, fps_area] = horizontal.areas(header_area);

        self.render_bg(frame, frame.area())?;
        self.render_tabs(frame, tabs_area)?;
        self.render_title(frame, title_area)?;
        self.render_fps(frame, fps_area)?;
        self.render_separator(frame, separator_area)?;

        if self.show_cmd_viewer {
            let inner_vertical = Layout::vertical([Fill(1), Length(12)]);
            let [inner_area, cmd_viewer_area] = inner_vertical.areas(inner_area);
            CmdViewer::new().render_ref(cmd_viewer_area, frame.buffer_mut());
            self.render_selected_tab(frame, inner_area)?;
        } else {
            self.render_selected_tab(frame, inner_area)?;
        }

        self.render_footer(frame, footer_area)?;
        self.render_server_switcher(frame, rect)?;
        self.render_app_options(frame, rect)?;
        self.render_toast(frame)?;

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        if self.show_server_switcher {
            elements.extend(self.server_list.footer_elements());
        }
        if self.show_app_options {
            elements.extend(self.app_options.footer_elements());
        }

        if !self.show_server_switcher && !self.show_app_options {
            elements.extend(match self.current_tab {
                CurrentTab::Explorer => self.explorer_tab.footer_elements(),
                CurrentTab::Cli => self.cli_tab.footer_elements(),
                CurrentTab::Logger => self.logger_tab.footer_elements(),
            });
            elements.push(("s", "Server"));
            elements.push(("o", "Options"));
        }

        elements.push(("^o", "Output"));
        elements.push(("^F5", "Reload"));
        elements.push(("^c", "Quit"));
        elements.push(("^h", "Help"));
        elements
    }

    fn handle_data(&mut self) -> Result<bool> {
        let mut needed = false;
        if let Some(ref toast) = self.toast
            && toast.expired_at < Instant::now()
        {
            self.toast = None;
            needed = true;
        }
        let current_tab = self.get_current_tab_as_mut();
        let current_tab_needed = current_tab.handle_data()?;
        Ok(needed || current_tab_needed)
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

        if self.show_app_options {
            if self.app_options.handle_key_event(key_event)? {
                return Ok(true);
            }
            if none_match(&key_event, KeyCode::Esc) {
                self.show_app_options = false;
                return Ok(true);
            }
        }

        if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::Char('h') {
            let _ = publish_msg(Message::warning("Not yet implemented."));
            return Ok(true);
        }

        if key_event.is_c_o() {
            self.show_cmd_viewer = !self.show_cmd_viewer;
            return Ok(true);
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
        if key_event.is_n_s() {
            self.show_server_switcher = true;
            return Ok(true);
        }
        if key_event.is_n_o() {
            self.show_app_options = true;
            self.app_options = Options::default();
            self.app_options.init_values(Arc::clone(&self.initial_configuration));
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        if self.show_server_switcher {
            if !self.server_list.handle_mouse_event(mouse_event)? && mouse_event.is_left_up() {
                self.show_server_switcher = false;
            }
            return Ok(true);
        }
        if self.show_app_options {
            if !self.app_options.handle_mouse_event(mouse_event)? && mouse_event.is_left_up() {
                self.show_app_options = false;
            }
            return Ok(true);
        }
        let current_tab = self.get_current_tab_as_mut();
        if current_tab.handle_mouse_event(mouse_event)? {
            return Ok(true);
        }
        if mouse_event.is_left_up() {
            if self.tab_area.contains(mouse_event.as_position()) {
                let tabs = self.get_all_tabs();
                let column = mouse_event.column as usize;
                let mut start_pos = 0;

                for (i, tab) in tabs.iter().enumerate() {
                    let end_pos = start_pos + tab.title().width();
                    if start_pos <= column && column < end_pos {
                        if let Some(t) = CurrentTab::iter().get(i) {
                            self.current_tab_index = i;
                            self.current_tab = t;
                        };
                        break;
                    }
                    // separator width 1
                    start_pos = end_pos + 1;
                }
                return Ok(true);
            }
        };

        Ok(false)
    }

    fn on_app_event(&mut self, app_event: AppEvent) -> Result<()> {
        match app_event.clone() {
            AppEvent::InitConfig(app_config, arguments) => {
                self.initial_configuration = Arc::clone(&app_config);
                apply_theme(&arguments, &app_config)?;
                let db_config = if arguments.once {
                    Databases::empty()
                } else {
                    load_database_configuration()?
                };
                self.server_list = ServerList::new(&db_config);
                apply_db(&arguments, &db_config)?;
            }
            AppEvent::Bus(global_event) => match global_event {
                GlobalEvent::ClientChanged => {
                    let v = redis_operations()
                        .and_then(|opt| opt.get_server_info("redis_version"))
                        .unwrap_or("?.?.?".to_string());
                    self.title = format!("redis ver: {v}");
                }
                _ => {}
            },
            _ => {}
        }
        self.explorer_tab.on_app_event(app_event.clone())?;
        self.cli_tab.on_app_event(app_event.clone())?;
        self.logger_tab.on_app_event(app_event.clone())?;
        self.server_list.on_app_event(app_event.clone())?;
        self.app_options.on_app_event(app_event.clone())?;
        Ok(())
    }
}

fn apply_theme(app_arguments: &AppArguments, app_config: &Configuration) -> Result<()> {
    let theme_name = app_arguments
        .theme
        .clone()
        .or_else(|| app_config.theme.clone());
    let theme = load_theme_configuration(theme_name)?;
    theme::set_theme(theme);
    Ok(())
}

fn apply_db(app_arguments: &AppArguments, db_config: &Databases) -> Result<()> {
    let default_db = app_arguments
        .target
        .clone()
        .or_else(|| db_config.default_database.clone());

    if let Some(db) = default_db {
        if let Some(database) = db_config.databases.get(&db) {
            let database_clone = database.clone();
            tokio::spawn(async move {
                match switch_client(db.clone(), &database_clone) {
                    Ok(_) => {
                        info!("Successfully connected to default database '{db}'");
                        info!("{database_clone}");
                    }
                    Err(_) => {
                        warn!("Failed to connect to default database.");
                    }
                };
            });
        } else {
            Err(anyhow!("Unknown database '{db}'."))?;
        }
    };
    Ok(())
}
