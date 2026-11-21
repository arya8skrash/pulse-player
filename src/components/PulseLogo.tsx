import { useId } from "react";

/**
 * The Pulse mark. A dark obsidian tile with a faint glass rim and a soft-gradient heartbeat line.
 * The tile is fixed-dark in every theme (the premium look); the line picks up the theme accents.
 */
export function PulseLogo({ size = 28, className }: { size?: number; className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" className={className} aria-label="Pulse" role="img">
      <defs>
        <linearGradient id={`${id}-bg`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#1c2140" />
          <stop offset="1" stopColor="#080a16" />
        </linearGradient>
        <linearGradient id={`${id}-line`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" style={{ stopColor: "rgb(var(--accent))" }} />
          <stop offset="1" style={{ stopColor: "rgb(var(--accent-2))" }} />
        </linearGradient>
        <linearGradient id={`${id}-rim`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.38" />
          <stop offset="0.4" stopColor="#fff" stopOpacity="0.02" />
          <stop offset="1" stopColor="#fff" stopOpacity="0.14" />
        </linearGradient>
        <filter id={`${id}-glow`} x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="1.6" result="b" />
          <feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>
      <rect x="2" y="2" width="60" height="60" rx="17" fill={`url(#${id}-bg)`} />
      <rect x="2.6" y="2.6" width="58.8" height="58.8" rx="16.4" fill="none" stroke={`url(#${id}-rim)`} strokeWidth="1.2" />
      <path d="M9 34h10l5-12 8 25 6-18 3 5h14" fill="none" stroke={`url(#${id}-line)`} strokeWidth="3.4"
        strokeLinecap="round" strokeLinejoin="round" filter={`url(#${id}-glow)`} />
    </svg>
  );
}

/** Just the heartbeat line, for use on gradients (album-art placeholder). */
export function PulseMark({ size = 48, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" className={className} aria-hidden>
      <path d="M6 34h12l6-14 9 28 7-20 3 6h15" fill="none" stroke="currentColor" strokeWidth="4"
        strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
