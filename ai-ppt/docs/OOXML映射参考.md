# OOXML 映射参考

本文档详细说明 `json2pptx` 的每个 JSON 字段对应 PPTX 文件（OOXML）的哪一部分，帮助你理解"这个字段最终变成 PPTX 里的什么"。

> **PPTX 本质**：`.pptx` 文件实际上是一个 ZIP 压缩包，里面是一堆 XML 文件。`json2pptx` 的工作就是把 JSON 转换成这些 XML 文件，然后打包成 `.pptx`。

---

## 目录

1. [PPTX 文件解剖](#1-pptx-文件解剖)
2. [顶层结构](#2-顶层结构)
3. [主题](#3-主题)
4. [幻灯片](#4-幻灯片)
5. [文本元素](#5-文本元素)
6. [形状元素](#6-形状元素)
7. [图片元素](#7-图片元素)
8. [表格元素](#8-表格元素)
9. [分组元素](#9-分组元素)
10. [高级组件](#10-高级组件)
11. [动画](#11-动画)
12. [过渡](#12-过渡)
13. [单位换算](#13-单位换算)
14. [附录：文件生成清单](#附录文件生成清单)

---

## 1. PPTX 文件解剖

把任意 `.pptx` 文件重命名为 `.zip` 并解压，会得到以下目录结构。本节逐文件说明每个文件的用途和内容。

### 完整文件树

```
.
├── [Content_Types].xml              # 文件清单：声明包内每个文件的 MIME 类型
├── _rels/
│   └── .rels                        # 根关系：指向 ppt/presentation.xml
├── docProps/
│   ├── core.xml                     # 文档元数据：标题、作者、创建/修改时间
│   └── app.xml                      # 应用属性：幻灯片数量、应用程序名称
└── ppt/
    ├── presentation.xml             # 主文件：幻灯片列表、幻灯片尺寸
    ├── presProps.xml                # 演示属性（如幻灯片放映设置）
    ├── viewProps.xml                # 视图属性（如 PowerPoint 的视图状态）
    ├── tableStyles.xml              # 表格样式模板
    ├── _rels/
    │   └── presentation.xml.rels    # 主关系：关联母版(slideMaster1)、版式(slideLayout1)、各页幻灯片(slideN)
    ├── theme/
    │   └── theme1.xml               # 主题：12 色配色方案 + 标题/正文字体
    ├── slideMasters/
    │   ├── slideMaster1.xml         # 幻灯片母版：占位符布局、背景、主题引用
    │   └── _rels/
    │       └── slideMaster1.xml.rels # 母版关系：关联版式和主题
    ├── slideLayouts/
    │   ├── slideLayout1.xml         # 幻灯片版式：继承母版的占位符布局
    │   └── _rels/
    │       └── slideLayout1.xml.rels # 版式关系：关联母版和主题
    ├── slides/
    │   ├── slide1.xml               # 第 1 页幻灯片内容（所有元素、背景、动画）
    │   ├── slide2.xml               # 第 2 页
    │   ├── ...
    │   └── _rels/
    │       ├── slide1.xml.rels      # 第 1 页的关系：引用的图片(media/)、超链接
    │       ├── slide2.xml.rels
    │       └── ...
    └── media/
        ├── image1.png               # 嵌入的图片（第一张）
        ├── image2.jpg               # 第二张
        └── ...
```

### 逐文件详解

#### `[Content_Types].xml` — 文件类型注册表

声明包中每个文件的 MIME 类型，PPTX 解析器通过它知道如何解读各个文件。

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <!-- ... 每个文件都有对应条目 -->
</Types>
```

| 文件/扩展名 | ContentType | 说明 |
|------------|-------------|------|
| `.rels` | `...relationships+xml` | 关系文件 |
| `.xml` | `application/xml` | 普通 XML |
| `.png` / `.jpg` 等 | `image/png` 等 | 嵌入的图片 |
| `presentation.xml` | `...presentationml.presentation.main+xml` | 演示文稿主文件 |
| `slideN.xml` | `...presentationml.slide+xml` | 幻灯片 |
| `slideMaster1.xml` | `...presentationml.slideMaster+xml` | 幻灯片母版 |
| `slideLayout1.xml` | `...presentationml.slideLayout+xml` | 幻灯片版式 |
| `theme1.xml` | `...officedocument.theme+xml` | 主题 |
| `core.xml` | `...package.core-properties+xml` | 核心属性 |
| `app.xml` | `...extended-properties+xml` | 扩展属性 |

> **json2pptx 生成策略**：为每个图片格式（png/jpg/gif/svg/webp）注册 `<Default>`，为所有 XML 部件注册 `<Override>`。

#### `_rels/.rels` — 根关系文件

关系文件是 OOXML 的核心机制——每个 XML 文件通过 `.rels` 文件引用其他资源。

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="...officeDocument" Target="ppt/presentation.xml"/>
</Relationships>
```

根关系只有一条：指向主文件 `ppt/presentation.xml`。

#### `docProps/core.xml` — 核心属性

```xml
<cp:coreProperties xmlns:cp="..." xmlns:dc="..." xmlns:dcterms="...">
  <dc:title>Presentation Title</dc:title>          <!-- JSON: meta.title -->
  <dc:creator>json2pptx</dc:creator>                <!-- JSON: meta.author -->
  <cp:lastModifiedBy>json2pptx</cp:lastModifiedBy>
  <dcterms:created xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:modified>
  <cp:revision>1</cp:revision>
</cp:coreProperties>
```

| 字段 | 含义 |
|------|------|
| `<dc:title>` | 文档标题（Windows 文件属性中的"标题"） |
| `<dc:creator>` | 作者 |
| `<cp:lastModifiedBy>` | 最后修改者 |
| `<dcterms:created>` | 创建时间 |
| `<dcterms:modified>` | 修改时间 |

#### `docProps/app.xml` — 应用属性

```xml
<Properties xmlns="...">
  <Application>json2pptx</Application>              <!-- 生成工具名称 -->
  <Slides>9</Slides>                                <!-- 幻灯片总数 -->
  <ScaleCrop>false</ScaleCrop>
  <LinksUpToDate>false</LinksUpToDate>
  <SharedDoc>false</SharedDoc>
</Properties>
```

#### `ppt/presentation.xml` — 演示文稿主文件

幻灯片列表 + 尺寸，这是 PPTX 的"目录"。

```xml
<p:presentation ...>
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>     <!-- 引用母版 -->
  </p:sldMasterIdLst>
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId2"/>                   <!-- 引用第 1 页 -->
    <p:sldId id="257" r:id="rId3"/>                   <!-- 引用第 2 页 -->
    <!-- ... -->
  </p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000"/>               <!-- ← JSON width/height -->
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>
```

| 节点 | 含义 |
|------|------|
| `<p:sldMasterIdLst>` | 幻灯片母版列表（json2pptx 固定一个母版） |
| `<p:sldIdLst>` | 所有幻灯片的引用列表，每页一个 `<p:sldId>` |
| `<p:sldSz>` | 幻灯片尺寸（JSON 的 `width`/`height`，英寸→EMU） |
| `<p:notesSz>` | 备注页尺寸（固定 7.5 × 10 英寸） |

#### `ppt/_rels/presentation.xml.rels` — 主关系文件

将 presentation.xml 中的 `rId` 映射到实际文件路径：

```xml
<Relationship Id="rId1" Type="...slideMaster" Target="slideMasters/slideMaster1.xml"/>
<Relationship Id="rId2" Type="...slide" Target="slides/slide1.xml"/>
<Relationship Id="rId3" Type="...slide" Target="slides/slide2.xml"/>
<Relationship Id="rId4" Type="...theme" Target="theme/theme1.xml"/>
<!-- ... -->
```

这里定义了：

- `rId1` → slideMaster（母版）
- `rId2` ~ `rId(N+1)` → 各页 slide（幻灯片）
- 最后几个 rId → theme（主题）、presProps（演示属性）、viewProps（视图属性）、tableStyles（表格样式）

#### `ppt/presProps.xml` — 演示属性

```xml
<p:presentationPr .../>
```

当前版本仅生成空根节点，未来可用于设置幻灯片放映参数。

#### `ppt/viewProps.xml` — 视图属性

```xml
<p:viewPr ...>
  <p:normalViewPr>
    <p:restoredLeft sz="15620"/>
    <p:restoredTop sz="94660"/>
  </p:normalViewPr>
</p:viewPr>
```

控制 PowerPoint 打开时的视图布局（左侧缩略图/顶部幻灯片的比例）。

#### `ppt/tableStyles.xml` — 表格样式

```xml
<a:tblStyleLst def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>
```

引用一个内置表格样式 GUID。PowerPoint 内置了 10+ 种表格样式，这个 GUID 指定默认样式。

#### `ppt/theme/theme1.xml` — 主题

PPTX 的"调色板 + 字体方案"。详细映射见[第 3 节](#3-主题)。

```xml
<a:theme name="json2pptx Theme">
  <a:themeElements>
    <a:clrScheme name="json2pptx">                     <!-- 12 色配色 -->
      <a:dk1><a:srgbClr val="000000"/></a:dk1>           <!-- 深色 1 -->
      <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>           <!-- 浅色 1 -->
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>   <!-- 强调色 1 -->
      <!-- ... 共 12 个色槽 -->
    </a:clrScheme>
    <a:fontScheme name="json2pptx">                      <!-- 字体方案 -->
      <a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont>  <!-- 标题字体 -->
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>        <!-- 正文字体 -->
    </a:fontScheme>
    <a:fmtScheme>                                         <!-- 填充/线条/效果样式 -->
      <a:fillStyleLst>...</a:fillStyleLst>
      <a:lnStyleLst>...</a:lnStyleLst>
      <a:effectStyleLst>...</a:effectStyleLst>
      <a:bgFillStyleLst>...</a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>
```

#### `ppt/slideMasters/slideMaster1.xml` — 幻灯片母版

所有幻灯片的"基类"。定义背景、主题引用、占位符（标题区、页脚等）。

```xml
<p:sldMaster ...>
  <p:cSld>
    <p:bg>                                             <!-- 母版背景 -->
      <p:bgRef><a:schemeClr val="bg1"/></p:bgRef>
    </p:bg>
    <p:spTree>
      <!-- 占位符：标题、页脚、页码等 -->
    </p:spTree>
  </p:cSld>
  <p:clrMap>                                            <!-- 颜色映射 -->
    <a:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2"/>
  </p:clrMap>
</p:sldMaster>
```

json2pptx 生成最小化的母版，没有占位符，所有元素都在 slide 级别绝对定位。

#### `ppt/slideLayouts/slideLayout1.xml` — 幻灯片版式

母版的"子类"，继承母版并补充占位符布局。json2pptx 生成空版式（仅引用母版和主题）。

#### `ppt/slides/slideN.xml` — 幻灯片内容

每页一张，这是**最核心**的文件，包含该页所有元素。详细映射见[第 4 节](#4-幻灯片)。

```xml
<p:sld ...>
  <p:cSld>
    <p:bg>...</p:bg>                                   <!-- 背景 ← JSON slide.background -->
    <p:spTree>
      <p:nvGrpSpPr>...</p:nvGrpSpPr>                    <!-- 形状树属性 -->
      <p:grpSpPr>...</p:grpSpPr>
      <!-- 元素在这里：<p:sp>（文本/形状）、<p:pic>（图片）、<p:graphicFrame>（表格）、<p:grpSp>（组合） -->
    </p:spTree>
  </p:cSld>
  <p:transition ...>...</p:transition>                  <!-- 过渡 ← JSON slide.transition -->
  <p:timing>...</p:timing>                              <!-- 动画 ← JSON element.animations -->
</p:sld>
```

#### `ppt/slides/_rels/slideN.xml.rels` — 幻灯片关系文件

每页幻灯片的关系清单。如果该页有图片或超链接，在这里建立 rId → 实际文件的映射：

```xml
<Relationships xmlns="...">
  <Relationship Id="rId1" Type="...slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="...image" Target="../media/image1.png"/>    <!-- 图片 -->
  <Relationship Id="rId3" Type="...hyperlink" Target="https://example.com" TargetMode="External"/>  <!-- 超链接 -->
</Relationships>
```

| rId | 用途 |
|-----|------|
| `rId1` | 固定指向 slideLayout（版式） |
| `rId2` | 第一张图片 |
| `rId3+` | 后续图片或超链接 |

#### `ppt/media/imageN.*` — 嵌入的图片

原始图片文件拷贝。命名规则：`image1.png`、`image2.jpg`……扩展名保持原样。

### 关系链总结

PPTX 的各个文件通过 `.rels` 文件串联成一个有向图：

```
[Content_Types].xml  ← 文件清单
       │
_rels/.rels  ──────────────────→  ppt/presentation.xml
                                       │
                            ppt/_rels/presentation.xml.rels
                            │
               ┌────────────┬┼─────────────┬──────────────┐
               ▼            ▼▼             ▼              ▼
     slideMaster1.xml   slide1.xml   slide2.xml   theme1.xml
          │                 │
          ▼                 ▼
   slideLayout1.xml    slide1.xml.rels
          │                 │
          ▼          ┌──────┴──────┐
       theme1.xml    ▼             ▼
              media/image1.png  hyperlink
```

> **JSON → PPTX 的简化过程**：JSON 模型 → Rust 数据结构 → 各 `generate_*()` 函数生成 XML 字符串 → ZIP 打包。每个 `generate_*` 函数对应一个上表中的文件（见[附录](#附录文件生成清单)）。

---

## 2. 顶层结构

```json
{
  "width": 13.333,
  "height": 7.5,
  "meta": { "title": "...", "author": "..." },
  "theme": { ... },
  "template": "modern",
  "slides": [...]
}
```

### 映射表

| JSON 字段 | 生成的文件 | PPTX 中的对应节点/内容 |
|-----------|-----------|----------------------|
| `width` | `ppt/presentation.xml` | `<p:sldSz cx="12192000"/>`（英寸 × 914400 = EMU） |
| `height` | `ppt/presentation.xml` | `<p:sldSz cy="6858000"/>` |
| `meta.title` | `docProps/core.xml` | `<dc:title>...</dc:title>` |
| `meta.author` | `docProps/core.xml` | `<dc:creator>...</dc:creator>` + `<cp:lastModifiedBy>...</cp:lastModifiedBy>` |
| `template` | 不生成文件 | 生成前将模板预设的 theme/background 合并到模型中，本身不出现在 XML 中 |

### PPTX 包结构

```
[Content_Types].xml          ← 声明所有文件的 MIME 类型
_rels/.rels                  ← 根关系文件
docProps/
  core.xml                   ← 元数据（标题、作者）
  app.xml                    ← 应用属性（幻灯片数量）
ppt/
  presentation.xml           ← 主文件（幻灯片列表、尺寸）
  presProps.xml              ← 演示属性
  viewProps.xml              ← 视图属性
  tableStyles.xml            ← 表格样式
  theme/theme1.xml           ← 主题（颜色、字体）
  slideMasters/slideMaster1.xml ← 幻灯片母版
  slideLayouts/slideLayout1.xml ← 幻灯片版式
  slides/slide1.xml          ← 每页幻灯片
  slides/_rels/slide1.xml.rels  ← 幻灯片关系（图片、超链接）
  media/image1.png           ← 嵌入的图片
```

---

## 3. 主题

```json
{
  "theme": {
    "colors": {
      "dk1": "000000",
      "lt1": "FFFFFF",
      "dk2": "44546A",
      "lt2": "E7E6E6",
      "accent1": "4472C4",
      "accent2": "ED7D31",
      "accent3": "A5A5A5",
      "accent4": "FFC000",
      "accent5": "5B9BD5",
      "accent6": "70AD47",
      "hlink": "0563C1",
      "folHlink": "954F72"
    },
    "major_font": "Calibri Light",
    "minor_font": "Calibri"
  }
}
```

### 映射表

| JSON 字段 | 生成的文件 | PPTX 中的对应节点 |
|-----------|-----------|------------------|
| `colors.dk1` | `ppt/theme/theme1.xml` | `<a:dk1><a:srgbClr val="000000"/></a:dk1>` |
| `colors.lt1` | 同上 | `<a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>` |
| `colors.dk2` | 同上 | `<a:dk2><a:srgbClr val="44546A"/></a:dk2>` |
| `colors.lt2` | 同上 | `<a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>` |
| `colors.accent1` ~ `accent6` | 同上 | `<a:accent1>...<a:accent6>` 共 6 个强调色 |
| `colors.hlink` | 同上 | `<a:hlink><a:srgbClr val="0563C1"/></a:hlink>`（超链接颜色） |
| `colors.folHlink` | 同上 | `<a:folHlink><a:srgbClr val="954F72"/></a:folHlink>`（已访问超链接颜色） |
| `major_font` | 同上 | `<a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont>`（标题字体） |
| `minor_font` | 同上 | `<a:minorFont><a:latin typeface="Calibri"/></a:minorFont>`（正文字体） |

### 主题色引用规则

在 JSON 的 `color` 字段中，可以使用以下**主题色别名**，它们会自动映射到 `<a:schemeClr>`：

| 别名 | 说明 |
|------|------|
| `dk1` | 深色 1（文字） |
| `lt1` | 浅色 1（背景） |
| `dk2` | 深色 2 |
| `lt2` | 浅色 2 |
| `accent1` ~ `accent6` | 强调色 1～6 |
| `hlink` | 超链接 |
| `folHlink` | 已访问超链接 |

例如 `"color": "accent1"` 生成 `<a:solidFill><a:schemeClr val="accent1"/></a:solidFill>`。

---

## 4. 幻灯片

```json
{
  "background": "#1a1a2e",
  "transition": {
    "type": "fade",
    "speed": "med",
    "advance_on_click": true,
    "advance_after": 3000
  },
  "elements": [...]
}
```

每张 slide 生成 `ppt/slides/slideN.xml`（N 从 1 开始）。

### background 映射

| JSON 写法 | 生成节点 | 说明 |
|-----------|---------|------|
| `"#1a1a2e"` | `<p:bg><p:bgPr><a:solidFill><a:srgbClr val="1a1a2e"/>` | 纯色背景（带 `#` 前缀） |
| `{"type":"gradient","stops":[...],"angle":135}` | `<p:bg><p:bgPr><a:gradFill><a:gsLst>...<a:lin ang="8100000"/>` | 渐变背景，angle 度 × 60000 |
| `{"type":"image","src":"./bg.png"}` | `<p:bg><p:bgPr><a:blipFill><a:blip r:embed="rId..."/>` | 图片背景，复制到 `ppt/media/` |

### transition 映射

| JSON 字段 | XML 属性/节点 | 可选值 |
|-----------|--------------|--------|
| `type` | `<p:fade/>` / `<p:push dir="r"/>` / `<p:wipe dir="d"/>` 等 | fade / push / wipe / split / cover / cut / dissolve / random |
| `speed` | `spd="slow|med|fast"` | slow / med / fast |
| `advance_on_click` | `advClick="1"` 或 `advClick="0"` | true / false |
| `advance_after` | `advTm="3000"`（毫秒） | 数值 |

过渡类型对应 OOXML 元素：

| JSON type | OOXML 节点 |
|-----------|-----------|
| `fade` | `<p:fade/>` |
| `push` | `<p:push dir="r"/>` |
| `wipe` | `<p:wipe dir="d"/>` |
| `split` | `<p:split orient="horz" dir="out"/>` |
| `cover` | `<p:cover dir="r"/>` |
| `cut` | `<p:cut/>` |
| `dissolve` | `<p:dissolve/>` |
| `random` | `<p:random/>` |

### elements 映射

`elements` 是一个数组，每个元素根据 `type` 字段生成不同的 XML 节点：

| type | 生成的 XML 标签 | 包装器 |
|------|----------------|--------|
| `text` | `<p:sp>`（其中包含 `<p:txBody>`） | 形状作为文本框 |
| `shape` | `<p:sp>`（其中包含 `<p:spPr>` 几何形状） | 形状 |
| `image` | `<p:pic>` | 图片 |
| `table` | `<p:graphicFrame>` | 图形框 |
| `group` | `<p:grpSp>` | 组合 |
| `progress_bar` 等组件 | `<p:grpSp>`（展开为多个子形状） | 组合 |

---

## 5. 文本元素

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
  "font_family": "Arial",
  "wrap": false,
  "fill": "4472C4",
  "line": { "color": "000000", "width": 1.5 },
  "shadow": { "blur": 6, "distance": 3, "angle": 45, "opacity": 0.5, "color": "000000" },
  "animations": [...]
}
```

生成 XML 结构：

```xml
<p:sp>
  <p:nvSpPr>
    <p:cNvPr id="2" name="TextBox 2"/>   <!-- 自增 ID -->
    <p:cNvSpPr txBox="1"/>                <!-- 标记为文本框 -->
  </p:nvSpPr>
  <p:spPr>
    <a:xfrm>                              <!-- ← position (x/y/w/h) -->
      <a:off x="914400" y="914400"/>
      <a:ext cx="9144000" cy="1828800"/>
    </a:xfrm>
    <a:prstGeom prst="rect"/>             <!-- 文本框本质是矩形 -->
    <a:solidFill><a:srgbClr val="4472C4"/></a:solidFill>  <!-- ← fill -->
    <a:ln w="19050"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln>  <!-- ← line -->
    <a:effectLst>...</a:effectLst>         <!-- ← shadow -->
  </p:spPr>
  <p:txBody>
    <a:bodyPr wrap="none" anchor="ctr"/>   <!-- ← wrap + vert_align -->
    <a:lstStyle/>
    <a:p>
      <a:pPr algn="ctr"/>                  <!-- ← align -->
      <a:r>
        <a:rPr lang="en-US" sz="4800" b="1" dirty="0">
          <a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill>
          <a:latin typeface="Arial"/>
        </a:rPr>                           <!-- ← font_size/bold/color/font_family -->
        <a:t>Hello World</a:t>             <!-- ← text -->
      </a:r>
    </a:p>
  </p:txBody>
</p:sp>
```

### 逐字段映射

| JSON 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `position.x` | `<a:off x="...">` | 英寸 → EMU（× 914400） |
| `position.y` | `<a:off y="...">` | 英寸 → EMU |
| `position.w` | `<a:ext cx="...">` | 英寸 → EMU |
| `position.h` | `<a:ext cy="...">` | 英寸 → EMU |
| `text` (字符串) | `<a:p>` → `<a:r>` → `<a:t>` | 简单字符串，`\n` 拆分为多个 `<a:r>` 中间插 `<a:br>` |
| `text` (段落数组) | 多个 `<a:p>`，每个包含自己的 `<a:r>` | 支持同一段落内多 run、不同样式 |
| `font_size` | `<a:rPr sz="4800">` | 点 × 100 = 百分之一度 |
| `bold` | `<a:rPr b="1">` | |
| `italic` | `<a:rPr i="1">` | |
| `underline` | `<a:rPr u="sng">` | |
| `color` | `<a:rPr>` → `<a:solidFill><a:srgbClr val="...">` | 6 位 hex 或主题别名 |
| `font_family` | `<a:rPr>` → `<a:latin typeface="...">` + `<a:ea typeface="...">` + `<a:cs typeface="...">` | 同时设置拉丁、东亚、复杂文字字体。CJK 文本需要 `<a:ea>` 确保正确渲染 |
| `align` | `<a:pPr algn="ctr">` | `left→l` / `center→ctr` / `right→r` / `justify→just` |
| `vert_align` | `<a:bodyPr anchor="ctr">` | `top→t` / `middle→ctr` / `bottom→b` |
| `wrap` | `<a:bodyPr wrap="none">` | `false→none` / `true`/缺省→square |
| `fill` | `<a:solidFill>` 或 `<a:gradFill>` | 给文本框本身加背景色 |
| `line` | `<a:ln w="...">` | 文本框边框，宽度点 × 12700 = EMU |
| `shadow` | `<a:effectLst><a:outerShdw>` | 阴影效果 |

### 多段落 / Run 示例

```json
{
  "text": [
    {
      "runs": [
        { "text": "加粗 ", "bold": true, "color": "FF0000" },
        { "text": "斜体", "italic": true }
      ],
      "align": "center",
      "bullet": true
    },
    {
      "text": "普通段落",
      "align": "left"
    }
  ]
}
```

生成：

```xml
<a:p>
  <a:pPr algn="ctr"><a:buChar char="•"/></a:pPr>         <!-- align + bullet -->
  <a:r>
    <a:rPr lang="en-US" b="1" dirty="0">
      <a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>
    </a:rPr>
    <a:t>加粗 </a:t>
  </a:r>
  <a:r>
    <a:rPr lang="en-US" i="1" dirty="0"/>
    <a:t>斜体</a:t>
  </a:r>
</a:p>
<a:p>
  <a:pPr algn="l"/>
  <a:r>
    <a:rPr lang="en-US" sz="1400" dirty="0"/>
    <a:t>普通段落</a:t>
  </a:r>
</a:p>
```

---

## 6. 形状元素

```json
{
  "type": "shape",
  "shape_type": "roundRect",
  "position": { "x": 1, "y": 1, "w": 4, "h": 3 },
  "rotation": 30,
  "fill": "ED7D31",
  "fill_alpha": 0.3,
  "no_fill": false,
  "line": { "color": "000000", "width": 1 },
  "line_alpha": 0.5,
  "shadow": { "blur": 8, "distance": 4, "angle": 135, "opacity": 0.6, "color": "000000" },
  "adjust": { "adj": 15000 },
  "text": "内部文字",
  "font_size": 18,
  "color": "FFFFFF",
  "align": "center",
  "animations": [...]
}
```

### 逐字段映射

| JSON 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `shape_type` | `<a:prstGeom prst="roundRect">` | JSON 名与 OOXML 名相同，`callout1→wedgeRoundRectCallout`、`callout2→wedgeEllipseCallout` 除外 |
| `rotation` | `<a:xfrm rot="1800000">` | 度 × 60000 |
| `fill` | `<a:solidFill>` 或 `<a:gradFill>` | 纯色或渐变填充 |
| `fill_alpha` | `<a:srgbClr><a:alpha val="30000"/>` | 填充透明度 (0.0-1.0)，默认 1.0，× 100000 |
| `no_fill: true` | `<a:noFill/>` | 透明形状（只留边框），优先级高于 fill |
| `line` | `<a:ln w="12700" cap="flat">` | 边框，默认线端为平角 |
| `line_alpha` | `<a:srgbClr><a:alpha val="50000"/>` | 线条透明度 (0.0-1.0)，默认 1.0 |
| `shadow` | `<a:effectLst><a:outerShdw>` | 阴影 |
| `adjust` | `<a:avLst><a:gd name="adj" fmla="val 15000"/>` | 形状调整值（如圆角半径） |
| `text` | `<p:txBody>` → `<a:p>` → `<a:r>` → `<a:t>` | 形状内部文字 |
| `font_size` | `<a:rPr sz="1800">` | 仅用于内部文字 |
| `color` | `<a:rPr>` → `<a:solidFill>` | 仅用于内部文字颜色 |
| `align` | `<a:pPr algn="ctr">` | 仅用于内部文字对齐 |

### shape_type 映射表

JSON 形状名与 OOXML `prstGeom` 预设名**一一对应**（86 种，含基础图形、箭头、流程符号、标注等），解析时自动反向映射回 JSON 名。标注类为别名映射：

| JSON | OOXML prst |
|------|-----------|
| `callout1` | `wedgeRoundRectCallout` |
| `callout2` | `wedgeEllipseCallout` |
| `borderCallout1` | `borderCallout1` |
| `borderCallout2` | `borderCallout2` |

其余形状（`rect`、`roundRect`、`parallelogram`、`trapezoid`、`flowChartProcess`、`star8` 等）JSON 名与 OOXML 名一致。完整清单见 [元素与组件参考](元素与组件参考.md) 的"86 种形状类型"。

> 形状调整值 `adjust` 映射为 `<a:avLst><a:gd name="adj" fmla="val N"/></a:avLst>`。

---

---

## 7. 图片元素

```json
{
  "type": "image",
  "src": "./assets/logo.png",
  "position": { "x": 0.5, "y": 0.5, "w": 5, "h": 4 },
  "rotation": 15,
  "crop": { "left": 10, "top": 20, "right": 30, "bottom": 40 },
  "hyperlink": "https://example.com",
  "line": { "color": "FF0000", "width": 3 },
  "animations": [...]
}
```

### 生成流程

1. `src` 路径的文件被复制到 `ppt/media/image1.png`（从 1 开始编号）
2. 在 `ppt/slides/_rels/slide1.xml.rels` 中建立关系：`rId2` → `media/image1.png`
3. 在 `[Content_Types].xml` 中注册图片 MIME 类型

> `src` 支持本地路径、HTTP/HTTPS URL。使用 `unpack` 可将 PPTX 内的图片解压到产物 `ppt/media/` 目录，JSON 中以绝对路径引用。

### 逐字段映射

| JSON 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `src` | `ppt/media/imageN.*` + `ppt/slides/_rels/slideN.xml.rels` | 源文件路径，嵌入为媒体文件 |
| `position` | `<a:xfrm>` | 同其他元素 |
| `rotation` | `<a:xfrm rot="...">` | 度 × 60000 |
| `crop.left` | `<a:srcRect l="10">` | 左侧裁剪 |
| `crop.top` | `<a:srcRect t="20">` | 顶部裁剪 |
| `crop.right` | `<a:srcRect r="30">` | 右侧裁剪 |
| `crop.bottom` | `<a:srcRect b="40">` | 底部裁剪 |
| `hyperlink` | `ppt/slides/_rels/slideN.xml.rels` 中建立 rId，`<a:blip>` 旁添加 `<a:hlinkClick r:id="...">` | 超链接关系 |
| `line` | `<a:ln w="38100">` | 图片边框 |

生成 XML 骨架：

```xml
<p:pic>
  <p:nvPicPr>
    <p:cNvPr id="4" name="Picture 4"/>
    <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
  </p:nvPicPr>
  <p:blipFill>
    <a:blip r:embed="rId2"/>                           <!-- ← src 关联 -->
    <a:srcRect l="10" t="20" r="30" b="40"/>           <!-- ← crop -->
    <a:stretch><a:fillRect/></a:stretch>
  </p:blipFill>
  <p:spPr>
    <a:xfrm rot="900000">                               <!-- ← position + rotation -->
      <a:off x="457200" y="457200"/>
      <a:ext cx="4572000" cy="3657600"/>
    </a:xfrm>
    <a:prstGeom prst="rect"/>
    <a:ln w="38100"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln>  <!-- ← line -->
  </p:spPr>
</p:pic>
```

---

## 8. 表格元素

```json
{
  "type": "table",
  "position": { "x": 0.5, "y": 0.5, "w": 9, "h": 3 },
  "header_row": true,
  "rows": [
    [
      { "text": "姓名", "bold": true, "fill": "4472C4", "color": "FFFFFF", "align": "center" },
      { "text": "数值", "bold": true, "fill": "4472C4", "color": "FFFFFF" }
    ],
    [
      { "text": "Alpha" },
      { "text": "42", "bold": true, "align": "right" }
    ]
  ],
  "font_size": 12,
  "color": "333333",
  "animations": [...]
}
```

生成 XML 骨架：

```xml
<p:graphicFrame>
  <p:nvGraphicFramePr>
    <p:cNvPr id="5" name="Table 5"/>
    <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr>
  </p:nvGraphicFramePr>
  <p:xfrm>                                            <!-- ← position -->
    <a:off x="457200" y="457200"/>
    <a:ext cx="8229600" cy="2743200"/>
  </p:xfrm>
  <a:graphic>
    <a:graphicData uri="...table">
      <a:tbl>
        <a:tblPr firstRow="1" bandRow="1"/>            <!-- ← header_row -->
        <a:tblGrid>
          <a:gridCol w="4114800"/>                     <!-- 列宽自动均分 -->
          <a:gridCol w="4114800"/>
        </a:tblGrid>
        <a:tr h="1371600">                             <!-- 行高自动均分 -->
          <a:tc>
            <a:txBody>...</a:txBody>                    <!-- ← 单元格文字 -->
            <a:tcPr><a:solidFill><a:srgbClr val="4472C4"/></a:tcPr>  <!-- ← cell.fill -->
          </a:tc>
          <a:tc>...</a:tc>
        </a:tr>
        <a:tr h="1371600">...</a:tr>
      </a:tbl>
    </a:graphicData>
  </a:graphic>
</p:graphicFrame>
```

### 逐字段映射

| JSON 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `position` | `<p:xfrm>` | 同其他元素 |
| `header_row: true` | `<a:tblPr firstRow="1">` | 首行启用特殊样式 |
| `rows` | `<a:tr>` × N，每行包含 `<a:tc>` × M | 行和列的数量自动确定 |
| `font_size` | `<a:rPr sz="1200">` | 整表默认字号，单元格级可覆盖 |
| `color` | `<a:rPr>` → `<a:solidFill>` | 整表默认颜色，单元格级可覆盖 |

**单元格（Cell）字段映射：**

| Cell 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `text` | `<a:t>...</a:t>` | 单元格文字 |
| `font_size` | `<a:rPr sz="...">` | 覆盖整表字号 |
| `bold` | `<a:rPr b="1">` | |
| `color` | `<a:rPr>` → `<a:solidFill>` | 覆盖整表颜色 |
| `fill` | `<a:tcPr><a:solidFill><a:srgbClr val="..."/>` | 单元格背景色 |
| `align` | `<a:pPr algn="ctr|r|l">` | `left→l` / `center→ctr` / `right→r` |

> **注意**：如果 `header_row: true`，第一行的每个单元格会自动加粗（即使没有显式写 `"bold": true`）。

---

## 9. 分组元素

```json
{
  "type": "group",
  "position": { "x": 0.5, "y": 0.5, "w": 9, "h": 4 },
  "rotation": 10,
  "children": [
    { "type": "text", "text": "分组文本", "position": { "x": 0, "y": 0, "w": 5, "h": 1 } },
    { "type": "shape", "shape_type": "rect", "fill": "00FF00", "position": { "x": 0, "y": 1.5, "w": 3, "h": 2 } }
  ],
  "animations": [...]
}
```

### 逐字段映射

| JSON 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `position` | `<a:xfrm>` → `<a:off>` + `<a:ext>` + `<a:chOff>` + `<a:chExt>` | 组合容器位置（子元素坐标相对此容器） |
| `rotation` | `<a:xfrm rot="600000">` | 组合级旋转，所有子元素跟随 |
| `children` | 递归生成各子元素的 XML，包裹在 `<p:grpSp>` 内部 | 每个子元素自增 sp_id |

```xml
<p:grpSp>
  <p:nvGrpSpPr>
    <p:cNvPr id="2" name="Group 2"/>
  </p:nvGrpSpPr>
  <p:grpSpPr>
    <a:xfrm rot="600000">
      <a:off x="457200" y="457200"/>
      <a:ext cx="8229600" cy="3657600"/>
      <a:chOff x="457200" y="457200"/>
      <a:chExt cx="8229600" cy="3657600"/>
    </a:xfrm>
  </p:grpSpPr>
  <!-- 子元素递归生成 -->
  <p:sp>...</p:sp>
  <p:sp>...</p:sp>
</p:grpSp>
```

---

## 10. 高级组件

高级组件不是 OOXML 原生概念，它们在生成时被**展开**为基本元素的 `<p:grpSp>` 组合。展开后不再保留组件类型信息（roundtrip 解析后会变成普通 Group）。

### 10.1 progressBar

```json
{
  "type": "progress_bar",
  "position": { "x": 0.5, "y": 0.3, "w": 9, "h": 0.6 },
  "value": 75,
  "color": "4472C4",
  "track_color": "E0E0E0",
  "rounded": true,
  "show_label": true,
  "label": "进度",
  "text_color": "000000",
  "font_size": 11
}
```

展开为 2～3 个形状：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 轨道（Track） | `<p:sp>` roundRect/rect，fill = `track_color` | 背景条 |
| 填充（Fill） | `<p:sp>` roundRect/rect，fill = `color`，width = `value%` | 进度条 |
| 标签（Label） | `<p:sp>` txBox，text = `label` 或 `"75%"` | 可选，`show_label` 控制 |

### 10.2 progressRing

```json
{
  "type": "progress_ring",
  "position": { "x": 0.5, "y": 1.2, "w": 1.5, "h": 1.5 },
  "value": 60,
  "color": "ED7D31",
  "track_color": "E0E0E0",
  "thickness": 8,
  "show_label": true,
  "text_color": "333333",
  "font_size": 14
}
```

展开为 2～3 个形状：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 轨道环 | `<p:sp>` ellipse + `<a:ln w="...">` + `noFill` | 空环 |
| 填充弧 | `<p:sp>` blockArc，adjust adj = `value/100 * 36000` | 彩色弧段 |
| 百分比标签 | `<p:sp>` txBox | 中心文字 |

### 10.3 barChart

```json
{
  "type": "bar_chart",
  "position": { "x": 2.5, "y": 1.2, "w": 4, "h": 1.5 },
  "data": [30, 50, 80, 40],
  "labels": ["Q1", "Q2", "Q3", "Q4"],
  "colors": ["FF0000", "00FF00", "0000FF", "FFAA00"],
  "max": 100,
  "show_values": true,
  "axis": true,
  "label_color": "333333",
  "font_size": 10
}
```

展开为 N+1 个形状：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 坐标轴 | `<p:sp>` 细长 rect | 可选，`axis` 控制 |
| 柱体 × N | `<p:sp>` rect，fill = `colors[i]`，高度按比例 | 每个数据点一个柱 |
| 数值标签 × N | `<p:sp>` txBox，text = 数值 | 可选，`show_values` 控制 |
| 底部标签 × N | `<p:sp>` txBox，text = `labels[i]` | 可选 |

> **注意**：这不是真正的 Excel 图表对象，而是用矩形形状模拟的柱状图。

### 10.4 kpiCard

```json
{
  "type": "kpi_card",
  "position": { "x": 7, "y": 1.2, "w": 2.5, "h": 1.5 },
  "value": "98.5%",
  "label": "满意度",
  "delta": "+5.2%",
  "bg": "FFFFFF",
  "accent": "00AA00",
  "rounded": true,
  "text_color": "333333"
}
```

展开为 3～4 个形状：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 卡片背景 | `<p:sp>` roundRect/rect，fill = `bg` | 卡片底色 |
| 强调条 | `<p:sp>` rect，fill = `accent` | 底部细条 |
| 数值 | `<p:sp>` txBox，36pt 粗体 | 大数字 |
| 标签 | `<p:sp>` txBox | 小标题 |
| Delta | `<p:sp>` txBox，正数绿色/负数红色 | 变化指示，可选 |

### 10.5 ratingStars

```json
{
  "type": "rating_stars",
  "position": { "x": 0.5, "y": 3, "w": 3, "h": 0.6 },
  "rating": 4.5,
  "max": 5,
  "color": "FFAA00",
  "empty_color": "E0E0E0"
}
```

展开为 N 个形状（每个星星一个）：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 星星 × N | `<p:sp>` star5，fill = `color` 或 `empty_color` | `i < rating` → 彩色，否则空色 |

### 10.6 timeline

```json
{
  "type": "timeline",
  "position": { "x": 0.5, "y": 3.8, "w": 9, "h": 1 },
  "items": [
    { "date": "2024-Q1", "title": "阶段一", "desc": "调研" },
    { "date": "2024-Q2", "title": "阶段二", "desc": "开发" }
  ],
  "line_color": "4472C4",
  "dot_color": "ED7D31",
  "label_color": "333333"
}
```

展开为 1 + N×2 个形状：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 基线 | `<p:sp>` 水平细长 rect + `line` | 水平线 |
| 圆点 × N | `<p:sp>` ellipse，fill = `dot_color` | 时间节点 |
| 标签 × N | `<p:sp>` txBox，text = item 的字符串值 | 文字描述 |

### 10.7 processFlow

```json
{
  "type": "process_flow",
  "position": { "x": 0.5, "y": 5, "w": 9, "h": 0.6 },
  "steps": ["计划", "执行", "检查", "改进"],
  "colors": ["4472C4", "ED7D31", "70AD47", "FF0000"],
  "text_color": "FFFFFF",
  "font_size": 12
}
```

展开为 N + (N-1) 个形状：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 步骤 × N | `<p:sp>` roundRect，fill = `colors[i]` | 圆角矩形 |
| 箭头 × N-1 | `<p:sp>` rightArrow，fill = `"95a5a6"` | 步骤间连接箭头 |

### 10.8 lineChart（折线图）

展开为坐标轴 + 连接线段 + 数据点 + 标签：

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 坐标轴 | `<p:sp>` rect + 灰色 `ln` | 基线 |
| 折线 × N-1 | `<p:cxnSp>` + `<a:custGeom>` 路径（smooth 时 `quadBezTo`） | 相邻数据点连线 |
| 数据点 × N | `<p:sp>` ellipse，fill = `colors[i]` | 圆形数据点 |
| 数值标签 × N | `<p:sp>` txBox 文本框 | 点上方数值 |
| 分类标签 × N | `<p:sp>` txBox 文本框 | 基线下方 |

### 10.9 pieChart（饼图）

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| 扇区 × N | `<p:sp>` `prst="pie"` + `<a:gd name="adj" fmla="val {角度×60000}"/>` + xfrm `rot={起始角×60000}` | 从 12 点方向顺时针排列 |
| 百分比标签 × N | `<p:sp>` txBox 文本框 | 扇区外侧（按中心角计算位置） |
| 图例 × N | `<p:sp>` txBox 文本框 | 图表下方 |

### 10.10 ringChart（环形图）

| 子形状 | OOXML | 说明 |
|--------|-------|------|
| track 环 × 2 | `<p:sp>` `prst="blockArc"` adj=`5400000`（180°），rot=`0`/`180°` | 浅灰整环打底 |
| 环段 × N | `<p:sp>` `prst="blockArc"` + adj=`扇区角/2×60000` + rot=`起始角` | 超过 180° 拆两段 |
| 百分比标签 / 图例 | 同饼图 | |

---

## 11. 动画

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

动画信息收集后统一生成 `<p:timing>` 节点（插在 slide XML 底部，`</p:sld>` 之前）。

### 逐字段映射

| JSON 字段 | OOXML 位置 | 说明 |
|-----------|-----------|------|
| `type` | `<p:cTn presetID="N" presetClass="entr|exit|emph|path">` | 决定动画类别和预设 ID |
| `duration` | `<p:cTn dur="500">` | 毫秒 |
| `delay` | `<p:cond delay="200">` | 毫秒延迟 |
| `trigger` | `<p:cTn nodeType="clickEffect|withEffect|afterEffect">` | `onClick` 触发新组，`withPrevious`/`afterPrevious` 归入同组 |
| `direction` | `presetSubtype="1|2|4|8..."` 或路径方向 | 方向值编码 |
| `order` | 排序依据 | 控制同一 slide 内动画播放顺序 |
| `scale` | `<a:scale sx="..." sy="...">` | 缩放动画的目标比例（百分比/100） |
| `degrees` | `<p:to><p:fltVal val="360"/>` | 旋转角度 |
| `color` | `<a:srgbClr val="FF0000">` | 颜色变化的目标色 |
| `opacity` | 透明度动画 | 透明度变化 |
| `points` | M/L 路径坐标 | 运动路径关键点 |
| `distance` | 路径长度 | 运动路径距离 |

### 动画类型 → OOXML 行为节点

| JSON type | 类别 | OOXML 行为节点 | 说明 |
|-----------|------|---------------|------|
| `appear` | 入场 | `<p:set>` style.visibility = visible | 瞬间出现 |
| `fadeIn` | 入场 | `<p:animEffect>` fade/in + `<p:set>` | 渐入 |
| `flyIn` | 入场 | `<p:animMotion>` path 从外部到当前位置 | 飞入 |
| `wipeIn` | 入场 | `<p:animEffect>` wipe(in)/in | 擦除进入 |
| `zoomIn` | 入场 | `<p:animScale>` 0→1 | 缩放进入 |
| `bounceIn` | 入场 | `<p:animScale>` 0→1.1→0.95→1 | 弹跳进入 |
| `floatIn` | 入场 | `<p:animMotion>` 短距离移动 | 浮动进入 |
| `swivel` | 入场 | `<p:set>` + `<p:animEffect>` + `<p:animScale>` | 旋转进入 |
| `dissolveIn` | 入场 | `<p:animEffect>` dissolve/in | 溶解进入 |
| `splitIn` | 入场 | `<p:animEffect>` split/in | 分裂进入 |
| `fadeOut` | 退场 | `<p:animEffect>` fade/out | 渐出 |
| `flyOut` | 退场 | `<p:animMotion>` 当前位置到外部 | 飞出 |
| `wipeOut` | 退场 | `<p:animEffect>` wipe(out)/out | 擦除退出 |
| `zoomOut` | 退场 | `<p:animScale>` 1→0 | 缩放退出 |
| `floatOut` | 退场 | `<p:animMotion>` 短距离移出 | 浮动退出 |
| `dissolveOut` | 退场 | `<p:animEffect>` dissolve/out | 溶解退出 |
| `pulse` | 强调 | `<p:animScale>` 1→1.5→1 | 脉冲 |
| `spin` | 强调 | `<p:animRot>` 0→360° | 旋转 |
| `growShrink` | 强调 | `<p:animScale>` 缩放到指定比例 | 缩放 |
| `colorChange` | 强调 | `<p:animClr>` 改变颜色 | 颜色变化 |
| `transparency` | 强调 | `<p:animEffect>` fade/in | 透明度变化 |
| `teeter` | 强调 | `<p:animRot>` 0→±15°→0 | 摇摆 |
| `blink` | 强调 | 多个 `<p:set>` 交替 visibility | 闪烁 |
| `motionPath` | 路径 | `<p:animMotion>` 自定义路径 | 自定义运动轨迹 |

### 触发分组逻辑

| trigger | 效果 |
|---------|------|
| `onClick` | 新起一组，点击触发 |
| `withPrevious` | 与上一动画同时播放 |
| `afterPrevious` | 上一动画结束后播放 |

---

## 12. 过渡

已在[第 4 节 - transition 映射](#transition-映射)中详述。此处补充 OOXML 对照：

| JSON 字段 | OOXML | 说明 |
|-----------|-------|------|
| `type` | `<p:fade/>` 等 | 过渡类型（含 `morph` → `<p14:morph/>`） |
| `speed` | `spd="med"` | `slow` / `med` / `fast` |
| `advance_on_click` | `advClick="1"` | 点击鼠标翻页 |
| `advance_after` | `advTm="3000"` | 自动翻页（毫秒） |

---

## 13. 单位换算

```rust
// 核心换算常量
const EMU_PER_INCH: i64 = 914400;        // 1 英寸 = 914400 EMU
// inch_to_emu(inches: f64) -> i64       // 英寸 → EMU
// pt_to_hundredths(pt: f64) -> i64      // 点 → 百分之一度
// 旋转: 度 × 60000
// 线宽: 点 × 12700
// 透明度: 0.0-1.0 → × 100000 (十万分比)
// 渐变停止: 0-100 → × 1000 (千分比)
```

| 量 | JSON 单位 | OOXML 单位 | 换算公式 |
|---|-----------|-----------|---------|
| 位置/尺寸 | 英寸 (inch) | EMU | × 914400 |
| 字号 | 点 (pt) | 百分之一度 (hundredths) | × 100 |
| 旋转/渐变角 | 度 (°) | 1/60000 度 | × 60000 |
| 线宽 | 点 (pt) | EMU | × 12700 |
| 渐变停止位置 | 0–100 | 千分比 (‰) | × 1000 |
| 透明度 (fill_alpha/line_alpha) | 0.0–1.0 | 十万分比 | × 100000 |
| 阴影不透明度 (shadow.opacity) | 0.0–1.0 | 千分比 (‰) | × 1000 |
| 动画时长/延迟 | 毫秒 (ms) | 毫秒 (ms) | 1:1 直接使用 |

---

## 14. 线条元素（line）

JSON `line` 元素映射为 OOXML **连接线** `<p:cxnSp>` + `<a:custGeom>` 路径：

```xml
<p:cxnSp>
  <p:nvCxnSpPr><p:cNvPr id="2" name="箭头线"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>
  <p:spPr>
    <a:xfrm><a:off x="914400" y="1828800"/><a:ext cx="4572000" cy="457200"/></a:xfrm>
    <a:custGeom>
      <a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>
      <a:rect l="0" t="0" r="4572000" b="457200"/>
      <a:pathLst>
        <a:path w="4572000" h="457200">
          <a:moveTo><a:pt x="914400" y="2057400"/></a:moveTo>
          <a:lnTo><a:pt x="5486400" y="2336800"/></a:lnTo>
        </a:path>
      </a:pathLst>
    </a:custGeom>
    <a:ln w="31750" cap="flat">
      <a:solidFill><a:srgbClr val="E74C3C"/></a:solidFill>
      <a:prstDash val="dash"/>
      <a:tailEnd type="arrow" w="med" len="med"/>
    </a:ln>
  </p:spPr>
</p:cxnSp>
```

| JSON 字段 | OOXML 映射 |
|-----------|-----------|
| `points` | `<a:moveTo>` + 多个 `<a:lnTo>`（路径点为**绝对 EMU**：position 偏移 + 相对点） |
| `smooth: true` | 相邻点之间 `<a:quadBezTo>`（控制点为中点），解析时取终点序列 |
| `dash` | `<a:prstDash val="solid\|dash\|dot">`（solid/dashed/dotted 双向映射） |
| `arrow_start` | `<a:headEnd type="arrow\|oval">`（dot → `oval`，none 省略标签） |
| `arrow_end` | `<a:tailEnd type="arrow\|oval">` |
| `color` / `width` | `<a:ln w="点×12700">` + `<a:solidFill>` |

> 解析时：`p:cxnSp`（quick-xml 已分发到 sp 解析器）内出现 `a:custGeom` 路径且 ≥2 个点 → `line` 元素；`prstGeom prst="line"` 属于预设形状，仍解析为 `shape`。

---

## 15. 演讲者备注（notes）

JSON 幻灯片字段 `notes` 生成 4 类额外部件：

| ZIP 路径 | 说明 |
|----------|------|
| `ppt/notesSlides/notesSlideN.xml` | 单页备注（`<p:notes>` + body 占位文本框，逐行一个 `<a:p>`） |
| `ppt/notesSlides/_rels/notesSlideN.xml.rels` | → `../notesMasters/notesMaster1.xml` + `../slides/slideN.xml` |
| `ppt/notesMasters/notesMaster1.xml` | 备注母版（spTree + `clrMap` + `notesStyle`） |
| `ppt/notesMasters/_rels/notesMaster1.xml.rels` | → `../theme/theme1.xml` |

关联关系：

```xml
<!-- ppt/presentation.xml -->
<p:notesMasterIdLst><p:notesMasterId r:id="rId5"/></p:notesMasterIdLst>
<p:notesIdLst><p:notesId id="256" r:id="rId6"/></p:notesIdLst>
```

```xml
<!-- ppt/_rels/presentation.xml.rels（rId 布局） -->
<Relationship Id="rId1" Type=".../slideMaster" Target="slideMasters/slideMaster1.xml"/>
<Relationship Id="rId2" Type=".../slide" Target="slides/slide1.xml"/>
<Relationship Id="rId5" Type=".../notesMaster" Target="notesMasters/notesMaster1.xml"/>
<Relationship Id="rId6" Type=".../notesSlide" Target="notesSlides/notesSlide1.xml"/>
```

`[Content_Types].xml` 增加 notesSlide / notesMaster 两个 Override。解析时：通过 `notesIdLst` 的 slideId → presentation.xml.rels → notesSlide 部件路径，提取 body 文本框文本还原为 `Slide.notes`。

---

## 附录：文件生成清单

每个 JSON 最终生成以下 PPTX 内部文件：

| ZIP 路径 | 生成函数 | 备注 |
|----------|---------|------|
| `[Content_Types].xml` | `content_types::generate()` | 注册所有文件 MIME |
| `_rels/.rels` | `rels::generate_root()` | 根关系 → ppt/presentation.xml |
| `docProps/core.xml` | `presentation::generate_core_props()` | 标题、作者 |
| `docProps/app.xml` | `presentation::generate_app_props()` | 幻灯片数量 |
| `ppt/presentation.xml` | `presentation::generate()` | 幻灯片列表、尺寸 |
| `ppt/_rels/presentation.xml.rels` | `rels::generate_presentation_rels()` | 关联 master/layout/slides |
| `ppt/presProps.xml` | `presentation::generate_pres_props()` | 演示属性 |
| `ppt/viewProps.xml` | `presentation::generate_view_props()` | 视图属性 |
| `ppt/tableStyles.xml` | `presentation::generate_table_styles()` | 表格样式 |
| `ppt/theme/theme1.xml` | `theme::generate()` | 颜色方案、字体方案 |
| `ppt/slideMasters/slideMaster1.xml` | `slide_master::generate_slide_master()` | 单一母版 |
| `ppt/slideMasters/_rels/slideMaster1.xml.rels` | `rels::generate_slide_master_rels()` | 关联 layout/theme |
| `ppt/slideLayouts/slideLayout1.xml` | `slide_master::generate_slide_layout()` | 单一版式 |
| `ppt/slideLayouts/_rels/slideLayout1.xml.rels` | `rels::generate_slide_layout_rels()` | 关联 master/theme |
| `ppt/slides/slideN.xml` | `slide::generate()` | 每页幻灯片内容 |
| `ppt/slides/_rels/slideN.xml.rels` | `rels::generate_slide_rels()` | 图片、超链接关系 |
| `ppt/media/imageN.*` | `generate()` 主循环 | 嵌入的图片文件 |
| `ppt/notesSlides/notesSlideN.xml` | `notes::generate_notes_slide()` | 有备注的页面（每页一个） |
| `ppt/notesSlides/_rels/notesSlideN.xml.rels` | `rels::generate_notes_slide_rels()` | 关联 notesMaster + slide |
| `ppt/notesMasters/notesMaster1.xml` | `notes::generate_notes_master()` | 备注母版（有备注时生成） |
| `ppt/notesMasters/_rels/notesMaster1.xml.rels` | `rels::generate_notes_master_rels()` | 关联 theme |
