import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { SearchIcon, PlayIcon, CloseIcon, StarIcon } from "../components/icons";
import { ArtImage } from "../components/ArtImage";
import { api } from "../lib/ipc";

interface OnlineTrack {
  videoId: string;
  title: string;
  artist: string;
  album: string | null;
  durationSecs: number;
  thumbnailUrl: string | null;
}

type SearchCacheEntry = { query: string; tracks: OnlineTrack[] };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeOnlineTracks(payload: unknown): OnlineTrack[] {
  let value = payload;
  for (let depth = 0; depth < 4 && !Array.isArray(value); depth += 1) {
    if (!isRecord(value)) return [];
    const record = value;
    const wrapper = ["Ok", "ok", "data", "results", "tracks"].find((key) => key in record);
    if (!wrapper) return [];
    value = record[wrapper];
  }
  if (!Array.isArray(value)) return [];

  return value.flatMap((entry): OnlineTrack[] => {
    if (!isRecord(entry)) return [];
    const videoId = entry.videoId ?? entry.video_id;
    const title = entry.title;
    if (typeof videoId !== "string" || typeof title !== "string") return [];

    const artist = entry.artist;
    const album = entry.album;
    const durationSecs = entry.durationSecs ?? entry.duration_secs;
    const thumbnailUrl = entry.thumbnailUrl ?? entry.thumbnail_url;
    return [{
      videoId,
      title,
      artist: typeof artist === "string" ? artist : "Unknown artist",
      album: typeof album === "string" ? album : null,
      durationSecs: typeof durationSecs === "number" ? durationSecs : 0,
      thumbnailUrl: typeof thumbnailUrl === "string" ? thumbnailUrl : null,
    }];
  });
}

function findPrefixResults(
  cache: Map<string, SearchCacheEntry>,
  query: string,
): SearchCacheEntry | undefined {
  const normalized = query.toLowerCase();
  return [...cache.values()]
    .filter((entry) => entry.query.length >= 3 && normalized.startsWith(entry.query.toLowerCase()))
    .sort((a, b) => b.query.length - a.query.length)[0];
}

const SKELETON_ROWS = 8;
const HEADER_GLASS_BLUR = 9;
const RECENT_KEY = "pulse.online.recent.v2";
const FAVORITES_KEY = "pulse.online.favorites.v1";

type FilterMode = "all" | "recent" | "favorites";
type RecentEntry = { track: OnlineTrack; at: number };

