use std::{env, fs, io};
use std::fs::File;
use std::io::BufRead;
use std::time::Duration;
use anyhow::Result;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::Position;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;

fn main() -> Result<()> {
    let mut terminal = ratatui::init();

    let mut paragraph = if let Some(path) = env::args().nth(1) {
        let file = fs::File::open(path)?;
        let lines: Vec<String> = io::BufReader::new(file)
            .lines()
            .collect::<io::Result<_>>()?;
        let mut text = Text::default();
        text.extend(lines);
        Paragraph::new(text)
    } else {
        Paragraph::default()
    };

    let mut position = Position::default();
    loop {
        if event::poll(Duration::from_millis(20))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Up => {
                            position.y = position.y.saturating_sub(3);
                            paragraph = paragraph.scroll((position.y, position.x))
                        }
                        KeyCode::Down => {
                            position.y = position.y.saturating_add(3);
                            paragraph = paragraph.scroll((position.y, position.x))
                        }
                        KeyCode::Char('q') => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        terminal.draw(|frame: &mut Frame| {
            frame.render_widget(&paragraph, frame.area());
        })?;
    }
    Ok(())
}