import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { api, pickFolder, type Folder } from "../lib/ipc";
import { plural } from "../lib/format";
import { useUi } from "../store/ui";
import { Btn } from "../components/ui";
import { FolderIcon, PlusIcon } from "../components/icons";

export function FoldersView() {
  const version = useUi((s) => s.version);
  const openDetail = useUi((s) => s.openDetail);
  const [folders, setFolders] = useState<Folder[]>([]);
  // Cancellable load: ignore responses that arrive after unmount or after a newer refresh.
  useEffect(() => {
    let cancelled = false;
    api.listFolders().then((f) => { if (!cancelled) setFolders(f); }).catch(() => { if (!cancelled) setFolders([]); });
    return () => { cancelled = true; };
  }, [version]);

  const add = async () => {
    const p = await pickFolder();
    if (p) await api.addFolder(p);
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="mb-4 flex items-center">
        <h2 className="flex-1 text-sm font-semibold uppercase tracking-wider text-dim">Music folders</h2>
        <Btn variant="primary" onClick={() => void add()} className="flex h-9 items-center gap-1.5"><PlusIcon size={16} />Add folder</Btn>
      </div>
      {folders.length === 0 ? (
        <p className="text-sm text-dim">No folders yet. Add one and Pulse will scan it, including subfolders.</p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3">
          {folders.map((f) => (
            <motion.button key={f.id} whileHover={{ y: -3 }} whileTap={{ scale: 0.98 }} transition={{ type: "spring", stiffness: 420, damping: 24 }}
              onClick={() => openDetail({ title: f.path.split(/[\\/]/).filter(Boolean).pop() ?? f.path, subtitle: f.path, query: { folderId: f.id } })}
              className="glass glow-hover flex items-center gap-3 p-4 text-left">
              <span className="grid h-11 w-11 shrink-0 place-items-center rounded-xl bg-ink/10 text-accent"><FolderIcon size={22} /></span>
              <span className="min-w-0">
                <span className="block truncate text-sm font-semibold">{f.path.split(/[\\/]/).filter(Boolean).pop() ?? f.path}</span>
                <span className="block truncate text-xs text-dim">{f.path}</span>
                <span className="block text-xs text-dim">{plural(f.trackCount, "song")}</span>
              </span>
            </motion.button>
          ))}
        </div>
      )}
    </div>
  );
}
