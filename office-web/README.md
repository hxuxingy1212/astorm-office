# @astorm/office-viewer

astorm-office 四格式（DOCX / XLSX / PPTX / PDF）**高保真预览 Vue3 组件库**。
输入是四个 CLI（`json2docx` / `json2xlsx` / `json2pptx` / `json2pdf`）`unpack` 产物的 JSON；
鼠标悬浮任意元素会**高亮该元素**并弹出**路径卡片**（元素名 + CLI 寻址路径 + 内容摘要），
点击元素通过 `select` 事件回传 `(path, info?)`——`path` 可直接用于 `edit` / `view` / `batch`，`info`（`{ path, type, name?, text? }`，与悬浮卡片同源）可直接做引用 chip 的展示标签。

```
office-web/
├── src/
│   ├── components/            # OfficeViewer（统一入口）/ PptxViewer / DocxViewer / XlsxViewer / PdfViewer / PathCard
│   ├── core/                  # 单位换算、CLI 路径构造、悬浮卡片状态、媒体解析
│   └── renderers/
│       ├── pptx/              # 元素渲染器（移植自 ai-ppt/web，只读化 + 悬浮事件）
│       ├── pdf/               # 8 种元素渲染（text/rect/path/polyline/shading/image/pattern_rect/group）
│       ├── docx/              # 块渲染 + 迷你 LaTeX（分式/根式/上下标/希腊字母）
│       └── xlsx/              # 单元格样式、数字格式引擎（与 Rust 端同构）、图表 SVG
├── demo/                      # 演示站（四格式切换 + 悬浮路径回显）
│   └── public/samples/        # 真实样例产物（CLI unpack 生成，含图片/图表/公式）
├── examples/basic/            # 外部应用接入示例（只 import 构建产物的公开 API）
├── scripts/rewrite-dts.mjs    # 库构建后处理：d.ts 内 `@/` 别名 → 相对路径
└── tests/core.test.ts         # 纯逻辑单测（数字格式/路径/LaTeX/单位）
```

## 快速开始

```bash
pnpm install          # 或 npm i（依赖与 ai-ppt/web 相同）
pnpm dev              # 演示站 http://localhost:5178
pnpm test             # 核心逻辑单测
pnpm build            # 组件库（dist/office-viewer.js + .css + index.d.ts，vue 必需 peer，echarts 可选 peer：按需动态加载，未装时 PPT 图表渲染占位块）
pnpm build:demo       # 演示站静态构建（dist-demo/）
pnpm preview          # 预览构建产物 http://localhost:4173（与 Pages 线上形态一致）
pnpm example          # 外部接入示例 http://localhost:5188（先跑 pnpm build）
```

**在线演示**：https://hxuxiny.gitee.io/astorm-office/
（`pages` 分支发布 = `build:demo` 产物；本地更新跑 [scripts/deploy-pages.sh](scripts/deploy-pages.sh) 后
`git push origin pages`，Gitee 页面上点"重新部署"；镜像到 GitHub 则由
[`.github/workflows/pages.yml`](../.github/workflows/pages.yml) 自动发布。）
演示站资源以相对路径 (`base: './'`) 构建，任意子路径部署无需另行配置。

```ts
import { OfficeViewer, loadXlsx } from '@astorm/office-viewer'
import '@astorm/office-viewer/style.css'

// data = CLI unpack 后的 presentation.json / document.json / workbook.json
<OfficeViewer
  kind="xlsx"
  :data="workbook"
  :resolve-media="(src) => `/samples/xlsx/${src}`"
  @hover="(path) => (hovered = path)"
  @select="(path) => copy(path)"
/>
```

也可以按格式单独引入：`PptxViewer` / `DocxViewer` / `XlsxViewer` / `PdfViewer`。

## 交付形态（接入其他应用）

`pnpm build` 产出自包含的库交付物（`files: ["dist", "src"]`）：

| 产物 | 内容 |
| --- | --- |
| `dist/office-viewer.js` | ESM 打包（`vue` / `echarts` 为外部 peer，不重复打包） |
| `dist/office-viewer.css` | 全部样式：设计令牌 `--ov-*`（`src/styles.css` 经库入口引入）+ 各组件 SFC 样式 |
| `dist/index.d.ts` | 完整类型声明树（`vue-tsc` 对 `.ts`/`.vue` 生成，`scripts/rewrite-dts.mjs` 把内部 `@/` 别名改写为相对路径，并在声明里剥掉 CSS 副作用导入） |

