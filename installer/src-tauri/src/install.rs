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

//! The install and uninstall procedures. Both report progress through a callback so the same code
//! drives the animated UI and the silent (/S) mode.

use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    thread::sleep,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::winsys;

static PAYLOAD_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pulse.exe.gz"));
include!(concat!(env!("OUT_DIR"), "/payload_meta.rs")); // pub const PAYLOAD_SIZE: u64

pub const LICENSE: &str = include_str!("../payload/LICENSE.txt");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_ID: &str = "app.pulse.player"; // must match tauri.conf.json of Pulse

const SCHEMES: &[&str] = &["midnight", "amethyst", "ocean", "rose", "emerald", "slate", "copper", "twilight"];
const MODES: &[&str] = &["dark", "light", "system"];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOptions {
    pub dir: String,
    pub desktop_shortcut: bool,
    pub start_menu_shortcut: bool,
    pub autostart: bool,
    pub scheme: String,
    pub mode: String,
}

#[derive(Clone, Serialize)]
pub struct Progress {
    pub percent: f32,
    pub step: String,
    pub detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub ok: bool,
    pub message: String,
    pub warning: Option<String>,
    pub normalized: String,
}

type Report<'a> = &'a dyn Fn(Progress);

fn emit(report: Report, percent: f32, step: &str, detail: &str) {
    report(Progress { percent, step: step.into(), detail: detail.into() });
}

/// Short pauses so the progress animation is visible on fast machines. Disabled in silent mode.
fn nap(pace: bool, ms: u64) {
    if pace {
        sleep(Duration::from_millis(ms));
    }
}

// ------------------------------------------------------------------ paths

