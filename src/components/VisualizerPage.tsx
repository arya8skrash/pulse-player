import { memo, useEffect, useId, useMemo, useRef, useState, type CSSProperties, type MutableRefObject } from "react";
import { AnimatePresence, motion, motionValue, useAnimationFrame, useMotionValue, useReducedMotion, useSpring, useTransform } from "framer-motion";
import { convertFileSrc } from "@tauri-apps/api/core";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";
import { onEvent } from "../lib/ipc";

export interface VisualizerTrack {
  title?: string;
  artist?: string;
  artPath?: string | null;
  durationMs?: number;
}

export interface VisualizerPageProps {
  current?: VisualizerTrack | null;
  playing?: boolean;
  positionMs?: number;
  toggle?: () => void;
  onClose?: () => void;
}

type Bands = MutableRefObject<number[]>;

const TAU = Math.PI * 2;
const BANDS = 10;
const IDLE = 0.04;

// ------------------------------------------------------------------------------------------
// Real FFT
// ------------------------------------------------------------------------------------------

function useRealSpectrum(active: boolean): Bands {
  const bands = useRef<number[]>(Array(BANDS).fill(IDLE));

  useEffect(() => {
    if (!active) {
      bands.current.fill(IDLE);
      return;
    }
    
    let off: (() => void) | undefined;
    onEvent<number[]>("player:fft", (newBands) => {
      for (let i = 0; i < BANDS; i++) {
        const val = newBands[i] || 0;
        // Increased the attack response from 0.8 to 0.9 for a faster rise.
        const rate = val > bands.current[i] ? 0.9 : 0.15;
        bands.current[i] += (val - bands.current[i]) * rate;
      }
    }).then((f) => { off = f; });

    return () => { off?.(); };
  }, [active]);

  return bands;
}

function levelAt(b: number[], u: number): number {
  const p = Math.min(Math.max(u, 0), 1) * (b.length - 1);
  const i = Math.floor(p);
  const f = p - i;
  const s = f * f * (3 - 2 * f);
  return b[i] * (1 - s) + b[Math.min(i + 1, b.length - 1)] * s;
}

// ------------------------------------------------------------------------------------------
// Orbs
// ------------------------------------------------------------------------------------------

interface OrbCfg {
  a: string; b: string;
  size: number;
  ratio: number;
  ax: number; ay: number;
  band: number; band2: number;
  orbit: number;
  f: [number, number];
  rot: number;
  op: number;
  ph: number;
}

const ORBS: OrbCfg[] = [
  { a: "--aurora-1", b: "--accent", size: 64, ratio: 0.72, ax: 50, ay: 48, band: 0, band2: 1, orbit: 9, f: [0.21, 0.17], rot: 14, op: 0.7, ph: 0 },
  { a: "--aurora-2", b: "--aurora-3", size: 50, ratio: 0.8, ax: 28, ay: 58, band: 2, band2: 3, orbit: 12, f: [0.26, 0.19], rot: -18, op: 0.6, ph: 1.7 },
  { a: "--accent-2", b: "--aurora-4", size: 46, ratio: 0.7, ax: 72, ay: 42, band: 4, band2: 5, orbit: 12, f: [0.19, 0.29], rot: 22, op: 0.6, ph: 3.1 },
  { a: "--accent", b: "--aurora-2", size: 32, ratio: 0.9, ax: 40, ay: 36, band: 6, band2: 7, orbit: 16, f: [0.33, 0.24], rot: -30, op: 0.6, ph: 4.4 },
  { a: "--aurora-3", b: "--accent-2", size: 28, ratio: 0.85, ax: 63, ay: 64, band: 8, band2: 9, orbit: 18, f: [0.37, 0.31], rot: 36, op: 0.65, ph: 5.6 },
];

