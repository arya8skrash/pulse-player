import { useState } from "react";
import { motion } from "framer-motion";
import { HeartIcon, StarIcon } from "./icons";

export function Rating({ value, onChange, size = 14, className = "" }: { value: number; onChange: (v: number) => void; size?: number; className?: string }) {
  const [hover, setHover] = useState(0);
  const shown = hover || value;
  return (
    <div role="radiogroup" aria-label="Rating" className={`flex items-center ${className}`} onMouseLeave={() => setHover(0)}>
      {[1, 2, 3, 4, 5].map((n) => (
        <button key={n} role="radio" aria-checked={value === n} aria-label={`${n} star${n > 1 ? "s" : ""}`}
          onMouseEnter={() => setHover(n)} onClick={(e) => { e.stopPropagation(); onChange(n); }}
          className={`p-0.5 transition-colors ${n <= shown ? "text-accent" : "text-ink/25 hover:text-ink/50"}`}>
          <StarIcon size={size} filled={n <= shown} />
        </button>
      ))}
    </div>
  );
}

export function HeartButton({ on, onToggle, size = 18, className = "" }: { on: boolean; onToggle: () => void; size?: number; className?: string }) {
  return (
    <motion.button
      aria-label={on ? "Remove from favorites" : "Add to favorites"}
      aria-pressed={on}
      onClick={(e) => { e.stopPropagation(); onToggle(); }}
      whileTap={{ scale: 0.7 }}
      animate={on ? { scale: [1, 1.35, 1] } : { scale: 1 }}
      transition={{ type: "spring", stiffness: 500, damping: 16 }}
      className={`grid h-8 w-8 shrink-0 place-items-center rounded-full transition-colors hover:bg-ink/10 ${on ? "text-accent" : "text-dim"} ${className}`}
    >
      <HeartIcon size={size} filled={on} />
    </motion.button>
  );
}
