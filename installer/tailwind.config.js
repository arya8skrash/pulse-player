// Every colour/size below resolves to a CSS variable from src/styles/tokens.css,
// so a theme change (or the custom theme editor) needs no Tailwind rebuild.
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        accent: "rgb(var(--accent) / <alpha-value>)",
        accent2: "rgb(var(--accent-2) / <alpha-value>)",
        ink: "rgb(var(--text) / <alpha-value>)",
        dim: "rgb(var(--text-dim) / <alpha-value>)",
        base: "rgb(var(--bg-base) / <alpha-value>)",
      },
      borderRadius: { glass: "var(--radius)" },
      fontFamily: { sans: ['"Inter Variable"', "Inter", "system-ui", "sans-serif"] },
      boxShadow: { glow: "0 0 40px -8px rgb(var(--accent) / 0.55)" },
    },
  },
  plugins: [],
};
