#![allow(unused)]

use crate::app::{Listenable, Renderable};
use crate::components::completion::{sort_commands, split_args, CompletableTextArea, CompletionItem, Doc, Parameter};
use anyhow::{Error, Result};
use bitflags::bitflags;
use crossbeam_channel::{unbounded, Receiver, Sender};
use futures::FutureExt;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Constraint::{Fill, Length, Percentage};
use ratatui::layout::{Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Clear};
use ratatui::Frame;
use ratisui_core::redis_opt::spawn_redis_opt;
use ratisui_core::theme::get_color;
use redis::{FromRedisValue, RedisResult, Value};
use std::collections::HashMap;
use once_cell::sync::Lazy;
use substring::Substring;

pub struct FtSearchPanel<'a> {
    editing: Editing,
    indexes: Option<Vec<String>>,
    index_info: Option<IndexInfo>,
    indexes_info: HashMap<String, IndexInfo>,
    index_area: CompletableTextArea<'a>,
    search_area: CompletableTextArea<'a>,
    index_block: Block<'a>,
    search_block: Block<'a>,
    data_sender: Sender<Data>,
    data_receiver: Receiver<Data>,
}

enum Editing {
    Index,
    Search,
}

bitflags! {
    #[derive(Default, Clone)]
    struct Flags: u8 {
        const NONE = 0b0000_0000;
        const INDEX = 0b0000_0001;
        const SEARCH = 0b0000_0010;
    }
}

#[derive(Default, Clone)]
struct Data {
    flags: Flags,
    indexes: Option<Vec<String>>,
    index_info: Option<IndexInfo>,
}

#[derive(Default, Clone)]
struct IndexInfo {
    name: String,
    key_type: String,
    prefixes: Vec<String>,
    attributes: Vec<AttributeInfo>,
    num_docs: i64,
    max_doc_id: i64,
    total_index_memory_sz_mb: f64,
}

#[derive(Default, Clone)]
struct AttributeInfo {
    identifier: String,
    attribute: String,
    kind: String,
}

