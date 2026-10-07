// 库构建后处理：dist/**/*.d.ts 里的 `@/*` 别名重写为相对路径。
// vue-tsc 声明树与 src/ 结构一致（dist/index.d.ts ↔ src/index.ts），
// 消费者的 TS 无法解析 `@/`，必须逐文件换成相对于该 d.ts 的真实路径。
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative, posix } from 'node:path'
import { fileURLToPath } from 'node:url'

const distDir = fileURLToPath(new URL('../dist', import.meta.url))

function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name)
    return statSync(p).isDirectory() ? walk(p) : p
  })
}

let files = 0
let rewrote = 0
for (const file of walk(distDir)) {
  if (!file.endsWith('.d.ts')) continue
  files++
  const relDir = posix.normalize(relative(distDir, dirname(file)))
  const depth = relDir === '.' ? './' : relDir.split('/').map(() => '..').join('/') + '/'
  const before = readFileSync(file, 'utf8')
  // `import './styles.css'` 是运行时副作用（由 exports 里的 ./style.css 承担），
  // 保留在声明里会让消费者 typecheck 报 TS2307（.css 无类型声明）
  const out = before
    .replace(/(['"])@\//g, (_, q) => `${q}${depth}`)
    .replace(/^import '\.\/styles\.css';\n/m, '')
  if (out !== before) {
    writeFileSync(file, out)
    rewrote++
  }
}
console.log(`rewrite-dts: ${rewrote}/${files} 个声明文件完成了 @/ 别名重写`)
