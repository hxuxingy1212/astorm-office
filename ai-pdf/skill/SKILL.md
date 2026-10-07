---
name: ai-pdf
description: 用 json2pdf 生成、解析、编辑 PDF 并与 Word/Excel/PPT 互转：unpack/repack/view/edit/render、页操作（合并/拆分/旋转）、内容提取（含 OCR）、qa 体检。用户要生成/修改/解析 PDF 或跨格式互转时使用，未提 JSON 也应触发。
version: 0.2.0
metadata:
  category: 办公自动化
  tags: pdf, json, unpack, repack, 文档处理, json2pdf, CLI, OCR, 跨格式转换
compatibility: 需要 Rust 工具链（cargo）或已安装 json2pdf；convert html/latex 需要本机 Chrome / tectonic；convert office 探测到本机 LibreOffice 走保真引擎，未装自动走 native 自研渲染；OCR 免安装（macOS 系统自带 Vision，tesseract 可选兜底，HTTP 服务可选 UNIOCR_URL）
---

# PDF 分册（json2pdf）

你负责用 **json2pdf** 完成 PDF 的**解析、查看、编辑、生成与质检**。
通用契约（I/O 纪律、退出码、全局 flag）与四格式工作流见统一总纲
[`skill/SKILL.md`](../../skill/SKILL.md)；本分册只讲 PDF 特有的内容。

两层结构，缺一不可：

1. **产物目录**（中间形态）：`document.json` + 按页拆分的 `pages/*.json` + `media/` 图片 + `fonts/` 内嵌字体。字段权威见 [references/schema.json](references/schema.json)（由 `json2pdf schema --json` 生成）；
2. **json2pdf CLI**（执行层）：`unpack` 解包、`view` 查看、`edit` 精准修改、`repack` 重建、`render` 渲染核对、`batch/serve/mcp` 批量与常驻，及内容提取/页操作/qa 质检等工具命令。

## 产物目录结构（核心概念）

```text
doc/                            # 产物根
├── document.json               # 顶层：version/meta/page_size + pages/fonts 文件路径数组
├── pages/
│   ├── page-001.json           # 每页一个 JSON（尺寸/旋转/裁剪/元素列表）
│   └── page-002.json
├── media/                      # 图片实体（image.src 指向这里的相对路径）
└── fonts/                      # 内嵌字体实体（font-NNN.ttf + font-NNN.json 元数据）
```

- `pages` 是**文件路径数组**，不含内嵌元素；一页一个文件，便于精准修改
- 图片 `src` 用产物根相对路径（`media/x.png`）或绝对路径
- 坐标单位一律 **pt**（1/72 英寸），y 向上；**文本 y 为基线**
- 元素 8 类（`type` 判别，snake_case）：`text` / `rect` / `path`（贝塞尔原样保留）/
  `polyline`（旧格式兼容）/ `shading`（轴向/径向渐变）/ `image` / `pattern_rect`
  （平铺图案）/ `group`（透明组）。unpack 对原 PDF 的复杂特性（内嵌字体、软掩膜、
  网格渐变、CMYK/JBIG2 图等）保留直通参数，repack 原样回写
- 字段、默认值与完整说明以 `json2pdf schema --json`（即
  [references/schema.json](references/schema.json)）为权威；`json2pdf schema` 输出人读指引

## 元素寻址与修改（view / edit，与其他三格式同构）

路径以 `/` 开头，每段 `类型[索引]`，索引从 1 起按**该类型**计数，group 可下钻：

```text
/page[1]                      # 页（get/set 页级属性 width/height/rotation/crop）
/page[1]/text[2]              # 第 2 个文本元素
/page[1]/group[1]/text[1]     # 组内元素
/page                         # 文档容器（edit add --type page 追加整页）
```

```bash
json2pdf view doc/ '/page[1]' text          # 文本+媒体（每项带路径，可直接 edit）
json2pdf view doc/ '/page[1]' layout        # 元素布局树（类型/几何/文字/路径）
json2pdf edit doc/ '/page[1]/text[2]' get
json2pdf edit doc/ '/page[1]/text[2]' set --prop 'text=新内容' --prop 'color=#cc0000'
json2pdf edit doc/ '/page[1]' add --type rect --prop 'x=100' --prop 'y=600' --prop 'w=150' --prop 'h=30' --prop 'fill=#3366cc'
json2pdf edit doc/ '/page' add --type page  # 追加一页
json2pdf edit doc/ '/page[2]' remove        # 删除整页
json2pdf edit in.pdf '/page[1]/text[1]' set --prop text=新 -o out.pdf   # 改二进制 PDF 必须 -o
json2pdf help text                          # 属性速查（--json 机器可读）
```