impl<'a> FtSearchPanel<'a> {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let mut index_area = CompletableTextArea::new();
        index_area.focus();
        let mut search_area = CompletableTextArea::new();
        search_area.blur();
        Self {
            editing: Editing::Index,
            indexes: None,
            index_info: None,
            index_area,
            search_area,
            indexes_info: HashMap::new(),
            index_block: Block::bordered()
                .title("Index ●")
                .border_type(BorderType::Double),
            search_block: Block::bordered()
                .title("Search")
                .border_type(BorderType::Plain),
            data_sender: tx,
            data_receiver: rx,
        }
    }

    pub fn list_indexes(&mut self) -> Result<()> {
        let sender = self.data_sender.clone();
        Ok(spawn_redis_opt(move |operations| async move {
            let indexes = operations.str_cmd("FT._LIST").await?;
            let indexes = Vec::<String>::from_redis_value(&indexes)?;
            sender.send(Data {
                flags: Flags::INDEX,
                indexes: Some(indexes.clone()),
                ..Default::default()
            })?;

            for index in indexes.iter() {
                let sender_clone = sender.clone();
                let opt_clone = operations.clone();
                let index_clone = index.clone();
                tokio::spawn(async move {
                    let v: IndexInfo = opt_clone
                        .str_cmd(format!("FT.INFO {}", index_clone))
                        .await?;
                    sender_clone.send(Data {
                        flags: Flags::SEARCH,
                        index_info: Some(v),
                        ..Default::default()
                    })?;
                    Ok::<(), Error>(())
                });
            }
            Ok::<(), Error>(())
        })?)
    }

    pub fn get_input(&self) -> Result<(String, String)> {
        Ok((self.index_area.get_input(), self.search_area.get_input()))
    }

    fn update_data(&mut self, data: Data) {
        if data.flags.contains(Flags::INDEX) {
            self.indexes = data.indexes;
        }
        if data.flags.contains(Flags::SEARCH) {
            if let Some(ref index_info) = data.index_info {
                self.indexes_info
                    .insert(index_info.name.clone(), index_info.clone());
            }
        }
    }

    fn next(&mut self) {
        self.editing = match self.editing {
            Editing::Index => {
                self.index_area.blur();
                self.search_area.focus();
                self.index_block = Block::bordered()
                    .title("Index")
                    .border_type(BorderType::Plain);
                self.search_block = Block::bordered()
                    .title("Search ●")
                    .border_type(BorderType::Double);
                let input_index = self.index_area.get_input();
                if let Some(index_info) = self.indexes_info.get(&input_index) {
                    self.index_info = Some(index_info.clone());
                } else {
                    self.index_info = None;
                }
                Editing::Search
            }
            Editing::Search => {
                self.index_area.focus();
                self.search_area.blur();
                self.index_block = Block::bordered()
                    .title("Index ●")
                    .border_type(BorderType::Double);
                self.search_block = Block::bordered()
                    .title("Search")
                    .border_type(BorderType::Plain);
                Editing::Index
            }
        };
    }

    fn get_index_items(&self, input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
        if let Some(ref indexes) = self.indexes {
            let mut items = indexes
                .iter()
                .map(|index_name| {
                    let mut item = CompletionItem::custom(index_name, "Index");
                    if let Some(index_info) = self.indexes_info.get(index_name) {
                        item = item.detail(format!("{:.3}M", index_info.total_index_memory_sz_mb));
                        let mut doc = Doc::default()
                            .syntax(index_info.name.clone())
                            .summary(format!("[{}]", index_info.prefixes.join("] [")))
                            .attribute("type                     ", index_info.key_type.clone())
                            .attribute("num_docs                 ", index_info.num_docs.to_string())
                            .attribute(
                                "max_doc_id               ",
                                index_info.max_doc_id.to_string(),
                            )
                            .attribute(
                                "total_index_memory_sz_mb ",
                                format!("{:.3}M", index_info.total_index_memory_sz_mb),
                            )
                            .attribute(
                                "attributes_count         ",
                                index_info.attributes.len().to_string(),
                            );
                        item = item.description(doc);
                    };
                    item
                })
                .collect::<Vec<CompletionItem>>();
            items = items.into_iter().filter(|x| {
                x.label.label.to_lowercase().contains(&input.to_lowercase())
            }).collect();
            return (items, input.to_string());
        }
        (vec![], "".to_string())
    }

    fn get_search_items(&self, input: &str, cursor_x: usize) -> (Vec<CompletionItem>, String) {
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
                segment = input.substring(*start_pos, cursor_x).to_string();
                break;
            }
        }

        let mut items: Vec<CompletionItem> = vec![];
        let (start, end) = if let Some((_, _, _, start_pos, end_pos)) = current_word {
            (start_pos as isize, end_pos as isize)
        } else {
            (0, -1)
        };
        if let Some(ref index_info) = self.index_info {
            items = index_info
                .attributes
                .iter()
                .filter_map(|info| {
                    if info
                        .attribute
                        .to_lowercase()
                        .contains(&segment.to_lowercase())
                    {
                        struct SimpleItem {
                            insert_text: String,
                            syntax: String,
                            summary: String,
                        }
                        let base_summary = r#"Dialect 2
1. AND, multi-word phrases that imply intersection
  @{}:(<q1> <q2>)
2. "..." (exact), ~ (optional), - (negation), and % (fuzzy)
3. OR, words separated by the | (pipe) character that imply union
  @{}:(<q1> | <q2>)
4. Wildcard characters
  @{}:"w'foo*bar'"
"#;
                        let simple_item = match info.kind.as_str() {
                            "NUMERIC" => {
                                let summary = format!(
                                    r#"Search by number
- Equal: @{}:[q] | @{}==<q>
- Not equal: @{}!=<q>
- Compare: @{}[ > | < | >= | <= ]<q>
- Range: @{}:[min max]

{}
"#,
                                    info.attribute, info.attribute,
                                    info.attribute,
                                    info.attribute,
                                    info.attribute,
                                    base_summary,
                                );
                                SimpleItem {
                                    insert_text: format!("'@{}$end'", info.attribute),
                                    syntax: format!("@{}<op><q>", info.attribute),
                                    summary,
                                }
                            }
                            "TAG" => {
                                let summary = format!(
                                    r#"Search by TAG:
- @{}:{{q}}

{}
"#,
                                    info.attribute,
                                    base_summary,
                                );
                                SimpleItem {
                                    insert_text: format!("'@{}:{{$end}}'", info.attribute),
                                    syntax: format!("@{}:{{<q>}}", info.attribute),
                                    summary,
                                }
                            }
                            "GEO" => {
                                let summary = format!(
                                    r#"Search by distance from LON/LAT:
@{}:[<LON> <LAT> <RADIUS> m|km|mi|ft]
Dialect 3 WITHIN and CONTAINS operator
@{}:[WITHIN|CONTAINS $poly] params 2 poly 'POLYGON((...))

{}
"#,
                                    info.attribute,
                                    info.attribute,
                                    base_summary,
                                );
                                SimpleItem {
                                    insert_text: format!("'@{}:[$end]'", info.attribute),
                                    syntax: format!("@{}:[...]", info.attribute),
                                    summary,
                                }
                            }
                            "TEXT" | "VECTOR" | _ => {
                                SimpleItem {
                                    insert_text: format!("'@{}:\"$end\"'", info.attribute),
                                    syntax: format!("@{}:<q>", info.attribute),
                                    summary: base_summary.to_string(),
                                }
                            }
                        };
                        Some(
                            CompletionItem::custom(format!("@{}", info.attribute), "attr")
                                .insert_text(simple_item.insert_text)
                                .detail(info.kind.clone())
                                .description(
                                    Doc::default()
                                        .syntax(simple_item.syntax)
                                        .summary(simple_item.summary)
                                        .attribute("identifier ", info.identifier.clone())
                                        .attribute("type       ", info.kind.clone()),
                                )
                                .range(start, end),
                        )
                    } else {
                        None
                    }
                })
                .collect::<Vec<CompletionItem>>();
        }
        let mut search_arguments = get_ft_search_arguments(start, end);
        search_arguments = search_arguments.into_iter().filter(|x| {
            x.label.label.to_lowercase().contains(&segment.to_lowercase())
        }).collect();
        items.extend(search_arguments);
        (items, segment)
    }
}

