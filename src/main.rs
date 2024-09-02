mod app;
mod cli;
mod configuration;
mod context;
mod input;
mod notify_mutex;
mod redis_opt;
mod tui;
mod tabs;
mod components;

use std::cell::Cell;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use log4rs::config::RawConfig;
use log::debug;
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::join;
use tokio::sync::RwLock;
use crate::app::{App, AppEvent, AppState, Listenable, Renderable};
use crate::configuration::{load_app_configuration, load_database_configuration};
use crate::input::InputEvent;
use crate::redis_opt::{redis_operations, switch_client};

#[tokio::main]
async fn main() -> Result<()> {
    let command = cli::cli()?;

    let matches = command.get_matches();
    let arguments = cli::AppArguments::from_matches(&matches);

    let app_config = load_app_configuration()?;

    log4rs::init_raw_config(RawConfig::default())?;

    let db_config = load_database_configuration()?;

    let mut default_db = db_config.default_database;

    if arguments.target.is_some() {
        default_db = arguments.target;
    }

    if let Some(db) = default_db {
        if let Some(database) = db_config.databases.get(&db) {
            let x = redis_operations();
            switch_client(database)?;
            debug!("{:?}", database);
            if let Some(c) = x {
                debug!("connected!");
                // let mut con = c.get_connection()?;
            }
        }
    }

    render(App::new())?;

    // let renderer = tokio::spawn(render(Arc::clone(&arc)));
    // let handler = tokio::spawn(handle_events(Arc::clone(&arc)));
    // let (renderer_result, handler_result) = join!(renderer, handler);
    //
    // if let Err(e) = renderer_result {
    //     eprintln!(
    //         "Failed to render: {}",
    //         e,
    //     )
    // }
    // if let Err(e) = handler_result {
    //     eprintln!(
    //         "Failed to handle event: {}",
    //         e,
    //     )
    // }

    if let Err(e) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {}",
            e,
        );
    }

    Ok(())
}

fn render(mut app: App) -> Result<()> {
    let mut terminal = tui::init()?;

    loop {
        if !app.health() {
            break;
        }
        let render_result = terminal.draw(|frame| {
            let _ = app.context.render_frame(frame, frame.area());
        });

        if let Err(e) = render_result {
            app.state = AppState::Closing;
            return Err(anyhow!(e));
        }

        if app.state == AppState::Preparing {
            app.context.on_app_event(AppEvent::Init)?;
            app.state = AppState::Running;
            continue;
        }

        let event_result = app.input.receiver().try_recv();

        if let Ok(input_event) = event_result {
            if let InputEvent::Input(event) = input_event {
                if let Event::Key(key_event) = event {
                    if key_event.kind == KeyEventKind::Press {
                        if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::Char('c') {
                            app.state = AppState::Closing;
                        } else {
                            let _ = app.context.handle_key_event(key_event);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// async fn handle_events(app_arc: Arc<RwLock<App>>) -> Result<()> {
//     loop {
//         let app_read_guard = app_arc.read().await;
//         if !app_read_guard.health() {
//             break;
//         }
//         if app_read_guard.state == AppState::Preparing {
//             let mut context_write_guard = app_read_guard.context.write().await;
//             context_write_guard.on_app_event(AppEvent::Init).await?;
//             drop(context_write_guard);
//             drop(app_read_guard);
//             let mut app_write_guard = app_arc.write().await;
//             app_write_guard.state = AppState::Running;
//             continue;
//         }
//         let event_result = app_read_guard.input.receiver().try_recv();
//         drop(app_read_guard);
//         if let Ok(input_event) = event_result {
//             if let InputEvent::Input(event) = input_event {
//                 if let Event::Key(key_event) = event {
//                     if key_event.kind == KeyEventKind::Press {
//                         if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::Char('c') {
//                             let mut app_write_guard = app_arc.write().await;
//                             app_write_guard.state = AppState::Closing;
//                             drop(app_write_guard);
//                         } else {
//                             let app_read_guard = app_arc.read().await;
//                             let mut context_write_guard = app_read_guard.context.write().await;
//                             let _ = context_write_guard.handle_key_event(key_event).await?;
//                             drop(context_write_guard);
//                             drop(app_read_guard);
//                         }
//                     }
//                 }
//             }
//         }
//     }
//
//     Ok(())
// }