import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { api, pickFolder, type AudioDevices, type Folder } from "../lib/ipc";
import { plural } from "../lib/format";
import { SCHEMES, type Mode } from "../themes";
import { useSettings } from "../store/settings";
import { useUi, type SettingsTab } from "../store/ui";
import { Btn, Dropdown, IconBtn, Modal, Segmented, Slider, Switch, VSlider } from "./ui";
import { CloseIcon, FolderIcon, PlusIcon, RefreshIcon, TrashIcon, EyeIcon, EyeOffIcon } from "./icons";
import { Knob, ZrToggle } from "./Knob";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "library", label: "Library" },
  { id: "appearance", label: "Appearance" },
  { id: "audio", label: "Audio" },
  { id: "playback", label: "Playback" },
  { id: "equalizer", label: "Equalizer" },
  { id: "virtualizer", label: "Virtualization" },
  { id: "crystalizer", label: "Crystalizer" },
  { id: "enhancer", label: "Pulse ZR" },
  { id: "general", label: "General" },
];

const Row = ({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) => (
  <div className="flex items-center justify-between gap-6 border-b border-ink/10 py-4 last:border-0">
    <div className="min-w-0"><p className="text-sm font-medium">{title}</p>{hint && <p className="mt-0.5 text-xs text-dim">{hint}</p>}</div>
    <div className="shrink-0">{children}</div>
  </div>
);

// ---------------------------------------------------------------- Library
function LibraryPanel() {
  const bump = useUi((s) => s.bump);
  const version = useUi((s) => s.version);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [confirm, setConfirm] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { void api.listFolders().then(setFolders); }, [version]);

  const add = async () => {
    try { const p = await pickFolder(); if (p) { await api.addFolder(p); bump(); } } catch (e) { setError(String(e)); }
  };
  
  return (
    <div>
      <div className="mb-4 flex gap-2">
        <Btn variant="primary" className="flex items-center gap-1.5" onClick={() => void add()}><PlusIcon size={16} />Add folder</Btn>
        <Btn className="flex items-center gap-1.5" onClick={() => void api.rescanAll()}><RefreshIcon size={16} />Rescan all</Btn>
      </div>
      {error && <p role="alert" className="mb-3 text-sm text-red-400">{error}</p>}
      {folders.length === 0 && <p className="text-sm text-dim">No folders yet. Pulse scans folders recursively and rescans them every time it starts.</p>}
      <ul className="space-y-2">
        {folders.map((f) => (
          <li key={f.id} className={`glass-pill flex items-center gap-3 px-4 py-2.5 transition-opacity ${f.isHidden ? "opacity-50" : ""}`} style={{ borderRadius: 16 }}>
            <FolderIcon size={18} className={`shrink-0 ${f.isHidden ? "text-dim" : "text-accent"}`} />
            <div className="min-w-0 flex-1">
              <p className={`truncate text-sm ${f.isHidden ? "line-through text-dim" : ""}`}>{f.path}</p>
              <p className="text-xs text-dim">{plural(f.trackCount, "song")} {f.isHidden && "(Hidden)"}</p>
            </div>
            
            <IconBtn label={f.isHidden ? "Show folder" : "Hide folder"} onClick={() => void api.setFolderHidden(f.id, !f.isHidden).then(() => bump())}>
              {f.isHidden ? <EyeOffIcon size={16} /> : <EyeIcon size={16} />}
            </IconBtn>

            {confirm === f.id ? (
              <Btn variant="danger" className="h-8 px-3" onClick={() => void api.removeFolder(f.id).then(() => { setConfirm(null); bump(); })}>Remove?</Btn>
            ) : (
              <IconBtn label={`Remove ${f.path}`} onClick={() => { setConfirm(f.id); setTimeout(() => setConfirm(null), 3500); }}><TrashIcon size={16} /></IconBtn>
            )}
          </li>
        ))}
      </ul>
      <p className="mt-4 text-xs text-dim">Removing a folder only removes it from Pulse. Your files are never touched.</p>
    </div>
  );
}

