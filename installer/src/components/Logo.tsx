import { motion } from "framer-motion";
import { useId } from "react";

/** The Pulse mark: dark obsidian tile, glass rim, soft-gradient heartbeat line (same as the player). */
export function PulseLogo({ size = 28, className }: { size?: number; className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" className={className} aria-label="Pulse" role="img">
      <defs>
        <linearGradient id={`${id}-bg`} x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#1c2140" /><stop offset="1" stopColor="#080a16" /></linearGradient>
        <linearGradient id={`${id}-line`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" style={{ stopColor: "rgb(var(--accent))" }} /><stop offset="1" style={{ stopColor: "rgb(var(--accent-2))" }} />
        </linearGradient>
        <linearGradient id={`${id}-rim`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.38" /><stop offset="0.4" stopColor="#fff" stopOpacity="0.02" /><stop offset="1" stopColor="#fff" stopOpacity="0.14" />
        </linearGradient>
        <filter id={`${id}-glow`} x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="1.6" result="b" /><feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>
      <rect x="2" y="2" width="60" height="60" rx="17" fill={`url(#${id}-bg)`} />
      <rect x="2.6" y="2.6" width="58.8" height="58.8" rx="16.4" fill="none" stroke={`url(#${id}-rim)`} strokeWidth="1.2" />
      <path d="M9 34h10l5-12 8 25 6-18 3 5h14" fill="none" stroke={`url(#${id}-line)`} strokeWidth="3.4" strokeLinecap="round" strokeLinejoin="round" filter={`url(#${id}-glow)`} />
    </svg>
  );
}

export function PulseMark({ size = 48, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" className={className} aria-hidden>
      <path d="M6 34h12l6-14 9 28 7-20 3 6h15" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/** The welcome-page logo: a lub-dub heartbeat with a breathing glow behind it. */
export function HeartbeatLogo({ size = 104 }: { size?: number }) {
  const beat = { duration: 1.6, repeat: Infinity, ease: "easeInOut" as const, times: [0, 0.12, 0.24, 0.38, 1] };
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <motion.div className="absolute -inset-5 rounded-full"
        style={{ background: "radial-gradient(closest-side, rgb(var(--accent) / 0.6), transparent)", filter: "blur(14px)" }}
        animate={{ opacity: [0.3, 0.95, 0.4, 0.8, 0.3], scale: [1, 1.2, 1.02, 1.12, 1] }} transition={beat} />
      <motion.div className="relative" animate={{ scale: [1, 1.08, 1, 1.05, 1] }} transition={beat}>
        <PulseLogo size={size} />
      </motion.div>
    </div>
  );
}

/** A heartbeat trace that draws itself, over and over. */
export function EcgLine({ className }: { className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg viewBox="0 0 400 60" preserveAspectRatio="none" className={className} aria-hidden>
      <defs>
        <linearGradient id={`${id}-g`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" style={{ stopColor: "rgb(var(--accent))", stopOpacity: 0 }} />
          <stop offset="0.35" style={{ stopColor: "rgb(var(--accent))" }} />
          <stop offset="1" style={{ stopColor: "rgb(var(--accent-2))" }} />
        </linearGradient>
      </defs>
      <motion.path d="M0 30 H120 L134 30 L150 8 L168 52 L184 18 L196 30 H400" fill="none" stroke={`url(#${id}-g)`} strokeWidth="2.6"
        strokeLinecap="round" strokeLinejoin="round" initial={{ pathLength: 0, opacity: 1 }}
        animate={{ pathLength: [0, 1, 1], opacity: [1, 1, 0] }} transition={{ duration: 2.8, repeat: Infinity, times: [0, 0.65, 1], ease: "easeInOut" }} />
    </svg>
  );
}