fn get_ft_search_arguments(start: isize, end: isize) -> Vec<CompletionItem> {
    let mut search_params: Vec<CompletionItem> = vec![];
    search_params.push(CompletionItem::option("DIALECT").range(start, end)
        .detail("version")
        .description(Doc::default()
            .syntax("DIALECT {dialect_version}")
            .summary("selects the dialect version under which to execute the query. If not specified, the query will execute under the default dialect version set during module initial loading or via FT.CONFIG SET command.")
            .attribute("default", "1")
            .attribute("option", "1|2|3|4")
            .attribute("see", "FT.CONFIG SET")
        )
    );
    search_params.push(CompletionItem::option("EXPANDER").range(start, end)
        .detail("expander")
        .description(Doc::default()
            .syntax("EXPANDER {expander}")
            .summary("uses a custom query expander instead of the stemmer.")
        )

    );
    search_params.push(CompletionItem::option("EXPLAINSCORE").range(start, end)
        .description(Doc::default()
            .syntax("EXPLAINSCORE")
            .summary("returns a textual description of how the scores were calculated. Using this option requires WITHSCORES.")
            .attribute("see", "WITHSCORES")
        )
    );
    search_params.push(CompletionItem::option("FRAGS").range(start, end)
        .detail("num")
        .description(Doc::default()
            .syntax("FRAGS {num}")
            .summary("The number of fragments to be returned. If not specified")
            .attribute("default", "3")
        )
    );
    search_params.push(CompletionItem::option("HIGHLIGHT").range(start, end)
        .detail("[FIELDS {num} {field}] [TAGS {openTag} {closeTag}]")
        .description(Doc::default()
            .syntax("HIGHLIGHT [FIELDS {num} {field}] [TAGS {openTag} {closeTag}]")
            .summary("Highlighting will surround the found term (and its variants) with a user-defined pair of tags. This may be used to display the matched text in a different typeface using a markup language, or to otherwise make the text appear differently.")
            .attribute("see", "FIELDS")
            .attribute("see", "TAGS")
            .attribute("see", "RETURN")
        )
    );
    search_params.push(CompletionItem::option("INFIELDS").range(start, end)
        .detail("{num} {attribute}")
        .description(Doc::default()
            .syntax("INFIELDS {num} {attribute}")
            .summary("filters the results to those appearing only in specific attributes of the document, like title or URL. You must include num, which is the number of attributes you're filtering by. For example, if you request title and URL, then num is 2.")
        )
    );
    search_params.push(CompletionItem::option("INKEYS").range(start, end)
        .detail("{num} {attribute}")
        .description(Doc::default()
            .syntax("INKEYS {num} {attribute}")
            .summary("limits the result to a given set of keys specified in the list. The first argument must be the length of the list and greater than zero. Non-existent keys are ignored, unless all the keys are non-existent.")
        )
    );
    search_params.push(CompletionItem::option("INORDER").range(start, end)
        .description(Doc::default()
            .syntax("INORDER")
            .summary("requires the terms in the document to have the same order as the terms in the query, regardless of the offsets between them. Typically used in conjunction with SLOP. Default is false.")
        )
    );
    search_params.push(CompletionItem::option("LANGUAGE").range(start, end)
        .detail("{language}")
        .description(Doc::default()
            .syntax("LANGUAGE {language}")
            .summary("use a stemmer for the supplied language during search for query expansion. If querying documents in Chinese, set to chinese to properly tokenize the query terms. Defaults to English. If an unsupported language is sent, the command returns an error. See FT.CREATE for the list of languages. If LANGUAGE was specified as part of index creation, it doesn't need to specified with FT.SEARCH.")
        )
    );
    search_params.push(CompletionItem::option("LEN").range(start, end)
        .detail("{fragLen}")
        .description(Doc::default()
            .syntax("LEN {fragLen}")
            .summary("The number of context words each fragment should contain. Context words surround the found term. A higher value will return a larger block of text. If not specified, the default value is 20.")
            .attribute("default", "20")
        )
    );
    search_params.push(CompletionItem::option("LIMIT").range(start, end)
        .detail("LIMIT {first} {num}")
        .description(Doc::default()
            .syntax("LIMIT {first} {num}")
            .summary(r#"limits the results to the offset and number of results given. Note that the offset is zero-indexed. The default is 0 10, which returns 10 items starting from the first result. You can use LIMIT 0 0 to count the number of documents in the result set without actually returning them.
LIMIT behavior:
If you use the LIMIT option without sorting, the results returned are non-deterministic, which means that subsequent queries may return duplicated or missing values. Add SORTBY with a unique field, or use FT.AGGREGATE with the WITHCURSOR option to ensure deterministic result set paging.
"#)
            .attribute("default", "0 10")
        )
    );
    search_params.push(CompletionItem::option("NOCONTENT").range(start, end)
        .description(Doc::default()
            .syntax("NOCONTENT")
            .summary("returns the document ids and not the content. This is useful if RediSearch is only an index on an external document collection.")
        )
    );
    search_params.push(CompletionItem::option("NOSTOPWORDS").range(start, end)
        .description(Doc::default()
            .syntax("NOSTOPWORDS")
            .summary("ignores any defined stop words in full text searches.")
        )
    );
    search_params.push(CompletionItem::option("PARAMS").range(start, end)
        .detail("{nargs} {name} {value}")
        .description(Doc::default()
            .syntax("PARAMS {nargs} {name} {value}")
            .summary("defines one or more value parameters. Each parameter has a name and a value. You can reference parameters in the query by a $, followed by the parameter name.")
            .attribute("require", "RedisSearch v2.4+")
        )
    );
    search_params.push(CompletionItem::option("PAYLOAD").range(start, end)
        .detail("{payload}")
        .description(Doc::default()
            .syntax("PAYLOAD {payload}")
            .summary("adds an arbitrary, binary safe payload that is exposed to custom scoring functions.")
        )
    );
    search_params.push(CompletionItem::option("RETURN").range(start, end)
        .detail("{num} {identifier} AS {property}")
        .description(Doc::default()
            .syntax("RETURN {num} {identifier} AS {property}")
            .summary("limits the attributes returned from the document. num is the number of attributes following the keyword. If num is 0, it acts like NOCONTENT. identifier is either an attribute name (for hashes and JSON) or a JSON Path expression (for JSON). property is an optional name used in the result. If not provided, the identifier is used in the result.")
        )
    );
    search_params.push(CompletionItem::option("SCORER").range(start, end)
        .detail("{scorer}")
        .description(Doc::default()
            .syntax("SCORER {scorer}")
            .summary("uses a built-in or a user-provided scoring function.")
        )
    );
    search_params.push(CompletionItem::option("SEPARATOR").range(start, end)
        .detail("{sepStr}")
        .description(Doc::default()
            .syntax("SEPARATOR {sepStr}")
            .summary("The string used to divide individual summary snippets. The default is ... which is common among search engines, but you may override this with any other string if you desire to programmatically divide the snippets later on. You may also use a newline sequence, as newlines are stripped from the result body during processing.")
            .attribute("default", "...")
        )
    );
    search_params.push(CompletionItem::option("SLOP").range(start, end)
        .detail("{slop}")
        .description(Doc::default()
            .syntax("SLOP {slop}")
            .summary("is the number of intermediate terms allowed to appear between the terms of the query. Suppose you're searching for a phrase hello world. If some terms appear in-between hello and world, a SLOP greater than 0 allows for these text attributes to match. By default, there is no SLOP constraint.")
        )
    );
    search_params.push(CompletionItem::option("SORTBY").range(start, end)
        .detail("{attribute} [ASC|DESC] [WITHCOUNT]")
        .description(Doc::default()
            .syntax("SORTBY {attribute} [ASC|DESC] [WITHCOUNT]")
            .summary("orders the results by the value of this attribute. This applies to both text and numeric attributes. Attributes needed for SORTBY should be declared as SORTABLE in the index, in order to be available with very low latency. Note that this adds memory overhead.")
            .attribute("note", "adds memory overhead")
        )
    );
    search_params.push(CompletionItem::option("SUMMARIZE").range(start, end)
        .detail("[FIELDS {num} {field}] [FRAGS {numFrags}] [LEN {fragLen}] [SEPARATOR {sepStr}]")
        .description(Doc::default()
            .syntax("SUMMARIZE [FIELDS {num} {field}] [FRAGS {numFrags}] [LEN {fragLen}] [SEPARATOR {sepStr}]")
            .summary("returns only the sections of the attribute that contain the matched text. Summarization will fragment the text into smaller sized snippets, each of which containing the found term(s) and some additional surrounding context.")
            .attribute("see", "FIELDS")
            .attribute("see", "FRAGS")
            .attribute("see", "LEN")
            .attribute("see", "SEPARATOR")
        )
    );
    search_params.push(CompletionItem::option("TIMEOUT").range(start, end)
        .detail("{milliseconds}")
        .description(Doc::default()
            .syntax("TIMEOUT {milliseconds}")
            .summary("overrides the timeout parameter of the module.")
        )
    );
    search_params.push(CompletionItem::option("VERBATIM").range(start, end)
        .description(Doc::default()
            .syntax("VERBATIM")
            .summary("does not try to use stemming for query expansion but searches the query terms verbatim.")
        )
    );
    search_params.push(CompletionItem::option("WITHPAYLOADS").range(start, end)
        .description(Doc::default()
            .syntax("WITHPAYLOADS")
            .summary("retrieves optional document payloads. See FT.CREATE. The payloads follow the document id and, if WITHSCORES is set, the scores.")
            .attribute("see", "FT.CREATE")
        )
    );
    search_params.push(CompletionItem::option("WITHSCORES").range(start, end)
        .description(Doc::default()
            .syntax("WITHSCORES")
            .summary("also returns the relative internal score of each document. This can be used to merge results from multiple instances.")
        )
    );
    search_params.push(CompletionItem::option("WITHSORTKEYS").range(start, end)
        .description(Doc::default()
            .syntax("WITHSORTKEYS")
            .summary("returns the value of the sorting key, right after the id and score and/or payload, if requested. This is usually not needed, and exists for distributed search coordination purposes. This option is relevant only if used in conjunction with SORTBY.")
        )
    );
    search_params
}

impl Renderable for FtSearchPanel<'_> {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        frame.render_widget(Clear::default(), rect);
        let vertical = Layout::vertical([Percentage(50), Percentage(50)]).split(rect);

        let index_area = self.index_block.inner(vertical[0]);
        let search_area = self.index_block.inner(vertical[1]);
        frame.render_widget(&self.index_block, vertical[0]);
        frame.render_widget(&self.search_block, vertical[1]);
        let block = Block::bordered();
        let inner_area = block.inner(rect);
        let frame_area = frame.area();
        self.index_area
            .update_frame(frame_area.height, frame_area.width);
        self.search_area
            .update_frame(frame_area.height, frame_area.width);
        self.index_area.render_frame(frame, index_area)?;
        self.search_area.render_frame(frame, search_area)?;
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("^Space", "Suggest"));
        elements
    }

    fn handle_data(&mut self) -> Result<bool> {
        let mut needed = false;
        while !self.data_receiver.is_empty() {
            let data = self.data_receiver.try_recv();
            if let Ok(data) = data {
                self.update_data(data);
                needed = true;
            }
        }
        Ok(needed)
    }
}