- 输入是产物目录 → 原地修改；输入是 PDF → 写操作必须 `-o` 输出新文件；
- `render` 为**近似还原**：页面按打印态（crop 优先）绘制，坐标/几何/颜色/裁剪精确；
  字体用 CSS 近似（宽度与内嵌字体有差异）、Tr 7 裁剪文本按"透过字形填充"配对渲染、
  渐变近似 CSS gradient（mesh/func0 直通渐变无色标时跳过或有 bbox 画兜底灰）、
  pattern_rect 画占位底色；**JPX/JBIG2/CCITT 图片已可预览**（unpack 解出
  `preview` PNG，解码器为纯 Rust：openjp2/hayro-jbig2/hayro-ccitt，逐像素对照
  pymupdf 验证一致）；
- 元素类型 8 种：`text / rect / pattern_rect / path / polyline / shading / image / group`；
  交互 `add` 支持 `text/rect/polyline/image/group/page`，复杂结构（path/shading 等）
  用 batch 整元素 props 回放或直接改分片 JSON；
- **寻址为纯索引制**（模型无 name/id 元数据）：删除/插入元素后，其后同类型元素的索引会漂移，
  多步编辑前先 `view` 刷新索引；
- `validate doc/` 校验颜色/引用/越界/旋转（error 级退出码 3）；`render doc/ -o preview.html`
  自包含 HTML 目视核对（`-o *.png` / `*.pdf` 需 Chrome）。

## CLI 速查

```bash
cargo build --release          # 产物 target/release/json2pdf（monorepo workspace 内）

# 正向解析：PDF → 产物目录
json2pdf unpack input.pdf -o doc/
# 逆向生成：产物目录 → PDF（也可从零手写 JSON 目录）
json2pdf repack doc/ -o new.pdf
# 查看产物 schema
json2pdf schema            # 人读指引
json2pdf schema --json     # 机器可读 schema
```

I/O 契约与三格式一致：写盘命令默认只在 stderr 出状态行、`--json` 才向 stdout 输出
结果 JSON；`extract`/`meta get`/`qa` 等数据命令的 JSON 直接走 stdout。退出码
`0/1/2/3`——`qa`、`check font/toc` **发现问题退出码 3**（报告 JSON 仍在 stdout）。

其他常用命令（同一二进制，点分写法 `extract.text` 兼容原 pdf.py 习惯）：

| 命令 | 功能 |
|---|---|
| `extract text <pdf> -p 1,3` | 按页提取文本（自研 tokenizer + ToUnicode/Differences 解码）；输出含每页 `complexity`（no_text/scanned/garbled 等判定与重叠 run 去重）。进阶开关：`--layout` 版式重建（XY-cut 多栏阅读序）、`--format markdown`（标题/列表/段落/表格）、`--ocr` 扫描页识别（引擎链：显式 HTTP 服务 `UNIOCR_URL`/`--ocr-server-url` → 本机原生 macOS 系统 Vision → tesseract 兜底；`--ocr-language`/`--ocr-dpi` 可调） |
| `extract table <pdf>` | 表格提取（行聚类近似） |
| `extract image <pdf> -o dir/` | 导出内嵌图片 |
| `pages merge/split/rotate/crop/clean` | 合并 / 逐页拆分 / 旋转 / 裁剪 / 删空白页 |
| `meta get/set/brand` | 元数据读写 / 批量品牌元数据 |
| `form info/fill` | AcroForm 字段分析与填写 |
| `convert office [--engine auto\|soffice\|native]` | Office → PDF：本机 LibreOffice 探测到即用（保真）；未装 → native 自研简化排版 |
| `convert docx/pptx/xlsx <pdf> -o out` | PDF → Office（产物目录为中间形态的架构内映射，不依赖 LibreOffice）：docx 标题/段落/列表/表格/图片语义重建、pptx 页→slide 绝对定位近无损、xlsx 表格逐张成 sheet |
| `palette generate/cascade` | HSL 调色板（intent/mode/harmony + WCAG，`--seed` 可复现） |
| `design svg/layout/derive/audit` | 算法化 SVG 背景 / 布局 / 关键词派生 / 调色板审计 |
| `code sanitize` / `content sanitize` | 生成脚本清洗 / 文本内容清洗（不可见字符等） |
| `check font/toc <pdf>` | 缺字形 / 目录页码校验（问题 → 退出码 3） |
| `qa <pdf> [--skip-cover]` | 成品体检（元数据/页尺寸/空白页/CJK 禁则/越界/边距） |
| `env check` | 探测 soffice / chrome / tectonic |
| `dump` / `batch` / `serve` / `mcp` | 可回放指令（页面 id 幂等）/ 批量回放（空目录可引导）/ 常驻编辑 / MCP |

## 工作流程

### A. 解析并编辑已有 PDF

```text
unpack → view 定位 → edit 修改 → render/validate 核对 → repack 回 PDF → extract/qa 终检
```

1. `json2pdf unpack in.pdf -o doc/`；
2. `json2pdf view doc/ '/page[N]' text` 定位要改的元素（每项自带 CLI 路径）；
3. `json2pdf edit doc/ <路径> set --prop k=v` 修改（或 add/remove）；
4. `json2pdf render doc/ -o preview.html` 目视核对 + `validate doc/`；
5. `json2pdf repack doc/ -o out.pdf`（原 PDF 的内嵌字体经 `fonts/` 原样复用，
   未改动的文本字形与字宽和原文一致）；
