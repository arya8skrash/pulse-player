export type Scheme = "midnight" | "amethyst" | "ocean" | "rose" | "emerald" | "slate" | "copper" | "twilight";
export type Mode = "dark" | "light" | "system";

export interface SchemeInfo {
  id: Scheme;
  name: string;
  tagline: string;
  dark: [string, string];
  light: [string, string];
}

// Same schemes as the player (see ../../src/styles/tokens.css).
export const SCHEMES: SchemeInfo[] = [
  { id: "midnight", name: "Midnight Blue", tagline: "Calm and deep", dark: ["#101e42", "#8ca8f5"], light: ["#9ebef5", "#3e5ebe"] },
  { id: "amethyst", name: "Soft Amethyst", tagline: "Quiet violet", dark: ["#2a1e46", "#b8a0e6"], light: ["#cdbeee", "#704aac"] },
  { id: "ocean", name: "Deep Ocean", tagline: "Cool and clear", dark: ["#082638", "#68c4ce"], light: ["#a8d8e8", "#10708c"] },
  { id: "rose", name: "Soft Rose", tagline: "Warm and gentle", dark: ["#3c1c28", "#e29cae"], light: ["#f5c6d2", "#a83e60"] },
  { id: "emerald", name: "Deep Emerald", tagline: "Lush and grounded", dark: ["#0a281e", "#96dcb4"], light: ["#aadcbe", "#1e7846"] },
  { id: "slate", name: "Carbon Slate", tagline: "Sleek and neutral", dark: ["#1e222a", "#c8d2dc"], light: ["#c8ccd2", "#3c4650"] },
  { id: "copper", name: "Muted Copper", tagline: "Warm and earthy", dark: ["#3c2214", "#e6b496"], light: ["#f0d2be", "#a05028"] },
  { id: "twilight", name: "Twilight Rose", tagline: "Deep and elegant", dark: ["#1e1828", "#d98f73"], light: ["#e6c8be", "#be6450"] },
];

export const prefersLight = () => !window.matchMedia("(prefers-color-scheme: dark)").matches;
export const isLight = (mode: Mode) => mode === "light" || (mode === "system" && prefersLight());

/** Applies the palette to the whole window, crossfading when the browser supports it. */
export function applyTheme(scheme: Scheme, mode: Mode, animate: boolean) {
  const swap = () => {
    const root = document.documentElement;
    root.dataset.theme = scheme;
    root.dataset.mode = isLight(mode) ? "light" : "dark";
  };
  const doc = document as Document & { startViewTransition?: (cb: () => void) => unknown };
  if (animate && doc.startViewTransition && !matchMedia("(prefers-reduced-motion: reduce)").matches) {
    doc.startViewTransition(swap);
  } else {
    swap();
  }
}