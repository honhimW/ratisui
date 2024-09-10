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
mod key_utils;
mod bus;

use crate::app::{App, AppEvent, AppState, Listenable, Renderable};
use crate::components::fps::FpsCalculator;
use crate::configuration::{load_app_configuration, load_database_configuration, Configuration};
use crate::input::InputEvent;
use crate::redis_opt::{switch_client};
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use std::cmp;
use std::time::Duration;
use tokio::time::{interval};
use crate::app::AppState::Closed;
use crate::bus::{publish_msg, try_take_msg, Message};

#[tokio::main]
async fn main() -> Result<()> {
    let command = cli::cli()?;

    let matches = command.get_matches();
    let arguments = cli::AppArguments::from_matches(&matches);

    let app_config = load_app_configuration()?;

    tui_logger::init_logger(log::LevelFilter::Trace).map_err(|e| anyhow!(e))?;
    tui_logger::set_default_level(log::LevelFilter::Trace);

    let db_config = load_database_configuration()?;

    let mut default_db = db_config.default_database.clone();

    if arguments.target.is_some() {
        default_db = arguments.target;
    }

    if let Some(db) = default_db {
        if let Some(database) = db_config.databases.get(&db) {
            match switch_client(db.clone(), database) {
                Ok(_) => {
                    info!("Successfully connected to default database '{db}'");
                    info!("{database}");
                }
                Err(_) => {warn!("Failed to connect to default database.");}
            };
        }
    }

    render(App::new(db_config), app_config).await?;

    if let Err(e) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {}",
            e,
        );
    }

    Ok(())
}

async fn render(mut app: App, config: Configuration) -> Result<()> {
    let mut terminal = tui::init()?;
    let fps = cmp::min(config.fps.unwrap_or(30) as usize, 60);
    let delay_millis = 1000 / fps;
    let delay_duration = Duration::from_millis(delay_millis as u64);
    let mut fps_calculator = FpsCalculator::default();
    let mut interval = interval(delay_duration);
    loop {
        interval.tick().await;
        if !app.health() {
            break;
        }
        let render_result = terminal.draw(|frame| {
            fps_calculator.calculate_fps();
            if let Some(fps) = fps_calculator.fps.clone() {
                app.context.fps = fps;
            } else {
                app.context.fps = 0.0;
            }

            if let Ok(msg) = try_take_msg() {
                app.context.toast = Some(msg);
            }
            let render_result = app.context.render_frame(frame, frame.area());
            if let Err(e) = render_result {
                error!("Render error: {:?}", e);
                let _ = publish_msg(Message::error(format!("{}", e)).title(String::from("Render Error")));
            }
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

        loop {
            let event_result = app.input.receiver().try_recv();
            if let Ok(input_event) = event_result {
                match input_event {
                    InputEvent::Input(input) => {
                        if let Event::Key(key_event) = input {
                            if key_event.kind == KeyEventKind::Press {
                                if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::Char('c') {
                                    app.state = AppState::Closing;
                                    app.context.on_app_event(AppEvent::Destroy)?;
                                } else if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::F(5) {
                                    app.context.on_app_event(AppEvent::Reset)?
                                } else {
                                    let handle_result = app.context.handle_key_event(key_event);
                                    if let Err(e) = handle_result {
                                        error!("Handle key event error: {:?}", e);
                                        let _ = publish_msg(Message::error(format!("{}", e)).title(String::from("Handle Error")));
                                    }
                                }
                            }
                        }
                    }
                    InputEvent::State(state) => {
                        info!("Input state changed: {:?}", state);
                    }
                }
            } else {
                break;
            }
        }

    }
    app.state = Closed;
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