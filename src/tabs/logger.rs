use async_trait::async_trait;
use crate::app::{Listenable, Renderable, TabImplementation};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::{symbols, Frame};
use ratatui::widgets::{Block, Padding, Paragraph, Widget};

#[derive(Default, Clone, Copy)]
pub struct LoggerTab {

}

impl TabImplementation for LoggerTab {
    fn palette(&self) -> tailwind::Palette {
        tailwind::AMBER
    }

    fn title(&self) -> Line<'static> {
        "  Logger  ".to_string()
            .fg(tailwind::SLATE.c200)
            .bg(self.palette().c900)
            .into()
    }
}

impl Renderable for LoggerTab {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        let paragraph = Paragraph::new("This is the tui logger tab")
            .block(Block::bordered()
                .border_set(symbols::border::PROPORTIONAL_TALL)
                .padding(Padding::horizontal(1))
                .border_style(self.palette().c700));
        frame.render_widget(paragraph, rect);

        Ok(())
    }
}

#[async_trait]
impl Listenable for LoggerTab {

}