消费方接入三步（与 [`examples/basic`](examples/basic) 完全一致的写法）：

```ts
import { OfficeViewer, loadProduct } from '@astorm/office-viewer'
import '@astorm/office-viewer/style.css'   // 或 package.json exports 的 "./style.css"
```

1. `npm i @astorm/office-viewer`（peer 自带 vue；用图表才需装 echarts）；
2. 按 props 传入 `kind` + unpack 产物 JSON，`resolveMedia` 决定图片从哪里加载；
3. 监听 `hover` / `select` / `page-change`（悬浮路径卡片是组件内置行为，无需额外接线）。

`examples/basic` 是一个免安装可跑的验证壳：vite 别名把包名指向 `../../dist`，
对**构建产物**（而非 src 源码）做消费——`pnpm build && pnpm example` 后可在
http://localhost:5188 实测四格式渲染、悬浮路径卡片与 select 事件；
`vue-tsc --noEmit -p examples/basic/tsconfig.json` 验证声明文件可被外部 TS 工程解析。


## 布局

| 查看器 | 布局 |
| --- | --- |
| `DocxViewer` | **左侧 = 全文目录 + 上下翻页**：目录按分片分组列出全部标题（点击自动切换分片并滚动定位到该标题），底部 ▲/▼ 翻分片并显示 `当前/总数`；中间为文档纸页——**整页（含全部内容）自动缩放到可视区**（默认 100% 以内适配、双向居中），底部缩放条可在适配基础上放大到 300% 后滚动查看 |
| `PptxViewer` | **左侧 = 缩略图大纲**（与 ai-ppt/web 同法：用同一套元素渲染器真实渲染整页，再按侧栏宽度 `transform: scale()` 缩放，页码 + 文本摘要，选中蓝环），右侧为幻灯片内容；舞台**自适应容器宽高**（默认适配不溢出，缩放滑杆在适配基础上 50%~220%） |
| `XlsxViewer` | 内容为主体，**底部 = 工作表标签栏**（Numbers 式底栏，单表也保留；标签色以顶部色条呈现），**底栏最右侧为缩放控件**（50%~200%，默认 100%，与 PPT 底栏同款 `.ov-zoom`，缩放含图表/图片并可横向滚动）；组件高度固定（`min(78vh, 860px)`），**切换工作表不改变整体高度**，内容超出时在表格区内滚动；`showHeadings` 控制行列标、`showGridlines` 控制网格线 |

三个查看器的事件一致：`select(path, info?)`（点击元素；`info` 为元素摘要）、`hover(path | null)`（悬浮进出）、`page-change(index)`（分片/幻灯片/工作表切换）。

**暗色主题**：`<OfficeViewer class="ov-dark" …>`（或在外层容器加 `ov-dark` 类）即得全套暗色
chrome；亮/暗两组的全部令牌（`--ov-*`）见 `src/styles.css`，可按令牌逐项覆盖。
文档页面（纸张/幻灯片/表格网格）在暗色下保持白底（同 macOS 预览），如需连页面一起暗，
覆盖 `--ov-page-bg` 即可。

## Props / Events

| 组件 | Props | Events |
| --- | --- | --- |
| `OfficeViewer` | `kind: 'docx' \| 'xlsx' \| 'pptx' \| 'pdf'`、`data`、`resolveMedia?`、`index?` | `select(path)`、`hover(path \| null)`、`page-change(index)` |
| `PptxViewer` | `presentation`（presentation.json）、`resolveMedia?`、`slideIndex?`、`showThumbnails?` | `select`、`hover`、`slide-change` |
| `DocxViewer` | `document`（document.json）、`resolveMedia?`、`partIndex?`、`pageFormat?` | `select`、`hover` |
| `XlsxViewer` | `workbook`（workbook.json）、`resolveMedia?`、`sheetIndex?`、`showHeadings?`、`showGridlines?` | `select`、`hover`、`sheet-change` |
| `PdfViewer` | `doc`（loadPdf 产物）、`resolveMedia?`、`pageIndex?`、`showThumbnails?` | `select`、`hover`、`page-change` |

