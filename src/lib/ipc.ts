import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

let resolveInitialOnlineProxy: () => void = () => {};
const initialOnlineProxy = new Promise<void>((resolve) => {
  resolveInitialOnlineProxy = resolve;
});
let onlineProxyUpdate: Promise<void> = Promise.resolve();

export interface Track {
  id: number;
  folderId: number | null;
  path: string;
  filename: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  genre: string;
  year: number | null;
  trackNo: number | null;
  durationMs: number;
  bitrateKbps: number | null;
  sampleRate: number | null;
  artPath: string | null;
  playCount: number;
  addedAt: number;
  rating: number;
  isFavorite: boolean;
  lastPlayed: number | null;
}

export interface Folder { id: number; path: string; watch: boolean; addedAt: number; trackCount: number; isHidden: boolean }
export interface Playlist { id: number; name: string; trackCount: number; createdAt: number }
export interface ArtistRow { name: string; trackCount: number; albumCount: number; artPath: string | null }
export interface AlbumRow { name: string; artist: string; trackCount: number; year: number | null; durationMs: number; artPath: string | null }
export interface GenreRow { name: string; trackCount: number; artPath: string | null }
export interface ScanProgress { folder: string; scanned: number; total: number; added: number; current: string; done: boolean }
export interface ReplayGainProgress { current: number; total: number; name: string }
export interface Tick { trackId: number; positionMs: number; durationMs: number; playing: boolean }
export interface LyricLine { timeMs: number | null; text: string }
export interface Lyrics { synced: boolean; source: string; lines: LyricLine[] }
export interface AudioDevices { devices: string[]; default: string | null; current: string | null }
export type RepeatMode = "off" | "all" | "one";
export interface PlayerSnapshot { shuffle: boolean; repeat: RepeatMode; volume: number }

export type SortKey = "title" | "artist" | "album" | "duration" | "added" | "plays" | "rating" | "year";

export interface TrackQuery {
  query?: string;
  sort?: SortKey;
  desc?: boolean;
  favoritesOnly?: boolean;
  minRating?: number;
  playedOnly?: boolean;
  artist?: string;
  album?: string;
  albumArtist?: string;
  genre?: string;
  folderId?: number;
  playlistId?: number;
  limit?: number;
  offset?: number;
}

export const api = {
  // online
  setOnlineProxy: (enabled: boolean) => {
    onlineProxyUpdate = invoke<void>("online_set_proxy", { enabled });
    resolveInitialOnlineProxy();
    return onlineProxyUpdate;
  },
  onlineSearch: async (query: string, limit = 50) => {
    await initialOnlineProxy;
    await onlineProxyUpdate;
    return invoke<unknown>("online_search", { query, limit });
  },
  onlinePlay: async (track: unknown) => {
    await initialOnlineProxy;
    await onlineProxyUpdate;
    return invoke<void>("online_play", { track });
  },
  // library
  getTracks: (q: TrackQuery) => invoke<Track[]>("get_tracks", { q }),
  listArtists: () => invoke<ArtistRow[]>("list_artists"),
  listAlbums: () => invoke<AlbumRow[]>("list_albums"),
  listGenres: () => invoke<GenreRow[]>("list_genres"),
  listFolders: () => invoke<Folder[]>("list_folders"),
  addFolder: (path: string) => invoke<Folder>("add_folder", { path }),
  removeFolder: (id: number) => invoke<void>("remove_folder", { id }),
  setFolderHidden: (id: number, hidden: boolean) => invoke<void>("set_folder_hidden", { id, hidden }),
  rescanAll: () => invoke<void>("rescan_all"),
  analyzeLibrary: () => invoke<void>("analyze_library"),
  // playlists
  listPlaylists: () => invoke<Playlist[]>("list_playlists"),
  createPlaylist: (name: string) => invoke<Playlist>("create_playlist", { name }),
  renamePlaylist: (id: number, name: string) => invoke<void>("rename_playlist", { id, name }),
  deletePlaylist: (id: number) => invoke<void>("delete_playlist", { id }),
  addToPlaylist: (playlistId: number, trackIds: number[]) => invoke<void>("add_to_playlist", { playlistId, trackIds }),
  removeFromPlaylist: (playlistId: number, trackId: number) => invoke<void>("remove_from_playlist", { playlistId, trackId }),
  // track details
  setRating: (id: number, rating: number) => invoke<void>("set_rating", { id, rating }),
  setFavorite: (id: number, favorite: boolean) => invoke<void>("set_favorite", { id, favorite }),
  updateMetadata: (id: number, title: string, artist: string, album: string) =>
    invoke<Track>("update_metadata", { id, title, artist, album }),
  getLyrics: (id: number) => invoke<Lyrics | null>("get_lyrics", { id }),
  // playback
  playQueue: (trackIds: number[], startIndex: number) => invoke<void>("play_queue", { trackIds, startIndex }),
  playFileByPath: (path: string) => invoke<void>("play_file_by_path", { path }),
  toggle: () => invoke<void>("toggle_playback"),
  next: () => invoke<void>("next_track"),
  prev: () => invoke<void>("prev_track"),
  stop: () => invoke<void>("stop_playback"),
  seekTo: (positionMs: number) => invoke<void>("seek_to", { positionMs }),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setShuffle: (on: boolean) => invoke<void>("set_shuffle", { on }),
  setRepeat: (mode: RepeatMode) => invoke<void>("set_repeat", { mode }),
  getPlayerState: () => invoke<PlayerSnapshot>("get_player_state"),
  // audio settings
  listAudioDevices: () => invoke<AudioDevices>("list_audio_devices"),
  setAudioDevice: (name: string | null) => invoke<void>("set_audio_device", { name }),
  setEq: (enabled: boolean, gains: number[]) => invoke<void>("set_eq", { enabled, gains }),
  setCrossfade: (ms: number) => invoke<void>("set_crossfade", { ms }),
  setGapless: (on: boolean) => invoke<void>("set_gapless", { on }),
  setVirtualizer: (enabled: boolean, profileName: string | null) => invoke<void>("set_virtualizer", { enabled, profileName }),
  setCrystalizer: (mode: string, intensity: number) => invoke<void>("set_crystalizer", { mode, intensity }),
  setEnhancer: (
    bass: number,
    treble: number,
    width: number,
    surround: boolean,
    booster: boolean,
  ) =>
    invoke<void>("set_enhancer", {
      bass,
      treble,
      width,
      surround,
      booster,
    }),
  setReplayGain: (enabled: boolean) => invoke<void>("set_replaygain", { enabled }),
  // app
  getSettings: () => invoke<Record<string, unknown>>("get_settings"),
  setSetting: (key: string, value: unknown) => invoke<void>("set_setting", { key, value }),
  setMiniPlayer: (on: boolean) => invoke<void>("set_mini_player", { on }),
  setAlwaysOnTop: (on: boolean) => invoke<void>("set_always_on_top", { on }),
};

export async function pickFolder(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

export const onEvent = <T,>(name: string, cb: (payload: T) => void): Promise<UnlistenFn> =>
  listen<T>(name, (e) => cb(e.payload));

export const artUrl = (path: string | null | undefined) =>
  path ? (/^https?:\/\//i.test(path) ? path : convertFileSrc(path)) : undefined;
