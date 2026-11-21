import { AnimatePresence, motion, type HTMLMotionProps } from "framer-motion";
import { useEffect, useRef, useState, type ReactNode } from "react";

const SPRING = { type: "spring", stiffness: 520, damping: 26 } as const;

/** Spring hover/press + click ripple, same as the player's buttons. */
type BtnProps = Omit<HTMLMotionProps<"button">, "children"> & {
  children?: ReactNode;
  variant?: "primary" | "glass" | "ghost" | "danger";
};
export function Btn({ variant = "glass", className = "", children, onPointerDown, ...rest }: BtnProps) {
  const [ripples, setRipples] = useState<{ id: number; x: number; y: number }[]>([]);
  const idRef = useRef(0);
  const styles = {
    primary: "text-white shadow-glow [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]",
    glass: "glass-pill glow-hover",
    ghost: "hover:bg-ink/10",
    danger: "text-white [background:linear-gradient(135deg,#b4536a,#8f3a52)] shadow-[0_8px_26px_-8px_rgb(180_83_106/0.7)]",
  }[variant];
  return (
    <motion.button type="button" whileHover={{ y: -1.5 }} whileTap={{ scale: 0.95 }} transition={SPRING}
      className={`relative overflow-hidden rounded-full px-6 py-2.5 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-40 ${styles} ${className}`}
      onPointerDown={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        const id = ++idRef.current;
        setRipples((rs) => [...rs, { id, x: e.clientX - r.left, y: e.clientY - r.top }]);
        onPointerDown?.(e);
      }}
      {...rest}>
      {children}
      {ripples.map((r) => (
        <span key={r.id} className="ripple" style={{ left: r.x, top: r.y }} onAnimationEnd={() => setRipples((rs) => rs.filter((x) => x.id !== r.id))} />
      ))}
    </motion.button>
  );
}

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button role="switch" aria-checked={checked} aria-label={label} onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full border border-ink/15 transition-colors ${checked ? "[background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]" : "bg-ink/10"}`}>
      <motion.span className="absolute top-0.5 h-5 w-5 rounded-full bg-white shadow" animate={{ left: checked ? 22 : 2 }} transition={{ type: "spring", stiffness: 600, damping: 32 }} />
    </button>
  );
}

export function Checkbox({ checked, onChange, children }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode }) {
  return (
    <button role="checkbox" aria-checked={checked} onClick={() => onChange(!checked)} className="flex items-center gap-3 text-left text-sm">
      <span className={`grid h-[22px] w-[22px] shrink-0 place-items-center rounded-md border transition-colors ${checked ? "border-transparent [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]" : "border-ink/30 bg-ink/5"}`}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="3.4" strokeLinecap="round" strokeLinejoin="round">
          <motion.path d="M5 12.5l4.5 4.5L19 7.5" initial={false} animate={{ pathLength: checked ? 1 : 0, opacity: checked ? 1 : 0 }} transition={{ duration: 0.22 }} />
        </svg>
      </span>
      <span>{children}</span>
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: {
  value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="glass-pill relative inline-flex p-1">
      {options.map((o) => (
        <button key={o.value} role="radio" aria-checked={o.value === value} onClick={() => onChange(o.value)}
          className={`relative z-10 rounded-full px-4 py-1.5 text-sm font-medium transition-colors ${o.value === value ? "text-white" : "text-dim hover:text-ink"}`}>
          {o.value === value && (
            <motion.span layoutId={`seg-${label}`} className="absolute inset-0 -z-10 rounded-full [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]"
              transition={{ type: "spring", stiffness: 500, damping: 34 }} />
          )}
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-ink/10 py-4 last:border-0">
      <div className="min-w-0"><p className="text-sm font-medium">{title}</p>{hint && <p className="mt-0.5 text-xs text-dim">{hint}</p>}</div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function Modal({ open, onClose, children, label }: { open: boolean; onClose: () => void; children: ReactNode; label: string }) {
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  return (
    <AnimatePresence>
      {open && (
        <motion.div className="fixed inset-0 z-50 grid place-items-center p-6" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }} onMouseDown={onClose}>
          <div className="absolute inset-0 bg-black/45 backdrop-blur-sm" />
          <motion.div role="dialog" aria-modal="true" aria-label={label} className="glass relative w-[min(420px,90vw)] p-7"
            initial={{ scale: 0.92, y: 24, opacity: 0 }} animate={{ scale: 1, y: 0, opacity: 1 }} exit={{ scale: 0.96, y: 12, opacity: 0 }}
            transition={{ type: "spring", stiffness: 380, damping: 30 }} onMouseDown={(e) => e.stopPropagation()}>
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
