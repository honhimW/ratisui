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
mod utils;
mod bus;
mod ssh_tunnel;
mod theme;
mod marcos;
mod constants;

use crate::app::{App, AppEvent, AppState, Listenable, Renderable};
use crate::components::fps::FpsCalculator;
use crate::configuration::{load_app_configuration, load_database_configuration, load_theme_configuration, Configuration, Databases};
use crate::input::InputEvent;
use crate::redis_opt::switch_client;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use std::cmp;
use std::time::Duration;
use tokio::time::interval;
use crate::app::AppState::Closed;
use crate::bus::{publish_event, publish_msg, subscribe_global_channel, try_take_msg, GlobalEvent, Message};
use crate::cli::AppArguments;
use crate::marcos::KeyAsserter;
use crate::tui::TerminalBackEnd;

#[tokio::main]
async fn main() -> Result<()> {
    let command = cli::cli()?;

    let matches = command.get_matches();
    let arguments = AppArguments::from_matches(&matches);

    tui_logger::init_logger(log::LevelFilter::Trace).map_err(|e| anyhow!(e))?;
    tui_logger::set_default_level(log::LevelFilter::Trace);

    let app_config = load_app_configuration()?;
    let db_config = load_database_configuration()?;

    apply_theme(&arguments, &app_config)?;
    apply_db(&arguments, &db_config)?;

    let terminal = tui::init()?;
    let app = App::new(db_config);
    let app_result = run(app, terminal, app_config).await;

    if let Err(e) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {}",
            e,
        );
    }

    if let Err(e) = app_result {
        eprintln!("{:?}", e);
    }

    Ok(())
}

async fn run(mut app: App, mut terminal: TerminalBackEnd, config: Configuration) -> Result<()> {
    let fps = cmp::min(config.fps as usize, 60);
    let delay_millis = 1000 / fps;
    let delay_duration = Duration::from_millis(delay_millis as u64);
    let mut fps_calculator = FpsCalculator::default();
    let mut interval = interval(delay_duration);
    let global_channel = subscribe_global_channel()?;
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
                let _ = publish_msg(Message::error(format!("{}", e)).title("Render Error"));
            }
        });

        if let Err(e) = render_result {
            app.state = AppState::Closing;
            return Err(anyhow!(e));
        }

        if app.state == AppState::Preparing {
            app.context.on_app_event(AppEvent::Init)?;
            app.context.on_app_event(AppEvent::InitConfig(config.clone()))?;
            app.state = AppState::Running;
            continue;
        }

        if let Ok(global_event) = global_channel.try_recv() {
            if matches!(global_event, GlobalEvent::Exit) {
                app.state = AppState::Closing;
                app.context.on_app_event(AppEvent::Destroy)?;
                continue;
            }
            app.context.on_app_event(AppEvent::Bus(global_event))?;
        }

        loop {
            let event_result = app.input.receiver().try_recv();
            if let Ok(input_event) = event_result {
                match input_event {
                    InputEvent::Input(input) => {
                        if let Event::Key(key_event) = input {
                            if key_event.kind == KeyEventKind::Press {
                                if key_event.modifiers == KeyModifiers::CONTROL && key_event.code == KeyCode::F(5) {
                                    app.context.on_app_event(AppEvent::Reset)?
                                } else {
                                    let handle_result = app.context.handle_key_event(key_event);
                                    match handle_result {
                                        Ok(accepted) => {
                                            if !accepted && key_event.is_c_c() {
                                                let _ = publish_event(GlobalEvent::Exit);
                                            }
                                        }
                                        Err(e) => {
                                            error!("Handle key event error: {:?}", e);
                                            let _ = publish_msg(Message::error(format!("{}", e)).title("Handle Error"));
                                        }
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

fn apply_theme(app_arguments: &AppArguments, app_config: &Configuration) -> Result<()> {
    let theme_name = app_arguments.theme.clone().or_else(|| app_config.theme.clone());
    let theme = load_theme_configuration(theme_name)?;
    theme::set_theme(theme);
    Ok(())
}

fn apply_db(app_arguments: &AppArguments, db_config: &Databases) -> Result<()> {
    let default_db = app_arguments.target.clone().or_else(|| db_config.default_database.clone());

    if let Some(db) = default_db {
        if let Some(database) = db_config.databases.get(&db) {
            let database_clone = database.clone();
            tokio::spawn(async move {
                match switch_client(db.clone(), &database_clone) {
                    Ok(_) => {
                        info!("Successfully connected to default database '{db}'");
                        info!("{database_clone}");
                    }
                    Err(_) => {warn!("Failed to connect to default database.");}
                };
            });
        } else {
            Err(anyhow!("Unknown database '{db}'."))?;
        }
    };
    Ok(())
}
