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




import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, MotionConfig, motion, type Variants } from "framer-motion";
import { Aurora } from "./components/Aurora";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { Btn, Modal } from "./components/ui";
import { api, onEvent, type Context, type Done, type Progress } from "./lib/ipc";
import { applyTheme, type Mode, type Scheme } from "./lib/theme";
import {
  ErrorPage, FinishPage, LicensePage, LocationPage, OptionsPage, ThemePage, UninstallConfirmPage, UninstallDonePage, WelcomePage, WorkingPage,
  type TaskOptions,
} from "./pages";

type Page =
  | "welcome" | "license" | "location" | "theme" | "options" | "installing" | "finish" | "error"
  | "u-confirm" | "u-working" | "u-done" | "u-error";

const INSTALL_STEPS = ["Welcome", "License", "Location", "Appearance", "Options", "Installing", "Finish"];
const UNINSTALL_STEPS = ["Uninstall", "Removing", "Done"];
const PAGE_INDEX: Record<Page, number> = {
  welcome: 0, license: 1, location: 2, theme: 3, options: 4, installing: 5, error: 5, finish: 6,
  "u-confirm": 0, "u-working": 1, "u-error": 1, "u-done": 2,
};
const INSTALL_LABELS = ["Preparing", "Closing Pulse", "Installing Pulse", "Adding files", "Creating shortcuts", "Registering with Windows", "All done"];
const UNINSTALL_LABELS = ["Closing Pulse", "Removing shortcuts", "Updating Windows", "Removing files", "Deleting your data", "Finishing up", "Removed"];

const slide: Variants = {
  enter: (d: number) => ({ opacity: 0, x: d * 44 }),
  center: { opacity: 1, x: 0 },
  exit: (d: number) => ({ opacity: 0, x: d * -44 }),
};

