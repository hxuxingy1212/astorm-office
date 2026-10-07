---
name: astorm-office
description: 使用 astorm-office 四个 CLI（json2docx / json2xlsx / json2pptx / json2pdf）生成与编辑专业 Office 文档与 PDF：Word 论文/报告/合同（.docx）、Excel 报表/看板（.xlsx）、PPT 演示文稿（.pptx）、PDF 报告/海报（.pdf 及页操作/提取/质检），并支持 PDF 与 Word/Excel/PPT 互转（convert docx/pptx/xlsx 与 convert office）。统一工作方式：以"产物目录"（document.json / workbook.json / presentation.json / document.json + 分片 JSON + 媒体）为中间形态，repack 构建、view 查看、edit 精准修改、render 渲染核对、dump/batch 回放、serve/mcp 常驻服务；PDF 另有内容提取（extract，含版式重建/markdown/扫描件 OCR）、页操作、qa 体检与调色板设计引擎。当用户要求生成或修改 Word/Excel/PPT/PDF 文档（写论文、做表格、做 PPT、出 PDF 报告、改第 N 段/行/页、套模板、加图表公式、合并拆分 PDF、PDF 转 Word/PPT/Excel），或需要程序化处理 OOXML/PDF 文件时使用，即使用户没提到 JSON 或命令行也应触发。
version: 2.2.0
metadata:
  category: 办公自动化
  tags: docx, xlsx, pptx, pdf, word, excel, ppt, office, json2docx, json2xlsx, json2pptx, json2pdf, CLI, agent, OCR, 跨格式转换
compatibility: 需要 Rust 工具链（cargo）或已安装对应 CLI；render 输出 HTML/PDF/PNG（PNG 需 Chrome）；json2pdf 的 convert office 探测到本机 LibreOffice 走保真引擎、未装自动 native 自研渲染（chrome/tectonic 同为可选外部依赖）；OCR 免安装（macOS 系统自带 Vision）
---

# astorm-office 文档技能（统一总纲）

你负责用 **astorm-office** 四个 CLI 完成专业 Office 文档与 PDF 的**生成、查看、修改与验证**全流程。
本文件是**唯一总纲**：四格式共用的契约、工作流、命令与约束都在这里；
**每个格式的 JSON 生成规范、设计原则与场景配方在各自的格式分册**（见文末「分层索引」，按需加载）。

工具是两层结构，缺一不可：

1. **产物目录**（中间形态，JSON 优先）：每格式一个顶层 JSON + 分片文件 + 媒体，是 CLI 操作文档的统一载体；
2. **CLI**（执行层）：`unpack / repack / view / edit / render / validate / dump / batch /
   serve / mcp` 等命令在产物目录与二进制文档间转换并精准修改（四 CLI 一致；
   pdf 另有一组内容提取/页操作/质检工具命令，见分册）。

## 一、通用工作流程（四格式一致）

```text
理解需求 → 选模板/风格 → 产出产物目录 JSON → repack 构建 → render 渲染核对 → 迭代 edit → 交付
```

| 步骤 | 做什么 | 用什么 |
|------|--------|--------|
| 1 定稿 | 明确主题、结构、受众、格式与风格 | — |
| 2 生成 | 写顶层 JSON + 分片 JSON | 规范见**格式分册**，字段查 schema.json |
| 3 构建 | 产物目录 → 二进制文档 | `json2xxx repack <产物目录> -o out.xxx` |
| 4 核对 | 渲染/提取预览，检查版式/溢出/图表/公式 | Office：`render … -o preview.html`；PDF：`extract text` / `qa` |
| 5 修改 | 针对问题精准修改 | Office：`edit`；PDF：直接改产物 JSON（或两者皆改 JSON 后重新 repack） |
| 6 交付 | 给出文件路径与结构摘要 | — |

> 交付前**必须**渲染 / view / extract / qa 核对一次，不要只凭命令退出码交付。

## 二、通用命令（`json2xxx` 指代 json2docx / json2xlsx / json2pptx / json2pdf）