const Orb = memo(function Orb({ cfg, bands }: { cfg: OrbCfg; bands: Bands }) {
  const x = useMotionValue(0);
  const y = useMotionValue(0);
  const scale = useMotionValue(0.8);
  const opacity = useMotionValue(cfg.op * 0.4);
  const rotate = useMotionValue(0);
  const sx = useSpring(x, { stiffness: 70, damping: 16 });
  const sy = useSpring(y, { stiffness: 70, damping: 16 });
  const ss = useSpring(scale, { stiffness: 120, damping: 12, mass: 0.8 });
  const so = useSpring(opacity, { stiffness: 80, damping: 20 });
  const clock = useRef(cfg.ph * 3);

    useAnimationFrame((_, deltaMs) => {
    const dt = Math.min(deltaMs, 50) / 1000;
    const lo = bands.current[cfg.band];
    const hi = bands.current[cfg.band2];
    const rawLevel = (lo + hi) / 2;
    
    const level = rawLevel < 0.05 ? 0 : rawLevel; 
    const vmin = Math.min(window.innerWidth, window.innerHeight) / 100;

    // Increased the speed multiplier from 3.0 to 6.0 for stronger motion at higher volume.
    clock.current += dt * (6.0 * (level * level));
    const t = clock.current;
    
    x.set(Math.sin(t * TAU * cfg.f[0] + cfg.ph) * cfg.orbit * vmin * level);
    y.set(Math.cos(t * TAU * cfg.f[1] + cfg.ph) * cfg.orbit * vmin * level * 0.7);
    
    scale.set(0.8 + 1.5 * level); // The size now reacts a little more strongly.
    opacity.set(cfg.op * Math.min(1, level * 3));
    rotate.set(rotate.get() + cfg.rot * dt * level);
  });

  const h = cfg.size * cfg.ratio;
  return (
    <motion.div aria-hidden className="pointer-events-none absolute rounded-full"
      style={{
        left: `${cfg.ax}%`, top: `${cfg.ay}%`, width: `${cfg.size}vmin`, height: `${h}vmin`, marginLeft: `${-cfg.size / 2}vmin`, marginTop: `${-h / 2}vmin`,
        x: sx, y: sy, scale: ss, opacity: so, rotate,
        background: `radial-gradient(closest-side, rgb(var(${cfg.a}) / 0.95), rgb(var(${cfg.b}) / 0.45) 55%, transparent 100%)`,
        willChange: "transform, opacity",
      }} />
  );
});

// ------------------------------------------------------------------------------------------
// Ribbons
// ------------------------------------------------------------------------------------------

const VB_W = 1440;
const VB_H = 600;
const SAMPLES = 64;

interface Wave { k: number; s: number; p: number; a: number }
interface Ribbon {
  y: number; amp: number; th: number; shift: number;
  waves: Wave[];
  stops: [string, string, string];
  opacity: number; blur: number; edge: boolean;
}

const RIBBONS: Ribbon[] = [
  { y: 0.5, amp: 0.34, th: 0.34, shift: 0, waves: [{ k: 0.9, s: 0.11, p: 0, a: 1 }, { k: 1.7, s: -0.17, p: 0.3, a: 0.45 }], stops: ["--aurora-1", "--accent", "--aurora-3"], opacity: 0.5, blur: 22, edge: false },
  { y: 0.49, amp: 0.4, th: 0.22, shift: 0.12, waves: [{ k: 1.2, s: -0.13, p: 0.2, a: 1 }, { k: 2.4, s: 0.21, p: 0.6, a: 0.4 }], stops: ["--aurora-2", "--aurora-3", "--accent-2"], opacity: 0.6, blur: 12, edge: false },
  { y: 0.51, amp: 0.3, th: 0.1, shift: -0.1, waves: [{ k: 1.4, s: 0.15, p: 0.9, a: 1 }, { k: 2.8, s: -0.23, p: 0.4, a: 0.35 }], stops: ["--accent-2", "--accent", "--aurora-2"], opacity: 0.85, blur: 4, edge: true },
  { y: 0.47, amp: 0.36, th: 0.03, shift: 0.2, waves: [{ k: 1.1, s: -0.2, p: 0.15, a: 1 }, { k: 2.2, s: 0.12, p: 0.7, a: 0.4 }], stops: ["--accent", "--accent-2", "--accent"], opacity: 0.95, blur: 0, edge: true },
  { y: 0.53, amp: 0.28, th: 0.025, shift: -0.2, waves: [{ k: 1.6, s: 0.17, p: 0.55, a: 1 }, { k: 3.1, s: -0.1, p: 0.2, a: 0.3 }], stops: ["--aurora-3", "--accent-2", "--aurora-1"], opacity: 0.8, blur: 0, edge: true },
];
const BLURS = Array.from(new Set(RIBBONS.map((r) => r.blur).filter((b) => b > 0)));

type Pt = [number, number];

