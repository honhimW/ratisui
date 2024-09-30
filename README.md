# Ratisui

> [!NOTE]
>
> Please note that the current project is still in its very early stages of development.
> Since this is my first Rust project, it might be shit👻👻

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

### Data storage
User's data will be stored in `~/.config/ratisui/`:

```ron
// ~/.config/ratisui/config.ron
(
    fps: Some(45),                               // tui render fps limitation
    scan_size = scan_size: Some(2000),           // redis key scan size
)
```

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
        "remote cluster": (
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