# Pulse Setup

The installer and uninstaller for Pulse. It is a small Tauri app that uses **Pulse's own stylesheet**
(`../src/styles/index.css`: design tokens, liquid glass, aurora, scrollbars, ripples), so it looks like the
player because it is built from the same CSS.

One exe does two jobs: `Pulse-Setup-<version>.exe` installs, and the copy it leaves in the install folder
(`uninstall.exe`) uninstalls.

## Build

From the **project root** (not this folder):

```
npm run installer
```

That runs `scripts/build-installer.ps1`, which builds Pulse, copies `pulse.exe` into
`installer/src-tauri/payload/`, builds this app around it, and writes `dist-installer\Pulse-Setup-<version>.exe`.
The first run takes a while (two Rust builds). Prerequisites are the same as the player's.

## Preview the UI without building any Rust

```
cd installer
npm install
npm run dev
```

Open http://localhost:1421 in a browser. A built-in mock backend plays out the whole wizard with a simulated
install. Add `?uninstall` to the address to preview the uninstaller. Nothing is installed in preview mode.
(`npm run tauri dev` runs the real app; it uses a placeholder payload, so don't use it to test a real install.)

## The wizard

Welcome (animated heartbeat logo) → License (must accept) → Location → Appearance → Options → Installing → Finish

- **Location**: defaults to `%LOCALAPPDATA%\Programs\Pulse` (no administrator rights needed). If Pulse is already
  installed, that folder is suggested instead and setup updates it in place. Browsing to a folder not named
  "Pulse" adds a `Pulse` subfolder. The path is validated as you type.
- **Appearance**: pick one of the four colour schemes and Dark / Light / Match Windows. The whole installer
  repaints live. The choice is written to `install-seed.json`; on its first launch Pulse applies it, but only if
  you haven't already chosen a theme inside the app.
- **Options**: desktop shortcut, Start menu shortcut, start with Windows (starts minimized to the tray).
- **Installing**: liquid progress bar driven by real bytes written, plus step checklist. Short pauses between steps
  keep the animation visible on fast machines (disabled in silent mode).
- **Finish**: optional "Launch Pulse now".

## What gets changed on the computer

| Where | What |
|---|---|
| install folder | `pulse.exe`, `uninstall.exe`, `LICENSE.txt`, `install-seed.json` |
| `HKCU\...\Uninstall\Pulse` | the "Apps & features" entry (per-user; no admin) |
| `HKCU\...\Run` value `Pulse` | only if "Start with Windows" was ticked |
| Desktop / Start menu | `Pulse.lnk` (only if ticked) |

The uninstaller removes exactly those. It deletes only the named files (never "everything in the folder"), only
removes shortcuts that point at this installation, and only removes the folder if it ends up empty. Your library,
playlists and settings (`%APPDATA%\app.pulse.player`) are kept unless you tick "Also delete my library…".

## Command line

```
Pulse-Setup-0.2.0.exe /S                          silent install (defaults: Midnight Blue, dark)
Pulse-Setup-0.2.0.exe /S /D=C:\Tools\Pulse        silent install to a folder (/D= must be last; spaces are fine)
Pulse-Setup-0.2.0.exe /S /NODESKTOP /AUTOSTART    skip the desktop shortcut / start with Windows
"<install folder>\uninstall.exe" /S               silent uninstall (keeps your data)
```
Exit code 0 = success, 1 = failure. Both modes append to `%TEMP%\pulse-setup.log`.

## Files

```
installer/
  src/                     React UI (App.tsx wizard, pages.tsx, components/, lib/)
  src-tauri/
    build.rs               compresses payload/pulse.exe into the setup exe
    payload/LICENSE.txt    license text shown in setup (edit this)
    payload/pulse.exe      copied in by the build script (git-ignored)
    src/install.rs         install + uninstall procedures, path validation
    src/winsys.rs          registry, shortcuts, process control (PowerShell), logging
    src/main.rs            commands, command-line handling, silent mode
```

## Things to know

- **Edit `payload/LICENSE.txt`.** The text shipped here is a plain placeholder, not legal advice.
- **SmartScreen.** An unsigned exe makes Windows show "Windows protected your PC" until the file builds reputation.
  A code-signing certificate is the only real fix; sign `Pulse-Setup-<version>.exe` with `signtool` after the build.
- **WebView2.** Setup (like Pulse) needs the WebView2 runtime. It ships with Windows 11 and current Windows 10.
- **Protected folders.** Setup doesn't elevate. Installing under `C:\Program Files` fails with a clear message
  telling the user to choose a folder in their profile.
- Setup leaves a small WebView2 cache in `%LOCALAPPDATA%\app.pulse.setup`; it is safe to delete.
