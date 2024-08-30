use std::ops::Not;
use anyhow::{Context, Result, Error, anyhow};
use serde_json::{json, Value};
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

const HIGHLIGHTS_QUERY: &'static str = r#"
(pair
  key: (_) @property)

(array
  "," @array.delimiter)

[
  (pair
    value: (string) @string.value)
  (array
    (string) @string.value)
  (document (string) @string.value)
]

(number) @number

[
  (null)
  (true)
  (false)
] @constant.builtin

(escape_sequence) @escape

(comment) @comment
"#;

pub struct HighlightProcessor {
    source: String,
    fragments: Vec<HighlightText>,
    content_type: ContentType,
    tree: Option<Tree>,
    do_formatting: bool,
}

#[derive(Debug, Clone)]
pub struct HighlightText {
    pub text: String,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone)]
pub enum HighlightKind {
    String,
    Number,
    Boolean,
    Null,
    Array,
    Object,
    Keyword,
    Property,
    Constant,
    Comment,
    Unknown,
}

#[derive(Default)]
pub enum ContentType {
    String,
    #[default]
    Json,
    // JavaSerialized,
    // PhpSerialized,
    // CSharpSerialized,
    // Protobuf,
}

impl HighlightProcessor {
    pub fn new(source: String) -> Self {
        Self {
            source,
            fragments: Vec::new(),
            content_type: ContentType::default(),
            tree: None,
            do_formatting: true,
        }
    }

    pub fn disable_formatting(&mut self) {
        self.do_formatting = false;
    }

    pub fn process(&mut self) -> Result<()> {
        let is_json = self.process_json()?;
        if !is_json {
            // TODO process other content types
        }
        Ok(())
    }

    pub fn get_cursor_path(&self, row: usize, column: usize) -> Result<String> {
        if let Some(tree) = &self.tree {
            let cursor_position = tree_sitter::Point::new(row, column);
            let node = tree.root_node().descendant_for_point_range(cursor_position, cursor_position)
                .context("unable to get node")?;
            let string = get_node_path(node)[1..].join(" > ");
            return Ok(string);
        }
        Ok("".to_string())
    }

    pub fn get_fragments(&self) -> &Vec<HighlightText> {
        &self.fragments
    }

    /// result with error or not
    fn process_json(&mut self) -> Result<bool> {
        if self.do_formatting {
            if let Ok(v) = serde_json::from_str::<Value>(self.source.as_ref()) {
                self.source = serde_json::to_string_pretty(&v)?;
            }
        }

        let mut parser = Parser::new();
        let language = tree_sitter_json::language();
        parser.set_language(&language)?;

        let tree = parser.parse(self.source.as_str(), self.tree.as_ref())
            .context("parse error")?;
        self.tree = Some(tree);

        let mut highlight_config = HighlightConfiguration::new(
            language,
            "json",
            HIGHLIGHTS_QUERY,
            "",
            "",
        )?;

        let highlight_names = vec![
            "document",              // 0
            "property",              // 1
            "string.value",          // 2
            "constant.builtin",      // 3
            "number",                // 4
            "comment",               // 5
        ];
        highlight_config.configure(&highlight_names);

        let mut highlighter = Highlighter::new();
        let highlights = highlighter.highlight(&highlight_config, self.source.as_bytes(), None, |_| None)?;
        let mut fragments: Vec<HighlightText> = vec![];
        let mut highlight_text: Option<HighlightText> = None;
        for event in highlights {
            match event? {
                HighlightEvent::Source { start, end } => {
                    let x = &self.source[start..end];
                    match highlight_text {
                        None => {
                            fragments.push(HighlightText { text: x.to_string(), kind: HighlightKind::Unknown });
                        }
                        Some(ref mut ht) => {
                            ht.text = x.to_string();
                        }
                    }
                }
                HighlightEvent::HighlightStart(s) => {
                    let mut ht = HighlightText { text: "".to_string(), kind: HighlightKind::Unknown };
                    match s.0 {
                        1 => ht.kind = HighlightKind::Property,
                        2 => ht.kind = HighlightKind::String,
                        3 => ht.kind = HighlightKind::Constant,
                        4 => ht.kind = HighlightKind::Number,
                        5 => ht.kind = HighlightKind::Comment,
                        _ => ht.kind = HighlightKind::Unknown
                    }
                    highlight_text = Some(ht);
                }
                HighlightEvent::HighlightEnd => {
                    if let Some(ref mut ht) = highlight_text {
                        fragments.push(ht.clone());
                        highlight_text = None;
                    }
                }
            }
        }
        self.fragments = fragments;

        Ok(true)
    }

}

fn get_node_path(node: Node) -> Vec<String> {
    let mut path = Vec::new();
    let mut current_node = node;
    while let Some(parent) = current_node.parent() {
        path.push(current_node.kind().to_string());
        current_node = parent;
    }
    path.push(current_node.kind().to_string());
    path.reverse();
    path
}

#[cfg(test)]
mod high_light_test {
    use serde_json::json;
    use anyhow::Result;
    use crate::components::highlight_value::HighlightKind;

    #[test]
    fn test_process_json() -> Result<()> {
        let json = json!({
            "tags": ["1", 2, "3"],
        }).to_string();
        assert_eq!(r#"{"tags":["1",2,"3"]}"#, json);
        let mut processor = super::HighlightProcessor::new(json);
        let x = processor.process()?;
        assert_eq!(
r#"{
  "tags": [
    "1",
    2,
    "3"
  ]
}"#, processor.source);
        let mut string = String::new();
        for highlight_text in processor.fragments {
            let mut has_highlight = true;
            match highlight_text.kind {
                HighlightKind::String => {string.push_str("\x1b[33m")}
                HighlightKind::Number => {string.push_str("\x1b[34m")}
                HighlightKind::Boolean |
                HighlightKind::Keyword |
                HighlightKind::Constant |
                HighlightKind::Null => {string.push_str("\x1b[3;31m")}
                HighlightKind::Property => {string.push_str("\x1b[35m")}
                HighlightKind::Comment => {string.push_str("\x1b[36m")}
                _ => { has_highlight = false}
            }
            string.push_str(highlight_text.text.as_str());
            if has_highlight {
                string.push_str("\x1b[0m");
            }
        }
        let result = vec![
            "{",
            "  \x1b[35m\"tags\"\x1b[0m: [",
            "    \x1b[33m\"1\"\x1b[0m,",
            "    \x1b[34m2\x1b[0m,",
            "    \x1b[33m\"3\"\x1b[0m",
            "  ]",
            "}"
        ].join("\n");

        assert_eq!(string, result);
        Ok(())
    }

}