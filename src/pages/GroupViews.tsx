import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { api, type AlbumRow, type ArtistRow, type GenreRow } from "../lib/ipc";
import { formatTotal, plural } from "../lib/format";
import { useUi } from "../store/ui";
import { ArtImage } from "../components/ArtImage";

function useGroup<T>(load: () => Promise<T[]>) {
  const version = useUi((s) => s.version);
  const [rows, setRows] = useState<T[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    load().then((r) => !cancelled && setRows(r)).catch(() => !cancelled && setRows([]));
    return () => { cancelled = true; };
  }, [version]); // eslint-disable-line react-hooks/exhaustive-deps
  return rows;
}

function Card({ title, sub, art, round, onClick }: { title: string; sub: string; art: string | null; round?: boolean; onClick: () => void }) {
  return (
    <motion.button onClick={onClick} whileHover={{ y: -5 }} whileTap={{ scale: 0.97 }} transition={{ type: "spring", stiffness: 420, damping: 24 }}
      className="group flex flex-col gap-2.5 rounded-2xl p-3 text-left transition-colors hover:bg-ink/[0.07]"
      style={{ contentVisibility: "auto", containIntrinsicSize: "230px" }}>
      <div className={`aspect-square w-full overflow-hidden shadow-lg transition-shadow group-hover:shadow-glow ${round ? "rounded-full" : "rounded-xl"}`}>
        <ArtImage path={art} className="h-full w-full" markSize={56} />
      </div>
      <div className="min-w-0 px-1">
        <p className="truncate text-sm font-semibold">{title}</p>
        <p className="truncate text-xs text-dim">{sub}</p>
      </div>
    </motion.button>
  );
}

const Grid = ({ children }: { children: React.ReactNode }) => (
  <div className="h-full overflow-y-auto p-4"><div className="grid grid-cols-[repeat(auto-fill,minmax(168px,1fr))] gap-1">{children}</div></div>
);

const Empty = ({ what }: { what: string }) => (
  <div className="grid h-full place-items-center text-center"><p className="text-sm text-dim">No {what} yet. Add a music folder from the Songs tab.</p></div>
);

export function ArtistsView() {
  const rows = useGroup<ArtistRow>(api.listArtists);
  const open = useUi((s) => s.openDetail);
  if (rows && rows.length === 0) return <Empty what="artists" />;
  return (
    <Grid>
      {rows?.map((a) => (
        <Card key={a.name} round title={a.name || "Unknown artist"} art={a.artPath}
          sub={`${plural(a.albumCount, "album")} · ${plural(a.trackCount, "song")}`}
          onClick={() => open({ title: a.name || "Unknown artist", subtitle: plural(a.albumCount, "album"), query: { artist: a.name } })} />
      ))}
    </Grid>
  );
}

export function AlbumsView() {
  const rows = useGroup<AlbumRow>(api.listAlbums);
  const open = useUi((s) => s.openDetail);
  if (rows && rows.length === 0) return <Empty what="albums" />;
  return (
    <Grid>
      {rows?.map((a) => (
        <Card key={`${a.name}|${a.artist}`} title={a.name || "Unknown album"} art={a.artPath}
          sub={[a.artist || "Unknown artist", a.year].filter(Boolean).join(" · ")}
          onClick={() => open({
            title: a.name || "Unknown album",
            subtitle: `${a.artist || "Unknown artist"} · ${formatTotal(a.durationMs)}`,
            query: { album: a.name, albumArtist: a.artist },
          })} />
      ))}
    </Grid>
  );
}

export function GenresView() {
  const rows = useGroup<GenreRow>(api.listGenres);
  const open = useUi((s) => s.openDetail);
  if (rows && rows.length === 0) return <Empty what="genres" />;
  return (
    <Grid>
      {rows?.map((g) => (
        <Card key={g.name} title={g.name || "No genre"} art={g.artPath} sub={plural(g.trackCount, "song")}
          onClick={() => open({ title: g.name || "No genre", query: { genre: g.name } })} />
      ))}
    </Grid>
  );
}
