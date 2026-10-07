# json2pptx Web 预览与编辑

基于 **Vue 3 + Vite + pnpm** 的 PPTX 网页预览与编辑器，后端为 **Rust (axum)**，直接复用 json2pptx 库的 unpack / repack 能力。

## 架构

```
web/                          # 前端（Vue3 + Vite + pnpm + TypeScript + Pinia）
├── src/
│   ├── api/                  # 后端 API 封装
│   ├── configs/shapes.ts     # 86 个 OOXML 形状 → SVG path（程序化生成 + 手写 + fallback）
│   ├── stores/               # slides（数据）/ editor（选中/缩放）/ snapshot（撤销重做）
│   ├── utils/                # 单位换算（英寸↔px）、文本解析、元素工具
│   └── components/
│       ├── canvas/           # 画布：缩放、选中、拖拽、8 手柄缩放、旋转
│       ├── elements/         # 7 类基础元素 + 10 种组件渲染（图表用 ECharts SVG）
│       ├── panels/           # 右侧属性面板（文本/形状/线条/组件/位置/幻灯片）
│       └── AppHeader / Thumbnails / ElementToolbar
web-server/                   # 后端（Rust axum，workspace 成员）
└── src/main.rs               # 上传/unpack、overview、媒体、slide 写回、repack 下载
```

## 工作流

```
上传 PPTX → json2pptx unpack → 前端渲染产物 JSON 预览
  → 选中/拖拽/缩放/旋转 + 属性面板编辑 + 增删元素 + 背景/备注
  → 保存（写回产物 slideN.json）→ 下载（repack 重建 PPTX）
```

- 产物中的高级组件（progressBar / pieChart 等）经 parse 后已展开为形状分组（与 json2pptx 设计一致），前端按展开结果渲染；
- 前端同时支持直接渲染组件类型（编辑手工 JSON 产物场景）。

## 启动

```bash
# 1. 构建 Rust 后端（workspace 根目录）
cargo build -p json2pptx-web-server

# 2. 启动后端（端口 8080，服务 web/dist 静态文件）
cargo run -p json2pptx-web-server

# 3. 构建前端（首次需安装依赖）
cd web
pnpm install
pnpm build          # 产物 dist/，由后端直接服务

# 3'. 开发模式（热更新，vite 代理 /api → 8080）
pnpm dev            # 访问 http://localhost:5173
```

生产使用只需前两步：`pnpm build` 一次后，`cargo run -p json2pptx-web-server` 即完整服务。

## 编辑能力

| 能力 | 说明 |
|------|------|
| 选中 | 单击选中、Shift/Ctrl 多选、点击空白取消 |
| 移动 | 拖拽移动（Shift 锁定水平/垂直，越界限制） |
| 缩放 | 8 方向手柄（Ctrl/Shift 等比缩放） |
| 旋转 | 顶部旋转手柄 |
| 属性面板 | 文本（内容/字号/粗斜下划/颜色/字体/对齐）、形状（类型/填充/边框/内部文字）、线条（颜色/宽度/虚线/端点/平滑）、组件（数据/进度）、位置尺寸 |
| 文本编辑 | 双击进入编辑（多行文本），失焦保存 |
| 增删元素 | 左侧工具栏插入文本/形状/线条，Delete 删除，Ctrl+D 复制 |
| 幻灯片 | 新增/复制/删除页、背景色、演讲者备注 |
| 撤销重做 | Ctrl+Z / Ctrl+Shift+Z（快照栈 20 条） |
| 保存/下载 | 保存写回产物；下载触发 repack 重建 PPTX |

## 说明

- 数据为本地单会话模式：后端内存持有当前打开的演示，刷新页面自动恢复；
- 编辑的是 unpack 中间产物（`web-server/data/<会话>/out/`），保存后可用 CLI `repack` 或网页下载重建 PPTX；
- 形状渲染：常用 ~60 种有精确 SVG path，未覆盖的形状以矩形 + 类型名占位（可编辑，渲染近似）。
