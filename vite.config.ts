import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri drives the dev server; fixed port, no auto-open, no clearScreen so
// Rust logs stay visible next to Vite's.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "chrome105", outDir: "dist", emptyOutDir: true },
});
