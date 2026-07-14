import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

// Tauri 期望固定的端口和文件结构
export default defineConfig(async () => ({
  plugins: [react()],
  // 防止 vite 清除 Rust 错误
  clearScreen: false,
  // Tauri 期望固定的 1420 端口
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    hmr: {
      protocol: 'ws',
      host: 'localhost',
      port: 1421,
    },
    watch: {
      // 告诉 vite 忽略 Rust 源码
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
      },
    },
  },
  optimizeDeps: {
    // 启动时立刻预构建, 避免首次启动触发 "optimized dependencies changed. reloading"
    // (那次 reload 会让 Tauri webview 在 React 树挂载中途 reload, 出现白屏).
    include: [
      'react',
      'react-dom',
      'react-dom/client',
      'zustand',
      'zustand/middleware',
    ],
  },
}));