function smooth(pts: Pt[], move: boolean): string {
  const f = (n: number) => n.toFixed(1);
  let d = `${move ? "M" : "L"}${f(pts[0][0])} ${f(pts[0][1])}`;
  for (let i = 1; i < pts.length - 1; i++) {
    d += ` Q${f(pts[i][0])} ${f(pts[i][1])} ${f((pts[i][0] + pts[i + 1][0]) / 2)} ${f((pts[i][1] + pts[i + 1][1]) / 2)}`;
  }
  const last = pts[pts.length - 1];
  return `${d} L${f(last[0])} ${f(last[1])}`;
}

function buildRibbon(r: Ribbon, t: number, bands: number[]): { fill: string; line: string } {
  const top: Pt[] = [];
  const bot: Pt[] = [];
  for (let i = 0; i <= SAMPLES; i++) {
    const u = i / SAMPLES;
    const env = Math.pow(Math.sin(Math.PI * u), 0.85);
    const lvl = levelAt(bands, u + r.shift);
    let w = 0;
    for (const wave of r.waves) w += wave.a * Math.sin(TAU * (wave.k * u + wave.s * t + wave.p));
    const centre = VB_H * (r.y + r.amp * (0.2 + 1.0 * lvl) * w * env * 0.5);
    const half = VB_H * r.th * (0.5 + 0.9 * lvl) * (0.6 + 0.4 * Math.sin(TAU * (1.4 * u - 0.12 * t + r.waves[0].p))) * env * 0.5;
    const x = u * VB_W;
    top.push([x, centre - half]);
    bot.push([x, centre + half]);
  }
  const line = smooth(top, true);
  return { line, fill: `${line} ${smooth([...bot].reverse(), false)} Z` };
}

const Ribbons = memo(function Ribbons({ bands }: { bands: Bands }) {
  const uid = useId().replace(/:/g, "");
  const paths = useMemo(() => RIBBONS.map(() => ({ fill: motionValue(""), line: motionValue("") })), []);
  const clock = useRef(0);

    useAnimationFrame((_, deltaMs) => {
    const dt = Math.min(deltaMs, 50) / 1000;
    const b = bands.current;
    const mean = b.reduce((s, v) => s + v, 0) / BANDS;
    
    const activeMean = mean < 0.05 ? 0 : mean;

    // Increased the speed multiplier from 3.0 to 6.0.
    clock.current += dt * (6.0 * (activeMean * activeMean));
    
    for (let i = 0; i < RIBBONS.length; i++) {
      const { fill, line } = buildRibbon(RIBBONS[i], clock.current, b);
      paths[i].fill.set(fill);
      paths[i].line.set(line);
    }
  });

  return (
    <svg aria-hidden viewBox={`0 0 ${VB_W} ${VB_H}`} preserveAspectRatio="none" className="pointer-events-none absolute inset-x-0 w-full overflow-visible"
      style={{ top: "46%", height: "78%", transform: "translateY(-50%)" }}>
      <defs>
        {RIBBONS.map((r, i) => (
          <linearGradient key={i} id={`${uid}-g${i}`} gradientUnits="userSpaceOnUse" x1="0" y1="0" x2={VB_W} y2="0">
            <stop offset="0" style={{ stopColor: `rgb(var(${r.stops[0]}))`, stopOpacity: 0 }} />
            <stop offset="0.22" style={{ stopColor: `rgb(var(${r.stops[0]}))`, stopOpacity: 1 }} />
            <stop offset="0.5" style={{ stopColor: `rgb(var(${r.stops[1]}))`, stopOpacity: 1 }} />
            <stop offset="0.78" style={{ stopColor: `rgb(var(${r.stops[2]}))`, stopOpacity: 1 }} />
            <stop offset="1" style={{ stopColor: `rgb(var(${r.stops[2]}))`, stopOpacity: 0 }} />
          </linearGradient>
        ))}
        {BLURS.map((b) => (
          <filter key={b} id={`${uid}-b${b}`} x="-20%" y="-80%" width="140%" height="260%"><feGaussianBlur stdDeviation={b} /></filter>
        ))}
      </defs>
      {RIBBONS.map((r, i) => (
        <g key={i} opacity={r.opacity}>
          <motion.path d={paths[i].fill} fill={`url(#${uid}-g${i})`} filter={r.blur > 0 ? `url(#${uid}-b${r.blur})` : undefined} />
          {r.edge && <motion.path d={paths[i].line} fill="none" stroke={`url(#${uid}-g${i})`} strokeWidth={1.6} strokeLinecap="round" vectorEffect="non-scaling-stroke" />}
        </g>
      ))}
    </svg>
  );
});

