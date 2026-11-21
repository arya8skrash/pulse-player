import { create } from "zustand";
import { api } from "../lib/ipc";
import type { Mode, Scheme } from "../themes";

export const DEFAULT_EQ = { enabled: false, gains: Array<number>(10).fill(0) };

interface SettingsState {
  loaded: boolean;
  mode: Mode;
  scheme: Scheme;
  animSpeed: number;
  blur: number;
  closeToTray: boolean;
  crossfade: number;
  gapless: boolean;
  eq: { enabled: boolean; gains: number[] };
  device: string | null;
  visualizer: boolean; 
  discordRpc: boolean; 
  virtualizerEnabled: boolean;
  virtualizerProfile: string | null;
  crystalizerMode: string;
  crystalizerIntensity: number;
  enhancerBass: number;
  enhancerTreble: number;
  enhancerWidth: number;
  enhancerSurround: boolean;
  enhancerBooster: boolean;
  replaygainEnabled: boolean;
  
  replayGainAnalyzing: boolean;
  replayGainProgress: { current: number; total: number; name: string } | null;

  load: () => Promise<void>;
  setMode: (m: Mode) => void;
  setScheme: (s: Scheme) => void;
  setAnimSpeed: (v: number) => void;
  setBlur: (v: number) => void;
  setCloseToTray: (on: boolean) => void;
  setCrossfade: (ms: number) => void;
  setGapless: (on: boolean) => void;
  setEq: (eq: { enabled: boolean; gains: number[] }) => void;
  setDevice: (name: string | null) => void;
  setVisualizer: (on: boolean) => void; 
  setDiscordRpc: (on: boolean) => void; 
  setVirtualizerEnabled: (on: boolean) => void;
  setVirtualizerProfile: (profile: string | null) => void;
  setCrystalizer: (mode: string, intensity: number) => void;
  setEnhancer: (
    bass: number,
    treble: number,
    width: number,
    surround?: boolean,
    booster?: boolean,
  ) => void;
  setEnhancerSurround: (enabled: boolean) => void;
  setEnhancerBooster: (enabled: boolean) => void;
  setReplayGain: (enabled: boolean) => void;
  
  startReplayGainAnalysis: () => Promise<void>;
  setReplayGainProgress: (progress: { current: number; total: number; name: string } | null) => void;
  setReplayGainAnalyzing: (analyzing: boolean) => void;
  
  useSystemProxy: boolean;
  setUseSystemProxy: (v: boolean) => void;
}

const mq = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)") : null;

function apply(mode: Mode, scheme: Scheme, animate: boolean) {
  const resolved = mode === "system" ? (mq?.matches ? "dark" : "light") : mode;
  const swap = () => {
    const root = document.documentElement;
    root.dataset.theme = scheme;
    root.dataset.mode = resolved;
  };
  const doc = document as Document & { startViewTransition?: (cb: () => void) => unknown };
  if (animate && doc.startViewTransition && !matchMedia("(prefers-reduced-motion: reduce)").matches) {
    doc.startViewTransition(swap);
  } else {
    swap();
  }
}

function applyScalars(animSpeed: number, blur: number) {
  const root = document.documentElement.style;
  root.setProperty("--anim-speed", String(animSpeed));
  root.setProperty("--blur", `${blur}px`);
}

