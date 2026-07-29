import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// 开发时通过 Vite 代理转发到 Rust 后端
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:7788',
      '/ws': {
        target: 'ws://127.0.0.1:7788',
        ws: true,
      },
    },
  },
  build: {
    // 构建产物输出到 server 的静态目录，由 Rust 后端托管
    outDir: '../crates/hiquant-server/static',
    emptyOutDir: true,
  },
});