export default function App() {
  const [ctx, setCtx] = useState<Context | null>(null);
  const [page, setPage] = useState<Page>("welcome");
  const [dir, setDir] = useState("");
  const [dirOk, setDirOk] = useState(false);
  const [accepted, setAccepted] = useState(false);
  const [scheme, setScheme] = useState<Scheme>("midnight");
  const [mode, setMode] = useState<Mode>("dark");
  const [tasks, setTasks] = useState<TaskOptions>({ desktop: true, startMenu: true, autostart: false });
  const [launch, setLaunch] = useState(true);
  const [deleteData, setDeleteData] = useState(false);
  const [progress, setProgress] = useState<Progress>({ percent: 0, step: "", detail: "" });
  const [error, setError] = useState<string | null>(null);
  const [confirmExit, setConfirmExit] = useState(false);

  const uninstall = ctx?.mode === "uninstall";
  const modeRef = useRef(uninstall);
  modeRef.current = uninstall;

  // Direction of travel for the page transition.
  const prevIndex = useRef(0);
  const dirSign = PAGE_INDEX[page] >= prevIndex.current ? 1 : -1;
  useEffect(() => { prevIndex.current = PAGE_INDEX[page]; }, [page]);

  // Load context once.
  useEffect(() => {
    api.getContext().then((c) => {
      setCtx(c);
      setDir(c.existingDir ?? c.defaultDir);
      setPage(c.mode === "uninstall" ? "u-confirm" : "welcome");
    }).catch((e) => { setError(String(e)); setPage("error"); });
  }, []);

  // Live theme: the whole window repaints as the user explores the Appearance step.
  const firstTheme = useRef(true);
  useEffect(() => {
    applyTheme(scheme, mode, !firstTheme.current);
    firstTheme.current = false;
  }, [scheme, mode]);

  // Progress + completion events from the backend.
  useEffect(() => {
    let cancelled = false;
    const offs: (() => void)[] = [];
    const keep = (p: Promise<() => void>) => void p.then((f) => (cancelled ? f() : offs.push(f)));
    keep(onEvent<Progress>("setup:progress", setProgress));
    keep(onEvent<Done>("setup:done", (d) => {
      setError(d.error);
      if (modeRef.current) setPage(d.ok ? "u-done" : "u-error");
      else setPage(d.ok ? "finish" : "error");
    }));
    return () => { cancelled = true; offs.forEach((f) => f()); };
  }, []);

  const beginInstall = useCallback(() => {
    setError(null);
    setProgress({ percent: 0, step: INSTALL_LABELS[0], detail: "Getting started" });
    setPage("installing");
    api.startInstall({
      dir, desktopShortcut: tasks.desktop, startMenuShortcut: tasks.startMenu, autostart: tasks.autostart, scheme, mode,
    }).catch((e) => { setError(String(e)); setPage("error"); });
  }, [dir, tasks, scheme, mode]);

  const beginUninstall = useCallback(() => {
    setError(null);
    setProgress({ percent: 0, step: UNINSTALL_LABELS[0], detail: "Getting started" });
    setPage("u-working");
    api.startUninstall(deleteData).catch((e) => { setError(String(e)); setPage("u-error"); });
  }, [deleteData]);

  const finish = async () => {
    if (launch && page === "finish") { try { await api.launchApp(dir); } catch { /* the finish page already confirmed the install */ } }
    await api.exitApp();
  };

  const busy = page === "installing" || page === "u-working";
  const terminal = page === "finish" || page === "u-done" || page === "error" || page === "u-error";
  const requestClose = () => {
    if (busy) return;
    if (terminal) void api.exitApp();
    else setConfirmExit(true);
  };

  if (!ctx) {
    return <><Aurora /><div className="grid h-full place-items-center text-sm text-dim">Starting setup…</div></>;
  }

  // What the footer shows on each page.
  const go = (p: Page) => () => setPage(p);
  type Foot = { back?: () => void; next?: { label: string; onClick: () => void; disabled?: boolean; danger?: boolean }; cancel?: boolean; extra?: { label: string; onClick: () => void } };
  const foot: Foot = (() => {
    switch (page) {
      case "welcome": return { next: { label: "Next", onClick: go("license") }, cancel: true };
      case "license": return { back: go("welcome"), next: { label: "Next", onClick: go("location"), disabled: !accepted }, cancel: true };
      case "location": return { back: go("license"), next: { label: "Next", onClick: go("theme"), disabled: !dirOk }, cancel: true };
      case "theme": return { back: go("location"), next: { label: "Next", onClick: go("options") }, cancel: true };
      case "options": return { back: go("theme"), next: { label: "Install", onClick: beginInstall }, cancel: true };
      case "finish": return { next: { label: "Finish", onClick: () => void finish() } };
      case "error": return { back: go("location"), next: { label: "Try again", onClick: beginInstall }, extra: { label: "Close", onClick: () => void api.exitApp() } };
      case "u-confirm": return { next: { label: "Uninstall", onClick: beginUninstall, danger: true }, cancel: true };
      case "u-done": case "u-error": return { next: { label: "Close", onClick: () => void api.exitApp() } };
      default: return {};
    }
  })();

  let content;
  switch (page) {
    case "welcome": content = <WelcomePage ctx={ctx} />; break;
    case "license": content = <LicensePage ctx={ctx} accepted={accepted} setAccepted={setAccepted} />; break;
    case "location": content = <LocationPage ctx={ctx} dir={dir} setDir={setDir} setOk={setDirOk} />; break;
    case "theme": content = <ThemePage scheme={scheme} setScheme={setScheme} mode={mode} setMode={setMode} />; break;
    case "options": content = <OptionsPage ctx={ctx} opts={tasks} setOpts={setTasks} dir={dir} />; break;
    case "installing": content = <WorkingPage progress={progress} labels={INSTALL_LABELS} title="Installing Pulse" subtitle="Sit back — this only takes a moment." tips />; break;
    case "finish": content = <FinishPage dir={dir} launch={launch} setLaunch={setLaunch} />; break;
    case "error": content = <ErrorPage title="Setup couldn’t finish" error={error} />; break;
    case "u-confirm": content = <UninstallConfirmPage ctx={ctx} deleteData={deleteData} setDeleteData={setDeleteData} />; break;
    case "u-working": content = <WorkingPage progress={progress} labels={deleteData ? UNINSTALL_LABELS : UNINSTALL_LABELS.filter((l) => l !== "Deleting your data")} title="Removing Pulse" subtitle="Cleaning up after the music." tips={false} />; break;
    case "u-done": content = <UninstallDonePage deleteData={deleteData} />; break;
    case "u-error": content = <ErrorPage title="Uninstall couldn’t finish" error={error} />; break;
  }

  return (
    <MotionConfig reducedMotion="user">
      <Aurora />
      <div className="flex h-full flex-col">
        <TitleBar title={uninstall ? "Pulse Uninstall" : "Pulse Setup"} onClose={requestClose} closeDisabled={busy} />
        <div className="flex min-h-0 flex-1 gap-4 px-4 pb-4">
          <Sidebar steps={uninstall ? UNINSTALL_STEPS : INSTALL_STEPS} current={PAGE_INDEX[page]} version={ctx.version} uninstall={!!uninstall} />
          <section className="glass flex min-w-0 flex-1 flex-col overflow-hidden">
            <div className="relative min-h-0 flex-1">
              <AnimatePresence mode="popLayout" custom={dirSign} initial={false}>
                <motion.div key={page} custom={dirSign} variants={slide} initial="enter" animate="center" exit="exit"
                  transition={{ type: "spring", stiffness: 380, damping: 36, mass: 0.8 }} className="absolute inset-0 w-full h-full">
                  {content}
                </motion.div>
              </AnimatePresence>
            </div>
            {!busy && (
              <footer className="flex items-center justify-between border-t border-ink/10 px-6 py-4">
                <div className="flex gap-2">
                  {foot.cancel && <Btn variant="ghost" onClick={requestClose}>Cancel</Btn>}
                  {foot.extra && <Btn variant="ghost" onClick={foot.extra.onClick}>{foot.extra.label}</Btn>}
                </div>
                <div className="flex gap-2">
                  {foot.back && <Btn onClick={foot.back}>Back</Btn>}
                  {foot.next && <Btn variant={foot.next.danger ? "danger" : "primary"} disabled={foot.next.disabled} onClick={foot.next.onClick} className="min-w-[110px]">{foot.next.label}</Btn>}
                </div>
              </footer>
            )}
          </section>
        </div>
      </div>

      <Modal open={confirmExit} onClose={() => setConfirmExit(false)} label="Exit setup">
        <h3 className="text-lg font-bold">{uninstall ? "Cancel uninstall?" : "Exit setup?"}</h3>
        <p className="mt-2 text-sm text-dim">{uninstall ? "Pulse will stay installed." : "Pulse hasn’t been installed yet. You can run setup again any time."}</p>
        <div className="mt-6 flex justify-end gap-2">
          <Btn onClick={() => setConfirmExit(false)}>Keep going</Btn>
          <Btn variant="primary" onClick={() => void api.exitApp()}>Exit</Btn>
        </div>
      </Modal>
    </MotionConfig>
  );
}