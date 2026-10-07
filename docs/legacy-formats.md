# 旧版 Office 格式支持调研（.doc / .xls / .ppt → OOXML）

> 状态：调研报告（2026-10）。**决策已定**：不引入 LibreOffice（打包体积不可接受），
> 走纯 Rust 路线，parse 入口自动识别旧格式并转换。本文档给出选型、体积实测、
> 保真度预期与分阶段实施路线。

## 一、背景与现状

三个 CLI（json2docx / json2xlsx / json2pptx）目前只支持 OOXML。对旧版二进制格式
（.doc / .xls / .ppt，OLE2/CFB 容器）的当前行为：

| CLI | 现状 | 位置 |
| --- | --- | --- |
| json2xlsx | 唯一做了魔数探测：非 zip 且前 8 字节为 CFB 签名时报「文件已加密或为受保护的 OLE/CFB 格式，暂不支持」 | `ai-excel/src/parse/mod.rs:33-38` |
| json2docx | 无探测，.doc 报笼统的「ZIP 错误」 | `ai-word/src/parse/mod.rs:85-97` |
| json2pptx | 无探测，.ppt 报「PPTX ZIP 结构错误」 | `ai-ppt/src/parse/mod.rs:606-609` |

旧格式规范：三种格式的容器都是 **OLE2/CFB**（魔数 `D0 CF 11 E0 A1 B1 1A E1`），
正文分别是 MS-XLS（BIFF8 记录流）、MS-DOC（WordDocument 流 + piece table）、
MS-PPT（PowerPoint Document 流 + 记录流）。手写完整解析器的工程量都在"人年级"，
因此选型以**现成成熟库**为前提。

## 二、分发体积实测（本轮关键数据）

用临时 crate 实测各纯 Rust 库对 release 二进制的增量
（macOS arm64，`--release` + strip，基线 = 空 main 334 KB）：

| 库 | 版本 | 必选依赖 | 实测二进制 | 增量 |
| --- | --- | --- | --- | --- |
| calamine（.xls/.ods/.xlsb 读取） | 0.36.1 | byteorder/codepage/encoding_rs/fast-float2/log/quick-xml/serde/zip（大半已在依赖树内） | 1043 KB | **+0.7 MB** |
| rwml 最小特性（.doc 读取，关掉 docx/pdf 特性） | 0.1.4 | cfb/encoding_rs/thiserror | 695 KB | **+0.4 MB** |
| office_oxide 关闭可选特性（.doc/.xls/.ppt 三合一读取 + save_as 转 OOXML） | 0.1.13 | encoding_rs/quick-xml/flate2/serde/serde_json/thiserror/zip/atoi_simd/fast-float2 | 2756 KB | **+2.4 MB** |

**结论：三个库全上，二进制增量 < 3.5 MB**（stripped），对比 LibreOffice 的
数百 MB~GB 级安装体积，分发约束完全满足。功能冒烟均已验证：calamine 读真实 xlsx ✓、
office_oxide 读真实 xlsx/docx 并提取文本 ✓、rwml 最小特性下 .doc 读取可用
（docx 支持需开 `docx` feature，其依赖 quick-xml/zip/flate2 在我们依赖树内已存在）。

## 三、Rust 生态盘点

### .xls（Excel 97-2003, BIFF8）——推荐 calamine ★★★★★（生产级）

