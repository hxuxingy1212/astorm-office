# JSON 格式规范

本文档定义 `json2pptx` 的完整 JSON 输入格式：顶层结构、主题、幻灯片、元素、动画、过渡与高级组件，以及单位、颜色等约定。

> 逐字段的 OOXML 映射细节见 [OOXML映射参考](OOXML映射参考.md)；元素与组件的详细参数见 [元素与组件参考](元素与组件参考.md)；动画与过渡详解见 [动画与过渡](动画与过渡.md)。机器可读的完整定义见 `skill/references/schema.json`（JSON Schema 草案 2020-12）。

---

## 1. 顶层结构（Presentation）

```json
{
  "width": 13.333,
  "height": 7.5,
  "meta": { "title": "演示标题", "author": "作者" },
  "template": "modern",
  "theme": {
    "colors": { "accent1": "4472C4", "dk1": "000000", "lt1": "FFFFFF" },
    "major_font": "Calibri Light",
    "minor_font": "Calibri"
  },
  "slides": [ ... ]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `width` | 数字 | 否 | 幻灯片宽度（英寸），默认 13.333（16:9） |
| `height` | 数字 | 否 | 幻灯片高度（英寸），默认 7.5（16:9） |
| `meta` | 对象 | 否 | 文档元信息：`title`、`author`、`company`、`application`、`last_modified_by`，写入 PPTX 文件属性（解析时从 `docProps/core.xml` 与 `app.xml` 还原） |
| `template` | 字符串 | 否 | 内置模板预设名（modern / dark / corporate / creative / nature / minimal），生成时合并到主题中，详见 [模板系统](模板系统.md) |
| `theme` | 对象 | 否 | 自定义主题（颜色方案 + 字体），见下节 |
| `style` | 字符串 | 否 | 设计风格名（对应 [设计风格指南](设计风格指南.md) 中的 12 种风格），供 AI 生成流程使用 |
| `slides` | 数组 | 是 | 幻灯片列表，每项是一个 Slide 对象，见 [第 3 节](#3-幻灯片-slide)（允许为空数组，即 0 页演示文稿） |
| `table_styles` | 字符串 | 否 | 原始 `ppt/tableStyles.xml` 内容（解析时保留内置表格样式，一般无需手写） |

### 1.1 主题（Theme）

```json
{
  "theme": {
    "colors": {
      "dk1": "000000", "lt1": "FFFFFF",
      "dk2": "44546A", "lt2": "E7E6E6",
      "accent1": "4472C4", "accent2": "ED7D31", "accent3": "A5A5A5",
      "accent4": "FFC000", "accent5": "5B9BD5", "accent6": "70AD47",
      "hlink": "0563C1", "folHlink": "954F72"
    },
    "major_font": "Calibri Light",
    "minor_font": "Calibri"
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `colors` | 对象 | 12 个色槽：`dk1` / `lt1` / `dk2` / `lt2`（深色 1、浅色 1、深色 2、浅色 2）、`accent1` ~ `accent6`（强调色 1～6）、`hlink`（超链接）、`folHlink`（已访问超链接） |
| `major_font` | 字符串 | 标题字体（主题的 majorFont） |
| `minor_font` | 字符串 | 正文字体（主题的 minorFont） |

**主题色引用规则**：元素颜色字段（如 `color`、`fill`）除了直接写 6 位 hex，还可以使用上面的主题色别名（如 `"accent1"`、`"dk1"`），它们会自动映射为 PPTX 的 `<a:schemeClr>`。

---

## 2. 单位与颜色约定

### 单位

| 量 | JSON 单位 | 说明 |
|----|----------|------|
| 位置 / 尺寸（`position`、`width`、`height`） | 英寸 | 默认画布 13.333 × 7.5 英寸 |
| 字号（`font_size`） | 点（pt） | 如 `48` 表示 48 磅 |
| 旋转 / 渐变角度（`rotation`、`angle`） | 度 | `0–360` |
| 线宽（`line.width`） | 点（pt） | |
| 渐变停止（`stops[].pos`） | 0–100 | 百分比 |
| 透明度（`fill_alpha` / `line_alpha`） | 0.0–1.0 | `0` 全透明，`1` 不透明 |
| 阴影不透明度（`shadow.opacity`） | 0.0–1.0 | |
| 动画时长 / 延迟 / 自动翻页 | 毫秒 | `duration`、`delay`、`advance_after` |
| 裁剪（`crop`） | 0–100 | 百分比 |

### 颜色

- 6 位 hex，**省略 `#`**：`"4472C4"`、`"FFFFFF"`
- 或主题色别名：`"accent1"`、`"dk1"`、`"lt1"` 等（见 [主题色引用规则](#11-主题theme)）

---

## 3. 幻灯片（Slide）

```json
{
  "notes": "演讲者备注\n第二行",
  "background": "#1a1a2e",
  "transition": { "type": "fade", "speed": "med" },
  "elements": [ ... ]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `notes` | 字符串 | 否 | 演讲者备注（纯文本，`\n` 多行），生成 notesSlide 部件，PowerPoint 备注窗格可见 |
| `background` | 字符串 / 对象 | 否 | 幻灯片背景，支持三种写法（见下） |
| `transition` | 对象 | 否 | 页面切换过渡，见 [第 6 节](#6-过渡transition) |
| `elements` | 数组 | 是 | 元素列表，见 [第 4 节](#4-元素类型) |

### 背景的三种写法

```json
/* 1. 纯色背景 */
"background": "#1a1a2e"

/* 2. 渐变背景 */
"background": {
  "type": "gradient",
  "stops": [
    { "pos": 0, "color": "1A365D" },
    { "pos": 100, "color": "0B1D3A" }
  ],
  "angle": 135
}

/* 3. 图片背景（本地路径或 HTTP(S) URL） */
"background": {
  "type": "image",
  "src": "./media/bg.png"
}
```

> 图片背景的图片会被嵌入 PPTX 的 `ppt/media/`；解析时图片背景还原为 `{"type":"image","src":"..."}` 格式。

---

## 4. 元素类型

`elements` 数组中每个元素通过 `type` 字段区分类型，共 **12 种基础元素 + 10 种高级组件**：

| `type` | 说明 | 详细参考 |
|--------|------|---------|
| `text` | 文本框（多段落、多 run、项目符号、超链接、高亮/字距/RTL、CJK） | [元素与组件参考](元素与组件参考.md) |
| `shape` | 形状（86 种预设类型；图案/图片填充、发光/倒影等效果、自适应） | 同上 |
| `line` | 线条（直线/折线/曲线，箭头端点、虚线） | 同上 |
| `image` | 图片（本地 / URL，裁剪、超链接、亮度对比度、填充模式、视频/音频 `media`） | 同上 |
| `table` | 表格（行列、单元格样式、合并、列宽、行高、边距、边框、图片填充） | 同上 |
| `group` | 分组（嵌套元素，组级旋转） | 同上 |
| `chart` | 真实图表（解析自 `ppt/charts/*.xml`，`raw` 保留原始部件 XML） | 同上（解析） |
| `comment` | 批注（`author`/`text`/位置） | 同上（解析） |
| `equation` | Office 公式（`omml` 原始 OMML） | 同上（解析） |
| `opaque` | 不透明富对象：OLE / SmartArt / 3D / zoom（原始 XML + 关系 + 部件透传） | 同上（解析） |
| `icon` | 内置矢量图标 | 同上 |
| `formula` | LaTeX 公式（文本化降级） | 同上 |
| `progress_bar` | 水平进度条 | 同上（高级组件） |
| `progress_ring` | 环形进度指示器 | 同上 |
| `bar_chart` | 柱状图 | 同上 |
| `line_chart` | 折线图 | 同上 |
| `pie_chart` | 饼图 | 同上 |
| `ring_chart` | 环形图 | 同上 |
| `kpi_card` | KPI 卡片 | 同上 |
| `rating_stars` | 星级评分 | 同上 |
| `timeline` | 水平时间线 | 同上 |
| `process_flow` | 流程步骤 | 同上 |

**通用字段**：所有元素都包含 `type`、`position`（`{x, y, w, h}`，英寸）；大部分元素支持 `name`（元素名）和 `animations`（动画数组）。

### 元素快速示例

```json
/* 文本：支持多段落、多 run、\n 换行、项目符号、run 级超链接 */
{
  "type": "text",
  "text": "你好" | [ { "runs": [...] }, ... ],
  "position": { "x": 1, "y": 1, "w": 10, "h": 2 },
  "font_size": 48, "bold": true, "color": "FFFFFF",
  "align": "center", "vert_align": "middle",
  "font_family": "Noto Sans SC"
}
/* run 内可写 "hyperlink": "https://example.com" 添加文本超链接 */

/* 形状：86 种预设类型，支持 fill/line/shadow/rotation/adjust */
{
  "type": "shape",
  "shape_type": "roundRect",
  "fill": "ED7D31" | { "stops": [ {"color":"1A365D","position":0.0}, {"color":"0B1D3A","position":1.0} ], "angle": 135 },
  "fill_alpha": 0.3,
  "line": { "color": "FFFFFF", "width": 2 },
  "line_alpha": 0.5,
  "shadow": { "blur": 10, "distance": 5, "angle": 45, "opacity": 0.5 },
  "adjust": { "adj": 15000 },
  "text": "内部文字"
}

/* 线条：points 为相对 position 的顶点列表（英寸） */
{
  "type": "line",
  "points": [[0, 0], [5, 0.5]],
  "color": "E74C3C", "width": 2, "dash": "dashed",
  "arrow_start": "none", "arrow_end": "arrow", "smooth": false
}

/* 图片：本地路径 / HTTP(S) URL，支持裁剪与超链接 */
{
  "type": "image",
  "src": "./assets/logo.png",
  "crop": { "left": 0, "top": 0, "right": 0, "bottom": 0 },
  "hyperlink": "https://example.com"
}

/* 表格：任意行列，单元格级样式；colspan/rowspan 合并单元格，column_widths 控制列宽 */
{
  "type": "table",
  "header_row": true,
  "column_widths": [3.0, 2.0, 4.0],
  "rows": [
    [ { "text": "A", "bold": true, "fill": "2c3e50", "color": "FFFFFF" } ],
    [ { "text": "B", "font_size": 12 } ]
  ]
}

/* 分组：嵌套元素，组级旋转 */
{
  "type": "group",
  "rotation": 0,
  "children": [ { "type": "text", ... }, { "type": "shape", ... } ]
}

/* 形状效果（effects）与自适应（autofit，解析保留自 p:ph/normAutofit） */
{
  "type": "shape", "shape_type": "roundRect",
  "fill": { "pattern": "pct20", "fg": "FF0000", "bg": "FFFFFF" },
  "effects": {
    "glow": { "color": "00FF00", "radius": 6, "opacity": 0.8 },
    "reflection": true, "soft_edge": 3, "blur": 2,
    "inner_shadow": { "blur": 4, "distance": 2, "angle": 90, "opacity": 0.3, "color": "000000" },
    "fill_overlay": { "color": "0000FF", "opacity": 0.4 }
  },
  "autofit": { "kind": "normal", "font_scale": 0.75 }
}

/* 图片增强 + 媒体 */
{
  "type": "image", "src": "ppt/media/poster.png",
  "brightness": 0.2, "contrast": -0.1, "fill_mode": "tile", "tooltip": "提示",
  "media": { "kind": "video", "src": "ppt/media/clip.mp4" }
}

/* 表格：单元格边框/边距/竖排/图片填充 */
{
  "type": "table", "style_id": "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}", "rtl": true,
  "row_heights": [0.4, 0.5],
  "rows": [[ {
    "text": "A", "fill": "2C3E50", "color": "FFFFFF", "colspan": 2,
    "border": { "color": "FF0000", "width": 2 }, "margin": [0.1, 0.05, 0.1, 0.05],
    "anchor": "ctr", "vert": "vert", "fill_image": "ppt/media/x.png"
  } ]]
}

/* 真实图表（解析产出；也可手写数据由生成端构造） */
{
  "type": "chart", "chart_type": "line", "title": "营收",
  "categories": ["Q1", "Q2"], "series": [ { "name": "2024", "values": [10, 20], "color": "4472C4" } ],
  "legend": true, "legend_position": "bottom"
}

/* 批注 / 公式 / 不透明富对象（解析产出） */
{ "type": "comment", "position": {"x":1,"y":1,"w":0,"h":0}, "author": "审阅者", "text": "请修改" }
{ "type": "equation", "position": {"x":2,"y":3,"w":4,"h":1}, "omml": "<a14:m>…</a14:m>", "text": "x=1" }
{ "type": "opaque", "position": {"x":1,"y":1,"w":5,"h":3}, "xml": "<p:graphicFrame>…</p:graphicFrame>" }
```

---

## 5. 动画（Animation）

动画挂载在元素的 `animations` 数组上：

```json
{
  "animations": [
    {
      "type": "flyIn",
      "duration": 500,
      "delay": 200,
      "trigger": "afterPrevious",
      "direction": "bottom",
      "order": 0
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `type` | 字符串 | 动画类型，共 24 种（见下） |
| `duration` | 毫秒 | 持续时间 |
| `delay` | 毫秒 | 触发延迟 |
| `trigger` | 字符串 | `onClick`（点击触发）/ `withPrevious`（同时）/ `afterPrevious`（之后） |
| `direction` | 字符串 | 方向：`top` `right` `bottom` `left` `topLeft` `topRight` `bottomLeft` `bottomRight` |
| `order` | 数字 | 同页动画的播放顺序 |
| `scale` | 数字 | 缩放动画目标比例（growShrink 用） |
| `degrees` | 数字 | 旋转角度（spin 用） |
| `color` | 字符串 | 目标颜色（colorChange 用） |
| `opacity` | 数字 | 目标透明度 |
| `points` | 数组 | 运动路径关键点（motionPath 用） |
| `distance` | 数字 | 运动路径距离 |

**24 种动画类型**：

| 类别 | 类型 |
|------|------|
| 入场 | `appear` `fadeIn` `flyIn` `wipeIn` `zoomIn` `bounceIn` `floatIn` `swivel` `dissolveIn` `splitIn` |
| 退场 | `fadeOut` `flyOut` `wipeOut` `zoomOut` `floatOut` `dissolveOut` |
| 强调 | `pulse` `spin` `growShrink` `colorChange` `transparency` `teeter` `blink` |
| 路径 | `motionPath` |

---

## 6. 过渡（Transition）

过渡定义在幻灯片级别的 `transition` 字段：

```json
{
  "transition": {
    "type": "fade",
    "speed": "med",
    "advance_on_click": true,
    "advance_after": 3000
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `type` | 字符串 | 过渡类型（见下） |
| `speed` | 字符串 | `slow` / `med` / `fast` |
| `advance_on_click` | 布尔 | 是否点击鼠标翻页 |
| `advance_after` | 毫秒 | 自动翻页延时（`3000` = 3 秒后自动切换） |

**过渡类型**：`fade`（淡入淡出）、`push`（推入）、`wipe`（擦除）、`split`（分裂）、`cover`（覆盖）、`cut`（直接切换）、`dissolve`（溶解）、`random`（随机）、`morph`（平滑变形，PowerPoint 2010+）。

---

## 7. 高级组件（Components）

高级组件不是 OOXML 原生概念，生成时自动**展开**为基本形状的 Group，展开后不保留组件类型信息（解析回 JSON 后变成普通 Group）。

| 组件 | 必填参数 | 可选参数 |
|------|---------|---------|
| `progress_bar` | `value`（0–100） | `color` `track_color` `rounded` `show_label` `label` `text_color` `font_size` |
| `progress_ring` | `value`（0–100） | `color` `track_color` `thickness` `show_label` `text_color` `font_size` |
| `bar_chart` | `data`（数值数组） | `labels` `colors` `max` `show_values` `axis` `label_color` `font_size` |
| `kpi_card` | `value`（字符串） | `label` `delta` `bg` `accent` `rounded` `text_color` |
| `rating_stars` | `rating` | `max`（默认 5）`color` `empty_color` |
| `timeline` | `items`（对象数组） | `line_color` `dot_color` `label_color` |
| `process_flow` | `steps`（字符串数组） | `colors` `text_color` `font_size` |

各组件的字段说明与展开行为详见 [元素与组件参考](元素与组件参考.md)。

---

## 8. 已知限制

| 限制 | 说明 |
|------|------|
| 图表组件 | `bar_chart`/`line_chart`/`pie_chart`/`ring_chart` 是**形状拼出的组件**，非 Excel 图表对象；真实图表请用 `chart` 类型 |
| 高级组件 | 生成时展开为 Group，回环解析后不保留组件类型 |
| `@id` 寻址 | 模型未保存形状 id，路径仅支持 `[序号]` 与 `[@name=]` |
| 表格内置样式底纹 | 已保留原始 `tableStyles.xml` 与 `a:tblPr` 标志，但不同渲染器对内置样式的应用存在差异 |

---



## 9. unpack 中间产物格式

`unpack` 命令把 PPTX 解包为按幻灯片拆分的 JSON 中间产物（详见 [命令行工具](命令行工具.md)）：

- `presentation.json`：顶层结构（`meta` / `theme` / `width` / `height`），`slides` 为**文件路径数组**
- `ppt/slides/slideN.json`：每张幻灯片一个文件，结构与本规范中的 `Slide` 完全一致
- `ppt/media/`：媒体文件实体，图片 `src` 统一为 `ppt/media/imageN.ext`（相对产物根）

编辑各 `slideN.json` 后可用 `repack` 重建 PPTX。

---

## 10. 编写 JSON 的建议

1. **以 `json2pptx unpack` 产物的 `slideN.json` 为起点**，在其上修改，可避免遗漏必填字段。
2. **中文文本务必设置 `font_family`**（如 `"Noto Sans SC"`、`"Microsoft YaHei"`），或使用内置模板（模板已配置中文字体），否则中文可能无法正确渲染。
3. **颜色统一使用主题色别名**，便于后续整体换肤（只需改 `theme.colors`）。
4. **需要 AI 生成演示时**，可结合 [设计风格指南](设计风格指南.md) 中的风格预设与 `skill/SKILL.md` 技能文件（含生成规范与 CLI 工作流）。