impl Listenable for FtSearchPanel<'_> {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<bool> {
        if event.kind == KeyEventKind::Press {
            match event.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    let accepted = match self.editing {
                        Editing::Index => self.index_area.handle_key_event(event)?,
                        Editing::Search => self.search_area.handle_key_event(event)?,
                    };
                    if !accepted {
                        self.next();
                    }
                    return Ok(true);
                }
                _ => {}
            }
            match self.editing {
                Editing::Index => {
                    let accepted = self.index_area.handle_key_event(event)?;
                    let (_, cursor_x) = self.index_area.get_cursor();
                    let raw_input = self.index_area.get_input();
                    let (mut items, segment) = self.get_index_items(&raw_input, cursor_x);
                    sort_commands(&mut items, &segment);
                    self.index_area.update_completion_items(items, segment);
                    Ok(accepted)
                }
                Editing::Search => {
                    let accepted = self.search_area.handle_key_event(event)?;
                    let (_, cursor_x) = self.search_area.get_cursor();
                    let raw_input = self.search_area.get_input();
                    let (mut items, segment) = self.get_search_items(&raw_input, cursor_x);
                    sort_commands(&mut items, &segment);
                    self.search_area.update_completion_items(items, segment);
                    Ok(accepted)
                }
            }
        } else {
            Ok(false)
        }
    }
}

