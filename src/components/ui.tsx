import { AnimatePresence, motion, type HTMLMotionProps } from "framer-motion";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { ChevronIcon } from "./icons";

const SPRING = { type: "spring", stiffness: 520, damping: 26 } as const;

/** Button with a spring hover/press and a click ripple. */
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
    danger: "glass-pill text-red-400 hover:bg-red-500/15",
  }[variant];
  return (
    <motion.button
      type="button"
      whileHover={{ y: -1.5 }}
      whileTap={{ scale: 0.95 }}
      transition={SPRING}
      className={`relative overflow-hidden rounded-full px-4 py-2 text-sm font-medium disabled:opacity-40 ${styles} ${className}`}
      onPointerDown={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        const id = ++idRef.current;
        setRipples((rs) => [...rs, { id, x: e.clientX - r.left, y: e.clientY - r.top }]);
        onPointerDown?.(e);
      }}
      {...rest}
    >
      {children}
      {ripples.map((r) => (
        <span key={r.id} className="ripple" style={{ left: r.x, top: r.y }}
          onAnimationEnd={() => setRipples((rs) => rs.filter((x) => x.id !== r.id))} />
      ))}
    </motion.button>
  );
}

/** Round icon-only button. */
export function IconBtn({ label, active, className = "", children, ...rest }: Omit<HTMLMotionProps<"button">, "children"> & { label: string; active?: boolean; children?: ReactNode }) {
  return (
    <motion.button
      type="button"
      aria-label={label}
      title={label}
      whileHover={{ scale: 1.1 }}
      whileTap={{ scale: 0.88 }}
      transition={SPRING}
      className={`grid h-9 w-9 shrink-0 place-items-center rounded-full transition-colors hover:bg-ink/10 disabled:opacity-40 ${active ? "text-accent" : "text-ink"} ${className}`}
      {...rest}
    >
      {children}
    </motion.button>
  );
}

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full border border-ink/15 transition-colors ${checked ? "[background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]" : "bg-ink/10"}`}
    >
      <motion.span
        className="absolute top-0.5 h-5 w-5 rounded-full bg-white shadow"
        animate={{ left: checked ? 22 : 2 }}
        transition={{ type: "spring", stiffness: 600, damping: 32 }}
      />
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

export function Slider({ value, min, max, step = 1, onChange, label, className = "", thick, disabled }: {
  value: number; min: number; max: number; step?: number; onChange: (v: number) => void; label: string; className?: string; thick?: boolean; disabled?: boolean;
}) {
  const fill = max > min ? ((value - min) / (max - min)) * 100 : 0;
  return (
    <input type="range" aria-label={label} min={min} max={max} step={step} value={value} disabled={disabled}
      className={`pulse-range ${thick ? "thick" : ""} ${className}`} style={{ ["--fill" as string]: `${fill}%` }}
      onChange={(e) => onChange(Number(e.target.value))} />
  );
}

/** Vertical slider for the equalizer; double-click resets to 0. */
export function VSlider({ value, min, max, onChange, label, height = 150 }: {
  value: number; min: number; max: number; onChange: (v: number) => void; label: string; height?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const setFromY = (clientY: number) => {
    const r = ref.current!.getBoundingClientRect();
    const t = 1 - Math.min(1, Math.max(0, (clientY - r.top) / r.height));
    onChange(Math.round((min + t * (max - min)) * 2) / 2);
  };
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div
      ref={ref}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-orientation="vertical"
      className="relative w-6 cursor-pointer touch-none"
      style={{ height }}
      onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); setFromY(e.clientY); }}
      onPointerMove={(e) => { if (e.currentTarget.hasPointerCapture(e.pointerId)) setFromY(e.clientY); }}
      onDoubleClick={() => onChange(0)}
      onKeyDown={(e) => {
        if (e.key === "ArrowUp") { e.preventDefault(); onChange(Math.min(max, value + 0.5)); }
        if (e.key === "ArrowDown") { e.preventDefault(); onChange(Math.max(min, value - 0.5)); }
      }}
    >
      <div className="absolute left-1/2 top-0 h-full w-1.5 -translate-x-1/2 rounded-full bg-ink/15" />
      <div className="absolute left-1/2 top-1/2 h-px w-3 -translate-x-1/2 bg-ink/40" />
      <div className="absolute left-1/2 w-1.5 -translate-x-1/2 rounded-full [background:linear-gradient(180deg,rgb(var(--accent)),rgb(var(--accent-2)))]"
        style={value >= 0 ? { bottom: "50%", height: `${pct - 50}%` } : { top: "50%", height: `${50 - pct}%` }} />
      <div className="absolute left-1/2 h-4 w-4 -translate-x-1/2 translate-y-1/2 rounded-full bg-ink shadow-[0_0_12px_rgb(var(--accent)/0.9)]"
        style={{ bottom: `${pct}%` }} />
    </div>
  );
}

/** Themed dropdown (the native <select> popup can't be styled). */
export function Dropdown<T extends string>({ value, options, onChange, label, className = "" }: {
  value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string; className?: string;
}) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => { if (!wrap.current?.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); setOpen(false); } };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", esc, true);
    return () => { window.removeEventListener("pointerdown", close); window.removeEventListener("keydown", esc, true); };
  }, [open]);
  const current = options.find((o) => o.value === value);
  return (
    <div ref={wrap} className={`relative ${className}`}>
      <button type="button" aria-label={label} aria-haspopup="listbox" aria-expanded={open} onClick={() => setOpen((o) => !o)}
        className="glass-pill glow-hover flex h-9 w-full items-center justify-between gap-3 px-4 text-sm">
        <span className="truncate">{current?.label ?? "—"}</span>
        <ChevronIcon size={14} className={`shrink-0 text-dim transition-transform ${open ? "rotate-180" : ""}`} />
      </button>
      <AnimatePresence>
        {open && (
          <motion.ul role="listbox" initial={{ opacity: 0, y: -6, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -4 }}
            transition={{ type: "spring", stiffness: 500, damping: 32 }}
            className="glass absolute left-0 z-40 mt-2 max-h-64 min-w-full overflow-y-auto p-1.5" style={{ borderRadius: 16 }}>
            {options.map((o) => (
              <li key={o.value} role="option" aria-selected={o.value === value}>
                <button onClick={() => { onChange(o.value); setOpen(false); }}
                  className={`w-full whitespace-nowrap rounded-xl px-3 py-2 text-left text-sm transition-colors hover:bg-ink/10 ${o.value === value ? "text-accent" : ""}`}>
                  {o.label}
                </button>
              </li>
            ))}
          </motion.ul>
        )}
      </AnimatePresence>
    </div>
  );
}

export function Modal({ open, onClose, children, className = "", label }: {
  open: boolean; onClose: () => void; children: ReactNode; className?: string; label: string;
}) {
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  return (
    <AnimatePresence>
      {open && (
        <motion.div 
          className="fixed inset-0 z-50 grid place-items-center p-6" 
          initial={{ opacity: 0 }} 
          animate={{ opacity: 1 }} 
          exit={{ opacity: 0, pointerEvents: "none" }} 
          style={{ pointerEvents: open ? "auto" : "none" }} 
          transition={{ duration: 0.2 }} 
          onMouseDown={onClose}
        >
          <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" />
          <motion.div role="dialog" aria-modal="true" aria-label={label} className={`glass relative ${className}`}
            initial={{ scale: 0.93, y: 28, opacity: 0 }} animate={{ scale: 1, y: 0, opacity: 1 }} exit={{ scale: 0.96, y: 14, opacity: 0 }}
            transition={{ type: "spring", stiffness: 380, damping: 30 }} onMouseDown={(e) => e.stopPropagation()}>
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

/** Seek bar that stays smooth while dragging and commits the seek on release. */
export function SeekBar({ positionMs, durationMs, onSeek, thick, disabled }: {
  positionMs: number; durationMs: number; onSeek: (ms: number) => void; thick?: boolean; disabled?: boolean;
}) {
  const [drag, setDrag] = useState<number | null>(null);
  const shown = drag ?? Math.min(positionMs, durationMs);
  const commit = () => { if (drag !== null) { onSeek(drag); setDrag(null); } };
  return (
    <input type="range" aria-label="Seek" disabled={disabled} min={0} max={Math.max(durationMs, 1)} step={250} value={shown}
      className={`pulse-range min-w-0 flex-1 ${thick ? "thick" : ""}`}
      style={{ ["--fill" as string]: `${durationMs > 0 ? (shown / durationMs) * 100 : 0}%` }}
      onChange={(e) => setDrag(Number(e.target.value))} onPointerUp={commit} onKeyUp={commit} onBlur={commit} />
  );
}