`resolveMedia(src)` 把产物内的包路径（`word/media/image1.png`、`xl/media/image1.png`、`ppt/media/image1.png`）
解析为可加载 URL。默认对 pptx 使用 `/api/media/` 前缀（与 `json2pptx-web-server` 一致）。

## 路径卡片与悬浮高亮

- 悬浮元素 → **元素高亮**（系统蓝细环 + 轻蓝罩层，由 `core/hover` 统一挂 `.ov-hovering` 类，四格式观感一致）；
  卡片显示**元素类型徽标 + 名称、CLI 路径、内容摘要**（pptx 文本含 Paragraph[] 拼接、xlsx 单元格取显示值、
  docx 段落含 runs 拼接）；
- `select` 事件回传路径（示例见 demo 右侧面板）；
- 四格式路径语法与 CLI 严格一致（含 pptx 分组下钻 `group[1]/shape[2]`、docx 表格
  `table[1]/row[2]/cell[1]/paragraph[1]`、pdf 页面/组寻址 `/page[1]/group[1]/text[1]`、
  稳定 ID 可继续用 `@id=` / `@paraId=` 寻址）。

## 保真范围

| 格式 | 已渲染 | 说明 |
| --- | --- | --- |
| XLSX | 列宽/行高（字符/磅→px）、字体/填充/分侧边框/对齐/换行、数字格式（分段/会计括号/货币/颜色修饰符/可选位）、合并区、行列标、网格线开关、批注提示、超链接、图片、图表（column/bar/line/area/pie/ring/scatter，SVG） | 条件格式的图标集/色阶未渲染；冻结窗格仅记录未实现滚动锁定 |
| DOCX | 纸张/方向/页边距、命名样式（含 based_on 继承）、标题/段落/runs（加粗/斜体/下划线/删除线/颜色/字号/字体/高亮/上下标）、列表、表格（表头/列宽/对齐）、图片+题注、公式（迷你 LaTeX）、代码、引用、目录、参考文献、形状/文本框、分页符、页眉/页脚页码 | 公式为轻量渲染（非完整 TeX）；浮动布局按流式近似 |
| PPTX | 完整元素渲染（形状/文本/线/图片/图标/公式/表格/分组/10 种图表组件）、背景（纯色/渐变/图片）、缩略图导航、缩放 | 过渡动画不播放；图表由 ECharts 渲染（peer 依赖，可选安装） |
| PDF | 8 种元素（文本/矩形/路径/折线/渐变/图片/图案占位/透明组矩阵映射）、页面缩略图、缩放、悬浮路径卡 | 渲染为近似还原（与 `json2pdf render html` 同一算法）：pattern_rect 画占位底色，渐变的 clip/mask/网格不还原 |

## 视觉风格（macOS）

组件库与演示站按 macOS 观感实现（纯 CSS，无额外依赖）：

- **字体**：SF Pro / `-apple-system` + PingFang SC 系统栈（文档正文保留论文/网格自身的字体语义）；
- **控件**：分段控件（工作表标签 / 分片标签 / 顶部格式切换）、圆角工具条按钮、毛玻璃统一工具条（`backdrop-filter: saturate(180%) blur(20px)`）；
- **配色**：系统蓝 `#007AFF` 作为强调色、系统灰阶（label / secondary / separator）、0.5px 发丝分隔线；
- **路径卡片**：macOS 弹出层（毛玻璃 + 12px 圆角 + 指向箭头 + 系统灰层级文字），路径以等宽字体显示在内凹 chip 中；
- **演示站**：macOS 窗口外壳（交通灯 + 毛玻璃标题栏）+ 毛玻璃 inspector 侧栏；pptx 舞台为 Keynote 式浅灰画布 + 胶片条缩略图（选中蓝环）。

设计令牌集中在 [`src/styles.css`](src/styles.css) 的 `--ov-*` 变量中，可整体替换以适配其他设计体系。

## 性能（xlsx 虚拟滚动）

大表按**行虚拟化**渲染：DOM 只包含视口内的行（含少量 overscan），滚动位置由「前缀高度」精确映射，
百万行与千行渲染成本相同。

实现要点（`src/renderers/xlsx/virtual.ts`，纯函数 + 单测）：

