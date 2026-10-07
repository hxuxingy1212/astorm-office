# ai-pdf（json2pdf）

astorm-office 的 PDF 工具箱 —— 复刻 ZCode 官方 pdf 插件（`pdf.py` 32 个子命令 +
`design_engine.py`）的命令面与核心算法，并新增 **PDF ↔ JSON 双向解析**（对齐
[ai-ppt](../ai-ppt) 的产物目录设计），编译为**单个静态二进制**（`json2pdf`，无
Python/Node 运行时依赖）。CLI 契约与仓库内 `json2docx` / `json2xlsx` / `json2pptx`
完全一致，见 [`docs/cli-conventions.md`](../docs/cli-conventions.md)。

## 双向解析：PDF ↔ 产物目录

**产物目录**是 unpack 与 repack 的统一中间形态（设计对齐 ai-ppt 的 json2pptx）：

```text
doc/
├── document.json        # 顶层：version/meta/page_size + pages/fonts 文件路径数组
├── pages/
│   ├── page-001.json    # 每页一个 JSON（尺寸/旋转/裁剪/元素列表）
│   └── page-002.json
├── media/               # 图片实体（image.src 指向这里的相对路径）
└── fonts/               # 内嵌字体实体 + 元数据（font-NNN.ttf / font-NNN.json）
```

元素 8 类，坐标一律 pt（y 向上，文本 y 为基线）：

```json
{ "type": "text",  "text": "你好 world", "x": 72, "y": 720, "size": 12,
  "font": "system:Arial Unicode", "color": "#23456e" }
{ "type": "rect",  "x": 50, "y": 700, "w": 200, "h": 40,
  "fill": "#ffffff", "stroke": "#000000", "line_width": 1 }
{ "type": "path", "segments": [{"op":"m","points":[[0,0]]},{"op":"c","points":[[1,1],[2,2],[3,3]]}] }
{ "type": "image", "src": "media/img-001.png", "x": 100, "y": 400, "w": 200, "h": 150 }
```

（另有 `polyline`（兼容旧格式）、`shading` 渐变、`pattern_rect` 平铺图案、
`group` 透明组；完整字段以 schema 为权威。）

```bash
json2pdf unpack input.pdf -o doc/     # 正向解析：PDF → 产物目录（状态走 stderr）
# …查看、按路径精准修改（与其他三个 CLI 同构；寻址 /page[1]/text[2] 纯索引制）…
json2pdf view doc/ '/page[1]' text
json2pdf edit doc/ '/page[1]/text[2]' set --prop 'text=新内容'
json2pdf repack doc/ -o out.pdf       # 逆向生成：产物目录 → PDF（默认静默，--json 出结果）
json2pdf render doc/ -o preview.html  # 自包含 HTML 目视核对（png/pdf 需 Chrome）
json2pdf schema                       # 人读字段指引；--json 输出机器可读 schema
```

**字体策略**（逆向生成的关键）：

- 纯 ASCII + 标准 14 字体（Helvetica/Times/Courier…）→ Type1 零嵌入；
- 含中文/非 ASCII（`font: "system:<family>"`）→ 按平台候选查找 TrueType 系统字体
  **整档嵌入**为 CIDFontType2（Identity-H + ToUnicode + 仅已用字形的 W 表）；
  ttc 集合字体自动抽取单 face（表重排 + 校验和重算）；CFF/OTF 自动跳过；
- **unpack 既有 PDF** 时原字体会作为 `fonts/` 资源保留（`text.font_id` 引用），
  repack 优先复用——未改动的文本字形与字宽和原文一致；
- 生成的 PDF 在任意查看器可正确显示与复制文本（经 ToUnicode 往返验证）。

**图片**：PNG 解码为像素流（RGBA 自动拆 SMask）、JPEG 原样 DCTDecode、
CMYK/JBIG2 等经直通参数原样回写，无法解码的占位并记 warning。

**保真度边界**：unpack 合并同行同款文本 run（语义编辑优先，非像素锁定）；
旋转 CTM 下的矩形/图片取包围盒；表单字段/注释不进产物目录。
端到端验证：真实 Chrome 生成 PDF → unpack → repack → 提取文本与原文逐字一致；
govdocs1 真实语料抽样评估见 [docs/eval-2026-10.md](docs/eval-2026-10.md)。

