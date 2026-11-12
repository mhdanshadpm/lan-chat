//! Bubble window lifecycle: positioning, showing without focus, summoning and
//! hiding. The bubble webview owns its own stack of recent conversations; the
//! backend only decides *when* it appears and forwards events to it.

use lanchat_core::{BubbleSettings, Corner, MessageRecord};
use lanchat_platform::BubbleWindowOptions;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::state::AppState;

const MARGIN: f64 = 24.0;
const EXPANDED_SIZE: (f64, f64) = (380.0, 190.0);
const COLLAPSED_SIZE: (f64, f64) = (78.0, 78.0);

fn window_size(settings: &BubbleSettings) -> (f64, f64) {
    if settings.collapsed {
        COLLAPSED_SIZE
    } else {
        EXPANDED_SIZE
    }
}

pub fn apply_size(app: &AppHandle, settings: &BubbleSettings) {
    let Some(window) = app.get_webview_window("bubble") else {
        return;
    };
    let (width, height) = window_size(settings);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)));
}

pub fn main_focused(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false)
}

pub fn sync_dock_icon(app: &AppHandle) {
    let main_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if let Err(err) = lanchat_platform::current().set_dock_icon_visible(app, main_visible) {
        tracing::debug!(%err, "dock icon toggle unavailable");
    }
}

pub fn apply_position(app: &AppHandle, settings: &BubbleSettings) {
    let Some(window) = app.get_webview_window("bubble") else {
        return;
    };

    if let (Some(x), Some(y)) = (settings.x, settings.y) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
        return;
    }

    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let (width, height) = window_size(settings);
    let size = tauri::PhysicalSize::new(
        (width * scale).round().max(1.0) as u32,
        (height * scale).round().max(1.0) as u32,
    );
    let mpos = monitor.position();
    let msize = monitor.size();
    let margin = (MARGIN * scale).round() as i32;

    let x = match settings.corner {
        Corner::TopLeft | Corner::BottomLeft => mpos.x + margin,
        Corner::TopRight | Corner::BottomRight => {
            mpos.x + msize.width as i32 - size.width as i32 - margin
        }
    };
    let y = match settings.corner {
        Corner::TopLeft | Corner::TopRight => mpos.y + margin,
        Corner::BottomLeft | Corner::BottomRight => {
            mpos.y + msize.height as i32 - size.height as i32 - margin
        }
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn show(app: &AppHandle, settings: &BubbleSettings, focus: bool) {
    let Some(window) = app.get_webview_window("bubble") else {
        return;
    };
    let platform = lanchat_platform::current();
    let options = BubbleWindowOptions {
        opacity: settings.opacity,
        collapsed: settings.collapsed,
        no_focus: !focus,
    };
    if let Err(err) = platform.configure_bubble(&window, options) {
        tracing::warn!(%err, "could not configure bubble window");
    }
    apply_size(app, settings);
    apply_position(app, settings);

    if focus {
        let _ = window.show();
        let _ = window.set_focus();
    } else if let Err(err) = platform.show_bubble_without_focus(&window) {
        tracing::debug!(%err, "show without focus failed, falling back to show()");
        let _ = window.show();
    }
}

/// Called for every incoming message event. Applies the bubble settings and
/// pushes the message into the bubble webview's stack.
pub async fn handle_incoming(app: &AppHandle, message: &MessageRecord) {
    if message.outgoing || matches!(message.kind, lanchat_core::protocol::MessageKind::System) {
        return;
    }
    let settings = {
        let state = app.state::<AppState>();
        state.node.settings().await
    };
    if !settings.bubble.enabled || settings.dnd {
        return;
    }
    let muted = app
        .state::<AppState>()
        .node
        .store()
        .conversation_muted(message.conv);
    if muted {
        return;
    }
    if settings.pause_bubble_when_app_focused && main_focused(app) {
        return;
    }

    let Some(window) = app.get_webview_window("bubble") else {
        return;
    };
    let already_visible = window.is_visible().unwrap_or(false);
    let _ = window.emit("bubble://incoming", message);

    if already_visible && !settings.bubble.show_on_every_message {
        return;
    }
    show(app, &settings.bubble, false);
}

/// Global hotkey: show the bubble for the last active conversation and focus
/// the reply field. Used from the tray as well.
pub fn summon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (settings, last) = {
            let state = app.state::<AppState>();
            (
                state.node.settings().await,
                state.node.store().last_active_conversation(),
            )
        };
        if !settings.bubble.enabled {
            return;
        }
        if let Some(window) = app.get_webview_window("bubble") {
            let _ = window.emit("bubble://summon", last);
        }
        show(&app, &settings.bubble, true);
    });
}

/// Opens the bubble for a specific conversation (from the main window).
pub fn open_for(app: &AppHandle, conv: Option<uuid::Uuid>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let settings = {
            let state = app.state::<AppState>();
            state.node.settings().await
        };
        if let Some(window) = app.get_webview_window("bubble") {
            let _ = window.emit("bubble://open", conv);
        }
        show(&app, &settings.bubble, true);
    });
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("bubble") {
        let _ = window.hide();
    }
    sync_dock_icon(app);
}

pub fn open_main(app: &AppHandle) {
    let platform = lanchat_platform::current();
    let _ = platform.set_dock_icon_visible(app, true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
