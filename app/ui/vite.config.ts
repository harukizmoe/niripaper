import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

// Tauri reads `ui/dist` (tauri.conf.json → build.frontendDist) and, in dev,
// proxies to the devUrl `http://localhost:5173`. The build target must stay
// modern-webview: WebKitGTK 2.40+ is ES2022-capable, so no legacy polyfills.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'es2022',
    outDir: 'dist',
    emptyOutDir: true,
  },
})
