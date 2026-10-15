import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Porta fixa em 1420: o `devUrl` do tauri.conf.json aponta para ela e o
// strictPort garante falha explícita se a porta já estiver em uso.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2021",
    outDir: "dist",
    emptyOutDir: true,
  },
});