// ---------------------------------------------------------------- Appearance
function AppearancePanel() {
  const s = useSettings();
  return (
    <div>
      <Row title="Mode" hint="Dark, light, or follow Windows.">
        <Segmented label="Mode" value={s.mode} onChange={(m: Mode) => s.setMode(m)}
          options={[{ value: "dark", label: "Dark" }, { value: "light", label: "Light" }, { value: "system", label: "System" }]} />
      </Row>
      <div className="py-4">
        <p className="mb-3 text-sm font-medium">Colour</p>
        <div className="grid grid-cols-2 gap-3">
          {SCHEMES.map((c) => {
            const light = s.mode === "light" || (s.mode === "system" && !window.matchMedia("(prefers-color-scheme: dark)").matches);
            const sw = light ? c.light : c.dark;
            const on = s.scheme === c.id;
            return (
              <motion.button key={c.id} onClick={() => s.setScheme(c.id)} whileHover={{ y: -2 }} whileTap={{ scale: 0.97 }} aria-pressed={on}
                transition={{ type: "spring", stiffness: 450, damping: 26 }}
                className={`flex items-center gap-3 rounded-2xl border p-3 text-left transition-colors ${on ? "border-accent bg-ink/10 shadow-glow" : "border-ink/10 hover:bg-ink/5"}`}>
                <span className="h-10 w-10 shrink-0 rounded-xl shadow-inner" style={{ background: `linear-gradient(135deg, ${sw[0]}, ${sw[1]})` }} />
                <span className="text-sm font-medium">{c.name}</span>
              </motion.button>
            );
          })}
        </div>
      </div>
      <Row title="Glass blur" hint={`${s.blur}px`}><Slider label="Glass blur" min={8} max={40} value={s.blur} onChange={s.setBlur} className="w-44" /></Row>
      <Row title="Background motion" hint={s.animSpeed === 0.25 ? "Calm" : `${s.animSpeed.toFixed(2)}x speed`}>
        <Slider label="Animation speed" min={0.25} max={2.5} step={0.25} value={s.animSpeed} onChange={s.setAnimSpeed} className="w-44" />
      </Row>
      <Row title="Audio Visualizer" hint="Show a dynamic visualizer in the player bar.">
        <Switch label="Visualizer" checked={s.visualizer} onChange={s.setVisualizer} />
      </Row>
    </div>
  );
}

// ---------------------------------------------------------------- Audio
function AudioPanel() {
  const { device, setDevice } = useSettings();
  const [info, setInfo] = useState<AudioDevices | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = () => api.listAudioDevices().then((d) => { setInfo(d); setError(null); }).catch((e) => setError(String(e)));
  useEffect(() => { void load(); }, []);

  const options = [{ value: "", label: info?.default ? `System default (${info.default})` : "System default" }, ...(info?.devices ?? []).map((d) => ({ value: d, label: d }))];
  return (
    <div>
      <Row title="Output device" hint="Playback continues from the same spot when you switch.">
        <div className="flex items-center gap-2">
          <Dropdown className="w-72" label="Output device" value={device ?? ""} options={options} onChange={(v) => setDevice(v || null)} />
          <IconBtn label="Refresh devices" onClick={() => void load()}><RefreshIcon size={16} /></IconBtn>
        </div>
      </Row>
      {error && <p role="alert" className="py-2 text-sm text-red-400">{error}</p>}
      <p className="pt-4 text-xs text-dim">
        Now using: <span className="text-ink">{info?.current ?? "—"}</span>. Pulse opens the device at its own sample rate and channel layout
        and converts songs to match, so pitch and speed are always correct.
      </p>
    </div>
  );
}

