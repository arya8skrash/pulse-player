import { useEffect, useRef, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";

export type KnobProps = {
  value: number;
  onChange: (value: number) => void;
  label: string;
  size?: number;
};

const START_ANGLE = 135;
const SWEEP_ANGLE = 270;
const STEP = 0.05;
const DRAG_DISTANCE = 180;

const clamp = (value: number, min = 0, max = 1) => Math.min(max, Math.max(min, value));
const snap = (value: number) => Math.round(clamp(value) / STEP) * STEP;

/** A glass rotary control driven by vertical pointer movement. */
export function Knob({ value, onChange, label, size = 104 }: KnobProps) {
  const normalizedValue = clamp(value);
  const dragRef = useRef({ startY: 0, startValue: 0, pointerId: -1, active: false });
  const lastValueRef = useRef(normalizedValue);

  useEffect(() => {
    lastValueRef.current = normalizedValue;
  }, [normalizedValue]);

  const commit = (next: number) => {
    const snapped = snap(next);
    if (Math.abs(snapped - lastValueRef.current) < STEP / 2) return;
    lastValueRef.current = snapped;
    onChange(snapped);
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      startY: event.clientY,
      startValue: normalizedValue,
      pointerId: event.pointerId,
      active: true,
    };
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag.active || drag.pointerId !== event.pointerId) return;
    const delta = (drag.startY - event.clientY) / DRAG_DISTANCE;
    commit(drag.startValue + delta);
  };

  const endDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag.active) return;
    drag.active = false;
    if (event.currentTarget.hasPointerCapture(drag.pointerId)) {
      event.currentTarget.releasePointerCapture(drag.pointerId);
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (
      event.key !== "ArrowLeft" &&
      event.key !== "ArrowRight" &&
      event.key !== "ArrowUp" &&
      event.key !== "ArrowDown"
    ) {
      return;
    }
    event.preventDefault();
    const direction = event.key === "ArrowRight" || event.key === "ArrowUp" ? 1 : -1;
    commit(normalizedValue + direction * STEP);
  };

  const center = size / 2;
  const radius = Math.max(4, size / 2 - 9);
  const strokeWidth = Math.max(3, size * 0.07);
  const knobSize = Math.max(34, size * 0.62);
  const startRad = (START_ANGLE * Math.PI) / 180;
  const endRad = ((START_ANGLE + SWEEP_ANGLE) * Math.PI) / 180;
  const trackPath = `M ${center + radius * Math.cos(startRad)} ${center + radius * Math.sin(startRad)} A ${radius} ${radius} 0 1 1 ${center + radius * Math.cos(endRad)} ${center + radius * Math.sin(endRad)}`;

  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={normalizedValue}
      aria-valuetext={`${Math.round(normalizedValue * 100)}%`}
      className="group flex select-none flex-col items-center outline-none cursor-ns-resize touch-none transition-transform active:scale-[0.98]"
      style={{ width: size }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={handleKeyDown}
    >
      <div
        className="relative grid place-items-center rounded-full"
        style={{ width: size, height: size }}
      >
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="absolute inset-0 overflow-visible" aria-hidden="true">
          <path d={trackPath} fill="none" stroke="rgb(var(--ink) / 0.12)" strokeWidth={strokeWidth} strokeLinecap="round" />
          <path
            d={trackPath}
            fill="none"
            stroke="rgb(var(--accent))"
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            pathLength={1}
            strokeDasharray={`${normalizedValue} 1`}
            className="transition-[stroke-dasharray] duration-150 ease-out"
          />
        </svg>
        <div
          className="pointer-events-none rounded-full border border-white/15 shadow-[inset_0_1px_1px_rgb(255_255_255/0.22),inset_0_-8px_14px_rgb(0_0_0/0.12),0_5px_15px_rgb(0_0_0/0.12)] transition-transform duration-200 group-hover:scale-[1.02]"
          style={{
            width: knobSize,
            height: knobSize,
            background:
              "radial-gradient(circle at 35% 25%, rgb(255 255 255 / 0.28), rgb(var(--ink) / 0.12) 48%, rgb(var(--ink) / 0.24))",
          }}
        />
      </div>
      <span className="mt-2 text-xs font-medium tracking-wide text-dim transition-colors group-hover:text-ink">
        {label}
      </span>
    </div>
  );
}

export function ZrToggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={checked}
      onClick={() => onChange(!checked)}
      className="group flex w-[104px] shrink-0 flex-col items-center outline-none transition-transform active:scale-[0.98] focus-visible:rounded-xl focus-visible:ring-2 focus-visible:ring-accent"
    >
      <span
        className={`grid h-[76px] w-[76px] place-items-center rounded-full border transition-all duration-200 group-hover:scale-[1.03] ${
          checked
            ? "border-accent text-accent"
            : "border-ink/15 text-dim group-hover:border-ink/30 group-hover:text-ink"
        }`}
        style={{
          background:
            "radial-gradient(circle at 35% 25%, rgb(255 255 255 / 0.28), rgb(var(--ink) / 0.12) 48%, rgb(var(--ink) / 0.24))",
          boxShadow: checked
            ? "inset 0 1px 1px rgb(255 255 255 / 0.22), inset 0 -8px 14px rgb(0 0 0 / 0.12), 0 0 18px rgb(var(--accent) / 0.3)"
            : "inset 0 1px 1px rgb(255 255 255 / 0.22), inset 0 -8px 14px rgb(0 0 0 / 0.12), 0 5px 15px rgb(0 0 0 / 0.12)",
        }}
      >
        <svg
          width="25"
          height="25"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M12 3v9" />
          <path d="M6.3 6.3a8 8 0 1 0 11.4 0" />
        </svg>
      </span>
      <span
        className={`mt-2 whitespace-nowrap text-[10px] font-medium tracking-wide transition-colors ${
          checked ? "text-accent" : "text-dim group-hover:text-ink"
        }`}
      >
        {label}
      </span>
    </button>
  );
}

export default Knob;