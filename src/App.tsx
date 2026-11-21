import { useEffect, useState } from "react";
import { AnimatePresence, MotionConfig } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";

import { AuroraBackground } from "./components/AuroraBackground";
import SplashScreen from "./components/SplashScreen";
import { EditTrackModal } from "./components/EditTrackModal";
import { MiniPlayer } from "./components/MiniPlayer";
import { NowPlaying } from "./components/NowPlaying";
import { PlayerBar } from "./components/PlayerBar";
import { PlaylistPicker } from "./components/PlaylistPicker";
import { SettingsModal } from "./components/SettingsModal";
import { TitleBar } from "./components/TitleBar";
import VisualizerPage from "./components/VisualizerPage";
import { useShortcuts } from "./hooks/useShortcuts";
import { api, onEvent, type ReplayGainProgress } from "./lib/ipc";
import { LibraryPage } from "./pages/LibraryPage";
import { usePlayer } from "./store/player";
import { useSettings } from "./store/settings";
import { useUi } from "./store/ui";

export default function App() {
  const mini = useUi((s) => s.mini);
  const showVisualizer = useUi((s) => s.showVisualizer);
  
  const current = usePlayer((s) => s.current); 
  const playing = usePlayer((s) => s.playing);
  const discordRpc = useSettings((s) => s.discordRpc);
  const useSystemProxy = useSettings((s) => s.useSystemProxy);
  const settingsLoaded = useSettings((s) => s.loaded);

  const [showSplash, setShowSplash] = useState(true);
  
  const [isHidden, setIsHidden] = useState(false);

  useShortcuts();

  useEffect(() => {
    if (!settingsLoaded) return;
    void api.setOnlineProxy(useSystemProxy).catch((error) => {
      console.error("Could not configure the online system proxy:", error);
    });
  }, [settingsLoaded, useSystemProxy]);
  
  useEffect(() => {
    const unlisten = onEvent<boolean>("ui:tray-state", (hidden) => {
      setIsHidden(hidden);
    });
    
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    void useSettings.getState().load();

    let cancelled = false;
    const offs: (() => void)[] = [];
    const keep = (p: Promise<() => void>) => void p.then((f) => (cancelled ? f() : offs.push(f)));
    keep(usePlayer.getState().init());
    keep(onEvent<boolean>("ui:mini", (on) => useUi.getState().setMini(on)));

    keep(onEvent<string>("play-file", (path) => {
      void api.playFileByPath(path);
    }));

    keep(onEvent<ReplayGainProgress>("replaygain:progress", (p) => {
      useSettings.getState().setReplayGainProgress(p);
    }));

    keep(onEvent<null>("replaygain:done", () => {
      useSettings.getState().setReplayGainAnalyzing(false);
      useSettings.getState().setReplayGainProgress(null);
    }));

    return () => {
      cancelled = true;
      offs.forEach((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!discordRpc) {
      invoke("clear_rpc").catch(console.error);
      return;
    }

    let isMounted = true;
    const tryConnect = () => {
      if (!isMounted) return;
      invoke("init_rpc").catch((e) => {
        console.warn("Discord not ready, retrying in 10s...", e);
        setTimeout(tryConnect, 10000); 
      });
    };

    tryConnect();

    return () => {
      isMounted = false;
    };
  }, [discordRpc]);

  useEffect(() => {
    if (!discordRpc) return; 
    if (!current) {
      void invoke("clear_rpc").catch(console.error);
      return;
    }

    const sendUpdate = () => {
      const state = usePlayer.getState();
      if (!state.current) return;

      const currentSec = Math.floor(state.positionMs / 1000);
      const totalSec = Math.floor((state.durationMs || state.current.durationMs || 0) / 1000);

      invoke("update_rpc", {
        title: state.current.title || "Unknown Title",
        artist: state.current.artist || "Unknown Artist",
        isPlaying: state.playing,
        currentTime: currentSec,
        duration: totalSec,
      }).catch(console.error);
    };

    sendUpdate();

    if (playing && !isHidden) {
      const interval = setInterval(sendUpdate, 5000);
      return () => clearInterval(interval);
    }
  }, [current, playing, discordRpc, isHidden]);

  return (
    <MotionConfig reducedMotion="user">
      {!isHidden && (
        <div className="relative z-0 h-full w-full">
          <AuroraBackground />
          {mini ? (
            <MiniPlayer />
          ) : showVisualizer ? (
            <VisualizerPage onClose={() => useUi.getState().setShowVisualizer(false)} />
          ) : (
            <div className="flex h-full flex-col">
              <TitleBar />
              <main className="glass mx-4 mb-3 min-h-0 flex-1 overflow-hidden">
                <LibraryPage />
              </main>
              <div className="px-4 pb-4">
                <PlayerBar />
              </div>
              <NowPlaying />
              <SettingsModal />
              <EditTrackModal />
              <PlaylistPicker />
            </div>
          )}
        </div>
      )}

      <AnimatePresence>
        {showSplash && <SplashScreen onAnimationComplete={() => setShowSplash(false)} />}
      </AnimatePresence>
    </MotionConfig>
  );
}
