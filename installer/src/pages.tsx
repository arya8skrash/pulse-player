import { useEffect, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { api, pickFolder, type Context, type Progress, type Validation } from "./lib/ipc";
import { SCHEMES, isLight, type Mode, type Scheme } from "./lib/theme";
import { CheckIcon, CrossIcon, FolderIcon, InfoIcon, PlayIcon, WarnIcon } from "./components/icons";
import { EcgLine, HeartbeatLogo, PulseMark } from "./components/Logo";
import { LiquidProgress } from "./components/LiquidProgress";
import { Btn, Checkbox, Row, Segmented, Switch } from "./components/ui";

const Pane = ({ children, center }: { children: ReactNode; center?: boolean }) => (
  <div className={`h-full overflow-y-auto px-9 py-8 ${center ? "flex flex-col justify-center" : ""}`}>{children}</div>
);
const Header = ({ title, sub }: { title: string; sub?: string }) => (
  <div className="mb-5">
    <h2 className="text-2xl font-bold tracking-tight">{title}</h2>
    {sub && <p className="mt-1 text-sm text-dim">{sub}</p>}
  </div>
);
const Note = ({ tone, children }: { tone: "info" | "warn" | "bad" | "ok"; children: ReactNode }) => {
  const color = { info: "text-accent", warn: "text-amber-300", bad: "text-red-400", ok: "text-emerald-300" }[tone];
  const Icon = tone === "bad" ? CrossIcon : tone === "warn" ? WarnIcon : tone === "ok" ? CheckIcon : InfoIcon;
  return (
    <p className={`mt-3 flex items-start gap-2 text-[13px] leading-snug ${color}`}>
      <Icon size={15} className="mt-[1px] shrink-0" />
      <span className="text-ink/85">{children}</span>
    </p>
  );
};

// ------------------------------------------------------------------ install pages

export function WelcomePage({ ctx }: { ctx: Context }) {
  return (
    <Pane center>
      <div className="flex items-center gap-7">
        <HeartbeatLogo size={104} />
        <div>
          <h1 className="text-4xl font-bold tracking-tight">Welcome to Pulse</h1>
          <p className="mt-2 text-lg text-dim">Your music, beautifully in motion.</p>
        </div>
      </div>
      <EcgLine className="mt-6 h-12 w-full max-w-md" />
      <p className="mt-5 max-w-lg text-sm leading-relaxed text-ink/85">
        Setup will install Pulse on this computer. It takes about a minute and doesn’t need administrator rights.
      </p>
      <ul className="mt-5 flex flex-wrap gap-2">
        {["Real-time DSP engine", "10-band equalizer", "Gapless & crossfade", "Synced lyrics"].map((f) => (
          <li key={f} className="glass-pill px-3.5 py-1.5 text-xs font-medium">{f}</li>
        ))}
      </ul>
      {ctx.existingDir && <Note tone="info">Pulse is already installed. Setup will update it in place and keep your library, playlists and settings.</Note>}
    </Pane>
  );
}

export function LicensePage({ ctx, accepted, setAccepted }: { ctx: Context; accepted: boolean; setAccepted: (v: boolean) => void }) {
  return (
    <Pane>
      <Header title="License agreement" sub="Please read and accept the terms to continue." />
      <div tabIndex={0} aria-label="License text" className="glass-pill h-[230px] overflow-y-auto p-5" style={{ borderRadius: 18 }}>
        <pre className="m-0 whitespace-pre-wrap font-sans text-[13px] leading-relaxed text-ink/85">{ctx.license}</pre>
      </div>
      <div className="mt-5"><Checkbox checked={accepted} onChange={setAccepted}>I accept the terms of the license agreement</Checkbox></div>
    </Pane>
  );
}

export function LocationPage({ ctx, dir, setDir, setOk }: { ctx: Context; dir: string; setDir: (d: string) => void; setOk: (ok: boolean) => void }) {
  const [v, setV] = useState<Validation | null>(null);

  useEffect(() => {
    let cancelled = false;
    setOk(false);
    const t = setTimeout(() => {
      api.validateDir(dir).then((r) => {
        if (cancelled) return;
        setV(r);
        setOk(r.ok);
      }).catch(() => {});
    }, 200);
    return () => { cancelled = true; clearTimeout(t); };
  }, [dir, setOk]);

  const browse = async () => {
    const picked = await pickFolder(v?.normalized || dir);
    if (!picked) return;
    const clean = picked.replace(/[\\/]+$/, "");
    const last = clean.split(/[\\/]/).pop() ?? "";
    // Like most installers: a folder named anything else gets a "Pulse" subfolder, so Pulse never mixes into a folder you already use.
    setDir(last.toLowerCase() === "pulse" ? clean : `${clean}\\Pulse`);
  };

  const needMb = Math.ceil(ctx.payloadMb * 2);
  return (
    <Pane>
      <Header title="Where should Pulse live?" sub="You can keep the suggested folder or choose your own." />
      <div className="flex gap-2">
        <label className="glass-pill glow-hover flex h-11 min-w-0 flex-1 items-center gap-3 px-4 focus-within:shadow-glow">
          <FolderIcon className="shrink-0 text-accent" />
          <input value={dir} onChange={(e) => setDir(e.target.value)} spellCheck={false} aria-label="Install folder"
            className="min-w-0 flex-1 bg-transparent text-sm outline-none" />
        </label>
        <Btn onClick={() => void browse()} className="h-11">Browse…</Btn>
      </div>
      {v && !v.ok && <Note tone="bad">{v.message}</Note>}
      {v?.ok && v.warning && <Note tone="warn">{v.warning}</Note>}
      {v?.ok && !v.warning && <Note tone="ok">{ctx.existingDir && v.normalized.toLowerCase() === ctx.existingDir.toLowerCase() ? "Pulse is installed here — setup will update it." : "This folder is good to go."}</Note>}
      <div className="mt-6 grid grid-cols-2 gap-3 text-sm">
        <div className="glass-pill px-4 py-3" style={{ borderRadius: 16 }}><p className="text-xs text-dim">Space needed</p><p className="font-semibold">Roughly {needMb} MB</p></div>
        <div className="glass-pill px-4 py-3" style={{ borderRadius: 16 }}><p className="text-xs text-dim">Permissions</p><p className="font-semibold">No administrator needed</p></div>
      </div>
    </Pane>
  );
}

export function ThemePage({ scheme, setScheme, mode, setMode }: { scheme: Scheme; setScheme: (s: Scheme) => void; mode: Mode; setMode: (m: Mode) => void }) {
  const light = isLight(mode);
  return (
    <Pane>
      <Header title="Choose your look" sub="Setup changes with every click — what you see is what Pulse will start with." />
      <div className="mb-4"><Segmented label="Mode" value={mode} onChange={setMode} options={[{ value: "dark", label: "Dark" }, { value: "light", label: "Light" }, { value: "system", label: "Match Windows" }]} /></div>
      <div className="grid grid-cols-2 gap-3">
        {SCHEMES.map((s) => {
          const sw = light ? s.light : s.dark;
          const on = scheme === s.id;
          return (
            <motion.button key={s.id} onClick={() => setScheme(s.id)} aria-pressed={on} whileHover={{ y: -2 }} whileTap={{ scale: 0.97 }}
              transition={{ type: "spring", stiffness: 450, damping: 26 }}
              className={`flex items-center gap-3 rounded-2xl border p-3 text-left transition-colors ${on ? "border-accent bg-ink/10 shadow-glow" : "border-ink/10 hover:bg-ink/5"}`}>
              <span className="h-11 w-11 shrink-0 rounded-xl shadow-inner" style={{ background: `linear-gradient(135deg, ${sw[0]}, ${sw[1]})` }} />
              <span><span className="block text-sm font-semibold">{s.name}</span><span className="block text-xs text-dim">{s.tagline}</span></span>
            </motion.button>
          );
        })}
      </div>
      {/* a live miniature of the player, painted with the chosen theme */}
      <div className="glass mt-5 flex items-center gap-4 p-3.5" style={{ borderRadius: 18 }} aria-hidden>
        <div className="grid h-12 w-12 shrink-0 place-items-center overflow-hidden rounded-xl text-ink/85"
          style={{ background: "radial-gradient(120% 120% at 20% 10%, rgb(var(--aurora-1) / 0.9), transparent 60%), radial-gradient(120% 120% at 90% 100%, rgb(var(--aurora-2) / 0.8), transparent 55%), rgb(var(--aurora-3) / 0.55)" }}>
          <PulseMark size={26} />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold">Midnight Drive</p>
          <p className="text-xs text-dim">Pulse Sessions</p>
          <div className="mt-2 h-1.5 rounded-full bg-ink/15"><div className="h-full w-2/5 rounded-full [background:linear-gradient(90deg,rgb(var(--accent)),rgb(var(--accent-2)))]" /></div>
        </div>
        <div className="grid h-10 w-10 shrink-0 place-items-center rounded-full text-white shadow-glow [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]"><PlayIcon size={18} /></div>
      </div>
    </Pane>
  );
}

export interface TaskOptions { desktop: boolean; startMenu: boolean; autostart: boolean }

export function OptionsPage({ ctx, opts, setOpts, dir }: { ctx: Context; opts: TaskOptions; setOpts: (o: TaskOptions) => void; dir: string }) {
  return (
    <Pane>
      <Header title="Almost there" sub="Pick the extras you’d like, then click Install." />
      <Row title="Desktop shortcut" hint="Put a Pulse icon on your desktop."><Switch label="Desktop shortcut" checked={opts.desktop} onChange={(desktop) => setOpts({ ...opts, desktop })} /></Row>
      <Row title="Start menu shortcut" hint="Find Pulse by typing its name in the Start menu."><Switch label="Start menu shortcut" checked={opts.startMenu} onChange={(startMenu) => setOpts({ ...opts, startMenu })} /></Row>
      <Row title="Start Pulse when I sign in" hint="Opens quietly in the system tray, ready for your media keys."><Switch label="Start with Windows" checked={opts.autostart} onChange={(autostart) => setOpts({ ...opts, autostart })} /></Row>
      <div className="glass-pill mt-5 px-4 py-3 text-xs leading-relaxed" style={{ borderRadius: 16 }}>
        <p className="text-dim">Pulse will be installed to</p>
        <p className="truncate font-medium text-ink">{dir}</p>
      </div>
    </Pane>
  );
}

// ------------------------------------------------------------------ shared: working + results

const TIPS = [
  "Press Space to play or pause. Your keyboard’s media keys work everywhere, too.",
  "Drop a .lrc file next to a song and its lyrics scroll in time.",
  "Turn on crossfade in Settings → Playback to blend songs into each other.",
  "The 10-band equalizer has presets for bass, vocals, rock and more.",
  "The mini player keeps your music in a small always-on-top window.",
];

function Tips() {
  const [i, setI] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setI((n) => (n + 1) % TIPS.length), 4200);
    return () => clearInterval(t);
  }, []);
  return (
    <div className="relative h-10 overflow-hidden text-center text-[13px] text-dim" aria-live="off">
      <AnimatePresence mode="wait">
        <motion.p key={i} className="absolute inset-x-0" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }} transition={{ duration: 0.35 }}>
          <span className="font-semibold text-accent">Tip · </span>{TIPS[i]}
        </motion.p>
      </AnimatePresence>
    </div>
  );
}

