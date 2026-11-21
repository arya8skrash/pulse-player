// Visual definitions live in styles/tokens.css; this list drives the pickers.
export type Scheme = "midnight" | "amethyst" | "ocean" | "rose" | "emerald" | "slate" | "copper" | "twilight";
export type Mode = "dark" | "light" | "system";

export interface SchemeInfo {
  id: Scheme;
  name: string;
  dark: [string, string];
  light: [string, string];
}

export const SCHEMES: SchemeInfo[] = [
  { id: "midnight", name: "Midnight Blue", dark: ["#101e42", "#8ca8f5"], light: ["#9ebef5", "#3e5ebe"] },
  { id: "amethyst", name: "Soft Amethyst", dark: ["#2a1e46", "#b8a0e6"], light: ["#cdbeee", "#704aac"] },
  { id: "ocean", name: "Deep Ocean", dark: ["#082638", "#68c4ce"], light: ["#a8d8e8", "#10708c"] },
  { id: "rose", name: "Soft Rose", dark: ["#3c1c28", "#e29cae"], light: ["#f5c6d2", "#a83e60"] },
  { id: "emerald", name: "Deep Emerald", dark: ["#0a281e", "#96dcb4"], light: ["#aadcbe", "#1e7846"] },
  { id: "slate", name: "Carbon Slate", dark: ["#1e222a", "#c8d2dc"], light: ["#c8ccd2", "#3c4650"] },
  { id: "copper", name: "Muted Copper", dark: ["#3c2214", "#e6b496"], light: ["#f0d2be", "#a05028"] },
  { id: "twilight", name: "Twilight Rose", dark: ["#1e1828", "#d98f73"], light: ["#e6c8be", "#be6450"] },
];