function formatDuration(secs: number): string {
  if (!secs) return "";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = String(secs % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

function readStorage<T>(key: string, fallback: T): T {
  try {
    const value = window.localStorage.getItem(key);
    return value ? JSON.parse(value) as T : fallback;
  } catch {
    return fallback;
  }
}

function SkeletonRow() {
  return <li className="flex h-[76px] items-center gap-3 rounded-2xl border border-white/[0.04] bg-ink/[0.025] px-3" aria-hidden="true">
    <div className="h-12 w-12 shrink-0 animate-pulse rounded-xl bg-ink/10" />
    <div className="min-w-0 flex-1 space-y-2"><div className="h-3 w-1/2 animate-pulse rounded-full bg-ink/10" /><div className="h-2.5 w-1/3 animate-pulse rounded-full bg-ink/10" /></div>
    <div className="h-3 w-8 animate-pulse rounded-full bg-ink/10" />
  </li>;
}

function Equalizer() {
  return <span aria-label="Now playing" className="flex h-4 items-end gap-0.5 text-accent">
    {[0, 1, 2].map((n) => <motion.i key={n} className="w-[3px] rounded-full bg-current" animate={{ height: [5, 14, 7, 11, 5] }} transition={{ duration: 0.8 + n * 0.16, repeat: Infinity, ease: "easeInOut", delay: n * 0.1 }} />)}
  </span>;
}

export function OnlinePage() {
  const [input, setInput] = useState("");
  const query = useDebounced(input.trim(), 500);
  const [results, setResults] = useState<OnlineTrack[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [playingId, setPlayingId] = useState<string | null>(null);
  const [playError, setPlayError] = useState<string | null>(null);
  const [fallbackQuery, setFallbackQuery] = useState<string | null>(null);
  const [filter, setFilter] = useState<FilterMode>("all");
  const [favorites, setFavorites] = useState<string[]>(() => readStorage(FAVORITES_KEY, []));
  const [recent, setRecent] = useState<RecentEntry[]>(() => readStorage(RECENT_KEY, []));
  const reqId = useRef(0);
  const currentQuery = useRef(query);
  const resultCache = useRef(new Map<string, SearchCacheEntry>());
  const headerRef = useRef<HTMLElement>(null);
  const pageId = useId();
  // Assumption: strip the colons React puts in useId() so the id is a plain, CSS-safe fragment for url(#...).
  const glassFilterId = `online-glass-${pageId.replace(/:/g, "")}`;
  const reduceMotion = useReducedMotion();
  const [glassHeight, setGlassHeight] = useState(0);
  currentQuery.current = query;

  useLayoutEffect(() => {
    const header = headerRef.current;
    if (!header) return;
    const measure = () => setGlassHeight(header.offsetHeight);
    measure();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
    const observer = new ResizeObserver(measure);
    observer.observe(header, { box: "border-box" });
    return () => observer.disconnect();
  }, []);

  useEffect(() => { try { window.localStorage.setItem(FAVORITES_KEY, JSON.stringify(favorites)); } catch { /* Storage may be unavailable. */ } }, [favorites]);
  useEffect(() => { try { window.localStorage.setItem(RECENT_KEY, JSON.stringify(recent.slice(0, 12))); } catch { /* Storage may be unavailable. */ } }, [recent]);

  const remember = useCallback((track: OnlineTrack) => {
    setRecent((current) => [{ track, at: Date.now() }, ...current.filter((entry) => entry.track.videoId !== track.videoId)].slice(0, 12));
  }, []);

  const runSearch = useCallback(async (q: string) => {
    const id = ++reqId.current;
    if (!q) { setResults([]); setFallbackQuery(null); setError(null); setLoading(false); return; }
    setLoading(true); setError(null);
    try {
      const payload = await api.onlineSearch(q, 50);
      const res = normalizeOnlineTracks(payload);
      const queryKey = q.toLowerCase();
      if (res.length > 0) {
        resultCache.current.set(queryKey, { query: q, tracks: res });
        if (resultCache.current.size > 20) {
          const oldest = resultCache.current.keys().next().value;
          if (oldest) resultCache.current.delete(oldest);
        }
      }
      if (id === reqId.current) {
        if (res.length > 0) { setResults(res); setFallbackQuery(null); }
        else { const fallback = findPrefixResults(resultCache.current, q); setResults(fallback?.tracks ?? []); setFallbackQuery(fallback?.query ?? null); }
      } else if (res.length > 0) {
        const activeQuery = currentQuery.current.trim();
        const activeKey = activeQuery.toLowerCase();
        if (queryKey === activeKey) { setResults(res); setFallbackQuery(null); }
        else if (activeKey.startsWith(queryKey) && !resultCache.current.has(activeKey)) {
          setResults((current) => current.length > 0 ? current : res);
          setFallbackQuery((current) => current ?? q);
        }
      }
    } catch (e) {
      if (id === reqId.current) { setResults([]); setFallbackQuery(null); setError(String(e)); }
    } finally { if (id === reqId.current) setLoading(false); }
  }, []);

  useEffect(() => { void runSearch(query); }, [query, runSearch]);
  const play = useCallback(async (track: OnlineTrack) => {
    setPlayError(null); setPlayingId(track.videoId);
    try { await api.onlinePlay(track); remember(track); } catch (e) { setPlayError(String(e)); }
    finally { setPlayingId(null); }
  }, [remember]);

  const visibleTracks = useMemo(() => {
    let tracks = results;
    if (filter === "recent") tracks = recent.map((entry) => entry.track);
    if (filter === "favorites") tracks = [...recent.map((entry) => entry.track), ...results].filter((track, i, all) => favorites.includes(track.videoId) && all.findIndex((t) => t.videoId === track.videoId) === i);
    const needle = input.trim().toLocaleLowerCase();
    return needle ? tracks.filter((t) => `${t.title} ${t.artist} ${t.album ?? ""}`.toLocaleLowerCase().includes(needle)) : tracks;
  }, [results, filter, recent, favorites, input]);

  const toggleFavorite = (id: string) => setFavorites((current) => current.includes(id) ? current.filter((item) => item !== id) : [id, ...current]);
  const playAll = () => { if (visibleTracks[0]) void play(visibleTracks[0]); };
  const counts = { all: results.length, recent: recent.length, favorites: favorites.length };
  const tabs: { id: FilterMode; label: string; count: number }[] = [
    { id: "all", label: "All Tracks", count: counts.all },
    { id: "recent", label: "Recent 12", count: counts.recent },
    { id: "favorites", label: "Favorites", count: counts.favorites },
  ];
  const showSkeleton = loading && results.length === 0 && filter === "all";
  const isSearchEmpty = input.trim() !== "" && visibleTracks.length === 0 && !loading;
  const listEmpty = visibleTracks.length === 0 && !loading && !error;

  return <div className="relative h-full w-full min-h-0 overflow-hidden text-ink">
    {/* The filter is applied to a non-scrolling wrapper that contains <main>. The visual result is identical to filtering <main>,
        but the compositor no longer has to run an SVG filter directly on a scrolling layer. */}
    <svg aria-hidden="true" focusable="false" width="0" height="0" className="pointer-events-none absolute">
      <defs>
        <filter id={glassFilterId} x="0%" y="0%" width="100%" height="100%" primitiveUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
          <feGaussianBlur in="SourceGraphic" stdDeviation={HEADER_GLASS_BLUR} x={0} y={0} width="100%" height={glassHeight} result="blurredUnderHeader" />
          <feFlood floodColor="white" x={0} y={0} width="100%" height={glassHeight} result="headerArea" />
          <feComposite in="SourceGraphic" in2="headerArea" operator="out" result="sharpBelowHeader" />
          <feMerge>
            <feMergeNode in="sharpBelowHeader" />
            <feMergeNode in="blurredUnderHeader" />
          </feMerge>
        </filter>
      </defs>
    </svg>
    <div className="absolute inset-0 overflow-hidden" style={glassHeight > 0 ? { filter: `url("#${glassFilterId}")` } : undefined}>
    <main className="absolute inset-0 h-full w-full overflow-y-auto px-3 pb-7 sm:px-5">
      <div className="mx-auto max-w-6xl pt-[168px] sm:pt-[168px]">
      {playError && <div role="alert" className="mt-3 rounded-xl border border-red-400/20 bg-red-400/5 px-4 py-2 text-xs text-red-300">{playError}</div>}
        {error && <div role="alert" className="mx-1 mb-4 flex items-center justify-between gap-3 rounded-2xl border border-white/10 bg-ink/5 p-4"><div><div className="text-sm font-medium">Search failed</div><div className="mt-1 break-all text-xs text-dim">{error}</div></div><button type="button" onClick={() => void runSearch(query)} className="shrink-0 rounded-full border border-white/15 px-4 py-2 text-xs hover:bg-ink/10">Try again</button></div>}
        {fallbackQuery && results.length > 0 && <div className="px-3 pb-3 text-xs text-dim">No exact matches for “{query}”; showing results for “{fallbackQuery}”.</div>}
        {showSkeleton && <ul className="grid grid-cols-1 gap-2 md:grid-cols-2" role="status" aria-label="Loading tracks">{Array.from({ length: SKELETON_ROWS }, (_, i) => <SkeletonRow key={i} />)}</ul>}
        {listEmpty && !error && <motion.section initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="mx-auto my-10 max-w-md rounded-3xl border border-white/10 bg-ink/[0.035] px-6 py-12 text-center shadow-xl shadow-black/5 backdrop-blur-xl">
          <div className="mx-auto mb-4 grid h-12 w-12 place-items-center rounded-2xl border border-white/10 bg-white/[0.04] text-accent">{filter === "favorites" ? <StarIcon className="h-5 w-5" /> : <SearchIcon className="h-5 w-5" />}</div>
          <h2 className="text-sm font-semibold">{isSearchEmpty ? "No tracks found" : filter === "favorites" ? "No favorites yet" : filter === "recent" ? "Your recent tracks will appear here" : query ? "No tracks found" : "Find something to play"}</h2>
          <p className="mt-2 text-xs leading-5 text-dim">{isSearchEmpty ? `Nothing matched “${input.trim()}”. Try another title or artist.` : filter === "favorites" ? "Star tracks as you browse to keep them close." : filter === "recent" ? "Search for music or start playback to build your recent list." : "Search for a song or artist to explore online music."}</p>
        </motion.section>}
        {!showSkeleton && visibleTracks.length > 0 && <ul className="grid grid-cols-1 gap-2 md:grid-cols-2" aria-label="Online tracks">
            {visibleTracks.map((track) => {
              const isFavorite = favorites.includes(track.videoId);
              const isPlaying = playingId === track.videoId;
              return <li key={track.videoId}
                onDoubleClick={() => void play(track)} className={`group relative flex min-h-[76px] items-center gap-3 rounded-2xl border px-3 transition-colors ${isPlaying ? "border-[rgb(var(--accent)/0.55)] bg-[rgb(var(--accent)/0.08)] shadow-[0_0_24px_rgb(var(--accent)/0.11)]" : "border-transparent bg-transparent hover:border-white/[0.08] hover:bg-white/[0.045]"}`}>
                <div className="relative h-12 w-12 shrink-0 overflow-hidden rounded-xl border border-white/10 bg-ink/10"><ArtImage path={track.thumbnailUrl} className="h-full w-full" markSize={24} />{isPlaying && <div className="absolute inset-0 grid place-items-center bg-black/35"><Equalizer /></div>}</div>
                <button type="button" onClick={() => void play(track)} className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent)/0.6)]" aria-label={`Play ${track.title}`}>
                  <span className={`block truncate text-sm font-medium ${isPlaying ? "text-accent" : "text-ink"}`}>{track.title}</span><span className="mt-1 block truncate text-xs text-dim">{track.artist}{track.album ? ` · ${track.album}` : ""}</span>
                </button>
                <span className="hidden shrink-0 text-xs tabular-nums text-dim sm:block">{formatDuration(track.durationSecs)}</span>
                <motion.button type="button" aria-label={isFavorite ? `Remove ${track.title} from favorites` : `Add ${track.title} to favorites`} aria-pressed={isFavorite} onClick={() => toggleFavorite(track.videoId)} whileTap={{ scale: [1, 1.45, 0.82, 1] }} transition={{ duration: 0.32 }} className={`grid h-9 w-9 shrink-0 place-items-center rounded-full transition-colors hover:bg-ink/10 ${isFavorite ? "text-accent" : "text-dim opacity-70 hover:opacity-100"}`}><StarIcon className="h-4 w-4" filled={isFavorite} /></motion.button>
                <button type="button" aria-label={`Play ${track.title}`} onClick={() => void play(track)} disabled={isPlaying} className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-ink/[0.06] text-ink transition hover:bg-[rgb(var(--accent)/0.2)] disabled:opacity-50">{isPlaying ? <Equalizer /> : <PlayIcon className="h-4 w-4" />}</button>
              </li>;
            })}
          </ul>}
        {loading && results.length > 0 && filter === "all" && <div className="mt-3 text-center text-[11px] text-dim">Updating results…</div>}

            </div>
    </main>
    </div>
    <header
      ref={headerRef}
      className="pointer-events-none absolute inset-x-0 top-0 z-20 border-b border-white/[0.08] px-4 pb-4 pt-5 sm:px-7"
      style={{
        background: [
          "radial-gradient(ellipse at 18% 0%, rgba(255, 240, 248, 0.10), transparent 56%)",
          "radial-gradient(ellipse at 82% 100%, rgba(242, 166, 204, 0.065), transparent 50%)",
          "linear-gradient(180deg, rgba(20, 12, 22, 0.32), rgba(20, 12, 22, 0.21))",
        ].join(", "),
        boxShadow: "inset 0 1px 0 rgba(255, 244, 251, 0.12), inset 0 -1px 0 rgba(255, 224, 242, 0.06), 0 6px 20px rgba(0, 0, 0, 0.21)",
      }}
    >
      <div className="pointer-events-auto">
        <div className="mx-auto max-w-6xl">
      <div className="mb-4 flex items-end justify-between gap-4">
        <div><h1 className="text-2xl font-semibold tracking-tight">Online</h1><p className="mt-1 text-xs text-dim">Discover music from SoundCloud</p></div>
        <motion.button type="button" whileHover={{ scale: 1.035, y: -1 }} whileTap={{ scale: 0.96 }} onClick={playAll} disabled={!visibleTracks.length || playingId !== null}
          className="flex h-10 shrink-0 items-center gap-2 rounded-full px-4 text-sm font-semibold text-white shadow-[0_8px_24px_rgb(var(--accent)/0.22)] [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))] disabled:cursor-not-allowed disabled:opacity-40">
          <PlayIcon className="h-4 w-4" /> <span>Play All</span>
        </motion.button>
      </div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <motion.div layoutScroll role="tablist" aria-label="Online track filters" className="relative isolate flex w-fit max-w-full gap-1 overflow-x-auto rounded-full border border-white/[0.08] bg-ink/[0.04] p-1">
          {tabs.map((tab) => <button key={tab.id} type="button" role="tab" aria-selected={filter === tab.id} onClick={() => setFilter(tab.id)} className={`relative flex shrink-0 items-center gap-2 rounded-full px-3 py-2 text-xs font-medium transition-colors motion-reduce:transition-none ${filter === tab.id ? "text-white" : "text-dim hover:text-ink"}`}>
            {filter === tab.id && <motion.span
              aria-hidden="true"
              layoutId={reduceMotion ? undefined : `online-tab-pill-${pageId}`}
              className="pointer-events-none absolute inset-0 z-0 rounded-full [background:linear-gradient(135deg,rgb(var(--accent)/0.9),rgb(var(--accent-2)/0.78))]"
              transition={{ layout: { type: "spring", stiffness: 420, damping: 34 } }}
            />}
            <span className="relative z-10">{tab.label}</span><span className="relative z-10 rounded-full bg-black/10 px-1.5 py-0.5 text-[10px] tabular-nums">{tab.count}</span>
          </button>)}
        </motion.div>
        <div className="relative min-w-0 flex-1 sm:max-w-xs">
          <span className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-dim"><SearchIcon className="h-4 w-4" /></span>
          <input value={input} onChange={(e) => setInput(e.target.value)} onKeyDown={(e) => { if (e.key === "Escape") setInput(""); }} placeholder="Search tracks and artists" aria-label="Search online music"
            className="h-10 w-full rounded-full border border-white/10 bg-ink/[0.045] pl-11 pr-11 text-sm text-ink outline-none transition focus:border-[rgb(var(--accent)/0.5)] focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent)/0.35)]" />
          {input && <button type="button" onClick={() => setInput("")} aria-label="Clear search" className="absolute right-2 top-1/2 grid h-8 w-8 -translate-y-1/2 place-items-center rounded-full text-dim hover:bg-ink/10 hover:text-ink"><CloseIcon className="h-4 w-4" /></button>}
        </div>
      </div>
        </div>
      </div>
    </header>
  </div>;
}