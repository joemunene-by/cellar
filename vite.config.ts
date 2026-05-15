import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

/**
 * Vite config for cellar. Single-page React app served by the Tauri
 * webview. Build output lands in dist/, which is what tauri.conf.json
 * frontendDist points at.
 */
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 5175,
    strictPort: true,
  },
  clearScreen: false,
});
