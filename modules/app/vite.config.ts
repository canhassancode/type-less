import { resolve } from 'node:path';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
  },
  build: {
    target: 'esnext',
    rollupOptions: {
      input: {
        settings: resolve(__dirname, 'settings.html'),
        pill: resolve(__dirname, 'pill.html'),
        overlay: resolve(__dirname, 'overlay.html'),
      },
    },
  },
});