6. `json2pdf extract text out.pdf` 或 `qa out.pdf` 终检。

### B. 从零生成 PDF

1. 手写产物目录：`document.json` + 每页一个 `pages/page-NNN.json`（图片放 `media/`）；
2. `json2pdf repack doc/ -o out.pdf`；
3. `render`/`extract`/`qa` 验证后交付。

### C. 指令化构建（dump → batch）

`json2pdf dump doc/` 导出整文档为可回放指令（每页一条 `add page`，props 含全部元素，
页面 id 1..N）。回放目标可以是**空目录**（自动引导创建 document.json）：

```bash
json2pdf dump doc/ > cmds.json
json2pdf batch fresh-dir/ --input-file cmds.json     # 从零重建
json2pdf batch fresh-dir/ --input-file cmds.json     # 再跑一遍：按页面 id 幂等替换，结果不变
```

对同一文档 dump→batch→dump 语义一致（保真闭环，契约测试覆盖）。

设计辅助（海报/封面类）：`design derive "标题"` 得 intent → `palette cascade`
得 12 角色色板（`--format reportlab/css` 直接可用）→ `design layout` 算版式 →
写产物目录 → repack → `qa`。

### D. PDF ↔ Office 互转（产物目录为中间形态，不依赖 LibreOffice）

```bash
json2pdf convert docx report.pdf -o report.docx    # 结构推断：标题/段落/列表/表格/图片
json2pdf convert pptx slides.pdf -o slides.pptx    # 页→slide 绝对定位近无损
json2pdf convert xlsx tables.pdf  -o tables.xlsx   # 识别表格逐张成 sheet（数字转数值）
json2pdf convert office report.docx -o out.pdf     # 引擎链 auto：探测到本机 LibreOffice 即用
json2pdf convert office report.docx -o out.pdf --engine native   # 未装 LO 的自研渲染
```

1. PDF→Office 是**语义重建**（可继续编辑的正文流，非排版克隆）：转出后用
   对端 CLI（`json2docx view/edit/repack`）继续精修；
2. 复杂视觉（旋转文本/裁剪/渐变）首版简化；打印级保真走 soffice 引擎；
3. `--engine soffice` 显式要求本机 LibreOffice，未装则报错（不静默降级）；
   native 的 docx 是"可读兜底"（等宽栅格表/估算折行），pptx/xlsx 保真度更高。

### E. 扫描件 OCR 提取

```bash
json2pdf extract text scan.pdf --ocr                       # 引擎链自动：HTTP 服务→系统 Vision→tesseract
json2pdf extract text scan.pdf --ocr --format markdown     # OCR 结果也可输出 markdown
json2pdf extract text scan.pdf --ocr --ocr-language chi_sim --ocr-dpi 300
```

`--json` 每页含 `complexity`（no_text/scanned/garbled 判定）与 `ocr.engine`
（实际使用的引擎），引擎链顺序：显式 HTTP 服务（`UNIOCR_URL`/`--ocr-server-url`，
liteparse 规范）→ 本机原生（macOS 系统自带 Vision，免安装）→ tesseract 兜底；
`needs_ocr` 为否的页不走 OCR。

## 字体规则（重要）

- **纯 ASCII 文本**：font 用标准 14（`Helvetica`、`Helvetica-Bold`、`Times-Roman`、
  `Courier`、…），零嵌入、体积最小；
- **含中文/非 ASCII**：font 写 `system:<family>`（如 `system:Arial Unicode`、
  `system:微软雅黑`），repack 会查找系统 TrueType 字体**整档嵌入**（CIDFontType2 +
  Identity-H + ToUnicode + 仅已用字形的 W 表），生成的 PDF 在任何查看器中可正确
  显示与复制文本；CFF/OTF（如 PingFang）不能以 FontFile2 嵌入，自动跳过换候选；
- **unpack 既有 PDF** 时字体会作为 `fonts/` 资源保留（`text.font_id` 引用），
  repack 优先用原字体，字形与字宽与原文一致。

## 保真度边界（unpack 的取舍）

- 文本：位置/字号/颜色/旋转/字距保留；同一基线连续 run 合并（便于编辑）；
  语义保真优先，重排后像素位置是近似；
- 图片：JPEG/PNG 原样或重建落盘 `media/`；CMYK/JBIG2/软掩膜等经直通参数原样回写；
- 路径与渐变：贝塞尔控制点、虚线、渐变色标/网格/采样函数尽量原样保留；
- 不进产物目录：表单字段、注释、书签（warnings 记录）。

## 硬性规则

1. 修改产物 JSON 前先 `unpack`，不要手工猜结构；字段以 `schema --json` 输出为准；
2. repack 前确保 `image.src` 指向的文件存在（相对路径相对产物根）；
3. 生成的 PDF 必须用 `extract text` 或 `qa` 验证后再交付；
4. 落盘 JSON 不写注释；数值/布尔不加引号；颜色用 6 位十六进制**带 `#`**
   （注意：与三格式 Office 产物目录的"不带 #"相反，这是 PDF 分册的特例）。
