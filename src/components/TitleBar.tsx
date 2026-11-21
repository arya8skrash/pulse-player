import { motion } from "framer-motion";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi, type Tab } from "../store/ui";
import { GearIcon, MiniIcon, SearchIcon, CloseIcon } from "./icons";
import { IconBtn } from "./ui";
import { PulseLogo } from "./PulseLogo";
import { api } from "../lib/ipc";

const TABS: { id: Tab; label: string }[] = [
  { id: "artists", label: "Artists" },
  { id: "genres", label: "Genres" },
  { id: "albums", label: "Albums" },
  { id: "songs", label: "Songs" },
  { id: "playlists", label: "Playlists" },
  { id: "folders", label: "Folders" },
  { id: "online", label: "Online" },
];

const winBtn = "grid h-8 w-10 place-items-center rounded-lg text-dim transition-colors hover:bg-ink/10 hover:text-ink";

// Native frame is off (tauri.conf.json), so Pulse draws its own title bar: logo, library tabs, search, window controls.
export function TitleBar() {
  const { tab, setTab, search, setSearch, openSettings } = useUi();
  const win = getCurrentWindow();

  return (
    <header data-tauri-drag-region className="flex h-14 shrink-0 items-center gap-3 px-4">
      <div data-tauri-drag-region className="flex items-center gap-2.5 pr-2">
        <PulseLogo size={28} />
        <span data-tauri-drag-region className="text-base font-semibold tracking-tight">Pulse</span>
      </div>

      <nav aria-label="Library" className="glass-pill relative flex p-1">
        {TABS.map((t) => (
          <button key={t.id} onClick={() => setTab(t.id)} aria-current={tab === t.id ? "page" : undefined}
            className={`relative rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors ${tab === t.id ? "text-white" : "text-dim hover:text-ink"}`}>
            {tab === t.id && (
              <motion.span layoutId="tab-pill" className="absolute inset-0 rounded-full shadow-glow [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]"
                transition={{ type: "spring", stiffness: 500, damping: 34 }} />
            )}
            <span className="relative">{t.label}</span>
          </button>
        ))}
      </nav>

      <div data-tauri-drag-region className="flex-1" />

      <label className="glass-pill glow-hover relative flex h-9 w-48 items-center gap-2 px-3 focus-within:w-60 focus-within:shadow-glow" style={{ transition: "width .3s cubic-bezier(.3,1.4,.5,1), box-shadow .25s" }}>
        <SearchIcon size={16} className="shrink-0 text-dim" />
        <input id="pulse-search" type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search"
          aria-label="Search your library (Ctrl+F)" className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-dim" />
        {search && <button aria-label="Clear search" onClick={() => setSearch("")} className="text-dim hover:text-ink"><CloseIcon size={14} /></button>}
      </label>

      <IconBtn label="Mini player" onClick={() => void api.setMiniPlayer(true)}><MiniIcon size={18} /></IconBtn>
      <IconBtn label="Settings (Ctrl+,)" onClick={() => openSettings()}><GearIcon size={18} /></IconBtn>

      <div className="ml-1 flex items-center">
        <button className={winBtn} aria-label="Minimize" onClick={() => void win.minimize()}>
          <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
        </button>
        <button className={winBtn} aria-label="Maximize" onClick={() => void win.toggleMaximize()}>
          <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.5" /></svg>
        </button>
        <button className={`${winBtn} hover:!bg-red-500/80 hover:!text-white`} aria-label="Close" onClick={() => void win.close()}>
          <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
        </button>
      </div>
    </header>
  );
}
