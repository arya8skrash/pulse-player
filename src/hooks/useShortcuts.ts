import { useEffect } from "react";
import { usePlayer } from "../store/player";
import { useUi } from "../store/ui";

const isTyping = (el: EventTarget | null) => {
  const t = el as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
};

/**
 * In-app shortcuts. (System-wide media keys are registered in Rust; Space and Ctrl+arrows are
 * deliberately app-scoped so Pulse doesn't hijack them from other programs.)
 *   Space  play/pause     Ctrl+Right / Ctrl+Left  next / previous
 *   Ctrl+F focus search   Ctrl+Up / Ctrl+Down     volume     Ctrl+, settings
 */
export function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const player = usePlayer.getState();
      const ui = useUi.getState();
      const ctrl = e.ctrlKey || e.metaKey;

      if (ctrl && e.key.toLowerCase() === "f") {
        e.preventDefault();
        if (ui.mini) return;
        const el = document.getElementById("pulse-search") as HTMLInputElement | null;
        el?.focus();
        el?.select();
        return;
      }
      if (ctrl && e.key === ",") {
        e.preventDefault();
        ui.openSettings();
        return;
      }
      if (isTyping(e.target)) return;

      if (e.code === "Space") {
        e.preventDefault();
        void player.toggle();
      } else if (ctrl && e.key === "ArrowRight") {
        e.preventDefault();
        void player.next();
      } else if (ctrl && e.key === "ArrowLeft") {
        e.preventDefault();
        void player.prev();
      } else if (ctrl && e.key === "ArrowUp") {
        e.preventDefault();
        player.setVolume(Math.min(1, player.volume + 0.05));
      } else if (ctrl && e.key === "ArrowDown") {
        e.preventDefault();
        player.setVolume(Math.max(0, player.volume - 0.05));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
