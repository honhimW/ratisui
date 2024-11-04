use anyhow::{Context, Result};
use log::{debug};
use serde_json::{ Value};
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use crate::utils::ContentType;

const HIGHLIGHTS_QUERY_JSON: &'static str = r#"
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

const HIGHLIGHTS_QUERY_XML: &'static str = r#"
(tag_name) @tag
(erroneous_end_tag_name) @tag.error
(doctype) @constant
(attribute_name) @attribute
(attribute_value) @string
(comment) @comment

[
  "<"
  ">"
  "</"
  "/>"
] @punctuation.bracket
"#;

pub struct HighlightProcessor {
    source: String,
    fragments: Vec<HighlightText>,
    content_type: Option<ContentType>,
    tree: Option<Tree>,
    do_formatting: bool,
}

#[derive(Debug, Clone)]
pub struct HighlightText {
    pub text: String,
    pub kind: HighlightKind,
}

#[allow(unused)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HighlightKind {
    String,
    Number,
    Property,
    Constant,
    Comment,
    Keyword,
    Boolean,
    Null,
    Array,
    Object,
    Unknown,
}

impl HighlightProcessor {
    pub fn new(source: String, content_type: Option<ContentType>) -> Self {
        Self {
            source: source.replace("\t", "    "), // \t Tab characters may cause some content to remain on the frame.
            fragments: Vec::new(),
            content_type,
            tree: None,
            do_formatting: true,
        }
    }

    pub fn disable_formatting(&mut self) {
        self.do_formatting = false;
    }

    pub fn process(&mut self) -> Result<()> {
        if let Some(content_type) = &self.content_type {
            let _ = match content_type {
                ContentType::String => self.process_plain()?,
                ContentType::Json => self.process_json()?,
                ContentType::Xml => self.process_xml()?,
                ContentType::Ron => self.process_ron()?,
            };
            return Ok(());
        }
        let is_json = self.process_json()?;
        if is_json {
            return Ok(());
        }
        let is_xml = self.process_xml()?;
        if is_xml {
            return Ok(());
        }
        let is_ron = self.process_ron()?;
        if is_ron {
            return Ok(());
        }
        let is_plain = self.process_plain()?;
        if is_plain {
            return Ok(());
        }
        Ok(())
    }

    #[allow(unused)]
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
                if let Ok(v) = serde_json::to_string_pretty(&v) {
                    self.source = v;
                }
            }
        }

        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_json::LANGUAGE.into())?;

        let tree = parser.parse(self.source.as_str(), self.tree.as_ref())
            .context("parse error")?;
        let node = tree.root_node();
        if node.kind() != "document" || (
            if let Some(first_child) = node.child(0) {
                first_child.kind() == "ERROR"
            } else {
                false
            }
        ) {
            debug!("Source value is not a JSON: {}, SEXP: {}", self.source, tree.root_node().to_sexp());
            return Ok(false);
        }
        self.tree = Some(tree);

        let mut highlight_config = HighlightConfiguration::new(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            HIGHLIGHTS_QUERY_JSON,
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

    fn process_xml(&mut self) -> Result<bool> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_html::LANGUAGE.into())?;

        let source = &self.source;
        let source = if source.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#) {
            &source.replace(r#"<?xml version="1.0" encoding="UTF-8"?>"#, r#"<!-- <?xml version="1.0" encoding="UTF-8"?> -->"#)
        } else {
            source
        };
        let tree = parser.parse(source, self.tree.as_ref())
            .context("parse error")?;
        let node = tree.root_node();
        debug!("{}", node);
        if node.kind() != "document" || (
            if let Some(first_child) = node.child(0) {
                first_child.kind() == "ERROR" || first_child.kind() == "text"
            } else {
                false
            }
        ) {
            debug!("Source value is not a XML: {}, SEXP: {}", self.source, tree.root_node().to_sexp());
            return Ok(false);
        }
        self.tree = Some(tree);

        let mut highlight_config = HighlightConfiguration::new(
            tree_sitter_html::LANGUAGE.into(),
            "xml",
            HIGHLIGHTS_QUERY_XML,
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
            "attribute",             // 6
            "boolean",               // 7
            "string",                // 8
            "string.special",        // 9
            "string.special.symbol", // 10
            "constant",              // 11
            "tag",                   // 12
            "markup.link",           // 13
            "keyword",               // 14
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
                        1 | 6 => ht.kind = HighlightKind::Property,
                        2 | 8 | 9 => ht.kind = HighlightKind::String,
                        3 | 7 | 10 | 11 | 12 | 14 => ht.kind = HighlightKind::Constant,
                        4 => ht.kind = HighlightKind::Number,
                        5 | 13 => ht.kind = HighlightKind::Comment,
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

    fn process_ron(&mut self) -> Result<bool> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_ron::language())?;

        let source = &self.source;
        let tree = parser.parse(source, self.tree.as_ref())
            .context("parse error")?;
        let node = tree.root_node();
        if node.kind() != "source_file" || (
            if let Some(first_child) = node.child(0) {
                first_child.kind() == "ERROR"
            } else {
                false
            }
        ) {
            debug!("Source value is not a RON: {}, SEXP: {}", self.source, tree.root_node().to_sexp());
            return Ok(false);
        }
        self.tree = Some(tree);

        let mut highlight_config = HighlightConfiguration::new(
            tree_sitter_ron::language(),
            "xml",
            tree_sitter_ron::HIGHLIGHTS_QUERY,
            "",
            "",
        )?;

        let highlight_names = vec![
            "property",               // 0
            "type",                   // 1
            "type.builtin",           // 2
            "comment",                // 3
            "constant",               // 4
            "string",                 // 5
            "character",              // 6
            "number",                 // 7
            "float",                  // 8
            "boolean",                // 9
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
                        0 => ht.kind = HighlightKind::Property,
                        1 | 2 => ht.kind = HighlightKind::Keyword,
                        5 | 6 => ht.kind = HighlightKind::String,
                        4 | 9 => ht.kind = HighlightKind::Constant,
                        7 | 8 => ht.kind = HighlightKind::Number,
                        3 => ht.kind = HighlightKind::Comment,
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

    fn process_plain(&mut self) -> Result<bool> {
        let mut fragments = vec![];
        fragments.push(HighlightText { text: self.source.clone(), kind: HighlightKind::String });
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
    use crate::components::highlight_value::HighlightKind;
    use anyhow::Result;
    use serde_json::json;

    #[test]
    fn test_process_json() -> Result<()> {
        let json = json!({
            "tags": ["1", 2, "3"],
        }).to_string();
        assert_eq!(r#"{"tags":["1",2,"3"]}"#, json);
        let mut processor = super::HighlightProcessor::new(json, Some(super::ContentType::Json));
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
                HighlightKind::String => { string.push_str("\x1b[33m") }
                HighlightKind::Number => { string.push_str("\x1b[34m") }
                HighlightKind::Constant |
                HighlightKind::Property => { string.push_str("\x1b[35m") }
                HighlightKind::Comment => { string.push_str("\x1b[36m") }
                _ => { has_highlight = false }
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