// ---------------------------------------------------------------- Playback
function PlaybackPanel() {
  const { 
    gapless, setGapless, 
    crossfade, setCrossfade, 
    replaygainEnabled, setReplayGain,
    replayGainAnalyzing, replayGainProgress, startReplayGainAnalysis 
  } = useSettings();

  return (
    <div>
      <Row title="Gapless playback" hint="Start the next song with no pause. Off: each song finishes completely first.">
        <Switch label="Gapless playback" checked={gapless} onChange={setGapless} />
      </Row>
      <Row title="Crossfade" hint={crossfade === 0 ? "Off" : `${(crossfade / 1000).toFixed(1)} seconds — songs blend into each other`}>
        <Slider label="Crossfade duration" min={0} max={12000} step={500} value={crossfade} onChange={setCrossfade} className="w-52" />
      </Row>
      <Row title="ReplayGain" hint="Normalize volume across tracks to avoid sudden loud or quiet songs.">
        <Switch label="ReplayGain" checked={replaygainEnabled} onChange={setReplayGain} />
      </Row>
      
      <div className="py-4">
        <p className="mb-2 text-sm font-medium">Analyze Library</p>
        <p className="mb-3 text-xs text-dim">
          Scan your library to calculate ReplayGain tags based on the EBU R128 standard. This can take a few minutes for large libraries.
        </p>
        <Btn 
          variant="primary" 
          onClick={() => void startReplayGainAnalysis()} 
          disabled={replayGainAnalyzing}
        >
          {replayGainAnalyzing ? "Analyzing..." : "Analyze ReplayGain"}
        </Btn>
        
        {replayGainAnalyzing && replayGainProgress && (
          <div className="mt-4 glass-pill p-4" style={{ borderRadius: 12 }}>
            <div className="flex justify-between text-xs text-dim mb-1">
              <span className="truncate max-w-[200px]">{replayGainProgress.name}</span>
              <span>{replayGainProgress.current} / {replayGainProgress.total}</span>
            </div>
            <div className="h-2 w-full bg-ink/20 rounded-full overflow-hidden">
              <div 
                className="h-full bg-accent transition-all duration-300"
                style={{ width: `${replayGainProgress.total > 0 ? (replayGainProgress.current / replayGainProgress.total) * 100 : 0}%` }}
              />
            </div>
          </div>
        )}
      </div>

      <p className="pt-4 text-xs text-dim">Skipping with Next or Previous always cuts straight to the new song. Crossfade only applies when a song ends on its own, and is skipped for very short tracks.</p>
    </div>
  );
}

