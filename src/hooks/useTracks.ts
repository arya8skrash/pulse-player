import { useCallback, useEffect, useState } from "react";
import { api, type Track, type TrackQuery } from "../lib/ipc";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";

/** Fetches tracks for a query, refetching when the query changes or the library does. */
export function useTracks(query: TrackQuery) {
  const version = useUi((s) => s.version);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const key = JSON.stringify(query);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    api
      .getTracks(JSON.parse(key) as TrackQuery)
      .then((t) => {
        if (!cancelled) {
          setTracks(t);
          setLoading(false);
        }
      })
      .catch(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [key, version]);

  /** Update one row locally (rating, favorite, edited tags) without refetching the list. */
  const patch = useCallback((id: number, p: Partial<Track>) => {
    setTracks((ts) => ts.map((t) => (t.id === id ? { ...t, ...p } : t)));
    usePlayer.getState().patchCurrent(id, p);
  }, []);

  return { tracks, loading, patch };
}

export function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}