export function WorkingPage({ progress, labels, title, subtitle, tips }: { progress: Progress; labels: string[]; title: string; subtitle: string; tips: boolean }) {
  const current = Math.max(0, labels.indexOf(progress.step));
  return (
    <Pane center>
      <Header title={title} sub={subtitle} />
      <div className="mt-2"><LiquidProgress percent={progress.percent} /></div>
      <div className="mt-3 flex items-baseline justify-between text-sm">
        <span className="font-medium">{progress.step || labels[0]}<span className="text-dim"> — {progress.detail}</span></span>
        <span className="tabular-nums text-dim">{Math.round(progress.percent)}%</span>
      </div>
      <ul className="mt-6 grid grid-cols-2 gap-x-6 gap-y-2">
        {labels.map((l, i) => (
          <li key={l} className={`flex items-center gap-2.5 text-sm transition-colors ${i < current ? "text-ink/85" : i === current ? "font-medium text-ink" : "text-dim"}`}>
            <span className={`grid h-5 w-5 shrink-0 place-items-center rounded-full ${i < current ? "text-white [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]" : "border border-ink/25"}`}>
              {i < current ? <CheckIcon size={12} /> : i === current ? <motion.span className="h-2 w-2 rounded-full bg-accent" animate={{ opacity: [1, 0.3, 1] }} transition={{ duration: 1.1, repeat: Infinity }} /> : null}
            </span>
            {l}
          </li>
        ))}
      </ul>
      {tips && <div className="mt-8"><Tips /></div>}
    </Pane>
  );
}

