import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion, useMotionValue, useSpring } from "framer-motion";
import { api, artUrl, type Lyrics } from "../lib/ipc";
import { formatTime } from "../lib/format";
import { rate, toggleFavorite } from "../lib/actions";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";
import { ArtImage } from "./ArtImage";
import { HeartButton, Rating } from "./TrackBits";
import { IconBtn, SeekBar } from "./ui"; // SeekBar kept for potential external use
import { CloseIcon, NextIcon, PauseIcon, PlayIcon, PrevIcon, RepeatIcon, RepeatOneIcon, ShuffleIcon } from "./icons";

function useLyrics(trackId: number | undefined) {
  const [lyrics, setLyrics] = useState<Lyrics | null>(null);
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    setLyrics(null);
    if (trackId === undefined) return;
    let cancelled = false;
    setLoading(true);
    api.getLyrics(trackId).then((l) => !cancelled && setLyrics(l)).catch(() => {}).finally(() => !cancelled && setLoading(false));
    return () => { cancelled = true; };
  }, [trackId]);
  return { lyrics, loading };
}

function LyricsPanel({ trackId }: { trackId: number | undefined }) {
  const { lyrics, loading } = useLyrics(trackId);
  const positionMs = usePlayer((s) => s.positionMs);
  const box = useRef<HTMLDivElement>(null);
  const refs = useRef<(HTMLParagraphElement | null)[]>([]);

  const active = useMemo(() => {
    if (!lyrics?.synced) return -1;
    let idx = -1;
    for (let i = 0; i < lyrics.lines.length; i++) {
      if ((lyrics.lines[i].timeMs ?? Infinity) <= positionMs + 150) idx = i; else break;
    }
    return idx;
  }, [lyrics, positionMs]);

  useEffect(() => {
    const el = refs.current[active];
    const b = box.current;
    if (el && b) b.scrollTo({ top: el.offsetTop - b.clientHeight / 2 + el.clientHeight / 2, behavior: "smooth" });
  }, [active]);

  if (loading) return <p className="text-sm text-dim">Looking for lyrics…</p>;
  if (!lyrics) return <p className="text-sm text-dim">No lyrics found. Put a .lrc file with the same name next to the song to see synced lyrics here.</p>;

  return (
    <div ref={box} className="relative h-full overflow-y-auto pr-2" style={{ maskImage: "linear-gradient(transparent, #000 12%, #000 88%, transparent)" }}>
      <div className="py-[30%]">
        {lyrics.lines.map((l, i) => (
          <p key={i} ref={(el) => { refs.current[i] = el; }}
            className={`py-1.5 text-2xl font-semibold leading-snug transition-all duration-300 ${
              !lyrics.synced ? "text-ink/80" : i === active ? "origin-left scale-105 text-ink" : "text-ink/30"}`}>
            {l.text || "♪"}
          </p>
        ))}
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Wavy Seek Bar - Straight unplayed, wavy played, pill playhead      */
/* ------------------------------------------------------------------ */
const WAVE_LENGTH = 24;      // Length of one full cycle
const AMPLITUDE = 7;         // Height of the wave
const WAVE_HEIGHT = 48;      // Container height

interface WavySeekBarProps {
  positionMs: number;
  durationMs: number;
  onSeek: (ms: number) => void;
}

function WavySeekBar({ positionMs, durationMs, onSeek }: WavySeekBarProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [hovered, setHovered] = useState(false);
  const [width, setWidth] = useState(800);

  const progress = durationMs > 0 ? Math.min(positionMs / durationMs, 1) : 0;
  const playheadX = progress * width;

  const path = useMemo(() => {
    const midY = WAVE_HEIGHT / 2;
    let d = `M ${-WAVE_LENGTH} ${midY}`;
    for (let x = -WAVE_LENGTH; x <= width + WAVE_LENGTH; x += WAVE_LENGTH) {
      d += ` Q ${x + WAVE_LENGTH / 4} ${midY - AMPLITUDE}, ${x + WAVE_LENGTH / 2} ${midY}`;
      d += ` Q ${x + (WAVE_LENGTH * 3) / 4} ${midY + AMPLITUDE}, ${x + WAVE_LENGTH} ${midY}`;
    }
    return d;
  }, [width]);

  useEffect(() => {
    if (!svgRef.current) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setWidth(entry.contentRect.width);
      }
    });
    ro.observe(svgRef.current);
    return () => ro.disconnect();
  }, []);

  const handleSeek = (clientX: number) => {
    if (!svgRef.current || durationMs <= 0) return;
    const rect = svgRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(clientX - rect.left, rect.width));
    const pct = x / rect.width;
    onSeek(pct * durationMs);
  };

  const handlePointerDown = (e: React.PointerEvent<SVGSVGElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    handleSeek(e.clientX);
  };

  const handlePointerMove = (e: React.PointerEvent<SVGSVGElement>) => {
    if (e.buttons === 0) return;
    handleSeek(e.clientX);
  };

  return (
    <div className="relative flex-1 select-none overflow-hidden" style={{ height: WAVE_HEIGHT, touchAction: "none" }}>
      <svg
        ref={svgRef}
        viewBox={`0 0 ${width} ${WAVE_HEIGHT}`}
        preserveAspectRatio="none"
        className="absolute inset-0 h-full w-full cursor-pointer"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerEnter={() => setHovered(true)}
        onPointerLeave={() => setHovered(false)}
      >
        <defs>
          <clipPath id="played-clip">
            <rect x="0" y="0" width={Math.max(0, playheadX)} height={WAVE_HEIGHT} />
          </clipPath>
        </defs>

        {/* Unplayed Background Straight Line */}
        <line
          x1={playheadX}
          y1={WAVE_HEIGHT / 2}
          x2={width}
          y2={WAVE_HEIGHT / 2}
          stroke="rgb(var(--accent) / 0.2)"
          strokeWidth={4}
          strokeLinecap="round"
        />

        {/* Played Foreground Wavy Line */}
        <g clipPath="url(#played-clip)">
          <motion.g
            animate={{ x: [0, -WAVE_LENGTH] }}
            transition={{ repeat: Infinity, ease: "linear", duration: 1.2 }}
          >
            <path
              d={path}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeWidth={4}
              strokeLinecap="round"
              style={{ filter: "drop-shadow(0 0 6px rgb(var(--accent) / 0.3))" }}
            />
          </motion.g>
        </g>

        {/* Playhead - Pill Shape */}
        <motion.rect
          x={playheadX - (hovered ? 5 : 4)}
          y={(WAVE_HEIGHT / 2) - 10}
          width={hovered ? 10 : 8}
          height={20}
          rx={hovered ? 5 : 4}
          fill="#ffffff"
          style={{ filter: "drop-shadow(0 0 6px rgb(var(--accent))) drop-shadow(0 0 2px rgba(255,255,255,0.8))" }}
          transition={{ type: "spring", stiffness: 400, damping: 25 }}
        />
      </svg>
    </div>
  );
}