```
┌────────────────────────────────────────────────────────────┐
│  CLI（clap，两层分组 + 点分兼容；契约见 docs/cli-conventions.md）│
├──────────────┬──────────────┬───────────────┬───────────────┤
│ extract.*    │ pages.*      │ meta.* / form.*│ convert.*    │
│ text/table/  │ merge/split/ │ get/set/brand  │ office/html/ │
│ image        │ rotate/crop/ │ info/fill      │ latex        │
│              │ clean        │                │              │
├──────────────┴──────────────┴───────────────┴───────────────┤
│ palette.*（HSL 几何调色板 + WCAG）  design.*（SVG/布局/推导） │
│ code/content sanitize   check font/toc   qa                 │
├────────────────────────────────────────────────────────────┤
│  lopdf（对象层/压缩/字体表）+ 自研：                          │
│  内容流 tokenizer · 文本状态机（CTM 栈/双文本矩阵/笔步进）      │
│  ToUnicode CMap 解析（1/2 字节码空间）· /Differences 字形名表  │
│  部首区归一化 · PNG 重建 · 调色板几何引擎                      │
└────────────────────────────────────────────────────────────┘
```

## 构建

```bash
cargo build --release          # monorepo workspace 内，产物 target/release/json2pdf
cargo test -p json2pdf         # 97 个测试（单测 + 集成）
```

## 子命令对照（原版 pdf.py → json2pdf）

| 原版（pdf.py） | json2pdf | 实现说明 |
|---|---|---|
| `env.check` / `env.fix` | `env check` | 探测 soffice/chrome/tectonic；Rust 版无需装 Python 包，无 fix |
| `extract.text` | `extract text -p 1,3,5-8` | 自研 tokenizer + 文本状态机 + ToUnicode/Differences 解码；进阶开关见下文（`--layout` / `--format markdown` / `--ocr`） |
| `extract.table` | `extract table` | 行聚类近似（≥2 列一致行合并），非 pdfplumber 线条聚类 |
| `extract.image` | `extract image -o dir` | 按滤镜落盘 jpg/jp2/bin；FlateDecode RGB/Gray 重建 PNG |
| `pages.merge` | `pages merge a.pdf b.pdf -o out` | 对象图环安全深拷贝 + 继承属性内联 |
| `pages.split` | `pages split` | 逐页重建页树 |
| `pages.rotate` / `pages.crop` | 同名 | /Rotate 累加（90/180/270）；MediaBox+CropBox 双写 |
| `pages.clean` | `pages clean` | 空白判定同原版：无字符 && 无图 && 无路径 |
| `meta.get` / `meta.set` | 同名 | UTF-16BE(BOM) 文本串编解码；set 自动更新 ModDate |
| `meta.brand` | `meta brand` | Author/Creator=Z.ai、Producer=http://z.ai；多文件原地 |
| `form.info` / `form.fill` | 同名 | 字段树遍历（嵌套 /T 全限定名）；checkbox/radio 写 /V+/AS；置 NeedAppearances |
| `form.detail` / `form.fill-legacy` | — | pypdf 路线，未复刻（fill 已覆盖主场景） |
| `form.annotate` / `form.render` / `form.validate` / `form.check-bbox` | — | 扫描件标注工作流，需矢量注释/渲染，未复刻 |
| `convert.office` | `convert office [--engine auto\|soffice\|native]` | 引擎链：探测到本机 LibreOffice → soffice 保真；未装 → native 自研简化排版（自研：docx 流式折行/表格栅格/图片嵌入、pptx 绝对定位近无损、xlsx 值渲染），详见下文跨格式转换 |
| `convert.html` | `convert html [--css]` | Chromium --print-to-pdf（原版走 Playwright） |
| `convert.latex` | `convert latex --runs N` | tectonic -X compile，日志 Overfull/Underfull 分类 |
| `convert.blueprint` | — | Creative 蓝图→HTML→PDF 管线未复刻（原版 0.1.7 此命令本身引用缺失文件不可用） |
| `font.check` | `check font` | 字符级扫描（FFFD/控制/零宽/Bidi/PUA）；字形级 .notdef 需字体渲染，未做 |
| `toc.check` | `check toc` | 目录标题定位 + "标题…页码" 条目解析 + 单调性/同页校验 |
| `palette.generate` | `palette generate [--format json\|css\|python]` | intent/mode/harmony → HSL 几何，WCAG 4.5:1 / 3:1 兜底，逐位对齐原版 |
| `palette.cascade` | `palette cascade [--format summary\|json\|css\|reportlab]` | 12 角色 × 5 面积层级（面积∝1/饱和度铁律）+ 4 语义色 + 生成时钳制 |
| `code.sanitize` | `code sanitize` | HTML 实体/&#NNN;/\uXXXX 还原、上下标→`<super>/<sub>`、符号回退表 |
| `content.sanitize` | `content sanitize [--apply]` | 控制/零宽/Bidi/变体选择符/PUA 剔除、全角→半角、连字分解 |
| （design_engine.py）`svg` | `design svg --svg-type flow\|grid\|noise\|supergraphic\|ordered_texture` | 算法化 SVG 背景（参数面一致，随机布局非逐位复刻） |
| （design_engine.py）`layout` | `design layout --elements hero,body --style offset\|centered\|overlap` | 12% 呼吸边距 + 黄金分割加权分带 |
| （design_engine.py）`derive` | `design derive "标题"` | 80+ 中英关键词 → intent，平分时避开 neutral |
| （design_engine.py）`audit` | `design audit --palette-json` | 模式 S/L 边界 + WCAG；级联版自动识别 |
| （design_engine.py）`compile` | — | Creative 蓝图编译器（HTML/CSS 生成）未复刻 |
| （pdf_qa.py） | `qa [--skip-cover]` | 元数据三字段/页尺寸一致/空白页/CJK 标点禁则/内容越界/边距对称 |

