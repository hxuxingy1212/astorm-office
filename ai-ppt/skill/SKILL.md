---
name: ai-ppt
description: 为 ai-ppt（json2pptx）项目生成与编辑专业 PPTX 演示文稿。工作方式：以 json2pptx 的"产物目录"为中间形态（presentation.json + 按页拆分的 ppt/slides/*.json），用 json2pptx CLI 完成从 JSON 构建 PPTX（repack）、逐页查看（view）、精准修改（edit）、渲染 PNG 预览（render）。当用户说"帮我做 PPT""生成幻灯片""写个演示文稿""把这篇文章整理成 PPT""给这个内容配个版式""修改这份 pptx 的第 N 页""渲染这一页看看效果"，或需要程序化处理 PPTX 文件（解包/编辑/重建）时使用，即使用户没有提到 JSON 或命令行也应触发。
version: 1.0.0
metadata:
  category: 办公自动化
  tags: pptx, 演示文稿, 幻灯片, presentation, json2pptx, 设计风格, CLI
compatibility: 需要 Rust 工具链（cargo）或已安装的 json2pptx；render --engine browser 需 Chrome/Edge 与已构建的 web/dist
---

# PPT 分册（json2pptx）

> 通用契约、工作流、CLI 速查、edit 语义与通用约束见统一总纲
> [../../skill/SKILL.md](../../skill/SKILL.md)。本册只含 **PPT 格式特有**内容：
> 产物目录结构、22 种元素、动画/过渡、设计原则与风格选择。

## 产物目录结构（核心概念）

AI 生成或 CLI 解包得到的都是同一个结构——**产物目录**：

```text
deck/                            # 产物根
├── presentation.json            # 顶层：width/height/meta/theme/template + slides 文件路径数组
└── ppt/
    ├── slides/
    │   ├── slide1.json          # 每张幻灯片一个 JSON（背景/过渡/元素），与单页 Slide 结构一致
    │   └── slide2.json
    └── media/                   # 媒体实体文件（图片 src 指向这里，URL 亦可）
```

- `presentation.json` 的 `slides` 字段是**文件路径数组**：`["ppt/slides/slide1.json", …]`，不含内嵌元素
- 图片 `src` 用产物根下的相对路径（`ppt/media/x.png` 或自行放置的 `assets/x.png`）或 URL；`repack` 会把本地路径解析为绝对路径打包
- `view` / `edit` / `render` 的输入可以是**产物目录**（直接读写，不留残留），也可以是 **.pptx**（一次性解包到临时目录，命令结束后自动删除）

> 默认一次生成 **8–12 页**；用户要求简短时 5–6 页，详细汇报可到 15 页。避免任何一页文字溢出或拥挤。

## PPT 特有路径语法（view / edit 共用）

以 `/` 开头，每段 `类型[索引]`，索引从 **1** 开始，按类型独立计数，中间节点只能是 `group`：

```text
/slide[1]                        # 第一张幻灯片
/slide[2]/text[3]                # 第二页第 3 个文本
/slide[1]/shape[@name=Logo]      # 按元素名称定位（选择窗格名称）
/slide[1]/group[1]/image[2]      # 第一页第 1 个分组内的第 2 张图片
/slide[1]/text[@id=5]            # 按稳定 ID 定位（add 回显 id，插删不漂移）
/slide[1]/text                   # 省略索引 = 该类型第 1 个
```

## PPT 特有命令与工作流

```bash
json2pptx query deck.pptx 'slide[1] shape[name=Logo]'   # 类 CSS 选择器筛选元素（ppt 特有）
json2pptx render deck.pptx /slide[1] -o p1.png          # PNG 预览（browser / native 双引擎）
json2pptx help shape --json                             # 能力速查（与实现同源）
json2pptx merge deck/ --data '{"title":"2026"}'         # {{key}} 占位符批量填充

# 从零生成一页然后微调（产物目录 + edit 原地修改，不需要反复 repack）
json2pptx repack deck/ -o deck.pptx
json2pptx edit deck/ /slide[1]/text[1] set --prop text=新标题
json2pptx edit deck/ /slide[1] add --type text --prop text=补充 --prop x=1 --prop y=2 --prop w=8 --prop h=1.5

# 修改他人 PPTX
json2pptx unpack their.pptx -o prod/
json2pptx view prod/ /slide[1] layout                   # 先摸清结构
json2pptx edit prod/ /slide[2]/text[1] set --prop text=新标题 -o fixed.pptx
```

> 深度编辑（run 级超链接、表格合并单元格、媒体替换等 CLI 不覆盖的能力）：直接改产物目录里的 `slideN.json` 与 `ppt/media/*` 后 `repack`。完整命令参考见 [references/cli.md](references/cli.md)。

---

## JSON 生成规范

> 通用输出纪律（不写注释、数值不带引号、图片 src 真实存在）见总纲。

### 单位

- 位置/尺寸：**英寸**。默认 16:9 页面 13.333 × 7.5 英寸
- 字号：**磅（pt）**；旋转角：**度**；透明度：0.0–1.0
- 颜色：纯色填充用 6 位十六进制**不带 `#`**（`"4472C4"`）；幻灯片背景用 7 位**带 `#`**（`"#1a1a2e"`）

### presentation.json（顶层）

```json
{
  "width": 13.333,
  "height": 7.5,
  "meta": { "title": "…", "author": "…" },
  "theme": { "colors": { "accent1": "4472C4", "dk1": "333333", "lt1": "FFFFFF", "accent2": "5B9BD5", "accent3": "ED7D31", "accent4": "BDD7EE", "accent5": "70AD47", "dk2": "1A1A1A", "lt2": "F2F2F2" },
             "major_font": "Calibri Light", "minor_font": "Calibri" },
  "template": null,
  "slides": ["ppt/slides/slide1.json", "ppt/slides/slide2.json"]
}
```

- `width`/`height`/`meta`/`theme` 均可省略；省略 `theme` 时从所选 `style`/`template` 派生配色与字体
- `meta` 可选字段：`title`、`author`、`company`、`application`、`last_modified_by`（后三者解析自 `docProps`，生成时回写）；`table_styles`（解析时原样保留的 `ppt/tableStyles.xml`，一般无需手写）
- `template` 用内置模板名（`modern`/`dark`/`corporate`/`creative`/`nature`/`minimal`，`templates` 命令可查）
- **`theme.colors` 至少给出 `accent1`、`dk1`、`lt1`**，并与所选风格一致
- `slides` 为产物内相对路径数组，按序对应页序

### 单页结构（slideN.json）

```json
{
  "background": "#1a1a2e",
  "transition": { "type": "fade", "speed": "med", "advance_on_click": true, "advance_after": 3000 },
  "elements": [ … ]
}
```

- `background`：`"#RRGGBB"` 纯色 / `{"type":"gradient","stops":[{"color":"…","position":N}],"angle":135}` 渐变 / `{"type":"image","src":"…"}` 背景图
- `transition` 可选；`notes` 可选（演讲者备注字符串，`\n` 换行）

### 元素类型（22 种）

| 类型 | 说明 |
|------|------|
| `text` | 文本（字符串或段落数组） |
| `shape` | 形状（86 种 preset，`shape_type` 指定） |
| `line` | 线条（`points` 多点/`arrow`/`dash`/`smooth`） |
| `image` | 图片（本地路径或 URL，可 `crop`；可承载视频/音频 `media`） |
| `icon` | 内置 SVG 图标（`icon` 名，如 check/circle/star） |
| `formula` | LaTeX 公式（`latex` 文本） |
| `table` | 表格（支持合并单元格/行高/单元格边距/边框） |
| `chart` | 真实图表（解析自 `ppt/charts/*.xml`，保留原始 XML） |
| `comment` | 批注 |
| `equation` | Office 公式（原始 OMML） |
| `opaque` | 不透明富对象（OLE/SmartArt/3D/zoom，原始 XML+部件透传） |
| `group` | 分组（唯一可嵌套容器） |
| `progress_bar` / `progress_ring` | 进度条 / 进度环 |
| `bar_chart` / `line_chart` / `pie_chart` / `ring_chart` | 四类图表组件（用形状拼出） |
| `kpi_card` | KPI 指标卡 |
| `rating_stars` | 星级评分 |
| `timeline` | 时间轴 |
| `process_flow` | 流程步骤条 |

> 精确字段（必填/可选、默认值、动画与过渡类型）以 [references/schema.json](references/schema.json) 为准；下面给出最常用子集的写法，未列出的字段按直觉补全时先查 schema。

### 1. Text — 基础元素

```json
{
  "type": "text",
  "text": "Hello World",
  "position": { "x": 1, "y": 1, "w": 10, "h": 2 },
  "font_size": 48,
  "bold": true,
  "color": "FFFFFF",
  "align": "center",
  "vert_align": "middle",
  "wrap": false,
  "line": { "color": "000000", "width": 1.5 },
  "fill": "4472C4",
  "shadow": { "blur": 6, "distance": 3, "angle": 45, "opacity": 0.5, "color": "000000" }
}
```

`text` 可以是字符串（`\n` 换行），也可以是段落数组以支持混合样式：

```json
{
  "text": [
    { "runs": [ {"text": "Bold ", "bold": true, "color": "FF0000"}, {"text": "Italic", "italic": true} ], "align": "center", "bullet": true },
    { "text": "Plain paragraph", "align": "left" }
  ]
}
```

段落字段：`runs`、`text`、`font_size`、`bold`、`italic`、`color`、`align`、`bullet`、`line_spacing`
run 字段：`text`、`font_size`、`bold`、`italic`、`underline`、`color`、`font_family`、`hyperlink`、`highlight`（高亮底色）、`underline_color`、`spacing`（字距，磅）、`lang`（如 zh-CN）、`rtl`

> 元素级还有 `autofit`（`{kind:"normal"/"shape"/"none", font_scale?, ln_spc_reduction?}`，解析时来自 `a:normAutofit`）与 `placeholder`（`"type|idx"`，解析时来自 `p:ph`）。行距 `line_spacing`：正值为倍数，负值为固定磅数（编码自 `a:spcPts`）。

### 2. Shape

```json
{
  "type": "shape",
  "shape_type": "roundRect",
  "position": { "x": 1, "y": 1, "w": 4, "h": 3 },
  "fill": "ED7D31",
  "rotation": 30,
  "adjust": { "adj": 15000 },
  "text": "inner text",
  "color": "FFFFFF"
}
```

- `fill`（untagged）：纯色字符串 `"ED7D31"` / 渐变 `{"stops":[{"color","position"}],"angle":90}` / 图案 `{"pattern":"pct20","fg":"FF0000","bg":"FFFFFF"}` / 图片 `{"src":"ppt/media/x.png"}`；`no_fill: true` 只留边框
- `effects`（可选）：`{"glow":{"color","radius","opacity"},"reflection":true,"soft_edge":N,"blur":N,"inner_shadow":{…},"fill_overlay":{"color","opacity"}}`
- 常用 `shape_type`：`rect`/`roundRect`/`ellipse`/`triangle`/`diamond`/`pentagon`/`hexagon`/`octagon`/`star5`/`star6`/`rightArrow`/`leftArrow`/`upArrow`/`downArrow`/`chevron`/`donut`/`blockArc`/`line`/`cloud`/`heart`/`lightningBolt`/`callout1`/`callout2`

### 3. Line / Icon / Formula — 轻元素

```json
{ "type": "line", "points": [1, 3, 5, 3, 7, 3], "color": "4472C4", "width": 2, "dash": "dashed", "arrow_end": "triangle" },
{ "type": "icon", "icon": "chart", "position": { "x": 1, "y": 1, "w": 1, "h": 1 }, "color": "ED7D31" },
{ "type": "formula", "latex": "E = mc^2", "position": { "x": 1, "y": 1, "w": 5, "h": 1 }, "font_size": 24 }
```

### 4. Image

```json
{ "type": "image", "src": "./assets/logo.png", "position": { "x": 0.5, "y": 0.5, "w": 5, "h": 4 },
  "crop": { "left": 10, "top": 20, "right": 30, "bottom": 40 }, "hyperlink": "https://example.com",
  "brightness": 0.2, "contrast": -0.1, "fill_mode": "stretch", "tooltip": "提示",
  "media": { "kind": "video", "src": "ppt/media/clip.mp4" } }
```

`src` 支持产物根下的相对路径与 HTTP/HTTPS URL。本地图片需真实存在于产物目录内（放在 `deck/assets/` 或 `deck/ppt/media/` 下均可，`repack` 会按相对产物根解析）。可选：`brightness`/`contrast`（-1.0~1.0）、`fill_mode`（`stretch`/`tile`）、`tooltip`（悬浮提示）、`media`（视频/音频，图片作为封面）。

### 5. Table

```json
{
  "type": "table",
  "position": { "x": 0.5, "y": 0.5, "w": 9, "h": 3 },
  "header_row": true,
  "rows": [
    [ { "text": "Name", "bold": true, "fill": "4472C4", "color": "FFFFFF" }, { "text": "Value", "bold": true, "fill": "4472C4", "color": "FFFFFF" } ],
    [ { "text": "Alpha" }, { "text": "42", "align": "right", "bold": true } ]
  ],
  "font_size": 12
}
```

单元格字段：`text`（必填）、`font_size`、`bold`、`color`、`fill`、`align`、`colspan`、`rowspan`、`border`（`{"color","width"}` 四边同色同宽）、`vert`（`vert` 竖排）、`margin`（`[left,top,right,bottom]` 英寸）、`anchor`（`t`/`ctr`/`b` 垂直对齐）、`fill_image`（单元格图片填充路径）

表格级字段：`header_row`、`font_size`、`column_widths`、`style_id`（内置表格样式 GUID）、`rtl`、`row_heights`、`tbl_attrs`（原始 `a:tblPr` 属性，解析保留）

### 6. Group — 唯一容器

```json
{
  "type": "group",
  "position": { "x": 0.5, "y": 0.5, "w": 9, "h": 4 },
  "rotation": 10,
  "children": [
    { "type": "text", "text": "Grouped", "position": { "x": 0, "y": 0, "w": 5, "h": 1 } }
  ]
}
```

子元素坐标为**组内相对坐标**。

### 7. 图表与组件

**barChart / lineChart / pieChart / ringChart** 用数据驱动：

```json
{ "type": "bar_chart", "position": { "x": 2.5, "y": 1.2, "w": 4, "h": 1.5 },
  "data": [30, 50, 80, 40], "labels": ["Q1", "Q2", "Q3", "Q4"],
  "colors": ["FF0000", "00FF00", "0000FF", "FFAA00"], "max": 100,
  "show_values": true, "axis": true }
```

**progressBar / progressRing**（进度）+ **kpiCard**（指标卡）+ **ratingStars**（评分）+ **timeline**（时间轴）+ **processFlow**（流程条）示例如下：

```json
{ "type": "progress_bar", "position": { "x": 0.5, "y": 0.3, "w": 9, "h": 0.6 }, "value": 75,
  "color": "4472C4", "track_color": "E0E0E0", "rounded": true, "show_label": true, "label": "Progress" },

{ "type": "kpi_card", "position": { "x": 7, "y": 1.2, "w": 2.5, "h": 1.5 },
  "value": "98.5%", "label": "Satisfaction", "delta": "+5.2%", "bg": "FFFFFF", "accent": "00AA00" },

{ "type": "rating_stars", "position": { "x": 0.5, "y": 3, "w": 3, "h": 0.6 }, "rating": 4.5, "max": 5, "color": "FFAA00" },

{ "type": "timeline", "position": { "x": 0.5, "y": 3.8, "w": 9, "h": 1 },
  "items": [ { "date": "2024-Q1", "title": "Phase 1", "desc": "Research" }, { "date": "2024-Q2", "title": "Phase 2", "desc": "Development" } ] },

{ "type": "process_flow", "position": { "x": 0.5, "y": 5, "w": 9, "h": 0.6 },
  "steps": ["Plan", "Do", "Check", "Act"], "colors": ["4472C4", "ED7D31", "70AD47", "FF0000"], "text_color": "FFFFFF" }
```

### 8. 动画与过渡

```json
{ "animations": [ { "type": "flyIn", "duration": 500, "delay": 200, "trigger": "afterPrevious",
                    "direction": "bottom", "order": 0, "repeat": "2", "restart": "whenNotActive", "auto_reverse": true } ] }
```

- 动画类型：入场 `appear`/`fadeIn`/`flyIn`/`wipeIn`/`zoomIn`/`bounceIn`/`floatIn`/`swivel`/`dissolveIn`/`splitIn`；退场 `fadeOut`/`flyOut`/`wipeOut`/`zoomOut`/`floatOut`/`dissolveOut`；强调 `pulse`/`spin`/`growShrink`/`colorChange`/`transparency`/`teeter`/`blink`；路径 `motionPath`
- 触发：`onClick`（默认）/ `withPrevious` / `afterPrevious`；方向：`top`/`right`/`bottom`/`left`/`topLeft`/`topRight`/`bottomLeft`/`bottomRight`
- 可选 `repeat`（次数或 `indefinite`）、`restart`（如 `whenNotActive`）、`auto_reverse`
- 过渡：`fade`/`push`/`wipe`/`split`/`cover`/`cut`/`dissolve`/`random`/`morph`；速度 `slow`/`med`/`fast`

---

## 设计原则

1. **内容优先**：根据用户主题组织文案、图表与视觉，不套模板硬塞
2. **视觉层级**：标题 36–54pt，副标题 20–28pt，正文 12–18pt，同一页字号层级不超过 4 档
3. **色彩和谐**：使用风格预设配色或互补色板，保证文字与背景足够对比度（深底白字 / 浅底深字）
4. **版面均衡**：元素均匀分布、对齐成组、留白充足；任何元素不得超出页面边界
5. **专业观感**：干净、清晰、现代；一页一个核心信息，不堆砌

## 风格选择

用户指定或暗示风格时，从 **12 套核心风格**（[references/design-styles.md](references/design-styles.md)）中选择；每套风格提供完整色板、字体搭配、版式规则与页面提示词。中文风格速查版见 `docs/设计风格指南.md`。

按受众/主题/语气映射：

| 用户场景 | 推荐风格 |
|----------|----------|
| 汇报 / 报告 / 周报 | Business Professional |
| 战略 / 咨询 / MBB | Consulting |
| 科技 / AI / 产品发布 | Tech/Neon |
| 论文 / 答辩 / 学术 | Academic |
| 创意 / 提案 / 设计 | Swiss International 或 Bauhaus |
| 简约 / 产品文档 | Minimalist Clean |
| 高端 / 品牌发布 | Dark Executive |
| 医疗 / 健康 / 生物 | Nature 系清新色板 |
| 教育 / 培训课件 | Playful/教育色板，字号加大 |

未指定风格时默认 **Business Professional**。JSON 顶层用 `"style": "<风格名>"` 引用风格；同时保证 `theme.colors` 与所选风格一致。

---

## 生成页面的套版建议

文案内容差异大，但每页骨架请按「内容块」规划：

- **封面**：大标题 40–54pt + 副标题/作者 + 底部装饰条
- **目录页**：编号 + 章节名列表（可加数字色块）
- **章节页**：大号章节号 + 标题 + 提示性副文
- **正文页**：左侧标题栏 + 内容区；数据优先用 `table`/图表，要点用项目符号，逻辑用 `process_flow`/`timeline`
- **结尾页**：致谢/联系方式 + 品牌色背景

多页时保持同一风格、字号体系与元素间距；相同功能元素（如页脚、页码）各页位置一致。

---

## 约束（PPT 特有）

1. 每个元素必须有 `position`（`x`/`y`/`w`/`h`）
2. 用风格预设的配色，不发明自定义色板；元素颜色尽量取自该页主题色系
3. 幻灯片背景色用 7 位带 `#`（`"#1a1a2e"`），与元素填充色（不带 `#`）不同——见「单位」

## 参考资源（按需加载，先查这里再问）

| 文件 | 内容 | 何时读 |
|------|------|--------|
| [references/schema.json](references/schema.json) | 完整 JSON Schema（草案 2020-12），字段/枚举/默认值的权威来源 | 字段不确定、元素类型/属性拿不准时 |
| [references/design-styles.md](references/design-styles.md) | 12 套设计风格完整定义（色板/字体/版式/提示词） | 选题风格或生成每页布局前 |
| [scenarios/pitch-deck.md](scenarios/pitch-deck.md) | 融资路演专项技能（10 页结构/组件选型/batch 工作流） | 需求命中 BP/路演/融资场景时优先 |
| [references/cli.md](references/cli.md) | CLI 命令全参考：参数、路径语法、产物结构、组合用法 | 执行 unpack/edit/render 等命令细节不确定时 |

中文版文档在仓库 `docs/` 目录：`JSON格式规范.md`、`设计风格指南.md`、`命令行工具.md`、`元素与组件参考.md`、`模板系统.md`。
