import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const root = fileURLToPath(new URL('.', import.meta.url));

// The app loads from Tauri's own protocol: no CDN, no remote font, nothing inlined as a data:
// URI (the CSP allows 'self' only), no modulepreload polyfill.
export default defineConfig({
  root,
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
    sourcemap: false,
  },
});