| 命令 | 功能 | 支持范围 |
|------|------|--------|
| `unpack <in.xxx> -o DIR/` | 二进制文档 → 产物目录 | 四 CLI 一致 |
| `repack <DIR/> -o out.xxx` | 产物目录 → 二进制文档（自动校验，问题非空退出码 3） | 四 CLI 一致 |
| `view <输入> [路径] text\|layout` | 只读查看：text（文本+媒体）/ layout（结构树），省 token | 四 CLI 一致 |
| `edit <输入> <路径> get\|set\|add\|remove [--prop k=v] [-o out]` | 精准修改 | 四 CLI 一致 |
| `render <输入> [路径] -o out` | 渲染核对（html / pdf / png） | 四 CLI 一致（pdf 的 png/pdf 需 Chrome） |
| `validate <输入>` | 结构校验（问题非空退出码 3） | 四 CLI 一致 |
| `dump <输入>` / `batch <输入> --input-file cmds.json` | 导出可回放指令 / 批量回放 | 四 CLI 一致（pdf 按页面 id 幂等替换） |
| `serve <输入>` / `mcp` | 常驻编辑（stdin JSON 行）/ MCP 服务（stdio JSON-RPC） | 四 CLI 一致 |
| `schema` / `help [元素]` | Schema 指引 / 能力速查（`--json` 机器可读） | 四 CLI 一致 |
| `templates` | 内置模板/风格列表 | Office 三格式 |
| `convert <旧文件> -o 新文件` | 旧版 .doc/.xls/.ppt → 新格式（`--engine auto\|libreoffice\|rust`） | Office 三格式 |
| `extract`（模板提取）/ `merge` / `query` / `import` / `raw` / `raw-set` | 模板提取 / `{{key}}` 填充 / 选择器查询 / CSV 导入（仅 xlsx）/ 原始 OPC 读写 | Office 三格式 |
| PDF 特有：`extract text/table/image`（text 支持 `--layout` 版式重建 / `--format markdown` / `--ocr` 扫描件识别）、`pages merge/split/rotate/crop/clean`、`meta get/set/brand`、`form info/fill`、`convert office/html/latex`（→PDF；office 引擎链 auto=本机 LibreOffice→native 自研）与 `convert docx/pptx/xlsx`（PDF→Office）、`palette`、`design`、`code/content sanitize`、`check font/toc`、`qa`、`env check` | PDF 内容提取 / 页操作 / 元数据 / 表单 / 跨格式转换 / 调色板与设计引擎 / 清洗 / 质检 | 仅 json2pdf（见分册） |

**运行方式**：开发用 `cargo run --release --bin json2xxx -- <命令>`；安装用 `cargo install --path <项目>/cli`。

**路径语法**：以 `/` 开头，每段 `类型[索引]`，索引从 **1** 起按类型计数。首段：
`/part[N]`（docx，别名 chapter）· `/sheet[N]`（xlsx，可用表名）· `/slide[N]`（pptx）·
`/page[N]`（pdf，元素段按类型计数如 `/page[1]/text[2]`，group 可下钻）；下钻语法见各分册。

**edit 语义**：输入为**产物目录** → 原地修改分片 JSON，之后 `repack` 重建；输入为**二进制文档** →
写操作（set/add/remove）必须带 `-o` 一次性输出新文件，**绝不回写原文件**（只读 `get` 不需要）。

**I/O 契约**：stdout 只输出数据（可 `jq`），状态走 stderr；全局 `--json / --quiet / --verbose`；
退出码 `0` 成功 / `1` 运行时错误 / `2` 用法错误 / `3` 产物存在校验问题。

> **dump → batch 回放是保真闭环**：对同一文档，`dump` 出的指令逐条回放后语义不变
> （xlsx 的 `add sheet` 同名幂等、cell 按 ref 幂等；ppt 的 `add` 按 `@id` 幂等原位替换；
> pdf 的整页 `add` 按页面 id 幂等原位替换，batch 可在空目录引导创建文档）。

## 三、输出纪律与通用约束

1. 落盘 JSON **不写注释**；数值/布尔**不加引号**；颜色用 6 位十六进制**不带 `#`**（ppt 幻灯片背景与 **pdf 全格式**除外，均带 `#`，见分册）；
2. 字符串换行用 `\n`；
3. 产物必须能直接 `repack`：顶层 JSON 的分片数组指向真实存在的分片文件（或按分册支持内联）；
4. 图片 `src` 为产物根相对路径或 URL；本地文件需真实存在；
5. 字段/枚举拿不准先查对应 `references/schema.json`（权威来源），再查分册；
6. **生成后必须渲染 / view / extract / qa 核对再交付**；
7. 编辑既有文档后建议做一次渲染比对（各项目 scripts/ 下有回环/视觉比对脚本）。

## 四、分层索引（按需加载，先索引后细读）

### 第 1 层 · 总纲（本文件）
通用契约、工作流、命令、约束。**任何格式都先读这里。**

