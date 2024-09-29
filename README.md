# Ratisui

> [!NOTE]
>
> Please note that the current project is still in its very early stages of development.
> Since this is my first Rust project, it might be shit👻👻

![gif](./assets/ratisui.gif)

## Quick Start

> build from source
```shell
# clone repo
git clone https://github.com/honhimW/ratisui.git
# enter
cd ratisui
# build and run
cargo run
```

> create a data source 

1. Press 's' (open server popup)
2. Press 'c' (create data source) 
3. config data source
4. select data source

> save configuration, auto save on exit

add files as fallow under `~/.config/ratisui/`:

```ron
// ~/.config/ratisui/config.ron
(
    fps: Some(45),                               # tui render fps limitation
    scan_size = scan_size: Some(2000),           # redis key scan size
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
            password: Some("pasword"),
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: RESP3,
        ),
    },
)
```