// ------------------------------------------------------------------------------------------
// Shared bits
// ------------------------------------------------------------------------------------------

const glass: CSSProperties = {
  background: "linear-gradient(135deg, rgb(var(--glass-tint) / calc(var(--glass-alpha) + 0.05)), rgb(var(--glass-tint) / calc(var(--glass-alpha) - 0.02)))",
  backdropFilter: "blur(var(--blur)) saturate(180%)",
  WebkitBackdropFilter: "blur(var(--blur)) saturate(180%)",
  border: "1px solid rgb(var(--glass-tint) / var(--glass-border))",
  boxShadow: "inset 0 1px 1px rgb(var(--glass-tint) / 0.3), inset 0 -1px 1px rgb(var(--shade) / 0.22), 0 22px 50px -20px rgb(var(--shade) / 0.55)",
};

const SPRING = { type: "spring", stiffness: 420, damping: 30 } as const;

function fmt(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function toUrl(path?: string | null): string | undefined {
  if (!path) return undefined;
  if (/^(https?:|data:|blob:|asset:)/.test(path)) return path;
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

const PlayIcon = () => (
  <svg width="30" height="30" viewBox="0 0 24 24" fill="currentColor" aria-hidden><path d="M8 5.5v13a1 1 0 001.5.86l10.5-6.5a1 1 0 000-1.72L9.5 4.64A1 1 0 008 5.5z" /></svg>
);
const PauseIcon = () => (
  <svg width="30" height="30" viewBox="0 0 24 24" fill="currentColor" aria-hidden><rect x="6.5" y="5" width="4" height="14" rx="1.2" /><rect x="13.5" y="5" width="4" height="14" rx="1.2" /></svg>
);
const CloseIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden><path d="M6 6l12 12M18 6L6 18" /></svg>
);

// ------------------------------------------------------------------------------------------
// Album art
// ------------------------------------------------------------------------------------------

function ArtFace({ src }: { src?: string }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);

  if (src && !failed) {
    return <img src={src} alt="" draggable={false} onError={() => setFailed(true)} className="h-full w-full rounded-[26px] object-cover" />;
  }
  return (
    <div aria-hidden className="relative grid h-full w-full place-items-center overflow-hidden rounded-[26px]"
      style={{ background: "radial-gradient(120% 120% at 18% 8%, rgb(var(--aurora-1) / 0.95), transparent 60%), radial-gradient(120% 120% at 92% 100%, rgb(var(--aurora-2) / 0.85), transparent 55%), rgb(var(--aurora-3) / 0.6)" }}>
      <div className="absolute inset-0" style={{ background: "linear-gradient(135deg, rgb(var(--glass-tint) / 0.22), transparent 45%, rgb(var(--shade) / 0.14))" }} />
      <svg viewBox="0 0 64 64" className="relative w-1/2" style={{ color: "rgb(var(--text) / 0.9)", filter: "drop-shadow(0 0 14px rgb(var(--accent) / 0.8))" }}>
        <path d="M6 34h12l6-14 9 28 7-20 3 6h15" fill="none" stroke="currentColor" strokeWidth="3.6" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

const ArtCard = memo(function ArtCard({ src, trackKey, bands }: { src?: string; trackKey: string; bands: Bands }) {
  const bass = useMotionValue(0);
  const sBass = useSpring(bass, { stiffness: 140, damping: 14 });
  const haloScale = useTransform(sBass, [0, 1], [0.92, 1.2]);
  const haloOpacity = useTransform(sBass, [0, 1], [0.4, 0.95]);
  useAnimationFrame(() => bass.set(bands.current[0] * 0.6 + bands.current[1] * 0.4));

  return (
    <div className="relative" style={{ width: "min(38vh, 36vw, 400px)", aspectRatio: "1 / 1" }}>
      <motion.div aria-hidden className="absolute -inset-[18%] rounded-full"
        style={{ scale: haloScale, opacity: haloOpacity, background: "radial-gradient(closest-side, rgb(var(--accent) / 0.5), rgb(var(--aurora-2) / 0.18) 60%, transparent)", filter: "blur(30px)" }} />
      <AnimatePresence mode="wait" initial>
        <motion.div key={trackKey} className="absolute inset-0 p-2" style={{ ...glass, borderRadius: 34, boxShadow: `${glass.boxShadow}, 0 40px 90px -34px rgb(var(--accent) / 0.55)` }}
          initial={{ opacity: 0, scale: 1.05, filter: "blur(10px)" }} animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }} exit={{ opacity: 0, scale: 0.96, filter: "blur(8px)" }}
          transition={{ ...SPRING, opacity: { duration: 0.25 } }}>
          <ArtFace src={src} />
        </motion.div>
      </AnimatePresence>
    </div>
  );
});

