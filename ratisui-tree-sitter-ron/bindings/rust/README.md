# tree-sitter-ron

this crate provides a Rusty Object Notation (RON) grammar for the [tree-sitter][] parsing library. to
use this crate, add it to the `[dependencies]` section of your `cargo.toml`
file. (note that you will probably also need to depend on the
[`tree-sitter`][tree-sitter crate] crate to use the parsed result in any useful
way.)

```toml
[dependencies]
tree-sitter = "~0.20.3"
tree-sitter-ron = "0.2.0"
```

typically, you will use the [language][language func] function to add this
grammar to a tree-sitter [parser][], and then use the parser to parse some code:

```rust
let code = r#"
    fn double(x: i32) -> i32 {
        x * 2
    }
"#;
let mut parser = parser::new();
parser.set_language(tree_sitter_ron::language()).expect("error loading RON grammar");
let parsed = parser.parse(code, none);
```

if you have any questions, please reach out to us in the [tree-sitter
discussions] page.

[language func]: https://docs.rs/tree-sitter-ron/*/tree_sitter_ron/fn.language.html
[parser]: https://docs.rs/tree-sitter/*/tree_sitter/struct.parser.html
[tree-sitter]: https://tree-sitter.github.io/
[tree-sitter crate]: https://crates.io/crates/tree-sitter
[tree-sitter discussions]: https://github.com/tree-sitter/tree-sitter/discussions
