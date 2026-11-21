// Pulse - A local-files music player.
// Copyright (C) 2026 Skrash!
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.




#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod install;
mod winsys;

use std::{
    env,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

use install::{InstallOptions, Progress, Validation};

/// True while files are being written; the window can't be closed in that moment.
static BUSY: AtomicBool = AtomicBool::new(false);

/// What the command line asked for.
///   Pulse-Setup.exe /S [/D=C:\path] [/NODESKTOP] [/AUTOSTART]     silent install
///   uninstall.exe [/S]                                            (re)moves Pulse
struct Launch {
    uninstall: bool,
    silent: bool,
    dir: Option<String>,
    no_desktop: bool,
    autostart: bool,
}

fn parse_args() -> Launch {
    let args: Vec<String> = env::args().skip(1).collect();
    let stem = env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mut l = Launch { uninstall: stem == "uninstall", silent: false, dir: None, no_desktop: false, autostart: false };

    for (i, a) in args.iter().enumerate() {
        let lower = a.to_lowercase();
        match lower.as_str() {
            "/s" | "--silent" => l.silent = true,
            "/uninstall" | "--uninstall" => l.uninstall = true,
            "/nodesktop" => l.no_desktop = true,
            "/autostart" => l.autostart = true,
            _ if lower.starts_with("/d=") => {
                // Like NSIS, /D= takes the rest of the line, so paths with spaces work unquoted.
                let mut path = a[3..].to_string();
                for rest in &args[i + 1..] {
                    path.push(' ');
                    path.push_str(rest);
                }
                l.dir = Some(path.trim().trim_matches('"').to_string());
                break;
            }
            _ => {}
        }
    }
    l
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    mode: &'static str,
    version: &'static str,
    default_dir: String,
    existing_dir: Option<String>,
    license: &'static str,
    payload_mb: f64,
    data_dirs: Vec<String>,
    installed_dir: Option<String>,
}

#[derive(Serialize, Clone)]
struct Done {
    ok: bool,
    error: Option<String>,
}

#[tauri::command]
fn get_context(launch: State<Launch>) -> Context {
    let installed_dir = if launch.uninstall {
        install::resolve_uninstall_dir().ok().map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };
    Context {
        mode: if launch.uninstall { "uninstall" } else { "install" },
        version: install::VERSION,
        default_dir: launch.dir.clone().unwrap_or_else(install::default_dir),
        existing_dir: winsys::existing_install(),
        license: install::LICENSE,
        payload_mb: install::PAYLOAD_SIZE as f64 / 1_048_576.0,
        data_dirs: install::data_dirs().iter().map(|p| p.to_string_lossy().to_string()).collect(),
        installed_dir,
    }
}

#[tauri::command]
fn validate_dir(path: String) -> Validation {
    install::validate_dir(&path)
}

fn run_job(app: AppHandle, job: impl FnOnce(&dyn Fn(Progress)) -> Result<(), String> + Send + 'static) {
    thread::spawn(move || {
        BUSY.store(true, Ordering::SeqCst);
        let emitter = app.clone();
        let result = job(&move |p| {
            let _ = emitter.emit("setup:progress", p);
        });
        BUSY.store(false, Ordering::SeqCst);
        if let Err(e) = &result {
            winsys::log(&format!("failed: {e}"));
        }
        let _ = app.emit("setup:done", Done { ok: result.is_ok(), error: result.err() });
    });
}

#[tauri::command]
fn start_install(app: AppHandle, opts: InstallOptions) -> Result<(), String> {
    if BUSY.load(Ordering::SeqCst) {
        return Err("Setup is already running.".into());
    }
    run_job(app, move |report| install::install(&opts, report, true));
    Ok(())
}

#[tauri::command]
fn start_uninstall(app: AppHandle, delete_data: bool) -> Result<(), String> {
    if BUSY.load(Ordering::SeqCst) {
        return Err("Setup is already running.".into());
    }
    run_job(app, move |report| install::uninstall(delete_data, report, true));
    Ok(())
}

#[tauri::command]
fn launch_app(dir: String) -> Result<(), String> {
    let dir = PathBuf::from(install::normalize(&dir));
    winsys::launch(&dir.join("pulse.exe"), &dir)
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    if !BUSY.load(Ordering::SeqCst) {
        app.exit(0);
    }
}

fn run_silent(l: &Launch) -> i32 {
    let quiet = |_p: Progress| {};
    let result = if l.uninstall {
        install::uninstall(false, &quiet, false)
    } else {
        let dir = l.dir.clone().or_else(winsys::existing_install).unwrap_or_else(install::default_dir);
        let opts = InstallOptions {
            dir,
            desktop_shortcut: !l.no_desktop,
            start_menu_shortcut: true,
            autostart: l.autostart,
            scheme: "midnight".into(),
            mode: "dark".into(),
        };
        install::install(&opts, &quiet, false)
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            winsys::log(&format!("silent run failed: {e}"));
            1
        }
    }
}

fn main() {
    let launch = parse_args();
    let silent = launch.silent;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(launch)
        .setup(move |app| {
            if silent {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    let code = run_silent(&handle.state::<Launch>());
                    handle.exit(code);
                });
            } else if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
            }
            Ok(())
        })
        .on_window_event(|_window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if BUSY.load(Ordering::SeqCst) {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_context,
            validate_dir,
            start_install,
            start_uninstall,
            launch_app,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pulse Setup");
}
