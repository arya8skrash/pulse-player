import { create } from "zustand";
import { api, onEvent, type RepeatMode, type Tick, type Track } from "../lib/ipc";

// The queue, shuffle and repeat live in Rust; this store mirrors playback state for the UI.
interface PlayerState {
  current: Track | null;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  error: string | null;

  init: () => Promise<() => void>;
  playTracks: (tracks: Track[], index: number) => Promise<void>;
  toggle: () => Promise<void>;
  next: () => Promise<void>;
  prev: () => Promise<void>;
  seek: (ms: number) => Promise<void>;
  setVolume: (v: number) => void;
  toggleShuffle: () => Promise<void>;
  cycleRepeat: () => Promise<void>;
  patchCurrent: (id: number, patch: Partial<Track>) => void;
}

let volTimer: ReturnType<typeof setTimeout> | undefined;

export const usePlayer = create<PlayerState>()((set, get) => ({
  current: null,
  playing: false,
  positionMs: 0,
  durationMs: 0,
  volume: 0.8,
  shuffle: false,
  repeat: "off",
  error: null,

  init: async () => {
    const snap = await api.getPlayerState();
    set({ shuffle: snap.shuffle, repeat: snap.repeat, volume: snap.volume });
    const offs = await Promise.all([
      onEvent<Track>("player:track-changed", (t) => set({ current: t, positionMs: 0, durationMs: t.durationMs, error: null })),
      onEvent<Tick>("player:tick", (t) => set({ positionMs: t.positionMs, durationMs: t.durationMs || get().durationMs, playing: t.playing })),
      onEvent<null>("player:queue-ended", () => set({ playing: false, positionMs: 0 })),
      onEvent<null>("player:stopped", () => set({ playing: false, positionMs: 0 })),
      onEvent<string>("player:error", (message) => set({ error: message })),
    ]);
    return () => offs.forEach((off) => off());
  },

  playTracks: async (tracks, index) => {
    if (!tracks[index]) return;
    set({ playing: true, error: null });
    await api.playQueue(tracks.map((t) => t.id), index);
  },

  toggle: async () => {
    if (!get().current) return;
    set({ playing: !get().playing });
    await api.toggle();
  },
  next: () => api.next(),
  prev: () => api.prev(),

  seek: async (ms) => {
    set({ positionMs: ms });
    await api.seekTo(Math.round(ms));
  },

  setVolume: (v) => {
    set({ volume: v });
    void api.setVolume(v);
    clearTimeout(volTimer);
    volTimer = setTimeout(() => void api.setSetting("player.volume", v), 300);
  },

  toggleShuffle: async () => {
    const on = !get().shuffle;
    set({ shuffle: on });
    await api.setShuffle(on);
  },

  cycleRepeat: async () => {
    const order: RepeatMode[] = ["off", "all", "one"];
    const mode = order[(order.indexOf(get().repeat) + 1) % 3];
    set({ repeat: mode });
    await api.setRepeat(mode);
  },

  patchCurrent: (id, patch) => {
    const cur = get().current;
    if (cur && cur.id === id) set({ current: { ...cur, ...patch } });
  },
}));
