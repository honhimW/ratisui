use crate::app::{Listenable, Renderable, TabImplementation};
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::{symbols, Frame};

#[derive(Default, Clone, Copy)]
pub struct ProfilerTab {

}

impl TabImplementation for ProfilerTab {
    fn palette(&self) -> tailwind::Palette {
        tailwind::GREEN
    }

    fn title(&self) -> Line<'static> {
        "  Profiler  ".to_string()
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }
}

impl Renderable for ProfilerTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        let paragraph = Paragraph::new("This is the profiler tab")
            .block(Block::bordered()
                .border_set(symbols::border::PROPORTIONAL_TALL)
                .padding(Padding::horizontal(1))
                .border_style(self.palette().c700));
        frame.render_widget(paragraph, rect);

        Ok(())
    }
}

impl Listenable for ProfilerTab {

}