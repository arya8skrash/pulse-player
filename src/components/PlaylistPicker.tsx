import { useEffect, useRef, useState } from "react";
import { api, type Playlist, type Track } from "../lib/ipc";
import { plural } from "../lib/format";
import { useUi } from "../store/ui";
import { Btn, Modal } from "./ui";

function Picker({ tracks, onDone }: { tracks: Track[]; onDone: () => void }) {
  const bump = useUi((s) => s.bump);
  const [lists, setLists] = useState<Playlist[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { void api.listPlaylists().then(setLists); }, []);

  const addTo = async (id: number) => {
    try { await api.addToPlaylist(id, tracks.map((t) => t.id)); bump(); onDone(); } catch (e) { setError(String(e)); }
  };
  const createAndAdd = async () => {
    if (!name.trim()) return;
    try { const p = await api.createPlaylist(name); await addTo(p.id); } catch (e) { setError(String(e)); }
  };

  return (
    <div className="w-[min(420px,90vw)] p-7">
      <h2 className="text-xl font-bold">Add to playlist</h2>
      <p className="mb-4 mt-1 truncate text-xs text-dim">{tracks.length === 1 ? tracks[0].title : plural(tracks.length, "song")}</p>
      <div className="mb-4 flex gap-2">
        <input value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void createAndAdd()} placeholder="New playlist"
          aria-label="New playlist name" className="glass-pill h-10 min-w-0 flex-1 bg-transparent px-4 text-sm outline-none placeholder:text-dim focus:shadow-glow" />
        <Btn variant="primary" onClick={() => void createAndAdd()}>Create</Btn>
      </div>
      {error && <p role="alert" className="mb-3 text-sm text-red-400">{error}</p>}
      <ul className="max-h-64 space-y-1 overflow-y-auto">
        {lists.length === 0 && <li className="text-sm text-dim">No playlists yet — create one above.</li>}
        {lists.map((p) => (
          <li key={p.id}>
            <button onClick={() => void addTo(p.id)} className="flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left text-sm transition-colors hover:bg-ink/10">
              <span className="truncate font-medium">{p.name}</span><span className="shrink-0 text-xs text-dim">{plural(p.trackCount, "song")}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="mt-4 flex justify-end"><Btn onClick={onDone}>Cancel</Btn></div>
    </div>
  );
}

export function PlaylistPicker() {
  const { pickerTracks, setPickerTracks } = useUi();
  const last = useRef<Track[] | null>(null);
  useEffect(() => { if (pickerTracks) last.current = pickerTracks; }, [pickerTracks]);
  const shown = pickerTracks ?? last.current;
  return (
    <Modal open={!!pickerTracks} onClose={() => setPickerTracks(null)} label="Add to playlist">
      {shown && <Picker tracks={shown} onDone={() => setPickerTracks(null)} />}
    </Modal>
  );
}
