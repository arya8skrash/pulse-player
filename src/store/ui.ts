import { create } from "zustand";
import type { Track, TrackQuery } from "../lib/ipc";

export type Tab = "artists" | "genres" | "albums" | "songs" | "playlists" | "folders" | "online";
export type SettingsTab = "library" | "appearance" | "audio" | "playback" | "equalizer" | "virtualizer" | "crystalizer" | "enhancer" | "general";

/** A drill-down: tracks matching `query`, shown under a title with a back button. */
export interface Detail {
  title: string;
  subtitle?: string;
  query: TrackQuery;
  playlistId?: number;
}

interface UiState {
  tab: Tab;
  detail: Detail | null;
  search: string;
  version: number; // bump to make library views refetch
  settingsOpen: boolean;
  settingsTab: SettingsTab;
  nowPlayingOpen: boolean;
  editing: Track | null;
  pickerTracks: Track[] | null;
  mini: boolean;
  showVisualizer: boolean; 

  setTab: (t: Tab) => void;
  openDetail: (d: Detail) => void;
  closeDetail: () => void;
  setSearch: (s: string) => void;
  bump: () => void;
  openSettings: (tab?: SettingsTab) => void;
  closeSettings: () => void;
  setSettingsTab: (t: SettingsTab) => void;
  setNowPlaying: (open: boolean) => void;
  setEditing: (t: Track | null) => void;
  setPickerTracks: (t: Track[] | null) => void;
  setMini: (on: boolean) => void;
  setShowVisualizer: (open: boolean) => void; 
}

export const useUi = create<UiState>()((set) => ({
  tab: "songs",
  detail: null,
  search: "",
  version: 0,
  settingsOpen: false,
  settingsTab: "library",
  nowPlayingOpen: false,
  editing: null,
  pickerTracks: null,
  mini: false,
  showVisualizer: false,

  setTab: (tab) => set({ tab, detail: null }),
  openDetail: (detail) => set({ detail }),
  closeDetail: () => set({ detail: null }),
  setSearch: (search) => set((s) => (search && s.tab !== "songs" ? { search, tab: "songs", detail: null } : { search })),
  bump: () => set((s) => ({ version: s.version + 1 })),
  openSettings: (tab) => set((s) => ({ settingsOpen: true, settingsTab: tab ?? s.settingsTab })),
  closeSettings: () => set({ settingsOpen: false }),
  setSettingsTab: (settingsTab) => set({ settingsTab }),
  setNowPlaying: (nowPlayingOpen) => set({ nowPlayingOpen }),
  setEditing: (editing) => set({ editing }),
  setPickerTracks: (pickerTracks) => set({ pickerTracks }),
  setMini: (mini) => set({ mini }),
  setShowVisualizer: (showVisualizer) => set({ showVisualizer }), 
}));