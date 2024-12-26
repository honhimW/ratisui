# Ratisui

![gif](./assets/ratisui.gif)

## Installation

### Cargo
```shell
cargo install --git=https://github.com/honhimW/ratisui.git
```

### Download
[Github release](https://github.com/honhimW/ratisui/releases)

### Build from source
```shell
# clone repo
git clone https://github.com/honhimW/ratisui.git
# enter
cd ratisui
# build and run
cargo run
```

## Quick Start

> create a data source 

1. Press 's' (open server popup)
2. Press 'c' (create data source) 
3. config data source
4. Press 'Enter' for confirm
5. select data source
6. Press 'Esc' (close data source popup)
7. Enjoy!

> save configuration, auto save on exit

## Data storage
User's data will be stored in `~/.config/ratisui/`:

### Application Configuration
```ron
// ~/.config/ratisui/config.ron
(
    fps: 45,                               // tui render fps limitation
    scan_size: 500,                        // redis key scan size,
    try_format: false,                     // fotmat raw data
    theme: Some("your-theme")              // ～/.config/ratisui/theme/your-theme.ron
)
```
### Databases Configuration
```ron
// ~/.config/ratisui/databases.ron
(
    default_database: Some("remote standalone"),
    databases: {
        "remote standalone": (
            host: "standalone.host",
            port: 6379,
            username: Some("foo"),
            password: Some("bar"),
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: RESP3,
        ),
        "remote cluster": (   // Cluster mode automatically detected
            host: "cluster.host",
            port: 6000,
            username: None,
            password: Some("password"),
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: RESP3,
        ),
    },
)
```
### Themes Configuration

> [!NOTE]
> 
> See [Theme template](./assets/theme-template.ron) for more details.
```ron
// ~/.config/ratisui/theme/your-theme.ron
(
    kind: Dark,
    toast: (
        info: Tailwind(GREEN, C700),
        warn: Yellow,
        error: Rgb(255, 0, 0),
    ),
)
```

## Features

**Connection mode**
- [x] standalone mode
- [x] cluster mode

**Data Type**
- [x] String
- [x] List
- [x] Set
- [x] Sorted Set
- [x] Hash
- [x] Stream
- [x] ReJson

**Explorer**
- [x] Key scan (Fuzzy/Pattern)
- [x] Keys tree
- [x] Key create, rename, delete
- [x] Async query & render
- [x] Highlight & format for raw data
    - [x] UTF-8/Hex deserialization
    - [x] Java deserialization ([jaded](https://crates.io/crates/jaded))
    - [x] Protobuf deserialization ([protobuf](https://crates.io/crates/protobuf))
    - [x] JSON highlighter ([tree-sitter-json](https://crates.io/crates/tree-sitter-json))
    - [x] XML highlighter ([tree-sitter-html](https://crates.io/crates/tree-sitter-html))
    - [x] RON highlighter ([tree-sitter-ron](https://crates.io/crates/tree-sitter-ron))
- [x] Table view for list, set, sorted set, hash, stream

**Command line tool**
- [x] Auto Completion
- [x] Command history
- [x] Command execution
- [x] Monitor/(P)Subscribe listening
- [x] Output format

**Misc**
- [x] Logger viewer([TuiLogger](https://crates.io/crates/tui-logger))
- [x] Non-blocking command execution
- [x] Configuration persistence
- [x] Use SSH tunnel([russh](https://crates.io/crates/russh))
- [x] Connection pool([deadpool-redis](https://crates.io/crates/deadpool-redis))
- [x] Configurable theme

**TODO**
- [ ] nothing to do...🤔