function SuccessMark() {
  return (
    <svg width="84" height="84" viewBox="0 0 84 84" fill="none" aria-hidden>
      <defs>
        <linearGradient id="ok-g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" style={{ stopColor: "rgb(var(--accent))" }} /><stop offset="1" style={{ stopColor: "rgb(var(--accent-2))" }} />
        </linearGradient>
      </defs>
      <motion.circle cx="42" cy="42" r="37" stroke="url(#ok-g)" strokeWidth="5" strokeLinecap="round" initial={{ pathLength: 0, rotate: -90 }} animate={{ pathLength: 1 }}
        style={{ originX: "50%", originY: "50%" }} transition={{ duration: 0.7, ease: "easeOut" }} />
      <motion.path d="M26 43l11 11 21-23" stroke="url(#ok-g)" strokeWidth="6" strokeLinecap="round" strokeLinejoin="round" initial={{ pathLength: 0 }} animate={{ pathLength: 1 }}
        transition={{ duration: 0.45, delay: 0.55, ease: "easeOut" }} />
    </svg>
  );
}

export function FinishPage({ dir, launch, setLaunch }: { dir: string; launch: boolean; setLaunch: (v: boolean) => void }) {
  return (
    <Pane center>
      <div className="flex flex-col items-center text-center">
        <SuccessMark />
        <h2 className="mt-5 text-3xl font-bold tracking-tight">Pulse is ready</h2>
        <p className="mt-2 max-w-sm text-sm text-dim">Installed to <span className="text-ink/85">{dir}</span>. Add a music folder and your library fills itself in.</p>
        <div className="mt-7"><Checkbox checked={launch} onChange={setLaunch}>Launch Pulse now</Checkbox></div>
      </div>
    </Pane>
  );
}

