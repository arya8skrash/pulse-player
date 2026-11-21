//! Desktop shell integration: system tray, media keys, mini-player window mode.

use std::sync::atomic::Ordering;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalSize, Manager, Size,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::{audio::AudioCmd, state::AppState};

const MIN_W: f64 = 960.0;
const MIN_H: f64 = 600.0;

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        let _ = app.emit("ui:tray-state", false);
    }
}

/// Switch between the full window and a small always-on-top mini player.
pub fn set_mini(app: &AppHandle, on: bool) -> tauri::Result<()> {
    let win = app.get_webview_window("main").ok_or(tauri::Error::WindowNotFound)?;
    let state = app.state::<AppState>();
    {
        let mut saved = state.saved_size.lock().unwrap_or_else(|e| e.into_inner());
        if on {
            if saved.is_none() {
                *saved = Some(win.inner_size()?);
            }
            if win.is_maximized()? {
                win.unmaximize()?;
            }
            win.set_min_size(Some(Size::Logical(LogicalSize::new(340.0, 110.0))))?;
            win.set_size(Size::Logical(LogicalSize::new(400.0, 132.0)))?;
            win.set_always_on_top(true)?;
        } else {
            win.set_always_on_top(false)?;
            win.set_min_size(Some(Size::Logical(LogicalSize::new(MIN_W, MIN_H))))?;
            match saved.take() {
                Some(size) => win.set_size(Size::Physical(size))?,
                None => win.set_size(Size::Logical(LogicalSize::new(1280.0, 800.0)))?,
            }
            win.center()?;
        }
    }
    state.mini.store(on, Ordering::Release);
    app.emit("ui:mini", on)?;
    win.show()?;
    app.emit("ui:tray-state", false)?;
    Ok(())
}

fn send(app: &AppHandle, cmd: AudioCmd) {
    app.state::<AppState>().audio.send(cmd);
}

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();

    // ---- tray ----
    let show = MenuItem::with_id(app, "show", "Show Pulse", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play_pause", "Play / Pause", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "Previous", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next", true, None::<&str>)?;
    let mini = MenuItem::with_id(app, "mini", "Mini player", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &play, &prev, &next, &mini, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Pulse")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "play_pause" => send(app, AudioCmd::Toggle),
            "prev" => send(app, AudioCmd::Prev),
            "next" => send(app, AudioCmd::Next),
            "mini" => {
                let on = !app.state::<AppState>().mini.load(Ordering::Acquire);
                let _ = set_mini(app, on);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;

    // ---- media keys (system-wide) ----
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            match shortcut.key {
                Code::MediaPlayPause => send(app, AudioCmd::Toggle),
                Code::MediaTrackNext => send(app, AudioCmd::Next),
                Code::MediaTrackPrevious => send(app, AudioCmd::Prev),
                Code::MediaStop => send(app, AudioCmd::Stop),
                _ => {}
            }
        })
        .build();
    handle.plugin(plugin)?;
    for code in [Code::MediaPlayPause, Code::MediaTrackNext, Code::MediaTrackPrevious, Code::MediaStop] {
        if let Err(e) = handle.global_shortcut().register(Shortcut::new(None::<Modifiers>, code)) {
            log::warn!("could not register media key {code:?}: {e}");
        }
    }
    Ok(())
}