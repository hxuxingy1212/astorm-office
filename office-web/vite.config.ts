import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// 组件库构建：lib 模式产出 ESM + 样式；demo 模式产出演示站
export default defineConfig(({ mode }) => ({
  base: './',
  plugins: [vue()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  publicDir: 'demo/public',
  build:
    mode === 'demo'
      ? { outDir: 'dist-demo', chunkSizeWarningLimit: 2000 }
      : {
          lib: {
            entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
            name: 'OfficeViewer',
            fileName: 'office-viewer',
            formats: ['es'],
          },
          rollupOptions: {
            external: (id: string) => id === 'vue' || id.startsWith('echarts'),
            output: { assetFileNames: 'office-viewer.[ext]' },
          },
          chunkSizeWarningLimit: 2000,
        },
  server: { port: 5178 },
}))
