#![forbid(unsafe_code)]
#![deny(
    unused_imports,
    unused_must_use,
    dead_code,
    unstable_name_collisions,
    unused_assignments
)]
#![deny(clippy::all, clippy::perf, clippy::nursery, clippy::pedantic)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::filetype_is_file,
    clippy::cargo,
    clippy::panic,
    clippy::match_like_matches_macro,
)]

pub mod redis_opt;
pub mod configuration;
pub mod utils;
pub mod bus;
pub mod ssh_tunnel;
pub mod theme;
pub mod marcos;
pub mod constants;
pub mod cli;
pub mod input;
pub mod highlight_value;
pub mod serde_wrapper;
pub mod mouse;

mod notify_mutex;
