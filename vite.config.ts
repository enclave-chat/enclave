import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import type { ProxyOptions } from "vite";

const newsProxy: Record<string, ProxyOptions> = {
  "/news/thn": {
    target: "https://feeds.feedburner.com",
    changeOrigin: true,
    rewrite: (p) => p.replace(/^\/news\/thn/, "/TheHackersNews"),
    headers: {
      "User-Agent": "Mozilla/5.0",
    },
  },
  "/news/krebs": {
    target: "https://krebsonsecurity.com",
    changeOrigin: true,
    rewrite: (p) => p.replace(/^\/news\/krebs/, "/feed/"),
    headers: {
      "User-Agent": "Mozilla/5.0",
    },
  },
  "/news/cisa": {
    target: "https://www.cisa.gov",
    changeOrigin: true,
    rewrite: (p) =>
      p.replace(/^\/news\/cisa/, "/cybersecurity-advisories/all.xml"),
    headers: {
      "User-Agent": "Mozilla/5.0",
    },
  },
  "/news/bc": {
    target: "https://www.bleepingcomputer.com",
    changeOrigin: true,
    rewrite: (p) => p.replace(/^\/news\/bc/, "/feed/"),
    headers: {
      "User-Agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Safari/537.36",
      Accept:
        "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8, */*;q=0.7",
    },
  },
};

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    proxy: newsProxy,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
