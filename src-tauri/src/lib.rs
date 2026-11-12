mod bubble;
mod commands;
mod events;
mod state;
mod tray;

use lanchat_core::{Identity, Node, NodeConfig, Settings};
use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

fn init_tracing() {
    let filter = EnvFilter::try_from_env("LANCHAT_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let data_dir = app.path().app_data_dir()?;
    let download_dir = app.path().download_dir()?.join("LAN Chat");
    std::fs::create_dir_all(&download_dir)?;

    let identity = Identity::load_or_create(&data_dir)?;
    let node = tauri::async_runtime::block_on(Node::start(NodeConfig {
        data_dir: data_dir.clone(),
        download_dir,
        identity,
        settings: Settings::default(),
    }))?;
    let settings = tauri::async_runtime::block_on(node.settings());
    app.manage(AppState::new(node.clone()));

    tracing::info!(
        port = node.local_port(),
        data_dir = %data_dir.display(),
        "LAN Chat ready"
    );

    if settings.launch_at_login {
        if let Err(error) = lanchat_platform::current().set_autostart(&handle, true) {
            tracing::warn!(%error, "could not enable launch at login");
        }
    }

    tray::setup(app)?;
    tray::register_hotkey(&handle);
    bubble::apply_position(&handle, &settings.bubble);
    bubble::sync_dock_icon(&handle);

    if let Some(window) = app.get_webview_window("main") {
        let handle = handle.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(main) = handle.get_webview_window("main") {
                    let _ = main.hide();
                }
                bubble::sync_dock_icon(&handle);
            }
        });
    }

    if let Some(window) = app.get_webview_window("bubble") {
        let handle = handle.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                bubble::hide(&handle);
            }
        });
    }

    events::spawn(handle, node);
    Ok(())
}

pub fn run() {
    init_tracing();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        bubble::summon(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_identity,
            commands::list_peers,
            commands::list_conversations,
            commands::list_groups,
            commands::messages,
            commands::search_messages,
            commands::send_text,
            commands::send_file,
            commands::send_file_bytes,
            commands::mark_read,
            commands::set_typing,
            commands::set_muted,
            commands::create_group,
            commands::add_group_member,
            commands::remove_group_member,
            commands::rename_group,
            commands::leave_group,
            commands::update_settings,
            commands::save_bubble_position,
            commands::reset_bubble_position,
            commands::show_bubble,
            commands::hide_bubble,
            commands::open_main,
            commands::last_active_conversation,
            commands::pending_files,
            commands::file_record,
            commands::respond_file,
            commands::file_thumbnail,
            commands::reveal_file,
            commands::open_file,
            commands::unread_total,
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("failed to build LAN Chat");

    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            tray::cleanup_hotkeys(app);
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(state.node.shutdown());
        }
    });
}