- 行高前缀：默认行高统一处理，显式行高只存稀疏增量，`rowTop()` 二分定位，O(log n) 且不占内存；
- 视口裁剪：`visibleRows()` 用滚动位置二分出可见行区间，渲染时表格整体按偏移绝对定位
  （避免"巨型占位行"——那是早期实现，滚动会触发上千万像素的布局）；
- 画布压缩：浏览器可滚动高度实测上限 16,777,215px（2^24-1），超过则按比例压缩画布高度，
  滚动位置同步换算（压缩像素 ↔ 逻辑像素），保证百万行都能滚到；
- 引用解析 `refToPos()` 手写解析（正则版在百万单元格场景明显偏慢）。

实测（Chrome，100 万行 × 6 列 = 600 万单元格，`demo` 的「⚡ 性能压测」页可复现）：

| 指标 | 数值 |
| --- | --- |
| 首屏渲染 | 0.6 ~ 1.5 s（含首次全表扫描列范围） |
| DOM 行数 / 节点数 | 44 ~ 60 行 / 约 635 节点（与总行数无关） |
| 滚动一次渲染 | **6 ~ 8 ms** |
| 滚动到底部 | 正确显示第 999,948 ~ 1,000,000 行 |

### 内存：列式打包（`src/renderers/xlsx/packed.ts`）

产物 JSON 是"每单元格一个对象"（约 140B/单元格，600 万单元格 ≈ 800MB+），是百万行场景的真正瓶颈。
`packSheet()` 把工作表压成 **TypedArray + 字符串池**：

| 存储 | 每单元格 | 说明 |
| --- | --- | --- |
| 列号 `Uint16Array` | 2B | 1-based 列号 |
| 数值 `Float64Array` | 8B | 非数值为 NaN，不额外分配 |
| 字符串池下标 `Int32Array` | 4B | 重复字符串只存一份（表头/标签/分类去重率高） |
| 数字格式池下标 `Int32Array` | 4B | 格式串去重 |
| 类型码 `Uint8Array` | 1B | number/string/bool/empty |
| 行前缀 + 行高 | 8B/行 | `rowStart` + `heights` |

公式/样式/超链接/批注等低频字段走**稀疏 side-map**（`extras`，无值不占条目），因此打包后可以
`sheet.rows = []` 丢弃对象模型——渲染器只通过 `SheetAdapter` 取数，两种形态（JSON 原样 / 已打包）共用同一套渲染逻辑。

```ts
// 推荐用法：加载即打包，内存立刻降到列式水平
const wb = await loadXlsx('/samples/book', { pack: true })   // { sheets: PackedSheet[] }
```

实测（Chrome，100 万行 × 6 列 = 600 万单元格）：

| 指标 | JSON 对象模型 | 列式打包 | 变化 |
| --- | --- | --- | --- |
| 合成/加载耗时 | 561 ~ 1038 ms | 138 ~ 275 ms | **约 3~4× 快** |
| 列式体积（确定性计算） | — | **116.3 MB**（约 19B/单元格） | — |
| JS 堆占用（实测，随 GC 波动） | 529 ~ 816 MB | 221 ~ 391 MB | **约 2~3.7× 少** |
| 首屏渲染 | 2 ~ 1507 ms | 2 ~ 653 ms | 打包形态无全表扫描，稳定在个位数~百毫秒 |
| 滚动一次渲染 | 6 ~ 12 ms | 6 ~ 12 ms | 同为视口级 |

> 堆占用受 GC 时机影响，绝对数字会波动；确定性结论是每单元格 140B → 19B，且打包形态无需
> 全表扫描（列范围、行号、字符串去重都在打包时一次完成）。

### 流式解析（`src/renderers/xlsx/stream.ts`）

`JSON.parse(整个 sheet.json)` 会在解析期产生「每单元格一个临时对象」的峰值（600 万单元格 ≈ 800MB 临时对象
+ 数百 MB 文本），这是打包之后的下一个瓶颈。`packSheetFromStream()` 改为**按行流式**：

- 在字符流上做括号匹配（识别字符串与转义，跨分块安全），每切出一个完整行对象就单独 `JSON.parse`
  这一个行对象并立即写入列式数组 —— 临时对象数量被限制在单行规模；