pub fn default_dir() -> String {
    env::var("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("Programs").join("Pulse").to_string_lossy().to_string())
        .unwrap_or_else(|_| r"C:\Pulse".to_string())
}

/// Folders Pulse stores your library/settings in (removed only if you tick "delete my data").
pub fn data_dirs() -> Vec<PathBuf> {
    ["APPDATA", "LOCALAPPDATA"]
        .iter()
        .filter_map(|v| env::var(v).ok())
        .map(|base| PathBuf::from(base).join(APP_ID))
        .collect()
}

pub fn normalize(raw: &str) -> String {
    let t = raw.trim().trim_matches('"').replace('/', "\\");
    let t = t.trim_end_matches('\\');
    if t.len() == 2 && t.ends_with(':') {
        format!("{t}\\")
    } else {
        t.to_string()
    }
}

pub fn validate_dir(raw: &str) -> Validation {
    let n = normalize(raw);
    let fail = |msg: &str| Validation { ok: false, message: msg.into(), warning: None, normalized: n.clone() };

    if n.is_empty() {
        return fail("Choose where to install Pulse.");
    }
    let b = n.as_bytes();
    if !(b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\') {
        return fail("Enter a full path, for example C:\\Users\\you\\Pulse.");
    }
    if b.len() == 3 {
        return fail("Pick a folder, not the root of a drive.");
    }
    if n.len() > 200 {
        return fail("That path is too long.");
    }
    if n[2..].chars().any(|c| "<>:\"|?*".contains(c)) {
        return fail("The path contains characters Windows doesn't allow.");
    }

    let p = Path::new(&n);
    match p.ancestors().find(|a| a.exists()) {
        None => return fail("That drive wasn't found."),
        Some(a) if !a.is_dir() => return fail("Part of that path is a file, not a folder."),
        Some(_) => {}
    }

    let lower = n.to_lowercase();
    let protected = ["ProgramFiles", "ProgramFiles(x86)", "windir"]
        .iter()
        .filter_map(|v| env::var(v).ok())
        .any(|root| lower.starts_with(&root.to_lowercase()));

    let warning = if protected {
        Some("This location usually needs administrator rights. If setup can't write there, pick a folder in your user profile.".to_string())
    } else if p.is_dir()
        && !p.join("pulse.exe").exists()
        && fs::read_dir(p).map(|mut d| d.next().is_some()).unwrap_or(false)
    {
        Some("This folder already contains other files. Pulse only adds and removes its own files.".to_string())
    } else {
        None
    };

    Validation { ok: true, message: "Looks good.".into(), warning, normalized: n }
}

// ------------------------------------------------------------------ install

fn write_payload(tmp: &Path, report: Report, pace: bool) -> Result<(), String> {
    let file = File::create(tmp).map_err(|e| format!("Couldn't write {}: {e}", tmp.display()))?;
    let mut out = BufWriter::new(file);
    let mut decoder = flate2::read::GzDecoder::new(PAYLOAD_GZ);
    let mut buf = vec![0u8; 256 * 1024];
    let mut written: u64 = 0;

    loop {
        let n = decoder.read(&mut buf).map_err(|e| format!("The installer data is damaged: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| format!("Couldn't write to disk: {e}"))?;
        written += n as u64;
        let frac = written as f32 / PAYLOAD_SIZE.max(1) as f32;
        emit(report, 10.0 + 65.0 * frac.min(1.0), "Installing Pulse", "Unpacking the player");
        nap(pace, 18);
    }

    let file = out.into_inner().map_err(|e| format!("Couldn't write to disk: {e}"))?;
    file.sync_all().map_err(|e| format!("Couldn't write to disk: {e}"))?;
    if written != PAYLOAD_SIZE {
        return Err("The installer data is damaged (size mismatch). Download Pulse Setup again.".into());
    }
    Ok(())
}

pub fn install(opts: &InstallOptions, report: Report, pace: bool) -> Result<(), String> {
    let v = validate_dir(&opts.dir);
    if !v.ok {
        return Err(v.message);
    }
    let dir = PathBuf::from(&v.normalized);
    winsys::log(&format!("install -> {}", dir.display()));

    emit(report, 2.0, "Preparing", "Checking the install folder");
    fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    let probe = dir.join(".pulse-write-test");
    fs::write(&probe, b"x").map_err(|e| {
        format!("Setup can't write to {} ({e}). Choose a folder in your user profile instead.", dir.display())
    })?;
    let _ = fs::remove_file(&probe);
    nap(pace, 350);

    emit(report, 6.0, "Closing Pulse", "Making sure no older copy is running");
    winsys::stop_pulse(&dir);
    nap(pace, 250);

    let exe = dir.join("pulse.exe");
    let tmp = dir.join("pulse.exe.new");
    if let Err(e) = write_payload(&tmp, report, pace) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, &exe) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Couldn't replace pulse.exe ({e}). Close Pulse and try again."));
    }

    emit(report, 78.0, "Adding files", "License, uninstaller and your theme");
    fs::write(dir.join("LICENSE.txt"), LICENSE).map_err(|e| format!("Couldn't write the license file: {e}"))?;

    let scheme = if SCHEMES.contains(&opts.scheme.as_str()) { opts.scheme.as_str() } else { "midnight" };
    let mode = if MODES.contains(&opts.mode.as_str()) { opts.mode.as_str() } else { "dark" };
    let seed = serde_json::json!({ "scheme": scheme, "mode": mode, "version": VERSION });
    fs::write(dir.join("install-seed.json"), seed.to_string()).map_err(|e| format!("Couldn't write settings: {e}"))?;

    let me = env::current_exe().map_err(|e| e.to_string())?;
    let uninstaller = dir.join("uninstall.exe");
    if me != uninstaller {
        fs::copy(&me, &uninstaller).map_err(|e| format!("Couldn't create the uninstaller: {e}"))?;
    }
    nap(pace, 300);

    emit(report, 84.0, "Creating shortcuts", "Desktop and Start menu");
    if let Err(e) = winsys::create_shortcuts(&dir, &exe, opts.desktop_shortcut, opts.start_menu_shortcut) {
        // Not fatal: Pulse still works, the user just won't have the shortcut.
        winsys::log(&format!("shortcuts failed: {e}"));
    }
    nap(pace, 350);

    emit(report, 93.0, "Registering with Windows", "Apps & features entry and startup option");
    let size_kb = ((PAYLOAD_SIZE + fs::metadata(&uninstaller).map(|m| m.len()).unwrap_or(0)) / 1024) as u32;
    winsys::register_uninstall(&dir, &exe, VERSION, size_kb).map_err(|e| format!("Couldn't register Pulse with Windows: {e}"))?;
    winsys::set_autostart(&exe, opts.autostart).map_err(|e| format!("Couldn't set the startup option: {e}"))?;
    
    let _ = winsys::register_file_associations(&exe);
    
    nap(pace, 350);

    emit(report, 100.0, "All done", "Pulse is ready");
    nap(pace, 250);
    winsys::log("install finished");
    Ok(())
}

// ------------------------------------------------------------------ uninstall

/// The folder to uninstall: the one this uninstall.exe lives in, or the registered location.
pub fn resolve_uninstall_dir() -> Result<PathBuf, String> {
    let me = env::current_exe().map_err(|e| e.to_string())?;
    if let Some(parent) = me.parent() {
        if parent.join("pulse.exe").exists() {
            return Ok(parent.to_path_buf());
        }
    }
    winsys::existing_install()
        .map(PathBuf::from)
        .ok_or_else(|| "Pulse doesn't appear to be installed on this computer.".to_string())
}

pub fn uninstall(delete_data: bool, report: Report, pace: bool) -> Result<(), String> {
    let dir = resolve_uninstall_dir()?;
    let exe = dir.join("pulse.exe");
    let me = env::current_exe().map_err(|e| e.to_string())?;
    winsys::log(&format!("uninstall <- {}", dir.display()));

    emit(report, 5.0, "Closing Pulse", "Making sure it isn't running");
    winsys::stop_pulse(&dir);
    nap(pace, 400);

    emit(report, 25.0, "Removing shortcuts", "Desktop and Start menu");
    winsys::remove_shortcuts(&exe);
    nap(pace, 400);

    emit(report, 45.0, "Updating Windows", "Startup entry and Apps & features");
    let _ = winsys::set_autostart(&exe, false);
    winsys::unregister_uninstall();
    
    winsys::unregister_file_associations();
    
    nap(pace, 400);

    emit(report, 65.0, "Removing files", "Only the files Pulse installed");
    for name in ["pulse.exe", "pulse.exe.new", "install-seed.json", "LICENSE.txt"] {
        let p = dir.join(name);
        if p.exists() {
            fs::remove_file(&p).map_err(|e| format!("Couldn't remove {} ({e}). Close Pulse and try again.", p.display()))?;
        }
    }
    nap(pace, 400);

    if delete_data {
        emit(report, 82.0, "Deleting your data", "Library, playlists and settings");
        for d in data_dirs() {
            if d.file_name().map(|n| n == APP_ID).unwrap_or(false) && d.is_dir() {
                if let Err(e) = fs::remove_dir_all(&d) {
                    winsys::log(&format!("could not delete {}: {e}", d.display()));
                }
            }
        }
        nap(pace, 400);
    }

    emit(report, 96.0, "Finishing up", "Cleaning up the uninstaller");
    // uninstall.exe is the running process; a helper removes it (and the empty folder) a moment later.
    let uninstaller = dir.join("uninstall.exe");
    winsys::schedule_self_delete(if me == uninstaller { &me } else { &uninstaller }, &dir);
    nap(pace, 300);

    emit(report, 100.0, "Removed", "Pulse has been uninstalled");
    winsys::log("uninstall finished");
    Ok(())
}