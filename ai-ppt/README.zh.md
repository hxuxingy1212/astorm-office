# json2pptx

**JSON ↔ PPTX 双向转换 Rust 库与 CLI 工具**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/badge/crate-json2pptx-orange)](https://crates.io)

通过直接操作 OOXML（Office Open XML）实现 JSON 与 PowerPoint 文件的双向转换，**零 PowerPoint 运行时依赖**。

受 [slidej](https://github.com/H4pplness/slidej) 架构启发，从零构建，支持完整回环。

---

## 特性

| 类别 | 支持情况 |
|------|---------|
| **核心** | JSON → PPTX 生成 与 PPTX → JSON 解析 |
| **文本** | 多段落、多 run、项目符号、CJK（含东亚字体）、换行 |
| **形状** | 86 种形状类型，填充/线条透明度，adjust 值 |
| **线条** | 独立 line 元素（直线/折线/曲线，箭头端点、虚线样式） |
| **图片** | JPEG/PNG，支持裁剪、超链接、边框 |
| **背景** | 纯色、渐变、图片背景 |
| **表格** | 任意行列，单元格级样式（粗体、颜色、填充、对齐） |
| **分组** | 嵌套元素分组，支持旋转 |
| **动画** | 24 种类型（入场 / 退场 / 强调 / 路径） |
| **过渡** | 9 种类型：fade、push、wipe、split、cover、cut、dissolve、random |
| **主题** | 自定义配色方案（12 色），标题/正文字体 |
| **模板** | 6 种内置模板预设（含中文字体配置） |
| **组件** | 10 种高级组件：progressBar、progressRing、barChart、lineChart、pieChart、ringChart、kpiCard、ratingStars、timeline、processFlow |

**无任何** MS Office 或 PPTX SDK 依赖，纯 Rust 实现 OOXML 生成与解析。

---

## 快速上手

```bash
# 构建（workspace 包含 CLI）
cargo build --release

# 解包为可编辑中间产物（按幻灯片拆分 JSON + 媒体）
cargo run --release --bin json2pptx -- unpack in.pptx -o out/

# 查看某页文本与媒体 / 组件布局
cargo run --release --bin json2pptx -- view in.pptx /slide[1] text
cargo run --release --bin json2pptx -- view in.pptx /slide[1] layout

# 精准修改（get/set/add/remove）
cargo run --release --bin json2pptx -- edit in.pptx /slide[1]/text[1] set --prop text=新标题

# 从产物重建 PPTX
cargo run --release --bin json2pptx -- repack out/ -o new.pptx

# 全局安装
cargo install --path cli
json2pptx unpack in.pptx -o out/
```

---

## CLI 命令

`unpack` / `repack` / `view`（text|layout）/ `edit`（get|set|add|remove）/ `render`（PNG）/ `query`（选择器）/
`validate`（结构校验，有问题退出码 3）/ `dump`（可回放指令）/ `extract`（提取主题模板）/
`raw` / `raw-set`（原始部件读写，`raw-set` 不覆盖原文件）/ `schema`（`--json` 输出机器可读 Schema）/ `templates`。

全局 flag：`--json` / `--quiet` / `--verbose` / `-o`；stdout 只输出数据，状态走 stderr；退出码 0/1/2/3。
完整参考见 [README.md](README.md) 与 [skill/references/cli.md](skill/references/cli.md)。

---

## 文档

完整中文文档见 [docs/文档索引.md](docs/文档索引.md)：

| 文档 | 内容 |
|------|------|
| [快速开始](docs/快速开始.md) | 环境要求、构建、CLI 与库使用、示例 |
| [JSON格式规范](docs/JSON格式规范.md) | 完整 JSON 结构、单位与颜色约定 |
| [元素与组件参考](docs/元素与组件参考.md) | 6 种基础元素 + 10 种高级组件字段详解 |
| [动画与过渡](docs/动画与过渡.md) | 24 种动画、8 种过渡 |
| [命令行工具](docs/命令行工具.md) | CLI 全部子命令与参数 |
| [模板系统](docs/模板系统.md) | 6 种内置模板配色与字体 |
| [设计风格指南](docs/设计风格指南.md) | 12 种设计风格（配色、字体、图片提示词） |
| [OOXML映射参考](docs/OOXML映射参考.md) | JSON 字段 → PPTX 逐字段映射 |
| [架构与开发](docs/架构与开发.md) | 架构、测试、扩展指南 |

英文版 README 参见 [README.md](README.md)。

---

## 许可

MIT
