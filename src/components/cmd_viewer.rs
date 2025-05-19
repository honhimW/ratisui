use log::LevelFilter;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Style;
use ratatui::widgets::{Block, Widget, WidgetRef};
use ratisui_core::theme::get_color;
use tui_logger::{TuiLoggerWidget, TuiWidgetState};

pub struct CmdViewer {
    state: TuiWidgetState,
}

impl CmdViewer {
    pub fn new() -> Self {
        let mut state = TuiWidgetState::new().set_default_display_level(LevelFilter::Off);
        state = state.set_level_for_target("cmd", LevelFilter::Info);
        Self { state }
    }
}

impl Widget for CmdViewer {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        self.render_ref(area, buf);
    }
}

impl WidgetRef for CmdViewer {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let widget = TuiLoggerWidget::default()
            .style_error(Style::default().fg(get_color(|t| &t.tab.logger.level.error)))
            .style_warn(Style::default().fg(get_color(|t| &t.tab.logger.level.warn)))
            .style_info(Style::default().fg(get_color(|t| &t.tab.logger.level.info)))
            .style_debug(Style::default().fg(get_color(|t| &t.tab.logger.level.debug)))
            .style_trace(Style::default().fg(get_color(|t| &t.tab.logger.level.trace)))
            .output_separator(' ')
            .output_timestamp(None)
            .output_level(None)
            .output_target(false)
            .output_file(false)
            .output_line(false)
            .state(&self.state);
        let block = Block::bordered().title("Cmd Output");
        let inner = block.inner(area);
        block.render(area, buf);
        widget.render(inner, buf);
    }
}