export const useSettings = create<SettingsState>()((set, get) => ({
  loaded: false,
  mode: "dark",
  scheme: "midnight",
  animSpeed: 1,
  blur: 25,
  closeToTray: false,
  crossfade: 0,
  gapless: true,
  eq: DEFAULT_EQ,
  device: null,
  visualizer: true, 
  discordRpc: true,
  virtualizerEnabled: false,
  virtualizerProfile: null,
  crystalizerMode: "crystalizer",
  crystalizerIntensity: 0.5,
  enhancerBass: 0.0,
  enhancerTreble: 0.0,
  enhancerWidth: 0.0,
  enhancerSurround: false,
  enhancerBooster: false,
  replaygainEnabled: false,
  
  replayGainAnalyzing: false,
  replayGainProgress: null,

  useSystemProxy: false,

  load: async () => {
    const s = await api.getSettings();
    const mode = (["dark", "light", "system"].includes(s["ui.mode"] as string) ? s["ui.mode"] : "dark") as Mode;
    const scheme = (["midnight", "amethyst", "ocean", "rose", "emerald", "slate", "copper", "twilight"].includes(s["ui.scheme"] as string) ? s["ui.scheme"] : "midnight") as Scheme;
    const animSpeed = typeof s["ui.animSpeed"] === "number" ? (s["ui.animSpeed"] as number) : 1;
    const blur = typeof s["ui.blur"] === "number" ? (s["ui.blur"] as number) : 25;
    const eqRaw = s["audio.eq"] as { enabled?: boolean; gains?: number[] } | undefined;
    const virtRaw = s["audio.virtualizer"] as { enabled?: boolean; profile?: string | null } | undefined;
    const cryRaw = s["audio.crystalizer"] as { mode?: string; intensity?: number } | undefined;
    const enhRaw = s["audio.enhancer"] as {
      bass?: number;
      treble?: number;
      width?: number;
      surround?: boolean;
      booster?: boolean;
    } | undefined;

    set({
      mode,
      scheme,
      animSpeed,
      blur,
      closeToTray: s["general.closeToTray"] === true,
      crossfade: typeof s["audio.crossfade"] === "number" ? (s["audio.crossfade"] as number) : 0,
      gapless: s["audio.gapless"] !== false,
      eq: eqRaw ? { enabled: !!eqRaw.enabled, gains: (eqRaw.gains ?? DEFAULT_EQ.gains).slice(0, 10) } : DEFAULT_EQ,
      device: typeof s["audio.device"] === "string" ? (s["audio.device"] as string) : null,
      visualizer: s["ui.visualizer"] !== false, 
      discordRpc: s["general.discordRpc"] !== false,
      virtualizerEnabled: virtRaw ? !!virtRaw.enabled : false,
      virtualizerProfile: virtRaw && typeof virtRaw.profile === "string" ? virtRaw.profile : null,
      crystalizerMode: cryRaw?.mode ?? "crystalizer",
      crystalizerIntensity: typeof cryRaw?.intensity === "number" ? cryRaw.intensity : 0.5,
      enhancerBass: typeof enhRaw?.bass === "number" ? enhRaw.bass : 0.0,
      enhancerTreble: typeof enhRaw?.treble === "number" ? enhRaw.treble : 0.0,
      enhancerWidth: typeof enhRaw?.width === "number" ? enhRaw.width : 0.0,
      enhancerSurround: enhRaw?.surround === true,
      enhancerBooster: enhRaw?.booster === true,
      replaygainEnabled: s["audio.replaygain"] === true,
      useSystemProxy: s["general.useSystemProxy"] === true,
      loaded: true,
    });
    apply(mode, scheme, false);
    applyScalars(animSpeed, blur);
    mq?.addEventListener("change", () => {
      if (get().mode === "system") apply("system", get().scheme, true);
    });
  },

  setMode: (mode) => {
    set({ mode });
    apply(mode, get().scheme, true);
    void api.setSetting("ui.mode", mode);
  },
  setScheme: (scheme) => {
    set({ scheme });
    apply(get().mode, scheme, true);
    void api.setSetting("ui.scheme", scheme);
  },
  setAnimSpeed: (animSpeed) => {
    set({ animSpeed });
    applyScalars(animSpeed, get().blur);
    void api.setSetting("ui.animSpeed", animSpeed);
  },
  setBlur: (blur) => {
    set({ blur });
    applyScalars(get().animSpeed, blur);
    void api.setSetting("ui.blur", blur);
  },
  setCloseToTray: (closeToTray) => {
    set({ closeToTray });
    void api.setSetting("general.closeToTray", closeToTray);
  },
  setCrossfade: (crossfade) => {
    set({ crossfade });
    void api.setCrossfade(crossfade);
  },
  setGapless: (gapless) => {
    set({ gapless });
    void api.setGapless(gapless);
  },
  setEq: (eq) => {
    set({ eq });
    void api.setEq(eq.enabled, eq.gains).catch(() => {});
  },
  setDevice: (device) => {
    set({ device });
    void api.setAudioDevice(device);
  },
  setVisualizer: (visualizer) => { 
    set({ visualizer });
    void api.setSetting("ui.visualizer", visualizer);
  },
  setDiscordRpc: (discordRpc) => { 
    set({ discordRpc });
    void api.setSetting("general.discordRpc", discordRpc);
  },
  setVirtualizerEnabled: (enabled) => {
    set({ virtualizerEnabled: enabled });
    void api.setVirtualizer(enabled, get().virtualizerProfile);
  },
  setVirtualizerProfile: (profile) => {
    set({ virtualizerProfile: profile });
    void api.setVirtualizer(get().virtualizerEnabled, profile);
  },
  setCrystalizer: (mode, intensity) => {
    set({ crystalizerMode: mode, crystalizerIntensity: intensity });
    void api.setCrystalizer(mode, intensity);
  },
  setEnhancer: (
    bass,
    treble,
    width,
    surround = get().enhancerSurround,
    booster = get().enhancerBooster,
  ) => {
    set({
      enhancerBass: bass,
      enhancerTreble: treble,
      enhancerWidth: width,
      enhancerSurround: surround,
      enhancerBooster: booster,
    });
    void api.setEnhancer(bass, treble, width, surround, booster);
  },
  setEnhancerSurround: (surround) => {
    const s = get();
    s.setEnhancer(
      s.enhancerBass,
      s.enhancerTreble,
      s.enhancerWidth,
      surround,
      s.enhancerBooster,
    );
  },
  setEnhancerBooster: (booster) => {
    const s = get();
    s.setEnhancer(
      s.enhancerBass,
      s.enhancerTreble,
      s.enhancerWidth,
      s.enhancerSurround,
      booster,
    );
  },
  setReplayGain: (enabled) => {
    set({ replaygainEnabled: enabled });
    void api.setReplayGain(enabled);
  },
  
  startReplayGainAnalysis: async () => {
    set({ replayGainAnalyzing: true, replayGainProgress: { current: 0, total: 0, name: "Starting..." } });
    try {
      await api.analyzeLibrary();
    } catch (e) {
      console.error(e);
      set({ replayGainAnalyzing: false, replayGainProgress: null });
    }
  },
  setReplayGainProgress: (progress) => {
    set({ replayGainProgress: progress });
  },
  setReplayGainAnalyzing: (analyzing) => {
    set({ replayGainAnalyzing: analyzing });
  },
  setUseSystemProxy: (useSystemProxy) => {
    set({ useSystemProxy });
    void api.setSetting("general.useSystemProxy", useSystemProxy);
  },
}));