use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::TextArea;

pub fn none_match(key_event: &KeyEvent, key_code: KeyCode) -> bool {
    none_modifier(key_event) && key_event.code == key_code
}

pub fn ctrl_match(key_event: &KeyEvent, key_code: KeyCode) -> bool {
    ctrl_modifier(key_event) && key_event.code == key_code
}

pub fn none_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::NONE
}

pub fn ctrl_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::CONTROL
}

pub fn alt_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::ALT
}

pub fn shift_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::SHIFT
}

/// Clean the text area: select all and backspace
pub fn clean_text_area(text_area: &mut TextArea) {
    text_area.select_all();
    text_area.input(tui_textarea::Input {
        key: tui_textarea::Key::Backspace,
        ..tui_textarea::Input::default()
    });
}

pub fn is_clean_text_area(text_area: &TextArea) -> bool {
    let lines = text_area.lines();
    if lines.len() == 1 {
        if let Some(first_line) = lines.get(0) {
            return first_line.is_empty();
        }
    }
    false
}

pub fn bytes_to_string(bytes: Vec<u8>) -> anyhow::Result<String> {
    if let Ok(string) = String::from_utf8(bytes.clone()) {
        Ok(string)
    } else {
        Ok(bytes.iter().map(|&b| {
            if b.is_ascii() {
                (b as char).to_string()
            } else {
                format!("\\x{:02x}", b)
            }
        }).collect::<String>())
    }
}

pub fn escape_string(s: impl Into<String>) -> String {
    let s = s.into();
    s
        .replace("\\", "\\\\")
        .replace("\t", "\\t")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
}

pub fn split_args(cmd: impl Into<String>) -> Vec<String> {
    let cmd = cmd.into();

    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';

    for c in cmd.chars() {
        if in_quotes {
            if c == quote_char {
                in_quotes = false;
                parts.push(current.clone());
                current.clear();
            } else {
                current.push(c);
            }
        } else {
            if c.is_whitespace() {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            } else if c == '\'' || c == '"' {
                in_quotes = true;
                quote_char = c;
            } else {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }
    parts
}