// ---------------------------------------------------------------- Equalizer
const LABELS = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];
const PRESETS: Record<string, number[]> = {
  Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  "Bass boost": [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  "Treble boost": [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  Vocal: [-2, -2, -1, 1, 3, 3, 2, 0, -1, -2],
  Rock: [4, 3, 2, 0, -1, -1, 1, 3, 4, 4],
  Acoustic: [3, 3, 2, 1, 1, 1, 2, 3, 3, 2],
};

function EqualizerPanel() {
  const { eq, setEq } = useSettings();
  const setBand = (i: number, v: number) => setEq({ ...eq, gains: eq.gains.map((g, k) => (k === i ? v : g)) });
  return (
    <div>
      <Row title="Equalizer" hint="10 bands, ±12 dB. A preamp lowers the level automatically so boosts can't clip.">
        <Switch label="Enable equalizer" checked={eq.enabled} onChange={(enabled) => setEq({ ...eq, enabled })} />
      </Row>
      <div className={`transition-opacity ${eq.enabled ? "" : "opacity-45"}`}>
        <div className="flex flex-wrap gap-2 py-4">
          {Object.entries(PRESETS).map(([name, gains]) => (
            <Btn key={name} className="h-8 px-3.5 text-xs" onClick={() => setEq({ enabled: true, gains })}>{name}</Btn>
          ))}
        </div>
        <div className="glass-pill flex items-end justify-between px-6 pb-3 pt-5" style={{ borderRadius: 20 }}>
          {eq.gains.map((g, i) => (
            <div key={i} className="flex flex-col items-center gap-2">
              <span className="w-10 text-center text-[11px] tabular-nums text-dim">{g > 0 ? "+" : ""}{g.toFixed(1)}</span>
              <VSlider label={`${LABELS[i]} Hz`} min={-12} max={12} value={g} onChange={(v) => setBand(i, v)} />
              <span className="text-xs text-dim">{LABELS[i]}</span>
            </div>
          ))}
        </div>
        <p className="mt-3 text-xs text-dim">Double-click a band to reset it.</p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- Virtualizer
const DEVICE_PROFILES = [
  { value: "Sony WH-1000XM5", label: "Sony WH-1000XM5 (Headphone)" },
  { value: "Sony WH-1000XM4", label: "Sony WH-1000XM4 (Headphone)" },
  { value: "Apple AirPods Max", label: "Apple AirPods Max (Headphone)" },
  { value: "Sennheiser HD 600", label: "Sennheiser HD 600 (Headphone)" },
  { value: "Sennheiser HD 650", label: "Sennheiser HD 650 (Headphone)" },
  { value: "Sennheiser HD 800 S", label: "Sennheiser HD 800 S (Headphone)" },
  { value: "Beyerdynamic DT 770 PRO (250 Ohm)", label: "Beyerdynamic DT 770 PRO (Headphone)" },
  { value: "Beyerdynamic DT 990 PRO", label: "Beyerdynamic DT 990 PRO (Headphone)" },
  { value: "Audio-Technica ATH-M50x", label: "Audio-Technica ATH-M50x (Headphone)" },
  { value: "HiFiMan Sundara", label: "HiFiMan Sundara (Headphone)" },
  { value: "AKG K371", label: "AKG K371 (Headphone)" },
  { value: "AKG K702", label: "AKG K702 (Headphone)" },
  { value: "Focal Clear Mg", label: "Focal Clear Mg (Headphone)" },
  { value: "Meze 109 Pro", label: "Meze 109 Pro (Headphone)" },
  { value: "Philips Fidelio X2HR", label: "Philips Fidelio X2HR (Headphone)" },
  { value: "Razer BlackShark V2 Pro", label: "Razer BlackShark V2 Pro (Headphone)" },
  { value: "Audeze LCD-5", label: "Audeze LCD-5 (Headphone)" },
  { value: "Dan Clark Audio Stealth", label: "Dan Clark Audio Stealth (Headphone)" },
  { value: "Apple AirPods Pro (1st Gen)", label: "Apple AirPods Pro (Earbud)" },
  { value: "Samsung Galaxy Buds2 Pro", label: "Samsung Galaxy Buds2 Pro (Earbud)" },
  { value: "Sony WF-1000XM5", label: "Sony WF-1000XM5 (Earbud)" },
  { value: "Sony WF-1000XM4", label: "Sony WF-1000XM4 (Earbud)" },
  { value: "Sennheiser Momentum True Wireless 4", label: "Sennheiser Momentum True Wireless 4 (Earbud)" },
  { value: "Moondrop Aria", label: "Moondrop Aria (Earbud)" },
  { value: "Moondrop Blessing 3", label: "Moondrop Blessing 3 (Earbud)" },
  { value: "7Hz Salnotes Zero", label: "7Hz Salnotes Zero (Earbud)" },
  { value: "Linsoul 7Hz Timeless", label: "Linsoul 7Hz Timeless (Earbud)" },
  { value: "Truthear x Crinacle Zero:RED", label: "Truthear x Crinacle Zero:RED (Earbud)" },
  { value: "Dunu Titan S", label: "Dunu Titan S (Earbud)" },
  { value: "Final Audio A4000", label: "Final Audio A4000 (Earbud)" },
];

function VirtualizerPanel() {
  const { virtualizerEnabled, setVirtualizerEnabled, virtualizerProfile, setVirtualizerProfile } = useSettings();
  
  return (
    <div>
      <Row title="Enable Simulation" hint="Simulate the acoustic profile of specific headphones using AutoEQ data.">
        <Switch label="Enable Virtualization" checked={virtualizerEnabled} onChange={setVirtualizerEnabled} />
      </Row>
      
      <div className={`pt-6 transition-opacity ${virtualizerEnabled ? "" : "opacity-40 pointer-events-none"}`}>
        <div className="mb-4">
          <p className="text-sm font-medium mb-1">Target Device</p>
          <p className="text-xs text-dim mb-4">
            Select a headphone or earbud to simulate its frequency response. 
            Speaker profiles are not available yet.
          </p>
          
          <Dropdown 
            className="w-full" 
            label="Target Device" 
            value={virtualizerProfile ?? ""} 
            options={[{ value: "", label: "None (Bypass)" }, ...DEVICE_PROFILES]} 
            onChange={(v) => setVirtualizerProfile(v || null)} 
          />
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- Crystalizer
function CrystalizerPanel() {
  const { crystalizerMode, crystalizerIntensity, setCrystalizer } = useSettings();

  return (
    <div>
      <Row title="Enable Crystalizer" hint="Restore high-frequency details and transients lost in compressed audio.">
        <Switch 
          label="Enable Crystalizer" 
          checked={crystalizerIntensity > 0} 
          onChange={(on) => setCrystalizer(crystalizerMode, on ? 0.5 : 0.0)} 
        />
      </Row>

      <div className={`pt-6 transition-opacity ${crystalizerIntensity > 0 ? "" : "opacity-40 pointer-events-none"}`}>
        <div className="mb-4">
          <p className="text-sm font-medium mb-1">Processing Mode</p>
          <p className="text-xs text-dim mb-4">
            HX and HX+ use multi-band processing for more precise restoration across frequencies.
          </p>
          
          <Dropdown 
            className="w-full" 
            label="Mode" 
            value={crystalizerMode} 
            options={[
              { value: "crystalizer", label: "Crystalizer (Standard)" },
              { value: "plus", label: "Crystalizer+" },
              { value: "hx", label: "Crystalizer HX" },
              { value: "hxplus", label: "Crystalizer HX+" },
            ]} 
            onChange={(v) => setCrystalizer(v, crystalizerIntensity || 0.5)} 
          />
        </div>

        <Row title="Intensity" hint={`${Math.round(crystalizerIntensity * 100)}%`}>
          <Slider 
            label="Intensity" 
            min={0} 
            max={1} 
            step={0.05} 
            value={crystalizerIntensity} 
            onChange={(v) => setCrystalizer(crystalizerMode, v)} 
            className="w-full" 
          />
        </Row>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- Pulse ZR (Enhancer)
function EnhancerPanel() {
  const {
    enhancerBass,
    enhancerTreble,
    enhancerWidth,
    enhancerSurround,
    enhancerBooster,
    setEnhancer,
    setEnhancerSurround,
    setEnhancerBooster,
  } = useSettings();
  const isEnabled =
    enhancerBass > 0 ||
    enhancerTreble > 0 ||
    enhancerWidth > 0 ||
    enhancerSurround ||
    enhancerBooster;

  const handleEnable = (on: boolean) => {
    if (on) {
      setEnhancer(0.5, 0.5, 0.5);
    } else {
      setEnhancer(0, 0, 0, false, false);
    }
  };

  return (
    <div>
      <Row
        title="Enable Pulse ZR"
        hint="Bass, treble, stereo width, virtual surround, and a protected +4.5 dB booster."
      >
        <Switch label="Enable Pulse ZR" checked={isEnabled} onChange={handleEnable} />
      </Row>

      <div className="pt-6">
        <div className="flex flex-col items-center gap-10 py-4">
          <div className="flex w-full items-center justify-center gap-3">
            <ZrToggle
              label="ZR-SURROUND"
              checked={enhancerSurround}
              onChange={setEnhancerSurround}
            />
            <div className="flex flex-col items-center gap-2">
              <span className="text-[11px] tabular-nums text-dim">{Math.round(enhancerBass * 100)}%</span>
              <Knob
                label="BASS"
                value={enhancerBass}
                onChange={(v) => setEnhancer(v, enhancerTreble, enhancerWidth)}
              />
            </div>
            <ZrToggle
              label="ZR-BOOSTER"
              checked={enhancerBooster}
              onChange={setEnhancerBooster}
            />
          </div>
          <div className="flex items-start justify-center gap-16">
            <div className="flex flex-col items-center gap-2">
              <span className="text-[11px] tabular-nums text-dim">{Math.round(enhancerTreble * 100)}%</span>
              <Knob
                label="TREBLE"
                value={enhancerTreble}
                onChange={(v) => setEnhancer(enhancerBass, v, enhancerWidth)}
              />
            </div>
            <div className="flex flex-col items-center gap-2">
              <span className="text-[11px] tabular-nums text-dim">{Math.round(enhancerWidth * 100)}%</span>
              <Knob
                label="WIDTH"
                value={enhancerWidth}
                onChange={(v) => setEnhancer(enhancerBass, enhancerTreble, v)}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- General
function GeneralPanel() {
  const { closeToTray, setCloseToTray, discordRpc, setDiscordRpc, useSystemProxy, setUseSystemProxy } = useSettings();
  const [onTop, setOnTop] = useState(false);
  return (
    <div>
      <Row title="Close to tray" hint="The close button hides Pulse in the system tray instead of quitting. Quit from the tray menu.">
        <Switch label="Close to tray" checked={closeToTray} onChange={setCloseToTray} />
      </Row>
      <Row title="Always on top" hint="Keep the window above other windows (until you restart).">
        <Switch label="Always on top" checked={onTop} onChange={(v) => { setOnTop(v); void api.setAlwaysOnTop(v); }} />
      </Row>
      
      <Row title="Discord Rich Presence" hint="Show the currently playing song on your Discord profile.">
        <Switch label="Discord Rich Presence" checked={discordRpc} onChange={setDiscordRpc} />
      </Row>

      <Row title="Use system proxy for online section" hint="Routes SoundCloud requests through your OS proxy settings.">
        <Switch label="Use system proxy" checked={useSystemProxy} onChange={setUseSystemProxy} />
      </Row>

      <div className="pt-5">
        <p className="mb-2 text-sm font-medium">Keyboard shortcuts</p>
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
          {[["Space", "Play / pause"], ["Ctrl + →  /  Ctrl + ←", "Next / previous"], ["Ctrl + ↑  /  Ctrl + ↓", "Volume up / down"], ["Ctrl + F", "Search"], ["Ctrl + ,", "Settings"], ["Media keys", "Work system-wide, even when Pulse is in the background"]].map(([k, v]) => (
            <div key={k} className="contents"><dt className="font-medium">{k}</dt><dd className="text-dim">{v}</dd></div>
          ))}
        </dl>
      </div>
    </div>
  );
}

const PANELS: Record<SettingsTab, () => React.JSX.Element> = {
  library: LibraryPanel, 
  appearance: AppearancePanel, 
  audio: AudioPanel, 
  playback: PlaybackPanel, 
  equalizer: EqualizerPanel, 
  virtualizer: VirtualizerPanel, 
  crystalizer: CrystalizerPanel,
  enhancer: EnhancerPanel,
  general: GeneralPanel,
};

export function SettingsModal() {
  const { settingsOpen, closeSettings, settingsTab, setSettingsTab } = useUi();
  const Panel = PANELS[settingsTab];
  return (
    <Modal open={settingsOpen} onClose={closeSettings} label="Settings" className="flex h-[min(620px,86vh)] w-[min(860px,94vw)] overflow-hidden">
      <nav aria-label="Settings sections" className="flex w-48 shrink-0 flex-col gap-1 border-r border-ink/10 p-4">
        <h2 className="mb-3 px-3 text-lg font-bold">Settings</h2>
        {TABS.map((t) => (
          <button key={t.id} onClick={() => setSettingsTab(t.id)} aria-current={settingsTab === t.id ? "page" : undefined}
            className={`relative rounded-xl px-3 py-2 text-left text-sm font-medium transition-colors ${settingsTab === t.id ? "text-ink" : "text-dim hover:text-ink"}`}>
            {settingsTab === t.id && <motion.span layoutId="settings-tab" className="absolute inset-0 rounded-xl bg-ink/10 ring-1 ring-ink/10" transition={{ type: "spring", stiffness: 500, damping: 36 }} />}
            <span className="relative">{t.label}</span>
          </button>
        ))}
      </nav>
      <div className="relative min-w-0 flex-1 overflow-y-auto p-7">
        <IconBtn label="Close settings" className="absolute right-4 top-4" onClick={closeSettings}><CloseIcon size={18} /></IconBtn>
        <h3 className="mb-4 text-xl font-bold">{TABS.find((t) => t.id === settingsTab)?.label}</h3>
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div key={settingsTab} className="w-full" initial={{ opacity: 0, x: 18 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -12 }} transition={{ type: "spring", stiffness: 420, damping: 36 }}>
            <Panel />
          </motion.div>
        </AnimatePresence>
      </div>
    </Modal>
  );
}
