import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// Tauri expects a fixed port and must not have its Rust sources watched by Vite.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // @abyss/ui is a link into the Abyss folder; resolve its imports (react, zustand…) from this app,
  // so there is exactly one React.
  resolve: { preserveSymlinks: true },
  server: {
    port: 1522,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1522 + 1 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // WebView2 on Windows is evergreen Chromium.
    target: "chrome120",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
