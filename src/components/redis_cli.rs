use crate::app::{Listenable, Renderable};
use crate::components::completion::{sort_commands, CompletableTextArea, CompletionItem, CompletionItemKind, Doc, Label, Parameter};
use anyhow::Result;
use once_cell::sync::Lazy;
use ratatui::crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;
use serde_json::Value;
use substring::Substring;

pub struct RedisCli<'a> {
    completable_text_area: CompletableTextArea<'a>,
}

impl RedisCli<'_> {
    pub fn new() -> Self {
        let mut completable_text_area = CompletableTextArea::new();
        completable_text_area.focus();
        Self {
            completable_text_area,
        }
    }
}

impl Renderable for RedisCli<'_> {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        self.completable_text_area.render_frame(frame, rect)?;
        Ok(())
    }
}

impl Listenable for RedisCli<'_> {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<bool> {
        if event.kind == KeyEventKind::Press {
            let accepted = self.completable_text_area.handle_key_event(event)?;
            let (_, cursor_x) = self.completable_text_area.get_cursor();
            let raw_input = self.completable_text_area.get_input();
            let (mut items, segment) = get_items(&raw_input, cursor_x);
            sort_commands(&mut items, &segment);
            self.completable_text_area.update_completion_items(items, segment);
            Ok(accepted)
        } else {
            Ok(false)
        }
    }
}

impl RedisCli<'_> {
    pub fn insert_str(&mut self, s: impl Into<String>) {
        self.completable_text_area.insert_str(s);
    }

    pub fn get_input(&self) -> String {
        self.completable_text_area.get_input()
    }

    pub fn set_auto_suggestion(&mut self, s: impl Into<String>) {
        self.completable_text_area.set_auto_suggestion(s);
    }

    pub fn update_frame(&mut self, frame_height: u16, frame_width: u16) {
        self.completable_text_area.update_frame(frame_height, frame_width);
    }
}

fn get_items(input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
    let args = split_args(input);

    // Find current word
    let mut current_word: Option<(usize, String, Option<char>, usize, usize)> = None;
    let mut segment = String::new();
    for (idx, (arg, quote, start_pos, end_pos)) in args.iter().enumerate() {
        if start_pos <= &cursor_x && &cursor_x <= end_pos {
            current_word = Some((
                idx,
                arg.clone(),
                quote.clone(),
                start_pos.clone(),
                end_pos.clone(),
            ));
            segment = input.substring(*start_pos, cursor_x).to_uppercase();
            break;
        }
    }

    let mut commands = vec![];
    // Find command by first word
    for item in COMMANDS.iter() {
        let mut item_clone = item.clone();
        if let Some((idx, _, _, start_pos, end_pos)) = current_word {
            if idx == 0 {
                if item.label.label.contains(&segment) {
                    item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                    commands.push(item_clone);
                }
            } else {
                if let Some((cmd, _, start_pos, end_pos)) = args.first() {
                    if &item.label.label == &cmd.to_uppercase() {
                        item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                        commands.push(item_clone);
                        break;
                    }
                }
            }
        } else {
            if let Some((cmd, _, start_pos, end_pos)) = args.first() {
                if &item.label.label == &cmd.to_uppercase() {
                    item_clone.range = (start_pos.clone() as isize, end_pos.clone() as isize);
                    commands.push(item.clone());
                    break;
                }
            } else {
                commands.push(item.clone());
            }
        }
    }

    if let Some((idx, _, _, _, _)) = current_word {
        if idx == 0 {
            return (commands, segment);
        }
    }

    if !commands.is_empty() {
        let mut parameters = vec![];
        let (start, end) = if let Some((_, _, _, start_pos, end_pos)) = current_word {
            (start_pos as isize, end_pos as isize)
        } else {
            (0, -1)
        };
        for item in commands.iter() {
            for param in item.parameters.iter() {
                match param {
                    Parameter::Flag(flag, detail) => {
                        if flag.contains(&segment) {
                            parameters.push(
                                CompletionItem::option(flag)
                                    .detail(detail)
                                    .range(start, end),
                            );
                        }
                    }
                    Parameter::Enum(es) => {
                        for (e, detail) in es {
                            if e.contains(&segment) {
                                parameters.push(
                                    CompletionItem::option(e).detail(detail).range(start, end),
                                );
                            }
                        }
                    }
                    Parameter::Arg { key, detail, .. } => {
                        if key.contains(&segment) {
                            parameters
                                .push(CompletionItem::option(key).detail(detail).range(start, end));
                        }
                    }
                    _ => {}
                }
            }
        }
        commands = parameters;
    }

    (commands, segment)
}

