import { useMemo } from "react";
import { motion } from "framer-motion";
import { api } from "../lib/ipc";
import { formatTime } from "../lib/format";
import { rate, toggleFavorite } from "../lib/actions";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";
import { useSettings } from "../store/settings";
import { ArtImage } from "./ArtImage";
import { HeartButton, Rating } from "./TrackBits";
import { IconBtn, SeekBar, Slider } from "./ui";
import { ExpandIcon, MiniIcon, NextIcon, PauseIcon, PlayIcon, PrevIcon, RepeatIcon, RepeatOneIcon, ShuffleIcon, VolumeIcon } from "./icons";

// Visualizer icon
const VisualizerIcon = ({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M4 14v4M8 10v8M12 6v12M16 10v8M20 14v4" />
  </svg>
);

const Visualizer = ({ playing }: { playing: boolean }) => {
  const barCount = 45;
  const bars = useMemo(() => {
    return Array.from({ length: barCount }).map((_, i) => {
      const distFromCenter = Math.abs(i - (barCount - 1) / 2) / ((barCount - 1) / 2);
      const envelope = Math.cos(distFromCenter * Math.PI / 2); 
      return {
        id: i,
        maxHeight: Math.max(15, envelope * 100), 
        duration: 0.5 + Math.random() * 0.4,
        delay: Math.random() * 0.2,
      };
    });
  }, []);

  return (
    <div className="absolute bottom-7 left-1/2 flex h-8 w-56 -translate-x-1/2 items-end justify-center gap-[3px] pointer-events-none mix-blend-screen opacity-70">
      {bars.map((bar) => (
        <motion.div
          key={bar.id}
          className="w-[3px] rounded-full [background:linear-gradient(0deg,rgb(var(--accent)/0.6),rgb(var(--accent-2)/0.9))] blur-[0.5px]"
          initial={{ height: "5%" }}
          animate={playing ? { height: [`${bar.maxHeight * 0.2}%`, `${bar.maxHeight}%`] } : { height: "5%" }}
          transition={playing ? { duration: bar.duration, repeat: Infinity, repeatType: "mirror", ease: "easeInOut", delay: bar.delay } : { duration: 0.4 }}
        />
      ))}
    </div>
  );
};

export function PlayerBar() {
  const { current, playing, positionMs, durationMs, volume, shuffle, repeat, error } = usePlayer();
  const { toggle, next, prev, seek, setVolume, toggleShuffle, cycleRepeat } = usePlayer.getState();
  const setNowPlaying = useUi((s) => s.setNowPlaying);
  const setShowVisualizer = useUi((s) => s.setShowVisualizer); // New state
  const showVisualizer = useSettings((s) => s.visualizer);

  return (
    <section aria-label="Player" className="glass grid grid-cols-[minmax(0,1fr)_minmax(340px,1.8fr)_minmax(0,1fr)] items-center gap-6 px-5 py-3">
      
      {/* Now playing (left side) */}
      <div className="flex min-w-0 items-center gap-3 z-10">
        <motion.button aria-label="Open Now Playing" disabled={!current} onClick={() => setNowPlaying(true)}
          whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.96 }} transition={{ type: "spring", stiffness: 400, damping: 22 }}
          className="group relative h-14 w-14 shrink-0 overflow-hidden rounded-xl shadow-lg">
          <ArtImage path={current?.artPath} className="h-full w-full" markSize={26} />
          {current && <span className="absolute inset-0 grid place-items-center bg-black/45 text-white opacity-0 transition-opacity group-hover:opacity-100"><ExpandIcon size={18} /></span>}
        </motion.button>
        
        <div className="flex min-w-0 flex-1 flex-col justify-center">
          <p className="truncate text-sm font-medium">{current?.title ?? "Nothing playing"}</p>
          <p className="truncate text-xs text-dim">{error ?? (current ? current.artist || "Unknown artist" : "Pick a song to start")}</p>
          {current && current.id > 0 && <Rating value={current.rating} onChange={(r) => void rate(current, r)} size={12} className="-ml-0.5 mt-0.5" />}
        </div>

        {current && current.id > 0 && <HeartButton on={current.isFavorite} onToggle={() => void toggleFavorite(current)} />}
      </div>

      {/* Transport and seek (center) */}
      <div className="relative flex flex-col items-center gap-1.5">
        
        {showVisualizer && current && <Visualizer playing={playing} />}
        
        <div className="relative z-10 flex items-center gap-1.5">
          <IconBtn label={shuffle ? "Shuffle on" : "Shuffle off"} active={shuffle} onClick={() => void toggleShuffle()}><ShuffleIcon size={18} /></IconBtn>
          <IconBtn label="Previous (Ctrl+Left)" disabled={!current} onClick={() => void prev()}><PrevIcon size={20} /></IconBtn>
          
          <motion.button
            aria-label={playing ? "Pause (Space)" : "Play (Space)"} disabled={!current} onClick={() => void toggle()}
            whileHover={{ scale: 1.08 }} whileTap={{ scale: 0.9 }} transition={{ type: "spring", stiffness: 500, damping: 22 }}
            className="mx-1 grid h-12 w-12 place-items-center rounded-full text-white shadow-glow disabled:opacity-40 [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]">
            <motion.span key={playing ? "pause" : "play"} initial={{ scale: 0.5, rotate: -30, opacity: 0 }} animate={{ scale: 1, rotate: 0, opacity: 1 }} transition={{ type: "spring", stiffness: 600, damping: 24 }}>
              {playing ? <PauseIcon size={22} /> : <PlayIcon size={22} />}
            </motion.span>
          </motion.button>
          
          <IconBtn label="Next (Ctrl+Right)" disabled={!current} onClick={() => void next()}><NextIcon size={20} /></IconBtn>
          <IconBtn label={`Repeat ${repeat}`} active={repeat !== "off"} onClick={() => void cycleRepeat()}>
            {repeat === "one" ? <RepeatOneIcon size={18} /> : <RepeatIcon size={18} />}
          </IconBtn>
        </div>
        
        <div className="relative z-10 flex w-full items-center gap-3 text-xs tabular-nums text-dim">
          <span className="w-10 text-right">{formatTime(positionMs)}</span>
          <SeekBar positionMs={positionMs} durationMs={durationMs} disabled={!current} onSeek={(ms) => void seek(ms)} />
          <span className="w-10">{formatTime(durationMs)}</span>
        </div>
      </div>

      {/* Volume, mini player, and visualizer (right side) */}
      <div className="flex items-center justify-end gap-2 z-10">
        <IconBtn label={volume === 0 ? "Unmute" : "Mute"} onClick={() => setVolume(volume === 0 ? 0.8 : 0)}>
          <VolumeIcon size={18} level={volume} />
        </IconBtn>
        <Slider label="Volume" min={0} max={1} step={0.01} value={volume} onChange={setVolume} className="w-24" />
        
        {/* New visualizer button */}
        <IconBtn label="Full Screen Visualizer" onClick={() => setShowVisualizer(true)}>
          <VisualizerIcon size={18} />
        </IconBtn>
        
        <IconBtn label="Mini player" onClick={() => void api.setMiniPlayer(true)}><MiniIcon size={18} /></IconBtn>
      </div>
    </section>
  );
}
