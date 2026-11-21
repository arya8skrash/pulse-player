import { useEffect, useState } from "react";
import { artUrl } from "../lib/ipc";
import { PulseMark } from "./PulseLogo";

/** Cover art, or — when a track has none — a glassy placeholder tinted by the current theme. */
export function ArtImage({ path, className = "", markSize }: { path: string | null | undefined; className?: string; markSize?: number }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [path]);
  const src = failed || !path
    ? undefined
    : /^https?:\/\//i.test(path)
      ? path
      : artUrl(path);

  if (src) {
    return <img src={src} alt="" loading="lazy" draggable={false} onError={() => setFailed(true)} className={`object-cover ${className}`} />;
  }
  return (
    <div
      aria-hidden
      className={`relative grid place-items-center overflow-hidden ${className}`}
      style={{
        background:
          "radial-gradient(120% 120% at 20% 10%, rgb(var(--aurora-1) / 0.9), transparent 60%)," +
          "radial-gradient(120% 120% at 90% 100%, rgb(var(--aurora-2) / 0.8), transparent 55%)," +
          "rgb(var(--aurora-3) / 0.55)",
      }}
    >
      <div className="absolute inset-0" style={{ background: "linear-gradient(135deg, rgb(255 255 255 / 0.22), transparent 45%, rgb(0 0 0 / 0.12))", boxShadow: "inset 0 0 0 1px rgb(255 255 255 / 0.18)" }} />
      <PulseMark size={markSize ?? 40} className="relative text-ink/85 drop-shadow-[0_0_10px_rgb(var(--accent)/0.7)]" />
    </div>
  );
}
