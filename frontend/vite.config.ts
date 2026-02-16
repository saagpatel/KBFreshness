import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  // Allow lean-dev mode to place Vite cache in a temporary directory.
  cacheDir: process.env.VITE_CACHE_DIR ?? '.vite',
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true,
      },
    },
  },
})
