use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{StatefulWidgetRef, Widget};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Fps {
    display_precision: usize,
    style: Style,
}

impl Default for Fps {
    fn default() -> Self {
        Self {
            display_precision: 1,
            style: Default::default(),
        }
    }
}

impl Fps {
    pub fn styled<S>(precision: usize, style: S) -> Self
    where
        S: Into<Style>,
    {
        Self {
            display_precision: precision,
            style: style.into(),
        }
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

#[derive(Clone)]
pub struct FpsState {
    frame_count: usize,
    last_instant: Instant,
    fps: f32,
}

impl Default for FpsState {
    fn default() -> Self {
        Self {
            frame_count: 0,
            last_instant: Instant::now(),
            fps: 0.0,
        }
    }
}

impl StatefulWidgetRef for Fps {
    type State = FpsState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.frame_count += 1;
        let elapsed = state.last_instant.elapsed();
        if elapsed > Duration::from_secs(1) && state.frame_count > 2 {
            state.fps = state.frame_count as f32 / elapsed.as_secs_f32();
            state.frame_count = 0;
            state.last_instant = Instant::now();
        }
        let text = format!("{:.*}", self.display_precision, state.fps);
        Span::raw(text).render(area, buf);
    }
}