点分形式兼容原版习惯：`json2pdf extract.text` ≡ `json2pdf extract text`。

此外新增原版没有的三个命令：`unpack`（PDF→产物目录）、`repack`（产物目录→PDF）、
`schema`（产物目录字段说明）——见上节。`check font/toc` 与 `qa` 发现问题时
**退出码 3**（报告 JSON 仍输出到 stdout），与其余三个 CLI 的校验语义一致。

## 调色板引擎（与原版逐参数对齐）

```bash
json2pdf palette generate --title "AI 数据平台年度报告" --seed 7     # 标题自动派生 intent=cold
json2pdf palette cascade --mode dark --format reportlab             # 可直接粘贴的 ReportLab 代码
json2pdf design derive "毕业论文开题报告"                            # → authority
json2pdf design audit --palette-json p.json                          # 生成后审计
```

intent（8 种 + 3 legacy 别名）→ 基础色相表、5 种 mode 的 S/L 硬边界、
5 种和谐律的 accent 几何、浑浊色相区（28°-105°）规避、WCAG 对比度强制——
常量与算法均自 `design_engine.py` 逐项移植，`--seed` 可复现。

## 文本提取的三个关键机制（真实 Chrome/Skia PDF 验证）

1. **CTM 矩阵栈**（q/Q/cm）：Chrome 页面级写入 `0.24 0 0 -0.24 0 792 cm` 翻转坐标系，
   不追 CTM 则行序颠倒、坐标全错。
2. **双文本矩阵模型**：line matrix（Tm/Td/T* 更新）与 text matrix（隐式字形步进更新，
   Td 时重置）。Skia 逐字形发 `Td <前一字宽> 0`（Type3 w0=0），ReportLab/LaTeX 依赖
   隐式步进——两类生成器都正确处理。
3. **自研 ToUnicode CMap 解析**：lopdf 只接受 `<0000> <FFFF>` 码空间，Chrome 的
   1 字节码空间（`<00> <FF>`）会被拒并回落 StandardEncoding 产生乱码。自研解析器
   支持 1/2 字节码空间、bfchar/bfrange（含数组）；另补 `/Differences` 字形名表
   （uniXXXX/AGL 名）与康熙部首/CJK 部首补充区归一化（⼆→二、⻓→长）。

## 跨格式转换：PDF ↔ Office（docx/pptx/xlsx）

四格式**产物目录是统一中间形态**（对齐 unpack/repack 的单一事实源设计），
转换全程不依赖 LibreOffice、不引入新重量依赖：

```bash
json2pdf convert docx report.pdf -o report.docx   # 结构推断：标题字号分层/段落/列表/表格/图片
json2pdf convert pptx report.pdf -o report.pptx   # 页→slide 绝对定位近无损（文本框/图片/矩形）
json2pdf convert xlsx report.pdf -o report.xlsx   # 识别表格逐张成 sheet（数字自动转数值）
json2pdf convert office report.docx -o out.pdf --engine auto   # 探测到的 soffice → native
json2pdf convert office slide.pptx -o out.pdf --engine native  # 强制自研渲染
```

**转换原则**（与 unpack 同口径）：**语义优先，非像素锁定**。PDF→Office 走
`unpack` 产物目录 + 行级数据（text.rs）→ 结构推断（`cross.rs`）→ 各格式
JSON 模型 → 各自 `generate()`：标题/列表/表格/图片按语义重建，正文流可继续
编辑；旋转文本/裁剪/渐变等复杂视觉在首版简化。Office→PDF 的 native 引擎是
"本机没有 LibreOffice 时的可读兜底"（docx 等宽栅格表/估算折行；pptx/xlsx
保真度更高），打印级保真请安装 LibreOffice 走 soffice 引擎。

```text
  Word/Excel/PPT ⇄ convert office（本机 LO 保真 ∕ native 自研）
                        ⇅
  PDF ⇄ [产物目录 + 行级数据] ⇅ cross.rs 结构推断 ⇅ [office JSON 模型]
                        ⇅
  json2docx / json2pptx / json2xlsx generate()
```