- 头部（`rows` 之前的字段）与尾部（`merges`/`charts`/`images`/`columns`/`print`）各累积后解析一次（体积很小）；
- 列式数组按块扩容（`Uint16Array`/`Float64Array`/`Int32Array` 等），行尾做一次行内有序化
  （真实文件同一行单元格顺序不保证升序，而行内定位用二分，必须有序）。

```ts
// 加载即流式打包：解析期不产生对象模型，峰值 ≈ 列式数组 + 分块文本
const wb = await loadXlsx('/samples/big-book', { pack: 'stream' })
```

### 三种形态实测对比（100 万行 × 6 列 = 600 万单元格）

| 形态 | 列式体积 | JS 堆占用 | 构造/解析耗时 | 首屏渲染 |
| --- | --- | --- | --- | --- |
| JSON 对象模型 | —（对象图约 800MB） | 529 ~ 816 MB | 561 ~ 1038 ms | 2 ~ 1507 ms |
| 列式打包（直接构造） | 168 MB（含字符串池） | 221 ~ 388 MB | 138 ~ 275 ms | 2 ms |
| **流式解析**（516MB 文本流） | 179 MB | **324 MB** | 6818 ms（含生成 516MB 文本 + 逐行解析） | 1 ms |

> 流式模式对应真实「加载一个大文件」的场景：若改用 `fetch text + JSON.parse`，需要同时驻留
> 516MB 文本与约 800MB 对象图（峰值 ≈ 1.3GB）；流式模式峰值 ≈ 列式数组 179MB + 有限分块。
> 三种形态下 DOM 都只有 44~60 行（虚拟滚动），滚动渲染 6~12ms。

#### 解析策略与实测吞吐（Chrome，20MB / 24 万单元格，每次刷新页面单跑一条路径）

我们实现了三条流式路径，并用「解析吞吐基准」按钮做了隔离对照（避免多次测量间的 GC 干扰）：

| 路径 | 吞吐 | 说明 |
| --- | --- | --- |
| 原生 `JSON.parse`（整段文本） | ~228 MB/s | 最快，但会构造整表对象图（内存问题依旧） |
| 流式 · 批解析（`batch`，**默认**） | ~42 MB/s | 约 1MB 行文本拼成数组交给原生解析器，瞬时对象限制在 MB 级 |
| 流式 · 逐行 `JSON.parse`（`json`） | ~44 MB/s | 每行一次解析调用，保留作对照 |
| 流式 · 字段级扫描（`scanner`） | ~37 MB/s | 单遍零键分配，完全无临时对象 |

结论（实测，非推测）：**三条流式路径速度相近**，瓶颈不在"解析器选型"，而在逐行的
`matchObjectEnd` 扫描 + 行文本 `slice` + 行级写入这三项固定开销（我们控制不了 V8 的
`JSON.parse` 单次调用成本）。因此默认取 `batch`：调用次数少、借用 V8 的转义/数字语义、
瞬时对象有界；`scanner` 保留给"完全不允许临时对象"的场景。

要让加载再上一个量级，方向已经不是把 JSON 解析得更快，而是**不解析 JSON**：
由 CLI 侧（`json2xlsx`）在产物目录里额外输出一份**列式二进制**（如 `xl/worksheets/sheet1.bin`：
行前缀 / 列号 / 数值 / 字符串池 用定长小端编码），前端 `ArrayBuffer` 直接读成 TypedArray，
加载从"解析"变成"读字节"（预期 GB/s 级，且零对象图）。组件库的列式结构已经与之同构，
只差 CLI 侧的写出与前端的一个 loader。

## 与 ai-ppt/web 的关系

`src/renderers/pptx/*` 移植自 `ai-ppt/web`（同一个 `json2pptx` 模型与渲染器），
改动：只读默认（`mode="view"`）、悬浮事件（`element-hover/move/leave`）、
媒体解析改为 `provide/inject`（默认仍回退 `/api/media/`）。
ai-ppt/web 仍是完整的**编辑器**；本库是面向四格式的**只读预览组件**。

## 验证

- `pnpm test`：数字格式/路径/LaTeX/单位 30+ 断言（离线可跑）
- `pnpm typecheck` / `pnpm build`：vue-tsc + vite 构建
- 演示站用于人工/浏览器截图核对（四格式 + 悬浮卡片）
