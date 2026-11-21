import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { onEvent, type ScanProgress } from "../lib/ipc";
import { useUi } from "../store/ui";
import { TrackView } from "./TrackView";
import { AlbumsView, ArtistsView, GenresView } from "./GroupViews";
import { PlaylistsView } from "./PlaylistsView";
import { FoldersView } from "./FoldersView";
import { OnlinePage } from "./OnlinePage";

function ScanStatus() {
  const bump = useUi((s) => s.bump);
  const [scan, setScan] = useState<ScanProgress | null>(null);
  useEffect(() => {
    let cancelled = false;
    let off: (() => void) | undefined;
    void onEvent<ScanProgress>("scan:progress", (p) => {
      setScan(p.done ? null : p);
      if (p.done) bump();
    }).then((f) => (cancelled ? f() : (off = f)));
    return () => { cancelled = true; off?.(); };
  }, [bump]);

  return (
    <AnimatePresence>
      {scan && (
        <motion.div role="status" initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden px-6">
          <div className="flex items-center gap-3 pt-3 text-xs text-dim">
            <span className="shrink-0">Scanning {scan.scanned.toLocaleString()} of {scan.total.toLocaleString()}</span>
            <span className="min-w-0 flex-1 truncate">{scan.current}</span>
          </div>
          <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-ink/10">
            <motion.div className="h-full rounded-full [background:linear-gradient(90deg,rgb(var(--accent)),rgb(var(--accent-2)))]"
              animate={{ width: `${scan.total ? (scan.scanned / scan.total) * 100 : 0}%` }} transition={{ type: "spring", stiffness: 120, damping: 24 }} />
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

// Standard views: same enter motion as before (fade + slight rise + tiny scale).
const ENTER_INITIAL = { opacity: 0, y: 14, scale: 0.995 };
const ENTER_ANIMATE = { opacity: 1, y: 0, scale: 1 };
const ENTER_TRANSITION = { type: "spring", stiffness: 380, damping: 34, mass: 0.8 } as const;

// Online view: opacity only. Assumption: transforming an ancestor of the SVG-filtered scroller
// forces the compositor to re-raster the filtered layer on every frame, so we avoid transforms there.
const ONLINE_INITIAL = { opacity: 0 };
const ONLINE_ANIMATE = { opacity: 1 };
const ONLINE_TRANSITION = { duration: 0.18, ease: "easeOut" } as const;

export function LibraryPage() {
  const { tab, detail, closeDetail } = useUi();
  const key = detail ? `detail:${detail.title}:${JSON.stringify(detail.query)}` : tab;
  const isOnline = !detail && tab === "online";

  let view;
  if (detail) {
    view = <TrackView title={detail.title} subtitle={detail.subtitle} baseQuery={detail.query} onBack={closeDetail} playlistId={detail.playlistId} />;
  } else {
    switch (tab) {
      case "artists": view = <ArtistsView />; break;
      case "genres": view = <GenresView />; break;
      case "albums": view = <AlbumsView />; break;
      case "playlists": view = <PlaylistsView />; break;
      case "folders": view = <FoldersView />; break;
      case "online": view = <OnlinePage />; break;
      default: view = <TrackView title="Songs" baseQuery={{}} useSearch offerAddFolder />;
    }
  }

  return (
    <div className="flex h-full flex-col">
      <ScanStatus />
      <div className="relative min-h-0 flex-1">
        {/*
          ROOT-CAUSE FIX:
          The previous <AnimatePresence mode="wait"> made the NEW view's mount depend on the OLD view finishing
          its exit animation. If that exit never resolved (or the old filtered layer stalled the compositor),
          the new view was never mounted and the screen stayed blank.
          Now the view is swapped by `key` with an enter-only animation: the old view is unmounted
          synchronously and the new one mounts immediately, so nothing can block it.
        */}
        <motion.div
          key={key}
          className="absolute inset-0"
          initial={isOnline ? ONLINE_INITIAL : ENTER_INITIAL}
          animate={isOnline ? ONLINE_ANIMATE : ENTER_ANIMATE}
          transition={isOnline ? ONLINE_TRANSITION : ENTER_TRANSITION}
        >
          {view}
        </motion.div>
      </div>
    </div>
  );
}
