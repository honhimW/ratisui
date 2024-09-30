use std::io::Cursor;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ron::ser::PrettyConfig;
use tui_textarea::TextArea;
use jaded::Parser;

#[allow(unused)]
#[derive(Default, Clone)]
pub enum ContentType {
    String,
    #[default]
    Json,
    Xml,
    Ron,
    // JavaSerialized,
    // PhpSerialized,
    // CSharpSerialized,
    // Protobuf,
}

pub fn none_match(key_event: &KeyEvent, key_code: KeyCode) -> bool {
    none_modifier(key_event) && key_event.code == key_code
}
#[allow(unused)]
pub fn ctrl_match(key_event: &KeyEvent, key_code: KeyCode) -> bool {
    ctrl_modifier(key_event) && key_event.code == key_code
}
#[allow(unused)]
pub fn none_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::NONE
}
#[allow(unused)]
pub fn ctrl_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::CONTROL
}
#[allow(unused)]
pub fn alt_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::ALT
}
#[allow(unused)]
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

pub fn deserialize_bytes(bytes: Vec<u8>) -> anyhow::Result<(String, Option<ContentType>)> {
    if let Ok(string) = String::from_utf8(bytes.clone()) {
        Ok((string, None))
    } else {
        let des_result = des_java(bytes.clone());
        if des_result.is_ok() {
            let string = des_result?;
            return Ok((string, Some(ContentType::Ron)));
        }

        Ok((bytes.iter().map(|&b| {
            if b.is_ascii() {
                (b as char).to_string()
            } else {
                format!("\\x{:02x}", b)
            }
        }).collect::<String>(), None))
    }
}

pub fn bytes_to_string(bytes: Vec<u8>) -> anyhow::Result<String> {
    if let Ok(string) = String::from_utf8(bytes.clone()) {
        Ok(string)
    } else {
        let des_result = des_java(bytes.clone());
        if des_result.is_ok() {
            return des_result;
        }

        Ok(bytes.iter().map(|&b| {
            if b.is_ascii() {
                (b as char).to_string()
            } else {
                format!("\\x{:02x}", b)
            }
        }).collect::<String>())
    }
}

pub fn des_java(bytes: Vec<u8>) -> anyhow::Result<String> {
    let cursor = Cursor::new(bytes);
    let mut parser = Parser::new(cursor)?;
    let content = parser.read()?;
    let ron = ron::ser::to_string_pretty(&content, PrettyConfig::default())?;
    Ok(ron)
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