impl FromRedisValue for IndexInfo {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        let mut this = Self::default();
        if let Value::Map(ref map) = v {
            for (key, value) in map {
                if let Value::SimpleString(key) = key {
                    match key.as_ref() {
                        "index_name" => {
                            if let Value::SimpleString(value) = value {
                                this.name = value.clone();
                            }
                        }
                        "index_definition" => {
                            if let Value::Map(value) = value {
                                for (key, value) in value {
                                    if let Value::SimpleString(key) = key {
                                        if key == "key_type" {
                                            if let Value::SimpleString(value) = value {
                                                this.key_type = value.clone();
                                            }
                                        }
                                        if key == "prefixes" {
                                            if let Value::Array(value) = value {
                                                for v in value.iter() {
                                                    if let Value::SimpleString(s) = v {
                                                        this.prefixes.push(s.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "attributes" => {
                            if let Value::Array(value) = value {
                                for attribute in value {
                                    if let Value::Map(attribute) = attribute {
                                        let mut attribute_info = AttributeInfo {
                                            identifier: "".to_string(),
                                            attribute: "".to_string(),
                                            kind: "".to_string(),
                                        };
                                        for (key, value) in attribute {
                                            if let Value::SimpleString(key) = key {
                                                match key.as_ref() {
                                                    "identifier" => {
                                                        if let Value::SimpleString(identifier) =
                                                            value
                                                        {
                                                            attribute_info.identifier =
                                                                identifier.clone();
                                                        }
                                                    }
                                                    "attribute" => {
                                                        if let Value::SimpleString(attribute) =
                                                            value
                                                        {
                                                            attribute_info.attribute =
                                                                attribute.clone();
                                                        }
                                                    }
                                                    "type" => {
                                                        if let Value::SimpleString(kind) = value {
                                                            attribute_info.kind = kind.clone();
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        this.attributes.push(attribute_info);
                                    }
                                }
                            }
                        }
                        "num_docs" => {
                            if let Value::Int(value) = value {
                                this.num_docs = value.clone();
                            }
                        }
                        "max_doc_id" => {
                            if let Value::Int(value) = value {
                                this.max_doc_id = value.clone();
                            }
                        }
                        "total_index_memory_sz_mb" => {
                            if let Value::Double(value) = value {
                                this.total_index_memory_sz_mb = value.clone();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(this)
    }

    fn from_owned_redis_value(v: Value) -> RedisResult<Self> {
        Self::from_redis_value(&v)
    }
}
