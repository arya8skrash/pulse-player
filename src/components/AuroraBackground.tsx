import { motion, useMotionValue, useSpring } from "framer-motion";
import { useEffect } from "react";

// Five drifting colour fields (CSS keyframes, GPU-composited) plus a soft glow that follows the cursor.
export function AuroraBackground() {
  const x = useMotionValue(-400);
  const y = useMotionValue(-400);
  const sx = useSpring(x, { stiffness: 60, damping: 20, mass: 1 });
  const sy = useSpring(y, { stiffness: 60, damping: 20, mass: 1 });

  useEffect(() => {
    const move = (e: PointerEvent) => {
      x.set(e.clientX - 260);
      y.set(e.clientY - 260);
    };
    window.addEventListener("pointermove", move);
    return () => window.removeEventListener("pointermove", move);
  }, [x, y]);

  return (
    <div aria-hidden className="fixed inset-0 -z-10 overflow-hidden bg-base">
      <div className="aurora-blob aurora-1" />
      <div className="aurora-blob aurora-2" />
      <div className="aurora-blob aurora-3" />
      <div className="aurora-blob aurora-4" />
      <div className="aurora-blob aurora-5" />
      <motion.div
        className="pointer-events-none absolute left-0 top-0 h-[520px] w-[520px] rounded-full"
        style={{ x: sx, y: sy, background: "radial-gradient(closest-side, rgb(var(--accent) / 0.16), transparent)" }}
      />
    </div>
  );
}
