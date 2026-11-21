import { minimizeWindow } from "../lib/ipc";
import { PulseLogo } from "./Logo";

const btn = "grid h-8 w-10 place-items-center rounded-lg text-dim transition-colors hover:bg-ink/10 hover:text-ink";

export function TitleBar({ title, onClose, closeDisabled }: { title: string; onClose: () => void; closeDisabled: boolean }) {
  return (
    <header data-tauri-drag-region className="flex h-12 shrink-0 items-center justify-between px-4">
      <div data-tauri-drag-region className="flex items-center gap-2.5">
        <PulseLogo size={22} />
        <span data-tauri-drag-region className="text-sm font-semibold tracking-tight text-ink/85">{title}</span>
      </div>
      <div className="flex items-center">
        <button className={btn} aria-label="Minimize" onClick={() => void minimizeWindow()}>
          <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
        </button>
        <button className={`${btn} hover:!bg-red-500/80 hover:!text-white disabled:opacity-30`} aria-label="Close" disabled={closeDisabled} onClick={onClose}>
          <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
        </button>
      </div>
    </header>
  );
}
