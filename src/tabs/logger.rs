use crate::app::{AppEvent, Listenable, Renderable, TabImplementation};
use log::LevelFilter;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Stylize};
use ratatui::style::{Color, Style};
use ratatui::Frame;
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiWidgetEvent, TuiWidgetState};
use crate::theme::get_color;

pub struct LoggerTab {
    state: TuiWidgetState,
}

impl LoggerTab {
    pub fn new() -> Self {
        let mut state = TuiWidgetState::new().set_default_display_level(LevelFilter::Info);
        state = state.set_level_for_target("ratisui::tabs::logger", LevelFilter::Off);
        Self {
            state,
        }
    }
}

impl TabImplementation for LoggerTab {
    fn highlight(&self) -> Color {
        get_color(|t| &t.tab.logger.highlight)
    }

    fn title(&self) -> Line<'static> {
        "   Logger   "
            .fg(get_color(|t| &t.tab.title))
            .bg(get_color(|t| &t.tab.logger.accent))
            .into()
    }
}

impl Renderable for LoggerTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        let widget = TuiLoggerSmartWidget::default()
            .highlight_style(Style::default().bg(get_color(|t| &t.tab.logger.highlight)))
            .style_show(Style::default().bg(get_color(|t| &t.tab.logger.highlight)))
            .style_error(Style::default().fg(get_color(|t| &t.tab.logger.level.error)))
            .style_warn(Style::default().fg(get_color(|t| &t.tab.logger.level.warn)))
            .style_info(Style::default().fg(get_color(|t| &t.tab.logger.level.info)))
            .style_debug(Style::default().fg(get_color(|t| &t.tab.logger.level.debug)))
            .style_trace(Style::default().fg(get_color(|t| &t.tab.logger.level.trace)))
            .title_target("Topic")
            .title_log("Log")
            .output_separator(' ')
            .output_timestamp(Some("%Y-%m-%dT%H:%M:%S%.3fZ".to_string()))
            .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(true)
            .output_file(true)
            .output_line(true)
            .state(&self.state)
            ;
        frame.render_widget(widget, rect);

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        elements.push(("←/h", "Level Up"));
        elements.push(("→/l", "Level Down"));
        elements.push(("f", "Focus"));
        elements.push(("v", "Hide"));
        elements
    }
}

impl Listenable for LoggerTab {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> anyhow::Result<bool> {
        if _key_event.modifiers == KeyModifiers::NONE && _key_event.kind == KeyEventKind::Press {
            match _key_event.code {
                KeyCode::Char('k') | KeyCode::Up => self.state.transition(TuiWidgetEvent::UpKey),
                KeyCode::Char('j') | KeyCode::Down => self.state.transition(TuiWidgetEvent::DownKey),
                KeyCode::Char('h') | KeyCode::Left => self.state.transition(TuiWidgetEvent::LeftKey),
                KeyCode::Char('l') | KeyCode::Right => self.state.transition(TuiWidgetEvent::RightKey),
                KeyCode::PageUp => self.state.transition(TuiWidgetEvent::PrevPageKey),
                KeyCode::PageDown => self.state.transition(TuiWidgetEvent::NextPageKey),
                KeyCode::Char('f') => self.state.transition(TuiWidgetEvent::FocusKey),
                KeyCode::Char('v') => self.state.transition(TuiWidgetEvent::HideKey),
                _ => {
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        Ok(false)
    }

    fn on_app_event(&mut self, _app_event: AppEvent) -> anyhow::Result<()> {
        match _app_event {
            AppEvent::Init => {
                // error!("Error Message");
                // warn!("Warning Message");
                // info!("Info Message");
                // debug!("Debug Message");
                // trace!("Trace Message");
            }
            _ => {}
        }
        Ok(())
    }
}