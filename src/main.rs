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
    clippy::match_like_matches_macro
)]

mod app;
mod components;
mod context;
mod tabs;
mod tui;

use crate::app::AppState::Closed;
use crate::app::{App, AppEvent, AppState, Listenable, Renderable};
use crate::components::fps::FpsCalculator;
use crate::tui::TerminalBackEnd;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratisui_core::bus::{
    publish_event, publish_msg, subscribe_global_channel, subscribe_message_channel, GlobalEvent,
    Message,
};
use ratisui_core::cli::AppArguments;
use ratisui_core::configuration::{
    load_app_configuration, load_database_configuration, load_theme_configuration, Configuration,
    Databases,
};
use ratisui_core::input::InputEvent;
use ratisui_core::marcos::KeyAsserter;
use ratisui_core::redis_opt::switch_client;
use ratisui_core::theme;
use std::cmp;
use std::time::Duration;
use clap::Parser;
use tokio::time::interval;

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = AppArguments::parse();

    tui_logger::init_logger(log::LevelFilter::Trace).map_err(|e| anyhow!(e))?;
    tui_logger::set_default_level(log::LevelFilter::Trace);

    let app_config = load_app_configuration()?;
    let db_config = if arguments.once { 
        Databases::empty()
    } else { 
        load_database_configuration()?
    };

    apply_theme(&arguments, &app_config)?;
    apply_db(&arguments, &db_config)?;

    let terminal = tui::init()?;
    let app = App::new(db_config);
    let app_result = run(app, terminal, app_config, arguments).await;

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

async fn run(mut app: App, mut terminal: TerminalBackEnd, config: Configuration, aguments: AppArguments) -> Result<()> {
    let fps = cmp::min(config.fps as usize, 60);
    let delay_millis = 1000 / fps;
    let delay_duration = Duration::from_millis(delay_millis as u64);
    let mut fps_calculator = FpsCalculator::default();
    let mut interval = interval(delay_duration);
    let global_channel = subscribe_global_channel()?;
    let message_channel = subscribe_message_channel()?;
    let input_channel = app.input.receiver();
    loop {
        if !app.health() {
            break;
        }
        interval.tick().await;

        if matches!(app.state, AppState::Preparing) {
            app.context.on_app_event(AppEvent::Init)?;
            app.context
                .on_app_event(AppEvent::InitConfig(config.clone(), aguments.clone()))?;
            app.state = AppState::Running;
            continue;
        }

        if global_channel.is_empty()
            && message_channel.is_empty()
            && input_channel.is_empty()
            && !app.context.handle_data()?
        {
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
            let event_result = input_channel.try_recv();
            if let Ok(input_event) = event_result {
                match input_event {
                    InputEvent::Input(input) => {
                        if let Event::Key(key_event) = input {
                            if key_event.kind == KeyEventKind::Press {
                                if key_event.modifiers == KeyModifiers::CONTROL
                                    && key_event.code == KeyCode::F(5)
                                {
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
                                            let _ = publish_msg(
                                                Message::error(format!("{}", e))
                                                    .title("Handle Error"),
                                            );
                                        }
                                    }
                                }
                            }
                        } else {
                            panic!("{:?}", input);
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

        let render_result = terminal.draw(|frame| {
            fps_calculator.calculate_fps();
            if let Some(fps) = fps_calculator.fps.clone() {
                app.context.fps = fps;
            } else {
                app.context.fps = 0.0;
            }

            if let Ok(msg) = message_channel.try_recv() {
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
    }
    app.state = Closed;
    Ok(())
}

fn apply_theme(app_arguments: &AppArguments, app_config: &Configuration) -> Result<()> {
    let theme_name = app_arguments
        .theme
        .clone()
        .or_else(|| app_config.theme.clone());
    let theme = load_theme_configuration(theme_name)?;
    theme::set_theme(theme);
    Ok(())
}

fn apply_db(app_arguments: &AppArguments, db_config: &Databases) -> Result<()> {
    let default_db = app_arguments
        .target
        .clone()
        .or_else(|| db_config.default_database.clone());

    if let Some(db) = default_db {
        if let Some(database) = db_config.databases.get(&db) {
            let database_clone = database.clone();
            tokio::spawn(async move {
                match switch_client(db.clone(), &database_clone) {
                    Ok(_) => {
                        info!("Successfully connected to default database '{db}'");
                        info!("{database_clone}");
                    }
                    Err(_) => {
                        warn!("Failed to connect to default database.");
                    }
                };
            });
        } else {
            Err(anyhow!("Unknown database '{db}'."))?;
        }
    };
    Ok(())
}
