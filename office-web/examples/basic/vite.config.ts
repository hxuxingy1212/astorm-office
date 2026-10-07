import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// 外部消费示例：模拟"别的应用"直接使用 @astorm/office-viewer 的构建产物。
// 真实项目中应用方 `npm i @astorm/office-viewer` 后无需任何别名；
// 这里用别名指向 ../../dist，以共享 node_modules（不重复安装）。
const pkgRoot = fileURLToPath(new URL('../..', import.meta.url))

export default defineConfig({
  plugins: [vue()],
  root: fileURLToPath(new URL('.', import.meta.url)),
  publicDir: fileURLToPath(new URL('../../demo/public', import.meta.url)),
  resolve: {
    alias: {
      '@astorm/office-viewer/style.css': `${pkgRoot}/dist/office-viewer.css`,
      '@astorm/office-viewer': `${pkgRoot}/dist/office-viewer.js`,
    },
  },
  server: { port: 5188 },
})