export function NowPlaying() {
  const open = useUi((s) => s.nowPlayingOpen);
  const setOpen = useUi((s) => s.setNowPlaying);
  const { current, playing, positionMs, durationMs, shuffle, repeat } = usePlayer();
  const { toggle, next, prev, seek, toggleShuffle, cycleRepeat } = usePlayer.getState();

  const rx = useMotionValue(0), ry = useMotionValue(0);
  const srx = useSpring(rx, { stiffness: 180, damping: 18 }), sry = useSpring(ry, { stiffness: 180, damping: 18 });
  const bg = artUrl(current?.artPath);

  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, setOpen]);

  return (
    <AnimatePresence>
      {open && current && (
        <motion.div role="dialog" aria-modal="true" aria-label="Now Playing" className="fixed inset-0 z-40 flex flex-col overflow-hidden bg-base"
          initial={{ opacity: 0, y: 40 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 40 }} transition={{ type: "spring", stiffness: 260, damping: 30 }}>
          {bg && <div className="absolute inset-0 scale-125 opacity-50 blur-3xl" style={{ backgroundImage: `url(${bg})`, backgroundSize: "cover", backgroundPosition: "center" }} />}
          <div className="absolute inset-0" style={{ background: "radial-gradient(80% 80% at 30% 20%, rgb(var(--aurora-1) / 0.35), transparent), radial-gradient(80% 80% at 90% 90%, rgb(var(--aurora-2) / 0.3), transparent), rgb(var(--bg-base) / 0.55)" }} />

          <div className="relative flex h-14 shrink-0 items-center justify-between px-6" data-tauri-drag-region>
            <span className="text-xs font-medium uppercase tracking-[0.2em] text-dim">Now playing</span>
            <IconBtn label="Close Now Playing" onClick={() => setOpen(false)}><CloseIcon size={20} /></IconBtn>
          </div>

          <div className="relative grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-12 px-14 pb-4">
            {/* art */}
            <div className="flex flex-col items-center justify-center gap-6">
              <div className="relative" style={{ perspective: 900 }}
                onPointerMove={(e) => {
                  const r = e.currentTarget.getBoundingClientRect();
                  ry.set(((e.clientX - r.left) / r.width - 0.5) * 16);
                  rx.set(-((e.clientY - r.top) / r.height - 0.5) * 16);
                }}
                onPointerLeave={() => { rx.set(0); ry.set(0); }}>
                {/* ring that turns while music plays */}
                <div className="absolute -inset-3 rounded-[34px] opacity-80"
                  style={{ background: "conic-gradient(from 0deg, rgb(var(--accent)), transparent 30%, rgb(var(--accent-2)) 55%, transparent 80%, rgb(var(--accent)))",
                    filter: "blur(10px)", animation: "spin 14s linear infinite", animationPlayState: playing ? "running" : "paused" }} />
                <motion.div style={{ rotateX: srx, rotateY: sry, transformStyle: "preserve-3d" }}
                  className="glass relative aspect-square w-[min(44vh,420px)] overflow-hidden p-2" >
                  <ArtImage path={current.artPath} className="h-full w-full rounded-[14px]" markSize={96} />
                </motion.div>
              </div>
            </div>

            {/* details + lyrics */}
            <div className="flex min-h-0 flex-col gap-4 py-2">
              <div>
                <div className="flex items-start gap-3">
                  <h2 className="flex-1 text-4xl font-bold leading-tight tracking-tight">{current.title}</h2>
                  {current.id > 0 && <HeartButton on={current.isFavorite} onToggle={() => void toggleFavorite(current)} size={26} className="mt-1 h-11 w-11" />}
                </div>
                <p className="mt-1 text-xl text-ink/90">{current.artist || "Unknown artist"}</p>
                <p className="text-base text-dim">{[current.album, current.year].filter(Boolean).join(" · ")}</p>
                {current.id > 0 && <Rating value={current.rating} onChange={(r) => void rate(current, r)} size={20} className="-ml-1 mt-3" />}
              </div>
              <div className="min-h-0 flex-1"><LyricsPanel trackId={current.id > 0 ? current.id : undefined} /></div>
            </div>
          </div>

          {/* big progress + controls */}
          <div className="relative mx-auto flex w-full max-w-4xl shrink-0 flex-col gap-3 px-10 pb-8">
            <div className="flex items-center gap-4 text-sm tabular-nums text-dim">
              <span className="w-12 text-right">{formatTime(positionMs)}</span>
              <WavySeekBar positionMs={positionMs} durationMs={durationMs} onSeek={(ms) => void seek(ms)} />
              <span className="w-12">{formatTime(durationMs)}</span>
            </div>
            <div className="flex items-center justify-center gap-3">
              <IconBtn label="Shuffle" active={shuffle} onClick={() => void toggleShuffle()}><ShuffleIcon size={20} /></IconBtn>
              <IconBtn label="Previous" onClick={() => void prev()}><PrevIcon size={26} /></IconBtn>
              <motion.button aria-label={playing ? "Pause" : "Play"} onClick={() => void toggle()} whileHover={{ scale: 1.07 }} whileTap={{ scale: 0.9 }}
                transition={{ type: "spring", stiffness: 500, damping: 22 }}
                className="grid h-16 w-16 place-items-center rounded-full text-white shadow-glow [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]">
                {playing ? <PauseIcon size={30} /> : <PlayIcon size={30} />}
              </motion.button>
              <IconBtn label="Next" onClick={() => void next()}><NextIcon size={26} /></IconBtn>
              <IconBtn label={`Repeat ${repeat}`} active={repeat !== "off"} onClick={() => void cycleRepeat()}>
                {repeat === "one" ? <RepeatOneIcon size={20} /> : <RepeatIcon size={20} />}
              </IconBtn>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
