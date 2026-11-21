import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import type { Track } from "../lib/ipc";
import { formatTime } from "../lib/format";
import { rate, toggleFavorite } from "../lib/actions";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";
import { ArtImage } from "./ArtImage";
import { HeartButton, Rating } from "./TrackBits";
import { PencilIcon, PlayIcon, PlusIcon, MinusIcon } from "./icons";

const ROW = 58;
const OVERSCAN = 8;
const COLS = "grid-cols-[48px_minmax(0,1.7fr)_minmax(0,1fr)_104px_34px_48px_96px]";

/** Virtualised list: only the rows in view are in the DOM, so 20,000 songs scroll smoothly. */
export function TrackList({ tracks, onPatch, onRemove, scrollTrigger }: {
  tracks: Track[];
  onPatch: (id: number, p: Partial<Track>) => void;
  /** Present when the list is a playlist: shows a "remove from playlist" action. */
  onRemove?: (t: Track) => void;
  scrollTrigger?: number; // New prop for scroll triggering
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const [top, setTop] = useState(0);
  const [height, setHeight] = useState(600);
  const currentId = usePlayer((s) => s.current?.id);
  const playing = usePlayer((s) => s.playing);
  const { setEditing, setPickerTracks } = useUi();

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const measure = () => setHeight(el.clientHeight);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => { scroller.current?.scrollTo({ top: 0 }); }, [tracks.length === 0]);

  // Scroll to the currently playing track
  useEffect(() => {
    if (!scrollTrigger || !currentId) return;
    const index = tracks.findIndex((t) => t.id === currentId);
    if (index !== -1 && scroller.current) {
      // Calculate the position to center the track in the view
      const targetScroll = Math.max(0, index * ROW - height / 2 + ROW / 2);
      scroller.current.scrollTo({ top: targetScroll, behavior: "smooth" });
    }
  }, [scrollTrigger, currentId, tracks, height]);

  const start = Math.max(0, Math.floor(top / ROW) - OVERSCAN);
  const end = Math.min(tracks.length, Math.ceil((top + height) / ROW) + OVERSCAN);
  const play = (i: number) => void usePlayer.getState().playTracks(tracks, i);

  return (
    <div ref={scroller} onScroll={(e) => setTop(e.currentTarget.scrollTop)} className="h-full overflow-y-auto px-2">
      <div style={{ height: tracks.length * ROW, position: "relative" }} role="list">
        <div style={{ position: "absolute", top: start * ROW, left: 0, right: 0 }}>
          {tracks.slice(start, end).map((t, k) => {
            const i = start + k;
            const active = t.id === currentId;
            return (
              <div key={t.id} role="listitem" tabIndex={0} aria-label={`${t.title}${t.artist ? ` by ${t.artist}` : ""}`}
                onDoubleClick={() => play(i)} onKeyDown={(e) => e.key === "Enter" && play(i)}
                className={`group grid ${COLS} items-center gap-3 rounded-xl px-2 transition-colors hover:bg-ink/10 focus-visible:bg-ink/10 ${active ? "bg-ink/[0.08]" : ""}`}
                style={{ height: ROW }}>
                <button aria-label={`Play ${t.title}`} onClick={() => play(i)} className="group/art relative h-11 w-11 overflow-hidden rounded-lg">
                  <ArtImage path={t.artPath} className="h-full w-full" markSize={22} />
                  <span className="absolute inset-0 grid place-items-center bg-black/45 text-white opacity-0 transition-opacity group-hover/art:opacity-100">
                    <PlayIcon size={18} />
                  </span>
                  {active && playing && (
                    <span className="absolute inset-0 flex items-end justify-center gap-[3px] bg-black/40 pb-2 group-hover/art:opacity-0" aria-hidden>
                      {[0, 1, 2].map((b) => (
                        <motion.i key={b} className="w-[3px] rounded-full bg-white" animate={{ height: [4, 14, 6, 12, 4] }}
                          transition={{ duration: 0.9, repeat: Infinity, delay: b * 0.15, ease: "easeInOut" }} />
                      ))}
                    </span>
                  )}
                </button>
                <div className="min-w-0">
                  <div className={`truncate text-sm font-medium ${active ? "text-accent" : ""}`}>{t.title}</div>
                  <div className="truncate text-xs text-dim">{t.artist || "Unknown artist"}</div>
                </div>
                <div className="truncate text-sm text-dim">{t.album}</div>
                <Rating value={t.rating} onChange={(r) => void rate(t, r, onPatch)} className="opacity-70 transition-opacity group-hover:opacity-100" />
                <HeartButton on={t.isFavorite} onToggle={() => void toggleFavorite(t, onPatch)} size={16} />
                <div className="text-right text-sm tabular-nums text-dim">{formatTime(t.durationMs)}</div>
                <div className="flex justify-end gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                  <button aria-label="Edit tags" title="Edit tags" onClick={() => setEditing(t)} className="grid h-8 w-8 place-items-center rounded-full text-dim hover:bg-ink/10 hover:text-ink"><PencilIcon size={15} /></button>
                  <button aria-label="Add to playlist" title="Add to playlist" onClick={() => setPickerTracks([t])} className="grid h-8 w-8 place-items-center rounded-full text-dim hover:bg-ink/10 hover:text-ink"><PlusIcon size={16} /></button>
                  {onRemove && <button aria-label="Remove from playlist" title="Remove from playlist" onClick={() => onRemove(t)} className="grid h-8 w-8 place-items-center rounded-full text-dim hover:bg-ink/10 hover:text-ink"><MinusIcon size={16} /></button>}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}