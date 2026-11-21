//! Everything that touches Windows: registry, shortcuts, processes, logging.
//! PowerShell is used for shortcuts and process control (no COM crate needed). Values are passed
//! through environment variables, never spliced into the script text, so odd paths can't break it.

use std::{
    fs::OpenOptions,
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use winreg::{enums::*, RegKey};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Pulse";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Appends to %TEMP%\pulse-setup.log (handy when a silent install misbehaves).
pub fn log(msg: &str) {
    if let Ok(tmp) = std::env::var("TEMP") {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(PathBuf::from(tmp).join("pulse-setup.log")) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

fn powershell(script: &str, envs: &[(&str, &str)]) -> Result<(), String> {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().map_err(|e| format!("could not start PowerShell: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn s(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

pub fn stop_pulse(dir: &Path) {
    let script = "$d = $env:PULSE_DIR.TrimEnd('\\') + '\\'; \
        Get-Process -Name pulse -ErrorAction SilentlyContinue | \
        Where-Object { $_.Path -and $_.Path.StartsWith($d, [System.StringComparison]::OrdinalIgnoreCase) } | \
        Stop-Process -Force -ErrorAction SilentlyContinue; \
        Start-Sleep -Milliseconds 600";
    if let Err(e) = powershell(script, &[("PULSE_DIR", &s(dir))]) {
        log(&format!("stop_pulse: {e}"));
    }
}

pub fn create_shortcuts(dir: &Path, exe: &Path, desktop: bool, start_menu: bool) -> Result<(), String> {
    if !desktop && !start_menu {
        return Ok(());
    }
    let script = "$ErrorActionPreference = 'Stop'; \
        $sh = New-Object -ComObject WScript.Shell; \
        function Make($path) { $l = $sh.CreateShortcut($path); $l.TargetPath = $env:PULSE_EXE; \
          $l.WorkingDirectory = $env:PULSE_DIR; $l.IconLocation = $env:PULSE_EXE + ',0'; \
          $l.Description = 'Pulse - music player'; $l.Save() }; \
        if ($env:PULSE_DESKTOP -eq '1') { Make (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Pulse.lnk') }; \
        if ($env:PULSE_STARTMENU -eq '1') { Make (Join-Path ([Environment]::GetFolderPath('Programs')) 'Pulse.lnk') }";
    powershell(
        script,
        &[
            ("PULSE_EXE", &s(exe)),
            ("PULSE_DIR", &s(dir)),
            ("PULSE_DESKTOP", if desktop { "1" } else { "0" }),
            ("PULSE_STARTMENU", if start_menu { "1" } else { "0" }),
        ],
    )
}

pub fn remove_shortcuts(exe: &Path) {
    let script = "$ErrorActionPreference = 'SilentlyContinue'; \
        $sh = New-Object -ComObject WScript.Shell; \
        foreach ($base in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) { \
          $f = Join-Path $base 'Pulse.lnk'; \
          if (Test-Path $f) { if ($sh.CreateShortcut($f).TargetPath -eq $env:PULSE_EXE) { Remove-Item $f -Force } } }";
    if let Err(e) = powershell(script, &[("PULSE_EXE", &s(exe))]) {
        log(&format!("remove_shortcuts: {e}"));
    }
}

pub fn register_uninstall(dir: &Path, exe: &Path, version: &str, size_kb: u32) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(UNINSTALL_KEY).map_err(|e| e.to_string())?;
    let uninstaller = dir.join("uninstall.exe");
    let set = |name: &str, v: String| key.set_value(name, &v).map_err(|e| e.to_string());
    set("DisplayName", "Pulse".to_string())?;
    set("DisplayVersion", version.to_string())?;
    set("Publisher", "Pulse".to_string())?;
    set("InstallLocation", s(dir))?;
    set("DisplayIcon", format!("{},0", s(exe)))?;
    set("UninstallString", format!("\"{}\"", s(&uninstaller)))?;
    set("QuietUninstallString", format!("\"{}\" /S", s(&uninstaller)))?;
    key.set_value("NoModify", &1u32).map_err(|e| e.to_string())?;
    key.set_value("NoRepair", &1u32).map_err(|e| e.to_string())?;
    key.set_value("EstimatedSize", &size_kb).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn unregister_uninstall() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey(UNINSTALL_KEY);
}

pub fn existing_install() -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(UNINSTALL_KEY).ok()?;
    let dir: String = key.get_value("InstallLocation").ok()?;
    if Path::new(&dir).join("pulse.exe").exists() {
        Some(dir)
    } else {
        None
    }
}

pub fn set_autostart(exe: &Path, on: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
    if on {
        key.set_value("Pulse", &format!("\"{}\" --minimized", s(exe))).map_err(|e| e.to_string())
    } else {
        let _ = key.delete_value("Pulse");
        Ok(())
    }
}

pub fn schedule_self_delete(me: &Path, dir: &Path) {
    let script = "for ($i = 0; $i -lt 30; $i++) { Start-Sleep -Milliseconds 500; \
        if (Test-Path -LiteralPath $env:PULSE_SELF) { Remove-Item -LiteralPath $env:PULSE_SELF -Force -ErrorAction SilentlyContinue }; \
        if (-not (Test-Path -LiteralPath $env:PULSE_SELF)) { break } }; \
        try { [System.IO.Directory]::Delete($env:PULSE_DIR) } catch { }";
    let spawned = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .env("PULSE_SELF", s(me))
        .env("PULSE_DIR", s(dir))
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn();
    if let Err(e) = spawned {
        log(&format!("schedule_self_delete: {e}"));
    }
}

pub fn launch(exe: &Path, dir: &Path) -> Result<(), String> {
    Command::new(exe)
        .current_dir(dir)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start Pulse: {e}"))
}

// =======================================================================
// 👈 ثبت فایل‌های صوتی در ویندوز (File Associations)
// =======================================================================

pub fn register_file_associations(exe: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let exe_str = s(exe);
    let extensions = ["mp3", "flac", "wav", "ogg", "m4a", "aac"];
    let prog_id = "Pulse.AudioFile";

    // 1. ثبت ProgID
    let (prog_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{}", prog_id)).map_err(|e| e.to_string())?;
    prog_key.set_value("", &"Pulse Audio File".to_string()).map_err(|e| e.to_string())?;

    let (icon_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{}\DefaultIcon", prog_id)).map_err(|e| e.to_string())?;
    icon_key.set_value("", &format!("{},0", exe_str)).map_err(|e| e.to_string())?;

    let (cmd_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{}\shell\open\command", prog_id)).map_err(|e| e.to_string())?;
    cmd_key.set_value("", &format!("\"{}\" \"%1\"", exe_str)).map_err(|e| e.to_string())?;

    // 2. اضافه کردن به OpenWith
    for ext in extensions {
        let (ext_key, _) = hkcu.create_subkey(format!(r"Software\Classes\.{}\OpenWithProgids", ext)).map_err(|e| e.to_string())?;
        ext_key.set_value(prog_id, &"".to_string()).map_err(|e| e.to_string())?;

        let (list_key, _) = hkcu.create_subkey(format!(r"Software\Classes\.{}\OpenWithList", ext)).map_err(|e| e.to_string())?;
        list_key.set_value("pulse.exe", &"".to_string()).map_err(|e| e.to_string())?;
    }

    // 3. ثبت در Applications (برای اینکه توی تنظیمات Default Apps دیده بشه)
    let (app_key, _) = hkcu.create_subkey(r"Software\Classes\Applications\pulse.exe").map_err(|e| e.to_string())?;
    app_key.set_value("FriendlyAppName", &"Pulse".to_string()).map_err(|e| e.to_string())?;

    let (app_cmd_key, _) = hkcu.create_subkey(r"Software\Classes\Applications\pulse.exe\shell\open\command").map_err(|e| e.to_string())?;
    app_cmd_key.set_value("", &format!("\"{}\" \"%1\"", exe_str)).map_err(|e| e.to_string())?;

    // 4. ثبت Capabilities (برای ویندوز ۱۰ و ۱۱)
    let (cap_key, _) = hkcu.create_subkey(r"Software\Pulse\Capabilities").map_err(|e| e.to_string())?;
    cap_key.set_value("ApplicationName", &"Pulse".to_string()).map_err(|e| e.to_string())?;
    cap_key.set_value("ApplicationDescription", &"A modern liquid-glass music player.".to_string()).map_err(|e| e.to_string())?;

    let (assoc_key, _) = hkcu.create_subkey(r"Software\Pulse\Capabilities\FileAssociations").map_err(|e| e.to_string())?;
    for ext in extensions {
        assoc_key.set_value(format!(".{}", ext), &prog_id.to_string()).map_err(|e| e.to_string())?;
    }

    // 5. اضافه کردن به RegisteredApplications
    let (reg_apps_key, _) = hkcu.create_subkey(r"Software\RegisteredApplications").map_err(|e| e.to_string())?;
    reg_apps_key.set_value("Pulse", &r"Software\Pulse\Capabilities".to_string()).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn unregister_file_associations() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey(r"Software\Classes\Pulse.AudioFile");
    let _ = hkcu.delete_subkey(r"Software\Classes\Applications\pulse.exe");
    let _ = hkcu.delete_subkey(r"Software\Pulse");

    let extensions = ["mp3", "flac", "wav", "ogg", "m4a", "aac"];
    for ext in extensions {
        if let Ok(key) = hkcu.open_subkey_with_flags(format!(r"Software\Classes\.{}\OpenWithProgids", ext), KEY_ALL_ACCESS) {
            let _ = key.delete_value("Pulse.AudioFile");
        }
        if let Ok(key) = hkcu.open_subkey_with_flags(format!(r"Software\Classes\.{}\OpenWithList", ext), KEY_ALL_ACCESS) {
            let _ = key.delete_value("pulse.exe");
        }
    }
    if let Ok(key) = hkcu.open_subkey_with_flags(r"Software\RegisteredApplications", KEY_ALL_ACCESS) {
        let _ = key.delete_value("Pulse");
    }
}