## 文本提取进阶：复杂度判定 / 版式重建 / markdown / OCR

`extract text` 对每页做**复杂度判定**并输出结构化结果（`source` 标明文本来源）：

```json
{ "page": 1, "chars": 0, "source": "native", "text": "…",
  "complexity": { "needs_ocr": true, "reasons": ["no_text", "embedded_images"],
                  "text_coverage": 0.0, "text_length": 0 } }
```

- **复杂度判定**（内建，无需开关）：提取前先做重叠文本 run 去重（水印/重复
  绘制常见）；再按页判定 `no_text`（无文本层）、`scanned`（少文本 + 大图铺页）、
  `sparse_text`、`embedded_images`、`garbled`（元音比 <20% 的坏 ToUnicode 乱码）
  五种原因——`no_text` / `scanned` / `garbled` 即 `needs_ocr`。
- `--layout`：**版式重建**输出——XY-cut 递归切分（先横切按 y 空隙 ≥1.4× 行高分块、
  再竖切按 x 密度投影谷分列），多栏/财报/论文版面按阅读顺序还原，列内以空格
  锚点对齐。
- `--format markdown`：标题层级（按字号）/ 列表 / 段落 / 表格（复用 extract
  table 的行聚类）的 markdown 输出。
- `--ocr`：`needs_ocr` 页自动送 OCR。引擎链按优先级：**显式配置的 HTTP OCR
  服务**（liteparse 规范接口，`UNIOCR_URL` 环境变量或 `--ocr-server-url`）→
  **本机原生**（macOS 系统自带 Vision，objc2 绑定、免安装、中英混排质量优于
  tesseract）→ **tesseract** 兜底（信创/离线）。`--ocr-language` 选语言
  （默认 eng），`--ocr-dpi` 控制页面渲染精度（默认 150）；OCR 文本可与
  `--format markdown` 组合输出，`--json` 每页标明实际使用的引擎。

## 与原版的已知差异

- **字形级 .notdef 检测、精确字宽/填充率/表格线条聚类**：需要字体渲染引擎与
  完整字体度量，未复刻；文本宽度用 Helvetica 风格分组估计（CJK 1.0em）。
- **form.* 的 pypdf 路线、扫描件标注工作流**：未复刻（AcroForm 读写已覆盖）。
- **convert.blueprint / design compile**：Creative 蓝图→HTML 管线未复刻；
  原版 0.1.7 的 `convert.blueprint` 因引用不存在的 `html2pdf.js` 本身不可用。
- **SVG/布局的随机成分**：算法与参数面一致，但随机源不同（xorshift64* vs
  Python random），`--seed` 下可复现但不与原版逐位相同。
- 原版 postcheck/qa 的部分规则（fill-rate、表格居中、CJK 标点禁则的完整版）
  依赖字形盒，`qa` 实现了其中坐标级可算的子集。
- **相对 Office 三格式的差异**：`raw` / `raw-set`（OPC 部件概念不适用于 PDF）、
  `templates` / `extract`（模板提取语义）与 `merge`（{{key}} 填充）未提供；
  稳定 ID 寻址不适用（PDF 模型无 name/id 元数据，寻址为纯索引制 `/page[N]/text[K]`）。
- **office-web 预览组件支持 PDF**（`PdfViewer`，与 `render html` 同一近似还原算法；
  web 端静态查看也可用 `render html` 的自包含预览）。
- **渲染保真度**（govdocs1 语料 100 份抽样 vs pymupdf 参照，`scripts/pdf_render_eval.py` /
  `scripts/pdf_web_eval.py`）：坐标/几何/颜色/裁剪/打印态页面框（crop）精确；已知近似：
  字体为 CSS 回退（度量有差异）、Tr 7 裁剪文本按"透过字形填充"配对、mesh/func0 直通
  渐变不还原（有 bbox 画兜底灰、无 bbox 跳过）、pattern_rect 占位、JP2/JBIG2 图无法
  内嵌预览。中位差异分 3.7（web 1.7），残余高分集中在上述边界。
- **图片编解码**：字节保真（unpack→repack 原样回写）覆盖全部滤镜；像素解码覆盖
  Flate/DCT/LZW/RunLength（自研）+ **JPX/JBIG2/CCITT**（纯 Rust 解码器 openjp2 /
  hayro-jbig2 / hayro-ccitt，静态链接无 C 依赖）——unpack 时生成 `preview` PNG
  供 render 与 web 预览，与 pymupdf 逐像素一致（测试见 `tests/codec_preview.rs`）。

## License

MIT