### 第 2 层 · 格式分册（JSON 生成规范与设计原则，命中格式必读）

| 分册 | 内容 | 何时读 |
|------|------|--------|
| [ai-word/skill/SKILL.md](../ai-word/skill/SKILL.md) | document.json 结构、18 种块类型、run 字段、设计原则、11 类场景速查、学术卷规范 | 生成/编辑 **Word** 时 |
| [ai-excel/skill/SKILL.md](../ai-excel/skill/SKILL.md) | workbook/sheet 结构、样式/数字格式/公式、图表图片、条件格式、结构化表格、16 类模板库、报表自查 | 生成/编辑 **Excel** 时 |
| [ai-ppt/skill/SKILL.md](../ai-ppt/skill/SKILL.md) | 22 种元素类型、动画/过渡、设计原则、12 套风格、套版建议 | 生成/编辑 **PPT** 时 |
| [ai-pdf/skill/SKILL.md](../ai-pdf/skill/SKILL.md) | 产物目录结构、8 种元素类型、字体策略（标准 14 / system: / 内嵌复用）、保真度边界、提取与质检 | 生成/编辑/**解析 PDF** 时 |

### 第 3 层 · 深度参考（字段/命令/配方的权威与细节）

| 项目 | 深度参考 | 何时读 |
|------|----------|--------|
| Word | [references/cli.md](../ai-word/skill/references/cli.md) · [references/schema.json](../ai-word/skill/references/schema.json) · [references/design-guide.md](../ai-word/skill/references/design-guide.md) · [references/writing-guide.md](../ai-word/skill/references/writing-guide.md) · [references/doc-templates.md](../ai-word/skill/references/doc-templates.md) · [templates/INDEX.md](../ai-word/skill/templates/INDEX.md) · [scenarios/](../ai-word/skill/scenarios/) | 命令细节 / 字段权威 / 排版规范 / 写作规范 / 模板 / 论文场景 |
| Excel | [references/cli.md](../ai-excel/skill/references/cli.md) · [references/schema.json](../ai-excel/skill/references/schema.json) · [references/recipes.md](../ai-excel/skill/references/recipes.md) · [templates/README.md](../ai-excel/skill/templates/README.md) · [scenarios/](../ai-excel/skill/scenarios/) | 命令细节 / 字段权威 / 场景配方 / 模板库 / 财务与看板场景 |
| PPT | [references/cli.md](../ai-ppt/skill/references/cli.md) · [references/schema.json](../ai-ppt/skill/references/schema.json) · [references/design-styles.md](../ai-ppt/skill/references/design-styles.md) · [scenarios/](../ai-ppt/skill/scenarios/) | 命令细节 / 字段权威 / 12 套风格 / 路演场景 |
| PDF | [references/schema.json](../ai-pdf/skill/references/schema.json)（`json2pdf schema --json` 生成） | 字段权威 |

### 附：格式差异速记

| | docx | xlsx | pptx | pdf |
| --- | --- | --- | --- | --- |
| 顶层 JSON | `document.json`（parts 数组） | `workbook.json`（sheets 数组） | `presentation.json`（slides 数组） | `document.json`（pages 路径数组） |
| 分片 | `word/parts/partN.json`（章） | `xl/worksheets/sheetN.json`（表） | `ppt/slides/slideN.json`（页） | `pages/page-NNN.json`（页，+ media/ fonts/） |
| 长度单位 | pt（页 A4/Letter） | 列宽字符/行高 pt；区域 A1 记法 | 英寸（默认 13.333×7.5）；字号 pt | 全 pt，y 向上（文本 y 为基线） |
| 颜色 | 不带 `#` | 不带 `#` | 不带 `#`（背景例外） | 带 `#` |
| 寻址首段 | `/part[N]` | `/sheet[N]` 或 `/sheet[表名]` | `/slide[N]` | `/page[N]`（纯索引制） |
| 核对方式 | render html / pdf / png | render html（预览含合并/图表） | render png（browser / native 引擎） | render html / png / pdf + extract text / qa |

> 可选配套：`office-web/` 的 `@astorm/office-viewer` 预览组件悬浮任意元素即显示其 CLI 路径——
> 在 web 端看到的问题可直接拿到路径用 `edit` 修改（四格式均支持）。
> `json2pdf` 与三格式的互转：`convert office`（office→PDF，本机 LibreOffice 保真或 native 自研）与 `convert docx/pptx/xlsx`（PDF→Office，产物目录映射语义重建）——见 PDF 分册工作流 D。
