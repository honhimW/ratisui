use std::borrow::Cow;
use ratatui::prelude::Text;
use ratatui::style::palette::tailwind;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use crate::utils::ContentType;
use crate::components::highlight_value::{HighlightKind, HighlightProcessor, HighlightText};

pub fn raw_value_to_highlight_text(value: Cow<str>, format: bool) -> Text {
    raw_value_to_highlight_text_with_content_type(value, None, format)
}

pub fn raw_value_to_highlight_text_with_content_type(value: Cow<str>, content_type: Option<ContentType>, format: bool) -> Text {
    let mut processor = HighlightProcessor::new(value.to_string(), content_type);
    if !format {
        processor.disable_formatting();
    }
    let result = processor.process();
    let fragments = match result {
        Ok(_) => { processor.get_fragments().clone() }
        Err(_) => {
            vec![HighlightText {
                text: value.to_string(),
                kind: HighlightKind::String,
            }]
        }
    };
    let mut text = Text::default();
    for highlight_text in fragments {
        let fragment = highlight_text.text.clone();
        let style= match highlight_text.kind {
            HighlightKind::String => Style::default().fg(tailwind::AMBER.c400),
            HighlightKind::Boolean |
            HighlightKind::Keyword |
            HighlightKind::Constant |
            HighlightKind::Null => Style::default().fg(tailwind::ROSE.c600),
            HighlightKind::Property => Style::default().fg(tailwind::FUCHSIA.c700),
            HighlightKind::Comment => Style::default().fg(tailwind::CYAN.c500),
            HighlightKind::Number => Style::default().fg(tailwind::BLUE.c600),
            _ => Style::default(),
        };

        let lines: Vec<&str> = fragment.lines().collect();
        if lines.len() > 1 {
            for (i, &l) in lines.iter().enumerate() {
                if i == 0 {
                    let span = Span::styled(l.to_string(), style);
                    text.push_span(span);
                } else {
                    let line = Line::styled(l.to_string(), style);
                    text.push_line(line);
                }
            }
        } else {
            for l in lines {
                let span = Span::styled(l.to_string(), style);
                text.push_span(span);
            }
        }
    }
    text
}
