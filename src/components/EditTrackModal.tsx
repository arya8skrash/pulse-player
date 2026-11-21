import { useEffect, useRef, useState } from "react";
import { api, type Track } from "../lib/ipc";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";
import { Btn, Modal } from "./ui";

function Form({ track, onDone }: { track: Track; onDone: () => void }) {
  const bump = useUi((s) => s.bump);
  const [title, setTitle] = useState(track.title);
  const [artist, setArtist] = useState(track.artist);
  const [album, setAlbum] = useState(track.album);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const updated = await api.updateMetadata(track.id, title, artist, album);
      usePlayer.getState().patchCurrent(track.id, updated);
      bump();
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const field = "glass-pill h-10 w-full bg-transparent px-4 text-sm outline-none focus:shadow-glow";
  return (
    <form className="w-[min(460px,90vw)] p-7" onSubmit={(e) => { e.preventDefault(); void save(); }}>
      <h2 className="text-xl font-bold">Edit tags</h2>
      <p className="mb-5 mt-1 truncate text-xs text-dim">{track.filename}</p>
      <label className="mb-1 block text-xs font-medium text-dim" htmlFor="et-title">Title</label>
      <input id="et-title" autoFocus className={`${field} mb-4`} value={title} onChange={(e) => setTitle(e.target.value)} />
      <label className="mb-1 block text-xs font-medium text-dim" htmlFor="et-artist">Artist</label>
      <input id="et-artist" className={`${field} mb-4`} value={artist} onChange={(e) => setArtist(e.target.value)} />
      <label className="mb-1 block text-xs font-medium text-dim" htmlFor="et-album">Album</label>
      <input id="et-album" className={`${field} mb-2`} value={album} onChange={(e) => setAlbum(e.target.value)} />
      <p className="mb-5 text-xs text-dim">Changes are written into the audio file itself and into your library.</p>
      {error && <p role="alert" className="mb-4 text-sm text-red-400">{error}</p>}
      <div className="flex justify-end gap-2">
        <Btn onClick={onDone}>Cancel</Btn>
        <Btn variant="primary" type="submit" disabled={busy}>{busy ? "Saving…" : "Save"}</Btn>
      </div>
    </form>
  );
}

export function EditTrackModal() {
  const { editing, setEditing } = useUi();
  const last = useRef<Track | null>(null);
  useEffect(() => { if (editing) last.current = editing; }, [editing]);
  const shown = editing ?? last.current;
  return (
    <Modal open={!!editing} onClose={() => setEditing(null)} label="Edit tags">
      {shown && <Form key={shown.id} track={shown} onDone={() => setEditing(null)} />}
    </Modal>
  );
}