export function ErrorPage({ title, error }: { title: string; error: string | null }) {
  return (
    <Pane center>
      <div className="flex flex-col items-center text-center">
        <span className="grid h-16 w-16 place-items-center rounded-full bg-red-500/15 text-red-400 ring-1 ring-red-400/40"><CrossIcon size={28} /></span>
        <h2 className="mt-5 text-2xl font-bold tracking-tight">{title}</h2>
        <p className="glass-pill mt-4 max-w-md px-4 py-3 text-left text-[13px] leading-relaxed text-ink/90" style={{ borderRadius: 16 }}>{error ?? "Something went wrong."}</p>
        <p className="mt-3 text-xs text-dim">Details are saved in %TEMP%\pulse-setup.log</p>
      </div>
    </Pane>
  );
}

// ------------------------------------------------------------------ uninstall pages

export function UninstallConfirmPage({ ctx, deleteData, setDeleteData }: { ctx: Context; deleteData: boolean; setDeleteData: (v: boolean) => void }) {
  return (
    <Pane center>
      <div className="flex items-center gap-6">
        <HeartbeatLogo size={84} />
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Uninstall Pulse?</h1>
          <p className="mt-2 text-sm text-dim">This removes the player from <span className="text-ink/85">{ctx.installedDir ?? "this computer"}</span>.</p>
        </div>
      </div>
      <div className="glass-pill mt-7 px-5 py-4" style={{ borderRadius: 18 }}>
        <Checkbox checked={deleteData} onChange={setDeleteData}>Also delete my library, playlists and settings</Checkbox>
        <p className="ml-[34px] mt-1.5 text-xs leading-relaxed text-dim">
          Leave this off to keep them for a future reinstall. Your music files are never deleted either way.
        </p>
      </div>
      {deleteData && <Note tone="warn">Your play counts, ratings, favorites and playlists will be erased and can’t be recovered.</Note>}
    </Pane>
  );
}

export function UninstallDonePage({ deleteData }: { deleteData: boolean }) {
  return (
    <Pane center>
      <div className="flex flex-col items-center text-center">
        <SuccessMark />
        <h2 className="mt-5 text-3xl font-bold tracking-tight">Pulse has been removed</h2>
        <p className="mt-2 max-w-sm text-sm text-dim">{deleteData ? "Your library and settings were deleted too." : "Your library and settings were kept, in case you come back."} Thanks for listening.</p>
      </div>
    </Pane>
  );
}
