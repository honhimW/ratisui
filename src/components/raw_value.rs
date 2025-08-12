use std::borrow::Cow;
use ratatui::prelude::Text;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratisui_core::theme::get_color;
use ratisui_core::utils::ContentType;
use ratisui_core::highlight_value::{HighlightKind, HighlightProcessor, HighlightText};

pub fn raw_value_to_highlight_text(value: Cow<str>, format: bool) -> (Text, Option<ContentType>) {
    raw_value_to_highlight_text_with_content_type(value, None, format)
}

pub fn raw_value_to_highlight_text_with_content_type(value: Cow<str>, content_type: Option<ContentType>, format: bool) -> (Text, Option<ContentType>) {
    let mut processor = HighlightProcessor::new(value.to_string(), content_type);
    if !format {
        processor.disable_formatting();
    }
    let result = processor.process();
    let fragments = match result {
        Ok(_) => processor.get_fragments().clone(),
        Err(_) => vec![HighlightText {
            text: value.to_string(),
            kind: HighlightKind::String,
        }],
    };
    let mut text = Text::default();
    for highlight_text in fragments {
        let fragment = highlight_text.text.clone();
        let style= match highlight_text.kind {
            HighlightKind::String => Style::default().fg(get_color(|t| &t.raw.string)),
            HighlightKind::Boolean => Style::default().fg(get_color(|t| &t.raw.boolean)),
            HighlightKind::Keyword => Style::default().fg(get_color(|t| &t.raw.keyword)),
            HighlightKind::Constant => Style::default().fg(get_color(|t| &t.raw.constant)),
            HighlightKind::Null => Style::default().fg(get_color(|t| &t.raw.null)),
            HighlightKind::Property => Style::default().fg(get_color(|t| &t.raw.property)),
            HighlightKind::Comment => Style::default().fg(get_color(|t| &t.raw.comment)),
            HighlightKind::Number => Style::default().fg(get_color(|t| &t.raw.number)),
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

    (text, processor.get_content_type())
}