fn split_args(cmd: impl Into<String>) -> Vec<(String, Option<char>, usize, usize)> {
    let cmd = cmd.into();

    let mut parts: Vec<(String, Option<char>, usize, usize)> = vec![];
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';

    let mut cursor: usize = 0;
    let mut start: usize = 0;
    for c in cmd.chars() {
        if in_quotes {
            if c == quote_char {
                in_quotes = false;
                parts.push((current.clone(), Some(quote_char), start, cursor));
                current.clear();
            } else {
                current.push(c);
            }
        } else {
            if c.is_whitespace() {
                if !current.is_empty() {
                    parts.push((current.clone(), None, start, cursor));
                    current.clear();
                }
                start = cursor + 1;
            } else if c == '\'' || c == '"' {
                in_quotes = true;
                quote_char = c;
                start = cursor + 1;
            } else {
                current.push(c);
            }
        }
        cursor += 1;
    }

    parts.push((current, None, start, cursor));
    parts
}

/// Redis Commands Completion Items Definition

static COMMANDS: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let redis_cmd_json = include_str!("./redis-cmd.json");
    let result = serde_json::from_str::<Vec<Value>>(redis_cmd_json);
    let mut items = vec![];
    let clear_item = CompletionItem {
        kind: CompletionItemKind::Other,
        label: Label {
            label: "CLEAR".to_string(),
            detail: Some("clear the cli screen".to_string()),
            description: None,
        },
        parameters: vec![],
        range: (0, -1),
        insert_text: "clear".to_string(),
    };
    let exit_item = CompletionItem {
        kind: CompletionItemKind::Other,
        label: Label {
            label: "EXIT".to_string(),
            detail: Some("exit the application".to_string()),
            description: None,
        },
        parameters: vec![],
        range: (0, -1),
        insert_text: "exit".to_string(),
    };
    items.push(clear_item);
    items.push(exit_item);
    if let Ok(commands) = result {
        resolve_commands(commands, &mut items);
    }
    items
});

fn resolve_commands(commands: Vec<Value>, items: &mut Vec<CompletionItem>) -> Option<()> {
    for command in commands.iter() {
        let cmd = command.get("command")?;
        let group = command.get("group")?;
        let syntax = command.get("syntax")?;
        let summary = command.get("summary")?;
        let since = command.get("since")?;
        let complexity = command.get("complexity")?;
        let acl = command.get("acl")?;
        let arguments = command.get("arguments")?;

        let mut item = CompletionItem::default(value_to_string(cmd));

        let kind: CompletionItemKind = match group.as_str()? {
            "generic" => CompletionItemKind::Generic,
            "string" => CompletionItemKind::String,
            "list" => CompletionItemKind::List,
            "set" => CompletionItemKind::Set,
            "sorted-set" => CompletionItemKind::ZSet,
            "hash" => CompletionItemKind::Hash,
            "stream" => CompletionItemKind::Stream,
            "pubsub" => CompletionItemKind::PubSub,
            "server" => CompletionItemKind::Server,
            _ => CompletionItemKind::Other,
        };
        item.kind = kind;
        item = item.description(
            Doc::default()
                .syntax(value_to_string(syntax))
                .summary(value_to_string(summary))
                .attribute("since", value_to_string(since))
                .attribute("complexity", value_to_string(complexity))
                .attribute("acl", value_to_string(acl))
        );
        let arguments = arguments.as_array()?;
        for argument in arguments.iter() {
            let arg_type = argument.get("type")?;
            if let Value::String(type_str) = arg_type {
                match type_str.as_str() {
                    "flag" => {
                        let value = argument.get("value")?;
                        let string = value_to_string(value);
                        if !string.is_empty() {
                            item = item.add_param(Parameter::flag(string.clone(), string.clone()));
                        }
                    }
                    "enum" => {
                        let values = argument.get("values")?;
                        let values = values.as_array()?;
                        let mut vec = vec![];
                        for value in values {
                            let string = value_to_string(value);
                            if !string.is_empty() {
                                vec.push((string.clone(), string.clone()));
                            }
                        }
                        item = item.add_param(Parameter::enums(vec));
                    }
                    "arg" => {
                        let key = argument.get("key")?;
                        let arg = argument.get("arg")?;
                        let detail = argument.get("detail")?;
                        let key = value_to_string(key);
                        let arg = value_to_string(arg);
                        let detail = value_to_string(detail);
                        if !key.is_empty() {
                            item = item.add_param(Parameter::arg(key, arg, detail));
                        }
                    }
                    "many" => {
                        let name = argument.get("name")?;
                        let name = value_to_string(name);
                        if !name.is_empty() {
                            item = item.add_param(Parameter::many(name));
                        }
                    }
                    "single" => {
                        let name = argument.get("name")?;
                        let name = value_to_string(name);
                        if !name.is_empty() {
                            item = item.add_param(Parameter::single(name));
                        }
                    }
                    _ => {}
                }
            }
        }
        item = item.build_label();
        items.push(item);
    }
    Some(())
}

fn value_to_string(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        String::new()
    }
}
