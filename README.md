# Ratisui

> [!NOTE]
>
> Please note that the current project is still in its very early stages of development.
> Since this is my first Rust project, it might be shit👻👻

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

> save configuration

add files as fallow under `~/.config/ratisui/`:

```toml
# ~/.config/ratisui/config.toml
fps = 45                               # tui render fps limitation
scan_size = 2000                       # redis key scan size
```

```toml
# ~/.config/ratisui/databases.toml
default_database = "remote standalone" # default connected on start up

[databases."remote standalone"]
host = "standalone.host"
port = 6379
username = "foo"                       # optional
password = "bar"                       # optional
use_tls = false
use_ssh_tunnel = false                 # not yet supported
db = 0
protocol = "RESP3"                     # RESP2 | RESP3

[databases."remote cluster"]
host = "cluster.host"
port = 6000
password = "xxxx"
use_tls = false
use_ssh_tunnel = false
db = 0
protocol = "RESP3"

```