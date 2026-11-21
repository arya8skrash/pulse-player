// src/components/SplashScreen.tsx
import { useEffect, useRef } from "react";
import { motion } from "framer-motion";

interface SplashScreenProps {
  onAnimationComplete: () => void;
}

const PULSE_PATH =
  "M0,40 L60,40 L80,40 L95,10 L110,70 L125,5 L140,75 L155,40 L175,40 L340,40 L400,40";

export default function SplashScreen({ onAnimationComplete }: SplashScreenProps) {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    timeoutRef.current = setTimeout(onAnimationComplete, 3200);
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [onAnimationComplete]);

  return (
    <motion.div
      key="splash"
      className="fixed inset-0 z-[9999] flex flex-col items-center justify-center overflow-hidden"
      style={{ background: "rgb(var(--bg-base))" }}
      initial={{ opacity: 1 }} 
      animate={{ opacity: 1 }}
      // Removed the blur effect here to prevent WebView2 from crashing.
      exit={{ opacity: 0, scale: 1.02 }}
      transition={{ duration: 0.6, ease: "easeInOut" }}
    >
      {/* ── Aurora haze (breathing animation) ── */}
      <motion.div
        aria-hidden
        className="pointer-events-none absolute inset-0"
        animate={{
          opacity: [0.6, 1, 0.6],
          scale: [1, 1.05, 1],
        }}
        transition={{
          duration: 4,
          repeat: Infinity,
          ease: "easeInOut",
        }}
        style={{
          background: `
            radial-gradient(ellipse 70% 40% at 50% 60%,
              rgb(var(--aurora-1) / 0.4) 0%,
              transparent 70%),
            radial-gradient(ellipse 50% 30% at 30% 40%,
              rgb(var(--aurora-2) / 0.3) 0%,
              transparent 60%),
            radial-gradient(ellipse 40% 25% at 70% 65%,
              rgb(var(--aurora-3) / 0.2) 0%,
              transparent 55%)
          `,
        }}
      />

      {/* ── Micro-glass reflection strip ── */}
      <div
        aria-hidden
        className="pointer-events-none absolute"
        style={{
          width: "480px",
          height: "1px",
          top: "calc(50% + 54px)",
          background:
            "linear-gradient(90deg, transparent 0%, rgb(var(--accent) / 0.15) 30%, rgb(var(--accent) / 0.3) 50%, rgb(var(--accent) / 0.15) 70%, transparent 100%)",
          filter: "blur(1px)",
        }}
      />

      {/* ── Main card ── */}
      <motion.div
        className="relative flex flex-col items-center gap-8 select-none"
        initial={{ opacity: 0, y: 18 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.7, ease: [0.22, 1, 0.36, 1], delay: 0.3 }}
      >
        {/* ECG pulse line */}
        <motion.div 
          className="relative" 
          style={{ width: 400, height: 80 }}
          animate={{ scale: [1, 1.02, 1] }}
          transition={{ duration: 2, repeat: Infinity, ease: "easeInOut" }}
        >
          {/* Glow layer */}
          <svg
            aria-hidden
            width="400"
            height="80"
            viewBox="0 0 400 80"
            className="absolute inset-0"
            style={{ filter: "blur(8px)", opacity: 0.6 }}
          >
            <motion.path
              d={PULSE_PATH}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeWidth="3"
              strokeLinecap="round"
              strokeLinejoin="round"
              pathLength={1}
              initial={{ pathLength: 0, opacity: 0 }}
              animate={{ pathLength: 1, opacity: 1 }}
              transition={{ duration: 1.4, ease: "easeInOut", delay: 0.6 }}
            />
          </svg>

          {/* Sharp layer */}
          <svg
            width="400"
            height="80"
            viewBox="0 0 400 80"
            role="img"
            aria-label="Pulse heartbeat line"
          >
            <defs>
              <linearGradient id="pulseGrad" x1="0%" y1="0%" x2="100%" y2="0%">
                <stop offset="0%"   stopColor="rgb(var(--accent-2))" stopOpacity="0.3" />
                <stop offset="35%"  stopColor="rgb(var(--accent))" stopOpacity="0.9" />
                <stop offset="55%"  stopColor="rgb(var(--text))" stopOpacity="1"   />
                <stop offset="75%"  stopColor="rgb(var(--accent))" stopOpacity="0.9" />
                <stop offset="100%" stopColor="rgb(var(--accent-2))" stopOpacity="0.3" />
              </linearGradient>
            </defs>
            <motion.path
              d={PULSE_PATH}
              fill="none"
              stroke="url(#pulseGrad)"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              pathLength={1}
              initial={{ pathLength: 0, opacity: 0 }}
              animate={{ pathLength: 1, opacity: 1 }}
              transition={{ duration: 1.4, ease: "easeInOut", delay: 0.6 }}
            />
          </svg>

          {/* Travelling bright dot */}
          <motion.div
            aria-hidden
            className="absolute rounded-full"
            style={{
              width: 6,
              height: 6,
              top: "calc(50% - 3px)",
              background: "rgb(var(--text))",
              boxShadow: "0 0 12px 4px rgb(var(--accent) / 0.8)",
            }}
            initial={{ left: 0, opacity: 0 }}
            animate={{ left: "100%", opacity: [0, 1, 1, 0] }}
            transition={{ duration: 1.4, ease: "easeInOut", delay: 0.6 }}
          />
        </motion.div>

        {/* App name */}
        <motion.div
          className="flex flex-col items-center gap-1"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.6, delay: 1.6 }}
        >
          <h1
            className="font-semibold tracking-[0.22em] uppercase"
            style={{
              fontSize: "2rem",
              letterSpacing: "0.22em",
              color: "rgb(var(--text))",
              fontFamily: "'Inter', 'SF Pro Display', system-ui, sans-serif",
              textShadow: "0 0 20px rgb(var(--accent) / 0.5)",
            }}
          >
            PULSE
          </h1>
          <p
            style={{
              fontSize: "0.7rem",
              letterSpacing: "0.3em",
              color: "rgb(var(--text-dim))",
              fontFamily: "'Inter', system-ui, sans-serif",
              textTransform: "uppercase",
            }}
          >
            Music Player
          </p>
        </motion.div>
      </motion.div>

      {/* ── Loading indicator ── */}
      <motion.div
        aria-hidden
        className="absolute bottom-10 flex gap-1.5"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 2.0, duration: 0.4 }}
      >
        {[0, 1, 2].map((i) => (
          <motion.span
            key={i}
            className="block rounded-full"
            style={{
              width: 4,
              height: 4,
              background: "rgb(var(--accent))",
            }}
            animate={{ opacity: [0.3, 1, 0.3], y: [0, -3, 0] }}
            transition={{
              duration: 0.9,
              repeat: Infinity,
              delay: i * 0.18,
              ease: "easeInOut",
            }}
          />
        ))}
      </motion.div>
    </motion.div>
  );
}