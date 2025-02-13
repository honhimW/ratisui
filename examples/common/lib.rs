use tracing::Level;

pub mod client;

pub fn enable_log() {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_thread_names(true)
        .init();
}