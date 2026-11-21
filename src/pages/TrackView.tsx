import { useState } from "react";
import { api, pickFolder, type SortKey, type TrackQuery } from "../lib/ipc";
import { formatTime, plural } from "../lib/format";
import { useDebounced, useTracks } from "../hooks/useTracks";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";
import { TrackList } from "../components/TrackList";
import { Btn, Dropdown, IconBtn } from "../components/ui";
import { BackIcon, HeartIcon, PlayIcon, ShuffleIcon } from "../components/icons";

const SORTS: { value: SortKey | ""; label: string }[] = [
  { value: "", label: "Default order" },
  { value: "title", label: "Title" },
  { value: "artist", label: "Artist" },
  { value: "album", label: "Album" },
  { value: "duration", label: "Duration" },
  { value: "added", label: "Recently added" },
  { value: "plays", label: "Most played" },
  { value: "rating", label: "Rating" },
  { value: "year", label: "Year" },
];

export function TrackView({ title, subtitle, baseQuery, onBack, playlistId, useSearch, offerAddFolder }: {
  title: string;
  subtitle?: string;
  baseQuery: TrackQuery;
  onBack?: () => void;
  playlistId?: number;
  useSearch?: boolean;
  offerAddFolder?: boolean;
}) {
  const [sort, setSort] = useState<SortKey | "">(baseQuery.sort ?? "");
  const [desc, setDesc] = useState(baseQuery.desc ?? false);
  const [favs, setFavs] = useState(false);
  const [scrollTrigger, setScrollTrigger] = useState(0); 
  const search = useDebounced(useUi((s) => s.search), 150);
  const bump = useUi((s) => s.bump);

  const query: TrackQuery = {
    ...baseQuery,
    sort: sort || undefined,
    desc,
    favoritesOnly: favs || baseQuery.favoritesOnly,
    query: useSearch && search ? search : undefined,
  };
  const { tracks, loading, patch } = useTracks(query);

  const totalMs = tracks.reduce((a, t) => a + t.durationMs, 0);
  const playAll = () => void usePlayer.getState().playTracks(tracks, 0);
  const shuffleAll = async () => {
    if (!usePlayer.getState().shuffle) await usePlayer.getState().toggleShuffle();
    await usePlayer.getState().playTracks(tracks, Math.floor(Math.random() * tracks.length));
  };
  const addFolder = async () => {
    const p = await pickFolder();
    if (p) await api.addFolder(p);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 px-6 pb-3 pt-5">
        {onBack && <IconBtn label="Back" onClick={onBack}><BackIcon size={20} /></IconBtn>}
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-2xl font-bold tracking-tight">{title}</h1>
          <p className="text-sm text-dim">
            {subtitle ? `${subtitle} · ` : ""}{plural(tracks.length, "song")}{tracks.length > 0 ? ` · ${formatTime(totalMs)}` : ""}
          </p>
        </div>
        <Dropdown className="w-44" label="Sort by" value={sort} options={SORTS} onChange={setSort} />
        <Btn aria-label="Reverse sort order" onClick={() => setDesc((d) => !d)} className="h-9 w-24">{desc ? "Z → A ↓" : "A → Z ↑"}</Btn>
        <IconBtn label={favs ? "Showing favorites only" : "Show favorites only"} active={favs} onClick={() => setFavs((f) => !f)}><HeartIcon size={18} filled={favs} /></IconBtn>
        
        <IconBtn label="Scroll to Now Playing" onClick={() => setScrollTrigger(Date.now())}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="3"/>
          </svg>
        </IconBtn>

        <Btn variant="primary" disabled={!tracks.length} onClick={playAll} className="flex h-9 items-center gap-1.5"><PlayIcon size={16} />Play</Btn>
        <Btn disabled={!tracks.length} onClick={() => void shuffleAll()} className="flex h-9 items-center gap-1.5"><ShuffleIcon size={16} />Shuffle</Btn>
      </div>

      <div className="min-h-0 flex-1">
        {!loading && tracks.length === 0 ? (
          <div className="grid h-full place-items-center px-6 text-center">
            <div className="flex flex-col items-center gap-4">
              <p className="text-lg font-medium">{search && useSearch ? "No songs match your search" : favs ? "No favorites here yet" : "Nothing here yet"}</p>
              <p className="max-w-sm text-sm text-dim">
                {search && useSearch ? "Try a different word." : offerAddFolder ? "Add a folder and Pulse will find your music." : "Songs will show up here."}
              </p>
              {offerAddFolder && !search && <Btn variant="primary" onClick={() => void addFolder()}>Add music folder</Btn>}
            </div>
          </div>
        ) : (
          <TrackList tracks={tracks} onPatch={patch} scrollTrigger={scrollTrigger}
            onRemove={playlistId ? (t) => void api.removeFromPlaylist(playlistId, t.id).then(bump) : undefined} />
        )}
      </div>
    </div>
  );
}