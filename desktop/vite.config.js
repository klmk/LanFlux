import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  // Tauri 不清屏，便于查看终端日志
  clearScreen: false,
  // Tauri 固定端口
  server: {
    port: 1420,
    strictPort: true,
  },
  // 暴露 VITE_ 与 TAURI_ 前缀的环境变量
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    // Tauri 使用相对路径加载前端资源
    target: 'esnext',
  },
})
