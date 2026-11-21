import { motion } from "framer-motion";
import { CheckIcon } from "./icons";
import { HeartbeatLogo } from "./Logo";

export function Sidebar({ steps, current, version, uninstall }: { steps: string[]; current: number; version: string; uninstall: boolean }) {
  return (
    <aside className="glass flex w-[236px] shrink-0 flex-col justify-between p-6" aria-label="Setup progress">
      <div>
        <div className="mb-8 flex items-center gap-3">
          <HeartbeatLogo size={40} />
          <div>
            <p className="text-lg font-bold leading-tight tracking-tight">Pulse</p>
            <p className="text-xs text-dim">{uninstall ? "Uninstall" : "Setup"}</p>
          </div>
        </div>
        <ol className="relative space-y-1">
          {steps.map((label, i) => {
            const done = i < current;
            const active = i === current;
            return (
              <li key={label} aria-current={active ? "step" : undefined} className="relative flex items-center gap-3 rounded-xl px-2 py-2">
                {active && <motion.span layoutId="step-pill" className="absolute inset-0 rounded-xl bg-ink/10 ring-1 ring-ink/10" transition={{ type: "spring", stiffness: 500, damping: 36 }} />}
                <span className={`relative grid h-6 w-6 shrink-0 place-items-center rounded-full text-[11px] font-semibold transition-colors ${
                  done ? "text-white [background:linear-gradient(135deg,rgb(var(--accent)),rgb(var(--accent-2)))]" : active ? "border-2 border-accent text-accent" : "border border-ink/25 text-dim"}`}>
                  {done ? <CheckIcon size={13} /> : i + 1}
                </span>
                <span className={`relative text-sm ${active ? "font-semibold" : done ? "text-ink/80" : "text-dim"}`}>{label}</span>
              </li>
            );
          })}
        </ol>
      </div>
      <p className="text-xs text-dim"></p>
    </aside>
  );
}