// ------------------------------------------------------------------------------------------
// Controls
// ------------------------------------------------------------------------------------------

function ProgressBar({ positionMs, durationMs }: { positionMs?: number; durationMs?: number }) {
  const storePos = usePlayer((s) => s.positionMs);
  const storeDur = usePlayer((s) => s.durationMs);
  const { seek } = usePlayer.getState();
  
  const pos = positionMs ?? storePos;
  const dur = durationMs && durationMs > 0 ? durationMs : storeDur;
  
  const [drag, setDrag] = useState<number | null>(null);
  const shown = drag ?? Math.min(pos, dur);
  const pct = dur > 0 ? Math.min(100, Math.max(0, (shown / dur) * 100)) : 0;
  
  const barRef = useRef<HTMLDivElement>(null);

  const handleSeek = (clientX: number) => {
    if (!barRef.current || dur === 0) return;
    const rect = barRef.current.getBoundingClientRect();
    const newPct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    setDrag(newPct * dur);
  };

  const commit = () => {
    if (drag !== null) {
      void seek(drag);
      setDrag(null);
    }
  };

  return (
    <div className="flex w-full items-center gap-4 text-sm tabular-nums select-none" style={{ color: "rgb(var(--text-dim))" }}>
      <span className="w-11 text-right">{fmt(shown)}</span>
      
      {/* Interactive progress bar (click and drag) */}
      <div
        ref={barRef}
        role="slider"
        tabIndex={0}
        aria-valuemin={0}
        aria-valuemax={dur}
        aria-valuenow={Math.round(shown)}
        className="relative h-1.5 flex-1 cursor-pointer rounded-full group"
        style={{ background: "rgb(var(--glass-tint) / 0.18)" }}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          handleSeek(e.clientX);
        }}
        onPointerMove={(e) => {
          if (e.currentTarget.hasPointerCapture(e.pointerId)) {
            handleSeek(e.clientX);
          }
        }}
        onPointerUp={(e) => {
          e.currentTarget.releasePointerCapture(e.pointerId);
          commit();
        }}
      >
        <div
          className="absolute inset-y-0 left-0 rounded-full"
          style={{
            width: `${pct}%`,
            transition: drag !== null ? "none" : "width 0.1s linear",
            background: "linear-gradient(90deg, rgb(var(--accent)), rgb(var(--accent-2)))",
            boxShadow: "0 0 14px rgb(var(--accent) / 0.7)",
          }}
        >
          <span
            className="absolute -right-1.5 top-1/2 h-3 w-3 -translate-y-1/2 rounded-full transition-transform group-hover:scale-125"
            style={{
              background: "rgb(var(--text))",
              boxShadow: "0 0 12px rgb(var(--accent) / 0.9)",
            }}
          />
        </div>
      </div>
      
      <span className="w-11">{fmt(dur)}</span>
    </div>
  );
}

const PlayButton = memo(function PlayButton({ playing, disabled, onToggle, reduced }: { playing: boolean; disabled: boolean; onToggle: () => void; reduced: boolean }) {
  return (
    <div className="relative grid place-items-center">
      {playing && !reduced && (
        <motion.span aria-hidden className="absolute h-[76px] w-[76px] rounded-full" style={{ border: "1px solid rgb(var(--accent) / 0.55)" }}
          animate={{ scale: [1, 1.5], opacity: [0.6, 0] }} transition={{ duration: 2.2, repeat: Infinity, ease: "easeOut" }} />
      )}
      <motion.button type="button" aria-label={playing ? "Pause" : "Play"} disabled={disabled} onClick={onToggle}
        whileHover={{ scale: 1.07 }} whileTap={{ scale: 0.92 }} transition={{ type: "spring", stiffness: 500, damping: 24 }}
        className="relative grid h-[76px] w-[76px] place-items-center rounded-full disabled:opacity-40"
        style={{ ...glass, borderRadius: 9999, color: "rgb(var(--text))", background: `linear-gradient(135deg, rgb(var(--accent) / 0.38), rgb(var(--accent-2) / 0.16)), ${glass.background}`, boxShadow: `${glass.boxShadow}, 0 10px 44px -8px rgb(var(--accent) / 0.6)` }}>
        <AnimatePresence mode="wait" initial={false}>
          <motion.span key={playing ? "pause" : "play"} className="grid place-items-center" initial={{ scale: 0.5, opacity: 0, rotate: -25 }} animate={{ scale: 1, opacity: 1, rotate: 0 }} exit={{ scale: 0.5, opacity: 0, rotate: 25 }} transition={{ type: "spring", stiffness: 600, damping: 28 }}>
            {playing ? <PauseIcon /> : <PlayIcon />}
          </motion.span>
        </AnimatePresence>
      </motion.button>
    </div>
  );
});

