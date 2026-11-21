import { motion } from "framer-motion";
import { useMemo } from "react";

const BAR_H = 30;
const PERIOD = 28; // the meniscus pattern repeats every 28px, so a 28px scroll loops seamlessly
const W = 18;
const AMP = 5;

/**
 * Liquid progress bar. The fill springs to its target, carries a drifting highlight and rising
 * bubbles, and its leading edge is a flowing wave (an SVG sine edge that scrolls vertically).
 */
export function LiquidProgress({ percent }: { percent: number }) {
  const pct = Math.max(0, Math.min(100, percent));

  const wave = useMemo(() => {
    const H = PERIOD * 3;
    let d = "M0 0";
    for (let y = 0; y <= H; y += 2) d += ` L${(W / 2 + AMP * Math.sin((2 * Math.PI * y) / PERIOD)).toFixed(2)} ${y}`;
    return `${d} L0 ${H} Z`;
  }, []);

  return (
    <div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}
      className="glass-pill relative overflow-hidden" style={{ height: BAR_H, boxShadow: "inset 0 2px 6px rgb(0 0 0 / 0.25), 0 0 30px -10px rgb(var(--accent) / 0.5)" }}>
      <motion.div className="absolute inset-y-0 left-0" initial={false} animate={{ width: `${pct}%`, opacity: pct > 0.4 ? 1 : 0 }} transition={{ type: "spring", stiffness: 110, damping: 22 }}>
        <div className="absolute inset-0 [background:linear-gradient(90deg,rgb(var(--accent)),rgb(var(--accent-2)))]" />
        <div className="shimmer absolute inset-0" />
        {[12, 38, 64].map((left, i) => (
          <span key={i} className="bubble absolute bottom-1 h-1.5 w-1.5 rounded-full bg-white/70" style={{ left: `${left}%`, animationDelay: `${i * 0.8}s` }} />
        ))}
        <svg width={W} height={PERIOD * 3} className="flow absolute top-0" style={{ right: -(W - 1) }} aria-hidden>
          <path d={wave} style={{ fill: "rgb(var(--accent-2))" }} />
        </svg>
      </motion.div>
    </div>
  );
}
