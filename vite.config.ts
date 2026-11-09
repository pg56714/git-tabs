/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// `vite --mode web` runs the UI in a normal browser against the dev server (crates/devserver).
export default defineConfig(({ mode }) => ({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5320,
    strictPort: true,
    host: '127.0.0.1',
    proxy: mode === 'web' ? { '/api': 'http://127.0.0.1:5319' } : undefined,
  },
  build: { target: 'es2023', chunkSizeWarningLimit: 2000 },
  test: { environment: 'node' },
}))
