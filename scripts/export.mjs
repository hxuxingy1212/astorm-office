#!/usr/bin/env node
// 官方导出包：把可交付物收敛到一个固定结构的目录，消费方（桌面端等）
// 只拷贝这个目录 + 校验 SHA256SUMS，不再感知本仓库内部布局。
//
// 用法：
//   node scripts/export.mjs                 # 构建（cargo release + viewer dist）并导出
//   node scripts/export.mjs --out <dir>     # 指定输出根（默认 dist-kit/）
//   node scripts/export.mjs --skip-build    # 复用已有 target/release 与 office-web/dist
//
// 产物结构（office-kit-<version>/）：
//   bin/        四个 CLI 可执行文件（json2docx/json2xlsx/json2pptx/json2pdf）
//   skill/      根总纲 SKILL.md + 四格式分册（ai-*/skill/ 完整子树）
//   viewer/     @astorm/office-viewer dist 三件套（js/css/d.ts）
//   samples/    四格式样例产物目录（预览/悬浮功能手测夹具）
//   VERSION     kit 版本（= workspace version，发布纪律：发版必须 bump）
//   SHA256SUMS  全部文件的 sha256 清单（`shasum -a 256 -c` 可验）
//   _metadata.json  版本/VCS commit/构建时间与工具链，机器可读
//
// 版本权威：workspace Cargo.toml 的 version。SKILL.md frontmatter 的 version
// 与 kit 版本保持一致；发版时一并 bump。

import { createHash } from 'node:crypto'
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { execSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('.', import.meta.url)), '..')
const args = process.argv.slice(2)
const skipBuild = args.includes('--skip-build')
const outIdx = args.indexOf('--out')
const OUT_ROOT = outIdx >= 0 ? resolve(args[outIdx + 1]) : join(ROOT, 'dist-kit')

const BINS = ['json2docx', 'json2xlsx', 'json2pptx', 'json2pdf']
const SKILL_BOOKS = ['ai-word', 'ai-excel', 'ai-ppt', 'ai-pdf']

function sh(cmd, opts = {}) {
  console.log(`  $ ${cmd}`)
  execSync(cmd, { stdio: 'inherit', cwd: ROOT, ...opts })
}

function kitVersion() {
  const cargo = readFileSync(join(ROOT, 'Cargo.toml'), 'utf8')
  const m = cargo.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/)
  if (!m) throw new Error('Cargo.toml 里找不到 [workspace.package] version')
  return m[1]
}

function gitCommit() {
  try {
    return execSync('git rev-parse HEAD', { cwd: ROOT }).toString().trim()
  } catch {
    return null // 非 git 环境（如源码包）允许为空
  }
}

function hashFile(p) {
  return createHash('sha256').update(readFileSync(p)).digest('hex')
}

function walk(dir, base = dir, acc = []) {
  for (const name of readdirSync(dir).sort()) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, base, acc)
    else acc.push(p)
  }
  return acc
}

async function main() {
  const version = kitVersion()
  const kitDir = join(OUT_ROOT, `office-kit-${version}`)
  console.log(`== 导出 office-kit ${version} → ${kitDir}`)

  if (!skipBuild) {
    console.log('== 构建 CLI（release）')
    sh(`cargo build --release -p json2docx-cli -p json2xlsx-cli -p json2pptx-cli -p json2pdf-cli`)
    console.log('== 构建 viewer（office-web dist）')
    sh('npm run build', { cwd: join(ROOT, 'office-web') })
  }

  rmSync(kitDir, { recursive: true, force: true })
  mkdirSync(join(kitDir, 'bin'), { recursive: true })

  // bin/
  for (const bin of BINS) {
    const src = join(ROOT, 'target/release', bin)
    if (!existsSync(src)) throw new Error(`缺少可执行文件 ${src}（先去掉 --skip-build 构建）`)
    cpSync(src, join(kitDir, 'bin', bin))
  }

  // skill/：总纲 + 四格式分册（含 references/scenarios/templates/samples 子树）
  cpSync(join(ROOT, 'skill'), join(kitDir, 'skill'), { recursive: true })
  for (const book of SKILL_BOOKS) {
    cpSync(join(ROOT, book, 'skill'), join(kitDir, 'skill', book), { recursive: true })
  }

  // viewer/：dist 三件套（vite publicDir 会把 demo 样例带进 dist，剔除——kit 根已有 samples/）
  const dist = join(ROOT, 'office-web/dist')
  if (!existsSync(join(dist, 'office-viewer.js'))) throw new Error(`缺少 ${dist}（先构建 viewer）`)
  cpSync(dist, join(kitDir, 'viewer'), { recursive: true })
  rmSync(join(kitDir, 'viewer', 'samples'), { recursive: true, force: true })

  // samples/：四格式样例（demo 手测夹具转正）
  cpSync(join(ROOT, 'office-web/demo/public/samples'), join(kitDir, 'samples'), { recursive: true })

  // VERSION + _metadata.json
  writeFileSync(join(kitDir, 'VERSION'), `${version}\n`)
  writeFileSync(
    join(kitDir, '_metadata.json'),
    JSON.stringify(
      {
        kit: 'office-kit',
        version,
        git_commit: gitCommit(),
        built_at: new Date().toISOString(),
        bins: BINS,
        viewer: JSON.parse(readFileSync(join(ROOT, 'office-web/package.json'), 'utf8')).version,
        node: process.version,
      },
      null,
      2,
    ) + '\n',
  )

  // SHA256SUMS：覆盖除自身外的全部文件（相对 kit 根路径）
  const files = walk(kitDir).filter((p) => !p.endsWith('SHA256SUMS'))
  const sums = files
    .map((p) => `${hashFile(p)}  ${p.slice(kitDir.length + 1)}`)
    .join('\n')
  writeFileSync(join(kitDir, 'SHA256SUMS'), sums + '\n')

  const total = files.reduce((n, p) => n + statSync(p).size, 0)
  console.log(`== 完成：${files.length} 个文件，约 ${(total / 1024 / 1024).toFixed(1)} MB`)
  console.log(`   校验：cd ${kitDir} && shasum -a 256 -c SHA256SUMS`)
}

main().catch((e) => {
  console.error(e.message)
  process.exit(1)
})
