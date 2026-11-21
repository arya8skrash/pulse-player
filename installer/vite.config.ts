import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const here = path.dirname(fileURLToPath(import.meta.url));

// The setup app shares Pulse's design tokens and glass/aurora CSS (../src/styles), so the dev
// server must be allowed to read one folder up.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1421, strictPort: true, fs: { allow: [path.resolve(here, "..")] } },
  envPrefix: ["VITE_", "TAURI_"],
  build: { target: "es2021", sourcemap: false },
});
