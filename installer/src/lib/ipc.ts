import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export interface Context {
  mode: "install" | "uninstall";
  version: string;
  defaultDir: string;
  existingDir: string | null;
  license: string;
  payloadMb: number;
  dataDirs: string[];
  installedDir: string | null;
}
export interface Validation { ok: boolean; message: string; warning: string | null; normalized: string }
export interface InstallOptions {
  dir: string;
  desktopShortcut: boolean;
  startMenuShortcut: boolean;
  autostart: boolean;
  scheme: string;
  mode: string;
}
export interface Progress { percent: number; step: string; detail: string }
export interface Done { ok: boolean; error: string | null }

interface Api {
  getContext(): Promise<Context>;
  validateDir(path: string): Promise<Validation>;
  startInstall(opts: InstallOptions): Promise<void>;
  startUninstall(deleteData: boolean): Promise<void>;
  launchApp(dir: string): Promise<void>;
  exitApp(): Promise<void>;
}

const real: Api = {
  getContext: () => invoke<Context>("get_context"),
  validateDir: (path) => invoke<Validation>("validate_dir", { path }),
  startInstall: (opts) => invoke<void>("start_install", { opts }),
  startUninstall: (deleteData) => invoke<void>("start_uninstall", { deleteData }),
  launchApp: (dir) => invoke<void>("launch_app", { dir }),
  exitApp: () => invoke<void>("exit_app"),
};

// ---------------------------------------------------------------------------------------------
// Browser preview: `npm run dev` and open http://localhost:1421 to see the whole wizard without
// building any Rust. Add ?uninstall to preview the uninstaller. Nothing is actually installed.
// ---------------------------------------------------------------------------------------------
const bus = new EventTarget();
const fire = (name: string, detail: unknown) => bus.dispatchEvent(new CustomEvent(name, { detail }));

function simulate(labels: [number, string, string][]) {
  let pct = 0;
  const timer = setInterval(() => {
    pct = Math.min(100, pct + 1.6);
    const [, step, detail] = [...labels].reverse().find(([from]) => pct >= from) ?? labels[0];
    fire("setup:progress", { percent: pct, step, detail } satisfies Progress);
    if (pct >= 100) {
      clearInterval(timer);
      setTimeout(() => fire("setup:done", { ok: true, error: null } satisfies Done), 400);
    }
  }, 60);
}

const mock: Api = {
  getContext: async () => ({
    mode: new URLSearchParams(location.search).has("uninstall") ? "uninstall" : "install",
    version: "0.2.0",
    defaultDir: "C:\\Users\\you\\AppData\\Local\\Programs\\Pulse",
    existingDir: null,
    license: "PULSE SOFTWARE LICENSE\n\n(Preview mode — the real license text is shown inside the installer.)\n\n".repeat(8),
    payloadMb: 14.2,
    dataDirs: ["C:\\Users\\you\\AppData\\Roaming\\app.pulse.player"],
    installedDir: "C:\\Users\\you\\AppData\\Local\\Programs\\Pulse",
  }),
  validateDir: async (path) => {
    const ok = /^[a-zA-Z]:\\.+/.test(path.trim());
    return { ok, message: ok ? "Looks good." : "Enter a full path, for example C:\\Users\\you\\Pulse.", warning: /program files/i.test(path) ? "This location usually needs administrator rights." : null, normalized: path.trim() };
  },
  startInstall: async () =>
    simulate([[0, "Preparing", "Checking the install folder"], [6, "Closing Pulse", "Making sure no older copy is running"], [10, "Installing Pulse", "Unpacking the player"], [78, "Adding files", "License, uninstaller and your theme"], [84, "Creating shortcuts", "Desktop and Start menu"], [93, "Registering with Windows", "Apps & features entry and startup option"], [100, "All done", "Pulse is ready"]]),
  startUninstall: async () =>
    simulate([[0, "Closing Pulse", "Making sure it isn't running"], [25, "Removing shortcuts", "Desktop and Start menu"], [45, "Updating Windows", "Startup entry and Apps & features"], [65, "Removing files", "Only the files Pulse installed"], [96, "Finishing up", "Cleaning up the uninstaller"], [100, "Removed", "Pulse has been uninstalled"]]),
  launchApp: async () => {},
  exitApp: async () => {},
};

export const api: Api = IS_TAURI ? real : mock;

export function onEvent<T>(name: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  if (IS_TAURI) return listen<T>(name, (e) => cb(e.payload));
  const handler = (e: Event) => cb((e as CustomEvent<T>).detail);
  bus.addEventListener(name, handler);
  return Promise.resolve(() => bus.removeEventListener(name, handler));
}

export async function pickFolder(defaultPath?: string): Promise<string | null> {
  if (!IS_TAURI) return "D:\\Apps\\Pulse";
  const picked = await open({ directory: true, multiple: false, defaultPath });
  return typeof picked === "string" ? picked : null;
}

export async function minimizeWindow() {
  if (!IS_TAURI) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}
