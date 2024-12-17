use std::collections::HashMap;
use std::io::Cursor;
use protobuf::reflect::MessageDescriptor;
use protobuf::UnknownValueRef;
use protobuf::well_known_types::any::Any;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ron::ser::PrettyConfig;
use serde::Serialize;
use tui_textarea::TextArea;
use jaded::Parser;
use strum::Display;

#[allow(unused)]
#[derive(Default, Clone, Display)]
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
    let des_result = des_java(bytes.clone());
    if des_result.is_ok() {
        let string = des_result?;
        return Ok((string, Some(ContentType::Ron)));
    }

    let des_result = des_protobuf(bytes.clone());
    if des_result.is_ok() {
        let string = des_result?;
        return Ok((string, Some(ContentType::Ron)));
    }

    if let Ok(string) = String::from_utf8(bytes.clone()) {
        Ok((string, None))
    } else {
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
    if bytes.is_empty() {
        return Ok("".to_string());
    }
    let des_result = des_java(bytes.clone());
    if des_result.is_ok() {
        return des_result;
    }

    let des_result = des_protobuf(bytes.clone());
    if des_result.is_ok() {
        return des_result;
    }
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

pub fn des_java(bytes: Vec<u8>) -> anyhow::Result<String> {
    let cursor = Cursor::new(bytes);
    let mut parser = Parser::new(cursor)?;
    let content = parser.read()?;
    let ron = ron::ser::to_string_pretty(&content, PrettyConfig::default())?;
    Ok(ron)
}

pub fn des_protobuf(bytes: Vec<u8>) -> anyhow::Result<String> {
    let descriptor = MessageDescriptor::for_type::<Any>();
    let dynamic_message = descriptor.parse_from_bytes(&bytes)?;
    let any_message: Box<Any> = dynamic_message.downcast_box().expect("");

    let fields = any_message.special_fields;
    let unknown_fields = fields.unknown_fields();
    #[derive(Serialize)]
    enum Field {
        Fixed32(u32),
        Fixed64(u64),
        Varint(u64),
        LengthDelimited(String)
    }

    let mut hash_map: HashMap<u32, Field> = HashMap::new();

    for (idx, unknown_field) in unknown_fields.iter() {
        match unknown_field {
            UnknownValueRef::Fixed32(fixed32) => hash_map.insert(idx, Field::Fixed32(fixed32)),
            UnknownValueRef::Fixed64(fixed64) => hash_map.insert(idx, Field::Fixed64(fixed64)),
            UnknownValueRef::Varint(varint) => hash_map.insert(idx, Field::Varint(varint)),
            UnknownValueRef::LengthDelimited(ld) => hash_map.insert(idx, Field::LengthDelimited(String::from_utf8(ld.to_vec())?)),
        };
    }

    let ron = ron::ser::to_string_pretty(&hash_map, PrettyConfig::default())?;
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