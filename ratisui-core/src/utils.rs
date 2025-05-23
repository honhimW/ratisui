use anyhow::anyhow;
use base64::Engine;
use jaded::Parser;
use protobuf::reflect::MessageDescriptor;
use protobuf::well_known_types::any::Any;
use protobuf::UnknownValueRef;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ron::ser::PrettyConfig;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use strum::Display;
use tui_textarea::TextArea;

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

    let des_utf8 = String::from_utf8(bytes.clone());
    if let Ok(string) = des_utf8 {
        return Ok((string, None));
    }
    let des_result = des_protobuf(bytes.clone());
    if des_result.is_ok() {
        let string = des_result?;
        return Ok((string, Some(ContentType::Ron)));
    }
    Ok((
        bytes
            .iter()
            .map(|&b| {
                if b.is_ascii() {
                    (b as char).to_string()
                } else {
                    format!("\\x{:02x}", b)
                }
            })
            .collect::<String>(),
        None,
    ))
}

pub fn bytes_to_string(bytes: Vec<u8>) -> anyhow::Result<String> {
    if bytes.is_empty() {
        return Ok("".to_string());
    }
    let des_result = des_java(bytes.clone());
    if des_result.is_ok() {
        return des_result;
    }

    if let Ok(string) = String::from_utf8(bytes.clone()) {
        return Ok(string);
    }

    let des_result = des_protobuf(bytes.clone());
    if des_result.is_ok() {
        return des_result;
    }

    Ok(bytes
        .iter()
        .map(|&b| {
            if b.is_ascii() {
                (b as char).to_string()
            } else {
                format!("\\x{:02x}", b)
            }
        })
        .collect::<String>())
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
    let any_message: Box<Any> = dynamic_message.downcast_box().map_err(|e| anyhow!(e))?;

    let mut hash_map: BTreeMap<u32, Field> = BTreeMap::new();

    if !any_message.type_url.is_empty() {
        hash_map.insert(1, Field::LengthDelimited(any_message.type_url.clone()));
    }
    if !any_message.value.is_empty() {
        hash_map.insert(2, Field::LengthDelimited(String::from_utf8(any_message.value.clone().to_vec())?));
    }

    let fields = any_message.special_fields;
    let unknown_fields = fields.unknown_fields();

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

#[derive(Serialize)]
enum Field {
    Fixed32(u32),
    Fixed64(u64),
    Varint(u64),
    LengthDelimited(String),
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
            } else if c == '\'' || c == '"' || c == '`' {
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

pub fn right_pad(s: &str, size: usize, pad_str: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let pad_str = if pad_str.is_empty() { " " } else { pad_str };
    let pad_len = pad_str.chars().count();
    let str_len = s.chars().count();
    let pads = size as isize - str_len as isize;

    if pads <= 0 {
        s.to_string()
    } else if pad_len == 1 && pads <= 8192 {
        format!("{}{}", s, pad_str.repeat(pads as usize))
    } else if pads == pad_len as isize {
        format!("{}{}", s, pad_str)
    } else if pads < pad_len as isize {
        format!("{}{}", s, &pad_str[..pads as usize])
    } else {
        let mut padding = String::with_capacity(pads as usize);
        let pad_chars: Vec<char> = pad_str.chars().collect();
        for i in 0..pads as usize {
            padding.push(pad_chars[i % pad_len]);
        }
        format!("{}{}", s, padding)
    }
}

pub fn compare_version_strings(s1: impl Into<String>, s2: impl Into<String>) -> std::cmp::Ordering {
    let s1 = s1.into();
    let s2 = s2.into();
    let parts1: Vec<&str> = s1.split('.').collect();
    let parts2: Vec<&str> = s2.split('.').collect();

    let len1 = parts1.len();
    let len2 = parts2.len();

    let max_len = usize::max(len1, len2);

    for i in 0..max_len {
        let num_str1 = if i < len1 { parts1[i] } else { "0" };
        let num_str2 = if i < len2 { parts2[i] } else { "0" };

        let num1 = num_str1.parse::<u32>().unwrap_or(0);
        let num2 = num_str2.parse::<u32>().unwrap_or(0);

        match num1.cmp(&num2) {
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Greater,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Less,
            std::cmp::Ordering::Equal => continue,
        }
    }

    std::cmp::Ordering::Equal
}

pub fn try_decode_arg(arg: &String) -> anyhow::Result<Vec<u8>> {
    let input = arg.clone();
    // Base64#Zm9vIGJhcg==#
    if let Some(start) = input.find('#') {
        if let Some(end) = input.rfind('#') {
            if start != end {
                let prefix = &input[..start];
                let content = &input[start + 1..end];

                let result = match prefix {
                    "base64" => base64::prelude::BASE64_STANDARD.decode(content)?,
                    "hex" => hex::decode(content)?,
                    "fs" => fs::read(Path::new(content))?,
                    _ => input.as_bytes().to_vec(),
                };
                return Ok(result);
            }
        }
    }
    Ok(input.as_bytes().to_vec())
}

#[cfg(test)]
mod test {
    use crate::utils::{compare_version_strings, right_pad, try_decode_arg};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::cmp::Ordering;

    macro_rules! ctrl {
    ($name:ident) => {{
        let chars = stringify!($name);
        if let Some(c) = chars.chars().next() {
            KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
        } else {
            KeyEvent::new(KeyCode::Null, KeyModifiers::CONTROL)
        }
    }};
}
    #[test]
    fn test_ctrl_macro() {
        assert!(matches!(ctrl!(a), KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL,.. }));
    }

    #[test]
    fn test_right_pad() {
        assert_eq!(right_pad("a", 1, " "), "a");
        assert_eq!(right_pad("a", 2, " "), "a ");
        assert_eq!(right_pad("a", 3, " "), "a  ");
    }

    #[test]
    fn test_compare_version() {
        assert_eq!(compare_version_strings("8.0.1", "8.0.1"), Ordering::Equal);
        assert_eq!(compare_version_strings("7.4.2", "8.0.1"), Ordering::Less);
        assert_eq!(compare_version_strings("8.1.1", "8.0.1"), Ordering::Greater);
        assert_eq!(compare_version_strings("18.1.1", "8.0.1"), Ordering::Greater);
        assert_eq!(compare_version_strings("8.0.2", "8.0.1"), Ordering::Greater);
        assert_eq!(compare_version_strings("8.1", "8.0.1"), Ordering::Greater);
    }

    #[test]
    fn test_decode() {
        let base64 = "Zm9vIGJhcg==".to_string();
        let arg = try_decode_arg(&format!("base64#{base64}#"));
    }
}
