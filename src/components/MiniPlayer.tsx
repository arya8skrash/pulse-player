import { useMemo, useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { api, type Track } from "../lib/ipc";
import { usePlayer } from "../store/player";
import { useSettings } from "../store/settings";
import { ArtImage } from "./ArtImage";
import { IconBtn, SeekBar, Slider } from "./ui";
import { ExpandIcon, NextIcon, PauseIcon, PlayIcon, PrevIcon, VolumeIcon } from "./icons";
import { toggleFavorite } from "../lib/actions";
import { HeartButton } from "./TrackBits";

// Search icon
const SearchIcon = ({ size = 16 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
  </svg>
);

const MiniVisualizer = ({ playing }: { playing: boolean }) => {
  const barCount = 24;
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
    <div className="absolute bottom-1 left-1/2 flex h-6 w-40 -translate-x-1/2 items-end justify-center gap-[2px] pointer-events-none mix-blend-screen opacity-40">
      {bars.map((bar) => (
        <motion.div
          key={bar.id}
          className="w-[2px] rounded-full [background:linear-gradient(0deg,rgb(var(--accent)/0.6),rgb(var(--accent-2)/0.9))]"
          initial={{ height: "5%" }}
          animate={playing ? { height: [`${bar.maxHeight * 0.2}%`, `${bar.maxHeight}%`] } : { height: "5%" }}
          transition={playing ? { duration: bar.duration, repeat: Infinity, repeatType: "mirror", ease: "easeInOut", delay: bar.delay } : { duration: 0.4 }}
        />
      ))}
    </div>
  );
};

export function MiniPlayer() {
  const { current, playing, positionMs, durationMs, volume } = usePlayer();
  const { toggle, next, prev, seek, setVolume, playTracks } = usePlayer.getState();
  const showVisualizer = useSettings((s) => s.visualizer);

  const [showSearch, setShowSearch] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Track[]>([]);

  useEffect(() => {
    if (!query.trim()) { setResults([]); return; }
    const timer = setTimeout(async () => {
      try {
        const tracks = await api.getTracks({ query, limit: 8 });
        setResults(tracks);
      } catch (e) { console.error(e); }
    }, 300);
    return () => clearTimeout(timer);
  }, [query]);

  const handlePlay = (track: Track, index: number) => {
    void playTracks(results, index);
    setShowSearch(false);
    setQuery("");
  };

  return (
    <>
      <div data-tauri-drag-region className="glass m-1.5 flex h-[calc(100%-12px)] items-center gap-3 p-2.5" style={{ borderRadius: 18 }}>
        <ArtImage path={current?.artPath} className="pointer-events-none h-[72px] w-[72px] shrink-0 rounded-xl" markSize={30} />
        
        <div data-tauri-drag-region className="flex min-w-0 flex-1 flex-col gap-1">
          <div data-tauri-drag-region className="flex items-center gap-1">
            <div data-tauri-drag-region className="min-w-0 flex-1">
              <p className="pointer-events-none truncate text-sm font-semibold">{current?.title ?? "Nothing playing"}</p>
              <p className="pointer-events-none truncate text-xs text-dim">{current?.artist ?? " "}</p>
            </div>
            
            {current && <HeartButton on={current.isFavorite} onToggle={() => void toggleFavorite(current)} size={16} />}
            
            <div className="flex items-center gap-1 px-1">
              <IconBtn label={volume === 0 ? "Unmute" : "Mute"} onClick={() => setVolume(volume === 0 ? 0.8 : 0)} className="h-7 w-7">
                <VolumeIcon size={14} level={volume} />
              </IconBtn>
              <Slider label="Volume" min={0} max={1} step={0.01} value={volume} onChange={setVolume} className="w-12" />
            </div>

            <IconBtn label="Search songs" onClick={() => setShowSearch(true)}>
              <SearchIcon size={16} />
            </IconBtn>

            <IconBtn label="Back to full window" onClick={() => void api.setMiniPlayer(false)}><ExpandIcon size={16} /></IconBtn>
          </div>
          
          <div className="relative flex items-center gap-1">
            {showVisualizer && current && <MiniVisualizer playing={playing} />}
            
            <IconBtn label="Previous" disabled={!current} onClick={() => void prev()}><PrevIcon size={16} /></IconBtn>
            <IconBtn label={playing ? "Pause" : "Play"} disabled={!current} onClick={() => void toggle()}>{playing ? <PauseIcon size={20} /> : <PlayIcon size={20} />}</IconBtn>
            <IconBtn label="Next" disabled={!current} onClick={() => void next()}><NextIcon size={16} /></IconBtn>
            <SeekBar positionMs={positionMs} durationMs={durationMs} disabled={!current} onSeek={(ms) => void seek(ms)} />
          </div>
        </div>
      </div>

      {/* Search menu as a modal, centered on the screen */}
      <AnimatePresence>
        {showSearch && (
          <motion.div 
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
            className="absolute inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
            onClick={() => setShowSearch(false)}
          >
            <motion.div 
              initial={{ scale: 0.9, y: 10, opacity: 0 }} animate={{ scale: 1, y: 0, opacity: 1 }} exit={{ scale: 0.9, y: 10, opacity: 0 }}
              transition={{ type: "spring", stiffness: 400, damping: 28 }}
              className="w-full max-w-sm rounded-2xl border border-white/10 bg-[#050a12]/95 p-3 shadow-2xl backdrop-blur-2xl"
              onClick={(e) => e.stopPropagation()}
            >
              <input
                autoFocus
                type="text"
                placeholder="Search songs..."
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                className="w-full rounded-xl border border-white/10 bg-black/40 px-4 py-2 text-sm text-white outline-none placeholder:text-dim focus:border-cyan-500/50 transition-colors"
              />
              
              {results.length > 0 && (
                <div className="mt-3 max-h-60 overflow-y-auto">
                  {results.map((t, i) => (
                    <button
                      key={t.id}
                      onClick={() => handlePlay(t, i)}
                      className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors hover:bg-white/10"
                    >
                      <ArtImage path={t.artPath} className="h-8 w-8 shrink-0 rounded-md" markSize={16} />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium text-white">{t.title}</p>
                        <p className="truncate text-xs text-dim">{t.artist || "Unknown"}</p>
                      </div>
                    </button>
                  ))}
                </div>
              )}
              {query && results.length === 0 && (
                <p className="mt-4 text-center text-sm text-dim">No songs found.</p>
              )}
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}