[calamine](https://crates.io/crates/calamine)（MIT/Apache-2.0，纯 Rust）是 Rust 生态
事实标准：支持 .xls（`Xls`，BIFF8）/ .xlsx / .xlsm / .xlsb / .xla / .xlam / .ods，
读值（字符串/数值/布尔/日期时间）、公式字符串（`worksheet_formula`）、VBA 工程。
局限：单元格**样式**（字体/填充/边框）提取有限——对"转换后重新处理"的场景够用，
对"像素级保真"不够。仓库测试语料本就收录了 5 份 calamine 项目的 fixture
（`ai-excel/tests/corpus/SOURCES.md`），生态契合。

### .doc（Word 97-2003, MS-DOC）——推荐 rwml ★★★☆（能力完整，新库需 PoC）

[rwml](https://docs.rs/rwml)（MIT，v0.1.4，2026-08）：纯 Rust 同时读 legacy .doc 与
.docx，自动 magic 检测。对 .doc 的提取**远超文本级**：统一 `DocModel` 含段落/字符 run
（粗斜体等格式）/标题/列表/超链接/表格（含合并单元格、边框）/图片（bytes+mime）/
浮动图形/文本框/批注/修订/脚注尾注/页眉页脚/页面设置，且提供 `write_docx`
（.doc → DocModel → .docx 纯 Rust 直转，无需 LibreOffice）、`to_markdown`/`to_html`。
panic-free（不可信输入）、支持 WASM。
依赖：必选仅 cfb/encoding_rs/thiserror；docx 输出与 pdf 渲染为可选特性。

- 备选：[unword](https://crates.io/crates/unword)（.doc 解析器，被 kcode-doc-extraction 使用）、
  [office_oxide::doc](https://docs.rs/office_oxide)（只读 + save_as）。

### .ppt（PowerPoint 97-2003, MS-PPT）——最薄弱 ★★☆（实验性）

- [office_oxide::ppt](https://docs.rs/office_oxide)：纯 Rust 只读，含 `save_as` 转 .pptx；
- [pptxboss](https://github.com/4thel00z/pptxboss)：按 MS-CFB/MS-PPT/MS-ODRAW 规范从零实现，
  读文本/备注/标题/图片（实验性）。
- 评估：.ppt 的纯 Rust 生态最不成熟，保真度预期最低（文本/标题级）。

### 三格式统一（一个依赖覆盖 .doc/.xls/.ppt）——office_oxide ★★★☆

[office_oxide](https://crates.io/crates/office_oxide)（MIT/Apache-2.0，v0.1.13，2026-09）：
统一 `Document::open` → `plain_text()` / `to_html()` / `to_markdown()` /
**`save_as(OOXML 路径)`（旧格式直转新格式）**，另有 `DocumentIR` 中间表示、
`limits`（不可信输入资源限制）。旧格式只读、新格式读写编辑。
对"转换成新的再处理"是**一个依赖解决三种格式**的方案。

### 容器与加密

- OLE2/CFB 容器：[cfb](https://crates.io/crates/cfb)（mdsteele，成熟）——rwml/office_oxide 内部已用；
- 加密 OLE（打开即乱码的受保护文档）：[msoffice_crypto](https://docs.rs/msoffice-crypto/latest/msoffice_crypto) 可解（需密码）。

### 备选路线存档（已按决策排除）

LibreOffice headless（`soffice --headless --infilter="MS Word 97" --convert-to docx …`）
是保真度最高的转换路线，但安装体积数百 MB~GB 级、跨平台分发困难——与"重视分发体积"
的决策冲突，故仅在此存档。若未来某些文件纯 Rust 转换质量不足，可把 LibreOffice 作为
**用户自装的可选外部工具**（检测到可用时启用高质量模式），不随我们分发。

## 四、保真度预期矩阵（诚实评估）

### 纯 Rust 引擎（--engine rust，无外部依赖）

| 旧格式 → 新格式 | 可保留 | 会丢失 | 预期保真 |
| --- | --- | --- | --- |
| .xls → .xlsx | 单元格值/公式字符串/数字格式（基础）/多表/列宽行高（部分） | 复杂样式（字体/填充/边框大部分）、图表、条件格式、数据透视 | ★★★☆ 可用 |
| .doc → .docx | 段落/标题/列表/run 格式（粗斜下划线）/表格（含合并）/图片/超链接/批注/页眉页脚 | 复杂版式（文本框环绕/艺术字/分栏细节）、嵌入 OLE、部分修订语义 | ★★★☆ 可用 |
| .ppt → .pptx | 幻灯片文本/标题/备注 | 图形对象（MS-ODRAW）细节、动画、母版样式 | ★★☆ 兜底 |

> 纯 Rust 路线的天花板由上游库决定；保真度随上游版本迭代提升，我们侧只需跟随升级。

### LibreOffice 引擎（--engine libreoffice，用户自装的 soffice）

2026-10 已落地：`office-core::soffice` 提供探测与无头转换（独立 UserInstallation profile，
不与运行中的 LibreOffice 抢锁），三个 CLI 的 `convert` 统一增加 `--engine auto|libreoffice|rust`
（默认 `auto`：检测到 soffice 用之，否则回退 rust，状态行注明所用引擎）。pptx 产物里的
WMF/EMF 图片浏览器无法显示，ppt 引擎转换后自动把包内元文件光栅化为 PNG 并改写引用
（个别转换失败保留原样）。

govdocs1 语料抽样（每格式 6 份，seed 20261004）实测，此前纯 Rust 丢失项全部恢复：
.xls 的填充/加粗/边框/列宽/换行、.doc 的信头图片、.ppt 的 4:3 画布/标题占位符
字号颜色/幻灯片背景渐变；WMF logo 光栅化后正常显示。保真 ★★★★☆，剩余差异：
母版/版式上的装饰元素（页脚 logo 条等）不随幻灯片 unpack；个别装饰线条依赖
查看器对 line 形状的特例渲染（已支持）。

## 五、推荐架构（parse 入口自动识别）

```
输入文件 → 读前 8 字节
  ├─ PK\x03\x04（zip）→ 现有 OOXML 流水线（不变）
  ├─ D0 CF 11 E0…（OLE2/CFB）
  │    ├─ 内部流嗅探（WordDocument / Workbook / PowerPoint Document）
  │    │   → 自动调对应转换器（下述三选一按扩展名/内部流名）
  │    │   → 得到新格式文件（或直接得到内存模型）→ 走现有 unpack 流水线
  │    └─ 加密容器 → 明确报错（提示 msoffice_crypto 需密码，阶段外）
  └─ 其他 → 现有错误路径
```

分阶段实施（每阶段独立可用、带真实旧格式语料 + 快照测试）：

| 阶段 | 内容 | 工作量 | 依赖 |
| --- | --- | --- | --- |
| 0 | 三 CLI parse 统一 OLE2 魔数探测：识别后报「旧版 Office 格式，请用 convert」而非 zip 错误（ai-word/ai-ppt 补齐 ai-excel 已有的探测） | ~半天 | 无 |
| 1 | `json2xlsx convert *.xls`：calamine → `json2xlsx::Workbook` → xlsx/产物目录；parse 自动识别 .xls | 1~2 天 | calamine |
| 2 | `json2docx convert *.doc`：rwml DocModel → json2docx blocks 映射（标题/段落/列表/表格/图片/run 格式）+ parse 自动识别 .doc；**先 PoC 验保真度** | 2~3 天 | rwml |
| 3 | `json2pptx convert *.ppt`：office_oxide（或 pptxboss）→ slides 映射 + parse 自动识别 .ppt；PoC 风险最高 | 2~3 天 | office_oxide / pptxboss |

替代组合：若阶段 2/3 想少维护映射代码，可直接用 office_oxide 的 `save_as`（旧→新文件），
映射工作量最小，但保真度受其 IR 渲染质量限制——建议阶段 2 前用真实样本 A/B 两条路线。

## 六、风险与 PoC 清单

1. **rwml / office_oxide / pptxboss 均为 0.1.x 新库**（单作者、2026 年发布）：
   实现前必须用真实旧文档样本做 PoC（每格式 ≥10 份：含表格/图片/页眉页脚的典型文档），
   核对 DocModel/IR 的实际还原度——这是最大不确定性。
2. **加密 OLE**（受密码保护）：超出现有解析器能力，阶段 0 明确报错并提示；
   如需支持接入 msoffice_crypto（需用户提供密码）。
3. **性能**：calamine/rwml/office_oxide 均为流式/惰性读取，旧格式文件普遍 <10MB，无预期风险；
   转换后走现有 OOXML 流水线（该链路已有百万行级优化）。
4. **样式保真**：.xls 的样式提取是 calamine 的弱项；若用户强需求，备选方案是把
   LibreOffice 保留为"用户自装可选工具"（检测到 soffice 时启用高保真模式），不随我们分发。

## 七、参考来源

- calamine：https://crates.io/crates/calamine ｜ https://docs.rs/calamine
- rwml：https://docs.rs/rwml ｜ https://github.com/HyunjoJung/rwml
- office_oxide：https://crates.io/crates/office_oxide ｜ https://docs.rs/office_oxide
- pptxboss：https://github.com/4thel00z/pptxboss
- unword：https://crates.io/crates/unword
- cfb：https://crates.io/crates/cfb ｜ msoffice_crypto：https://docs.rs/msoffice-crypto/latest/msoffice_crypto
- LibreOffice 转换过滤器：https://help.libreoffice.org/latest/en-US/text/shared/guide/convertfilters.html
- 体积实测方法与本节数据：2026-10，临时 crate 实测（macOS arm64，release + strip），
  复现脚本思路见本文第二节（每库一个最小 main，共享 CARGO_TARGET_DIR，strip 前后取值）。
