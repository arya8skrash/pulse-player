import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { api, type Playlist, type TrackQuery } from "../lib/ipc";
import { plural } from "../lib/format";
import { useUi } from "../store/ui";
import { Btn, IconBtn } from "../components/ui";
import { PencilIcon, PlusIcon, TrashIcon } from "../components/icons";

const SMART: { title: string; sub: string; query: TrackQuery }[] = [
  { title: "Favorites", sub: "Songs you hearted", query: { favoritesOnly: true } },
  { title: "Top rated", sub: "4 stars and up", query: { minRating: 4, sort: "rating", desc: true } },
  { title: "Most played", sub: "Your repeat listens", query: { playedOnly: true, sort: "plays", desc: true } },
  { title: "Recently added", sub: "Newest 100 songs", query: { sort: "added", desc: true, limit: 100 } },
];

const tile = "glass glow-hover flex items-center gap-3 p-4 text-left";

export function PlaylistsView() {
  const version = useUi((s) => s.version);
  const { openDetail, bump } = useUi();
  const [lists, setLists] = useState<Playlist[]>([]);
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<{ id: number; name: string } | null>(null);
  const [confirmDel, setConfirmDel] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Cancellable load: a late response can never overwrite newer state, and a failure no longer goes unhandled.
  useEffect(() => {
    let cancelled = false;
    api.listPlaylists().then((l) => { if (!cancelled) setLists(l); }).catch((e) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, [version]);

  const create = async () => {
    if (!name.trim()) return;
    try { await api.createPlaylist(name); setName(""); setError(null); bump(); } catch (e) { setError(String(e)); }
  };
  const rename = async () => {
    if (!renaming) return;
    try { await api.renamePlaylist(renaming.id, renaming.name); setRenaming(null); bump(); } catch (e) { setError(String(e)); }
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wider text-dim">Smart playlists</h2>
      <div className="mb-8 grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3">
        {SMART.map((s) => (
          <motion.button key={s.title} whileHover={{ y: -3 }} whileTap={{ scale: 0.98 }} transition={{ type: "spring", stiffness: 420, damping: 24 }}
            className={tile} onClick={() => openDetail({ title: s.title, subtitle: s.sub, query: s.query })}>
            <span className="grid h-11 w-11 shrink-0 place-items-center rounded-xl text-white [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]">★</span>
            <span className="min-w-0"><span className="block truncate text-sm font-semibold">{s.title}</span><span className="block truncate text-xs text-dim">{s.sub}</span></span>
          </motion.button>
        ))}
      </div>

      <div className="mb-3 flex items-center gap-3">
        <h2 className="flex-1 text-sm font-semibold uppercase tracking-wider text-dim">Your playlists</h2>
        <input value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void create()}
          placeholder="New playlist name" aria-label="New playlist name" className="glass-pill h-9 w-56 bg-transparent px-4 text-sm outline-none placeholder:text-dim focus:shadow-glow" />
        <Btn variant="primary" onClick={() => void create()} className="flex h-9 items-center gap-1.5"><PlusIcon size={16} />Create</Btn>
      </div>
      {error && <p role="alert" className="mb-3 text-sm text-red-400">{error}</p>}

      {lists.length === 0 ? (
        <p className="text-sm text-dim">No playlists yet. Create one above, then use the + button on any song to add it.</p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
          {/* Assumption: the layout prop was removed from these cards. Layout projection under an animating, transformed parent can leave views stuck or blank, and order only changes on add/delete. */}
          {lists.map((p) => (
            <div key={p.id} className={`${tile} group`}>
              {renaming?.id === p.id ? (
                <input autoFocus value={renaming.name} onChange={(e) => setRenaming({ id: p.id, name: e.target.value })}
                  onKeyDown={(e) => { if (e.key === "Enter") void rename(); if (e.key === "Escape") setRenaming(null); }} onBlur={() => void rename()}
                  aria-label="Playlist name" className="min-w-0 flex-1 bg-transparent text-sm font-semibold outline-none" />
              ) : (
                <button className="min-w-0 flex-1 text-left" onClick={() => openDetail({ title: p.name, subtitle: plural(p.trackCount, "song"), query: { playlistId: p.id }, playlistId: p.id })}>
                  <span className="block truncate text-sm font-semibold">{p.name}</span>
                  <span className="block text-xs text-dim">{plural(p.trackCount, "song")}</span>
                </button>
              )}
              <div className="flex opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                <IconBtn label="Rename playlist" onClick={() => setRenaming({ id: p.id, name: p.name })}><PencilIcon size={15} /></IconBtn>
                {confirmDel === p.id ? (
                  <Btn variant="danger" className="h-8 px-3" onClick={() => void api.deletePlaylist(p.id).then(() => { setConfirmDel(null); bump(); })}>Delete?</Btn>
                ) : (
                  <IconBtn label="Delete playlist" onClick={() => { setConfirmDel(p.id); setTimeout(() => setConfirmDel(null), 3000); }}><TrashIcon size={15} /></IconBtn>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