// ------------------------------------------------------------------------------------------
// Page
// ------------------------------------------------------------------------------------------

export default function VisualizerPage(props: VisualizerPageProps) {
  const storeCurrent = usePlayer((s) => s.current);
  const storePlaying = usePlayer((s) => s.playing);
  const storeToggle = usePlayer((s) => s.toggle);
  const closeNowPlaying = useUi((s) => s.setNowPlaying);

  const current = props.current !== undefined ? props.current : storeCurrent;
  const playing = props.playing ?? storePlaying;
  const toggle = props.toggle ?? storeToggle;
  const onClose = props.onClose ?? (() => closeNowPlaying(false));

  const reduced = !!useReducedMotion();
  const bands = useRealSpectrum(playing && !reduced);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const title = current?.title || "Nothing playing";
  const artist = current?.artist || (current ? "Unknown artist" : "Pick a song to start the flow");
  const trackKey = current ? `${current.title}|${current.artist}|${current.artPath ?? ""}` : "none";

  return (
    <motion.section role="region" aria-label="Pulse Flow visualizer" className="relative flex h-full w-full flex-col items-center overflow-hidden"
      style={{ background: "rgb(var(--bg-base))", color: "rgb(var(--text))" }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.5 }}>
      <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
        {ORBS.map((cfg, i) => <Orb key={i} cfg={cfg} bands={bands} />)}
        <div className="absolute inset-0" style={{ background: "radial-gradient(ellipse at 50% 45%, transparent 38%, rgb(var(--shade) / 0.4) 100%)" }} />
      </div>

      <Ribbons bands={bands} />

      <motion.button type="button" aria-label="Close visualizer" onClick={onClose} whileHover={{ scale: 1.08, rotate: 90 }} whileTap={{ scale: 0.9 }}
        transition={{ type: "spring", stiffness: 420, damping: 22 }} className="absolute right-6 top-6 z-20 grid h-11 w-11 place-items-center rounded-full"
        style={{ ...glass, borderRadius: 9999, color: "rgb(var(--text))" }}>
        <CloseIcon />
      </motion.button>

      <div className="relative z-10 flex min-h-0 flex-1 flex-col items-center justify-center gap-7 px-8 pt-8">
        <ArtCard src={toUrl(current?.artPath)} trackKey={trackKey} bands={bands} />
        <AnimatePresence mode="wait" initial>
          <motion.div key={trackKey} className="max-w-[min(80vw,820px)] text-center" initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }} transition={{ ...SPRING, opacity: { duration: 0.25 } }}>
            <h1 className="truncate font-semibold leading-tight tracking-tight" style={{ fontSize: "clamp(1.75rem, 3.4vw, 3.25rem)", textShadow: "0 0 44px rgb(var(--accent) / 0.4)" }}>{title}</h1>
            <p className="mt-1.5 truncate text-lg" style={{ color: "rgb(var(--text-dim))" }}>{artist}</p>
          </motion.div>
        </AnimatePresence>
      </div>

      <div className="relative z-10 w-full max-w-2xl px-6 pb-8 pt-4">
        <div className="flex flex-col items-center gap-5 px-8 py-6" style={{ ...glass, borderRadius: 32 }}>
          <PlayButton playing={playing} disabled={!current} onToggle={() => void toggle()} reduced={reduced} />
          <ProgressBar positionMs={props.positionMs} durationMs={current?.durationMs} />
        </div>
      </div>
    </motion.section>
  );
}