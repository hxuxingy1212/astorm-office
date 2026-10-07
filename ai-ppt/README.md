# json2pptx

**JSON ↔ PPTX bidirectional conversion library & CLI — JSON 与 PPTX 双向转换工具**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/badge/crate-json2pptx-orange)](https://crates.io)

Generate PowerPoint (`.pptx`) files from JSON, or parse existing `.pptx` files back to JSON — all by directly manipulating OOXML, with zero PowerPoint runtime dependency.

通过直接操作 OOXML（Office Open XML）实现 JSON 与 PowerPoint 文件的双向转换，零 PowerPoint 运行时依赖。

---

## Overview / 概述

**json2pptx** is a Rust library and CLI tool that provides:

- **JSON → PPTX**: Generate a complete `.pptx` file from a JSON description
- **PPTX → JSON**: Parse an existing `.pptx` file into a JSON representation
- **Roundtrip**: The two directions are designed to produce structurally equivalent results

受 [slidej](https://github.com/H4pplness/slidej) 架构启发，从零构建，支持完整回环。

---

## Features / 特性

| Category | Support |
|----------|---------|
| **Core** | JSON → PPTX generation & PPTX → JSON parsing / 生成与解析 |
| **Text** | Multi-paragraph/run, bullets, highlight, underline color, char spacing, RTL, lang, autofit, CJK / 多段落、多 run、项目符号、高亮、字距、RTL、语言、自适应、CJK |
| **Shapes** | 86 shape types, solid/gradient/pattern/image fill, glow/reflection/blur/soft-edge/inner-shadow/fill-overlay, alpha, adjust / 86 种形状、四类填充、六种效果、透明度、adjust |
| **Lines** | Dedicated line element: straight/polyline/curve with arrowheads & dash styles / 独立 line 元素（直线/折线/曲线，箭头端点、虚线） |
| **Images** | JPEG/PNG/SVG with cropping, hyperlinks, borders, brightness/contrast, fill mode, video/audio / 裁剪、超链接、边框、亮度对比度、填充模式、视频音频 |
| **Backgrounds** | Solid color, gradient, and image backgrounds / 纯色、渐变、图片背景 |
| **Tables** | Rows/cols, per-cell styles, merge, row heights, cell margins/borders/vertical text/image fill / 行列、单元格样式、合并、行高、边距边框竖排、图片填充 |
| **Groups** | Nested element groups with rotation / 嵌套分组，支持旋转 |
| **Animations** | 24 types: appear, fadeIn, flyIn, wipeIn, zoomIn, bounceIn, floatIn, swivel, dissolveIn, splitIn, fadeOut, flyOut, wipeOut, zoomOut, floatOut, dissolveOut, pulse, spin, growShrink, colorChange, transparency, teeter, blink, motionPath / 24 种动画 |
| **Transitions** | 9 types: fade, push, wipe, split, cover, cut, dissolve, random, morph / 9 种过渡 |
| **Charts** | Real chart parts (parse + preserve, incl. bubble/stock/3D) / 真实图表解析与保真 |
| **Parse fidelity** | Namespace-agnostic OOXML, layout/master placeholder inheritance, comments, video/audio, equations (OMML), OLE/SmartArt/3D/zoom passthrough / 真实 PPTX 高保真解析 |
| **Theme** | Custom color scheme (12 colors), major/minor fonts / 自定义配色方案与字体 |
| **Components** | 10 high-level components: progressBar, progressRing, barChart, lineChart, pieChart, ringChart, kpiCard, ratingStars, timeline, processFlow / 10 种高级组件 |

**Zero dependencies** on MS Office or any PPTX SDK — pure Rust OOXML generation and parsing.
**无任何** MS Office 或 PPTX SDK 依赖。

---

## Quick Start / 快速开始

### Build / 构建

```bash
git clone <astorm-office 仓库>
cd astorm-office
cargo build -p json2pptx -p json2pptx-cli --release
```

### CLI Usage / CLI 使用

```bash
# Unpack PPTX to editable intermediate product / 解包为可编辑中间产物
cargo run --bin json2pptx -- unpack input.pptx -o out/

# View slide text & media / 查看某页文本与媒体（媒体为绝对路径）
cargo run --bin json2pptx -- view input.pptx /slide[1] text

# Edit precisely / 精准修改（get/set/add/remove）
cargo run --bin json2pptx -- edit input.pptx /slide[1]/text[1] set --prop text=新标题

# Rebuild PPTX from the product / 从产物重建 PPTX
cargo run --bin json2pptx -- repack out/ -o output.pptx
```

Install globally / 全局安装:

```bash
cargo install --path cli
json2pptx unpack input.pptx -o out/
```

### Library Usage / 库使用

Add to your `Cargo.toml` / 添加到项目:

```toml
[dependencies]
json2pptx = { git = "..." }
serde_json = "1"
```

```rust
use json2pptx::model::*;
use json2pptx::model::elements::*;
use json2pptx::generate;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pres = Presentation {
        width: 13.333,
        height: 7.5,
        meta: Some(Meta {
            title: Some("Hello".to_string()),
            author: Some("json2pptx".to_string()),
        }),
        theme: Some(Theme {
            colors: Some(std::collections::HashMap::from([
                ("accent1".to_string(), "4472C4".to_string()),
            ])),
            major_font: Some("Calibri Light".to_string()),
            minor_font: Some("Calibri".to_string()),
        }),
        slides: vec![Slide {
            background: Some(serde_json::json!("#1a1a2e")),
            transition: None,
            elements: vec![
                Element::Text(TextElement {
                    text: TextContent::Simple("Hello, PowerPoint!".to_string()),
                    position: Position { x: 1.0, y: 2.0, w: 10.0, h: 2.0 },
                    font_size: Some(48.0),
                    bold: Some(true),
                    color: Some("FFFFFF".to_string()),
                    align: Some("center".to_string()),
                    ..Default::default()
                }),
            ],
        }],
    };

    let result = generate(&pres, "output.pptx")?;
    println!("Generated: {} ({} slides)", result.path, result.slides);
    Ok(())
}
```

### JSON Input Format / JSON 输入格式

```json
{
  "width": 13.333,
  "height": 7.5,
  "meta": { "title": "Sample", "author": "json2pptx" },
  "theme": {
    "colors": { "accent1": "4472C4", "dk1": "000000", "lt1": "FFFFFF" },
    "major_font": "Calibri Light",
    "minor_font": "Calibri"
  },
  "slides": [
    {
      "background": "#1a1a2e",
      "transition": { "type": "fade", "speed": "med" },
      "elements": [
        {
          "type": "text",
          "text": "Hello, World!",
          "position": { "x": 1, "y": 2, "w": 10, "h": 2 },
          "font_size": 48, "bold": true, "color": "FFFFFF", "align": "center"
        },
        {
          "type": "shape",
          "shape_type": "ellipse",
          "position": { "x": 5, "y": 5, "w": 2, "h": 2 },
          "fill": { "Solid": "ED7D31" },
          "line": { "color": "FFFFFF", "width": 2 }
        }
      ]
    }
  ]
}
```

See [docs/JSON格式规范.md](docs/JSON格式规范.md) for the complete JSON schema and all supported element types.
完整 JSON Schema 及所有元素类型参见 [docs/JSON格式规范.md](docs/JSON格式规范.md)。

---

## Architecture / 架构

```
┌──────────────┐      ┌───────────────┐      ┌─────────────┐
│  JSON Input  │ ──▶  │  OOXML Parts  │ ──▶  │  PPTX File  │
│  (serde)     │ gen  │  (XML String) │ zip  │  (.zip)     │
└──────────────┘      └───────────────┘      └─────────────┘
       ▲                                             │
       └────────────────── parse ────────────────────┘
```

- **Generate**: JSON → Rust Model → OOXML XML Strings → ZIP → `.pptx`
- **Parse**: `.pptx` → ZIP → XML Parse → Rust Model → JSON

| Dependency | Purpose |
|------------|---------|
| `serde` + `serde_json` | JSON serialization |
| `quick-xml` 0.36 | Streaming XML reader/writer |
| `zip` 2.x | PPTX (ZIP) container |
| `clap` 4.x | CLI argument parsing (CLI only) |

---

## Project Structure / 项目结构

```
json2pptx/
├── Cargo.toml              # Workspace root (workspace members: cli)
├── src/
│   ├── lib.rs              # Library entry: generate/parse/unpack/repack/Error
│   ├── model/              # Data models (serde-based) / 数据模型
│   │   ├── elements/       # Text, Shape, Image, Table, Group, Animation + 7 components
│   │   ├── presentation.rs # Presentation, Meta
│   │   ├── slide.rs        # Slide
│   │   ├── theme.rs        # Theme
│   │   ├── transition.rs   # Transition
│   │   └── template.rs     # 6 template presets
│   ├── generate/           # JSON → PPTX generation pipeline / 生成管线
│   ├── parse/              # PPTX → JSON parsing pipeline / 解析管线
│   ├── error.rs            # Library error type
│   └── utils/              # Constants, XML helpers / 常量与 XML 辅助
├── cli/                    # CLI binary (clap-based), 13 subcommands
├── examples/               # basic.rs, comprehensive.rs, generate_templates.rs
├── tests/ + cli/tests/     # integration tests (roundtrip / unpack-repack / CLI / contract)
└── docs/                   # Complete Chinese documentation / 完整中文文档
    ├── 文档索引.md           # Doc center / 文档中心
    ├── JSON格式规范.md       # JSON schema / JSON 规范
    └── ...                 # See docs/文档索引.md for the full list
```

---

## Testing / 测试

Run all roundtrip tests / 运行全部回环测试:

```bash
cargo test
```

测试覆盖范围 / test coverage:

- Empty slides, multi-slide, custom dimensions
- All element types (text, shape, image, table, group)
- Backgrounds (solid, gradient)
- Transitions (all 8 types, speed, auto-advance)
- Themes & metadata
- 24 animation types
- CJK text, special characters, edge cases
- Full roundtrip fidelity validation

---

## CLI Reference / CLI 参考

| Command / 命令 | Alias / 别名 | Function / 功能 |
|----------------|-------------|-----------------|
| `json2pptx unpack <input.pptx> -o <DIR>` | — | PPTX → editable slide-split JSON + media |
| `json2pptx repack <DIR> -o out.pptx` | — | Rebuild PPTX from unpacked product |
| `json2pptx view <input> /slide[N] text\|layout` | — | L1 view: text+media / layout tree (pptx or product dir, stateless) |
| `json2pptx edit <input> <path> get\|set\|add\|remove [-o out.pptx]` | — | L2 precise editing (pptx needs `-o` for writes) |
| `json2pptx render <input> /slide[N] [-o out.png]` | — | Render slide to PNG (browser / native engine) |
| `json2pptx query <input> "<selector>"` | — | CSS-like element query / 选择器查询 |
| `json2pptx validate <input>` | — | Structural checks (bounds/empty/missing media…) / 结构校验 |
| `json2pptx dump <input>` | — | Export replayable edit commands JSON / 导出可回放指令 |
| `json2pptx extract <input> [-o DIR]` | — | Extract theme/style template (template.json + TEMPLATE.md) / 提取主题模板 |
| `json2pptx raw <input> <part>` / `raw-set <input> <part> --file F -o out.pptx` | — | Raw OPC part byte-level read/replace (`raw-set` never overwrites input) / 原始部件读写 |
| `json2pptx schema [--json]` | — | Print schema guide (`--json` = machine-readable JSON Schema) / 打印指引 |
| `json2pptx templates [--json]` | — | List available template presets / 列出可用模板 |

Global flags (shared by all three astorm-office CLIs): `--json` (result JSON on stdout for write commands; status/errors as JSON lines on stderr), `--quiet`, `--verbose`, `-o/--output`. stdout carries data only; status goes to stderr; exit codes: `0` ok, `1` runtime error, `2` usage, `3` validation issues. See `../docs/cli-conventions.md`.

---

## Unit Conversion / 单位换算

| Quantity | JSON Unit | OOXML Unit | Factor |
|----------|-----------|------------|--------|
| Position/Size | inch | EMU | × 914400 |
| Font size | point | hundredths | × 100 |
| Rotation | degree | 1/60000° | × 60000 |
| Line width | point | EMU | × 12700 |
| Opacity (fill_alpha/line_alpha) | 0.0–1.0 | hundred-thousandths | × 100000 |
| Opacity (shadow) | 0.0–1.0 | thousandths | × 1000 |

---

## Documentation / 文档

Full documentation (Chinese) is available in [docs/文档索引.md](docs/文档索引.md) — including JSON schema, element/component reference, animations & transitions, CLI guide, templates, design styles, OOXML mapping, and architecture notes.

完整中文文档见 [docs/文档索引.md](docs/文档索引.md)：含 JSON 格式规范、元素与组件参考、动画与过渡、命令行工具、模板系统、设计风格指南、OOXML 映射参考、架构与开发等 10 篇文档。

---

## License / 许可

MIT
