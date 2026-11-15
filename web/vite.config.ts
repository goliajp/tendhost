import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// where the dev server sends /api and /hostd
const server = process.env.TENDHOST_SERVER ?? 'http://127.0.0.1:7080'

const port = Number(process.env.TENDHOST_WEB_PORT ?? 5173)
const proxy = { '/api': server, '/hostd': server }

export default defineConfig({
  base: '/',
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  server: { port, strictPort: true, proxy },
  // `vite preview` serves the built dist the same way, for checking the
  // build before it is embedded in the server
  preview: { port, strictPort: true, proxy },
  build: { outDir: 'dist', emptyOutDir: true },
  test: { environment: 'jsdom', include: ['src/**/*.test.{ts,tsx}'] },
})
