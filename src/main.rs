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

use crate::app::{App, AppEvent, AppState, Listenable, Renderable};
use crate::tui::{TerminalBackEnd, Tui};
use anyhow::{Result, anyhow, bail};
use clap::Parser;
use log::{error, info};
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use ratisui_core::bus::{
    GlobalEvent, Message, publish_event, publish_msg, subscribe_global_channel,
    subscribe_message_channel,
};
use ratisui_core::cli::AppArguments;
use ratisui_core::configuration::{Configuration, load_app_configuration};
use ratisui_core::input::InputEvent;
use ratisui_core::marcos::KeyAsserter;
use std::cmp;
use std::sync::Arc;
use std::time::Duration;
use ratatui::crossterm::event::KeyEventKind::Press;
use ratatui::crossterm::style::Stylize;
use tokio::time::interval;

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = AppArguments::parse();

    tui_logger::init_logger(log::LevelFilter::Trace).map_err(|e| anyhow!(e))?;
    tui_logger::set_default_level(log::LevelFilter::Trace);

    let mut app_result;
    loop {
        let app_config = load_app_configuration()?;
        let tui = Tui::new(app_config.enable_mouse_capture);
        let terminal = tui.init()?;
        let app = App::new();
        app_result = run(app, terminal, app_config, arguments.clone()).await;
        if let Err(e) = tui.restore() {
            eprintln!(
                "failed to restore terminal. Run `reset` or restart your terminal to recover: {}",
                e,
            );
            break;
        }
        match app_result {
            Ok(false) => break,
            Ok(true) => tui_logger::move_events(),
            Err(e) => {
                let content = e.to_string().red().bold();
                eprintln!("{}", content);
                break;
            }
        }
    }

    Ok(())
}

async fn run(
    mut app: App,
    mut terminal: TerminalBackEnd,
    config: Configuration,
    arguments: AppArguments,
) -> Result<bool> {
    let fps = cmp::min(config.fps as usize, 60);
    let delay_millis = 1000 / fps;
    let delay_duration = Duration::from_millis(delay_millis as u64);
    let mut interval = interval(delay_duration);
    let global_channel = subscribe_global_channel()?;
    let message_channel = subscribe_message_channel()?;
    let input_channel = app.input.receiver();

    let config_arc = Arc::new(config);
    let arguments_arc = Arc::new(arguments);
    app.context.on_app_event(AppEvent::InitConfig(
        Arc::clone(&config_arc),
        Arc::clone(&arguments_arc),
    ))?;
    app.state = AppState::Running;

    // In windows and Linux, there is a Resize input event after initialize.
    // Mac may not having such event.
    publish_event(GlobalEvent::Tick)?;
    loop {
        if !app.health() {
            break;
        }
        interval.tick().await;

        if !app.context.handle_data()?
            && global_channel.is_empty()
            && message_channel.is_empty()
            && input_channel.is_empty()
        {
            continue;
        }

        if let Ok(global_event) = global_channel.try_recv() {
            if matches!(global_event, GlobalEvent::Exit) {
                app.state = AppState::Closing;
                app.context.on_app_event(AppEvent::Destroy)?;
                continue;
            }
            match global_event {
                GlobalEvent::Exit => {
                    app.close()?;
                    continue;
                }
                GlobalEvent::Restart => {
                    app.close()?;
                    app.state = AppState::Closed;
                    return Ok(true);
                }
                GlobalEvent::Tick => {}
                _ => app.context.on_app_event(AppEvent::Bus(global_event))?,
            }
        }

        let mut input_event_counter = 0;
        loop {
            let event_result = input_channel.try_recv();
            match event_result {
                Ok(InputEvent::Input(input)) => {
                    match input {
                        Event::Key(key_event) => {
                            if key_event.kind == Press {
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
                                                Message::error(format!("{}", e)).title("Handle Error"),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Event::Mouse(mouse_event) => {
                            app.context.handle_mouse_event(mouse_event)?;
                        }
                        Event::Resize(width, height) => {
                            if width < 64 || height < 16 {
                                app.close()?;
                                bail!("This application requires a larger frame size(64*16) to run as expected.");
                            }
                        }
                        _ => {}
                    }
                }
                Ok(InputEvent::State(state)) => {
                    info!("Input state changed: {:?}", state);
                }
                Err(_) => break,
            }
            input_event_counter += 1;
            // Prevent from too many input
            if input_event_counter > 50 {
                break;
            }
        }

        if let Ok(msg) = message_channel.try_recv() {
            app.context.toast = Some(msg);
        }

        let render_result = terminal.draw(|frame| {
            app.context.fps_calculator.calculate_fps();
            let render_result = app.context.render_frame(frame, frame.area());
            if let Err(e) = render_result {
                error!("Render error: {:?}", e);
                let _ = publish_msg(Message::error(format!("{}", e)).title("Render Error"));
            }
        });

        if let Err(e) = render_result {
            app.close()?;
            bail!(e);
        }
    }
    app.state = AppState::Closed;
    Ok(false)
}
