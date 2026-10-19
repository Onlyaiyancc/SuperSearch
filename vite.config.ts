import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // 本地工具链和验收存档体积很大，不作为前端入口或热更新监听目标。
  optimizeDeps: { entries: ['index.html'] },
  server: {
    port: 1420, strictPort: true, host: '127.0.0.1',
    watch: { ignored: ['**/.tools/**', '**/src-tauri/**', '**/artifacts/**', '**/test-results/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: { target: 'chrome105' },
});
