# astorm-office 三项目整合审查报告

> 日期：2026-10-02 · 范围：ai-word（json2docx）/ ai-excel（json2xlsx）/ ai-ppt（json2pptx）
> 结论先行：三个项目是刻意的姊妹设计，架构同构度很高，核心心智模型已经一致；整合的主要工作是消除外围约定（CLI 契约、命名、schema、文档）的漂移，不需要推翻任何核心设计。

## 一、三项目速览

| | ai-word | ai-excel | ai-ppt |
|---|---|---|---|
| 核心库 crate | `json2docx` | `json2xlsx` | `json2pptx` |
| CLI bin | `aiworld-cli`（clap 显示 `aiworld`） | `json2xlsx-cli` | `json2pptx-cli`（clap 显示 `json2pptx`） |
| 目标格式 | Word/DOCX | Excel/XLSX | PPT/PPTX |
| 额外面 | mcp / serve / render pdf-png | import(CSV/TSV) / raw / 公式求值器 | web 编辑器 / web-server / render png |
| 测试规模 | 回环 27 + 语料 28 + CLI 6 | 回环 + 语料 27 份 + CLI e2e | 回环 41 + unpack 4 + CLI 7 |
| 产物目录 | `document.json + word/parts/*.json` | `workbook.json + xl/worksheets/sheetN.json` | `presentation.json + ppt/slides/*.json` |

**共同架构**（三者一致，是整合的基础）：

- 核心库：serde 数据模型 → 直接拼/解析 OOXML（quick-xml + zip），零 Office 运行时依赖；
- CLI：clap 4 derive，无状态设计，位置参数输入，对二进制文档的写操作强制 `-o` 不回写原件，TempGuard 自动清理临时解包；
- "产物目录"中间表示：unpack/repack 双向，agent 直接读写 JSON 文件；
- agent 工具面：view 两档省 token 视图、`edit get/set/add/remove --prop k=v`、`--prop` 树形路径寻址（1 基索引）、render 目视验证闭环、schema/templates 命令、skill 技能包（SKILL.md + references/cli.md + schema.json + templates/）；
- 测试分层：模型回环 → 真实语料回归 → CLI 子进程 e2e → 视觉 diff 脚本；
- 错误消息统一中文带修复建议，依赖树极轻（无 tokio/thiserror/anyhow）。

## 二、问题清单

### A. 真实缺陷（含数据丢失风险）

| # | 问题 | 位置 |
|---|---|---|
| A1 | `raw-set` 实现忽略 `-o`，直接原地覆写输入 xlsx；而 `skill/references/cli.md:177` 声称"`-o` 必填、不覆盖原文件"，文档与实现相反 | ai-excel `cli/src/main.rs:196-207` → `src/raw.rs:114` |
| A2 | clap 显示名与 bin 名不一致，`--help` 头部显示错误名字（aiworld / json2pptx） | ai-word `cli/src/main.rs:18`、ai-ppt `cli/src/main.rs:30` |
| A3 | `schema` 命令指向不存在的 `docs/JSON格式规范.md`；`templates` 命令硬编码 3 个模板名，与实际 16 套模板库脱节，对 agent 是主动误导 | ai-excel `cli/src/main.rs:239-248` |
| A4 | 10 处 `to_str().unwrap()` 处理路径，非 UTF-8 路径直接 panic | ai-word `cli/src/main.rs`（6 处）、`product.rs:135`、`mcp.rs:145,172`、`serve.rs:80` |
| A5 | `@id=N` 选择器是死代码——模型根本不保存形状 id，文档也声明不支持 | ai-ppt `cli/src/path.rs:88-97` |

### B. CLI 风格不一致（本次统一的核心）

| # | 问题 | 现状 |
|---|---|---|
| B1 | stdout 契约不干净：中文状态行与 pretty JSON 混在 stdout（如 ai-word `edit -o` 先打"已生成:"再打 JSON，`cli/src/main.rs:237-239`），均无 stderr 日志通道、无 `--json/--quiet` 全局开关，agent 解析需文本嗅探 | 三项目共有 |
| B2 | 退出码体系不一：ai-excel 仅 0/1；ai-word/ppt 0/1/2；ai-word repack 有 validate issues 仍退出 0 | `ai-word cli/src/main.rs:170-174` |
| B3 | view 模式名不一：world/ppt `text\|layout`，excel `values\|structure` | — |
| B4 | 模板提取命令位置不一：world 顶层 `extract`；excel 嵌套 `template extract`；ppt 无 | — |
| B5 | render 语义不一：world `--format html\|pdf\|png`；excel 仅 HTML；ppt `--engine browser\|native` PNG | — |
| B6 | excel `--prop` 别名面过宽且 camelCase/snake_case 混用（`wrapText` 与 `wrap_text` 并存，`cli/src/ops.rs:304-398`）；ppt 高级元素类型 camelCase（`progressBar` 等 10 个）与全局 snake_case 约定冲突 | — |
| B7 | `schema`/`templates` 命令三处均为硬编码字符串，与权威 schema.json / 模板库双源漂移 | 三项目共有 |
| B8 | 能力不对齐：excel 有 `raw/raw-set`（OPC 部件读写，格式无关）另两家没有；ppt 有 `validate/dump/query` 另两家没有；world 独有 `merge/serve/mcp` | — |

### C. 工程与文档

| # | 问题 |
|---|---|
| C1 | 三个项目各自为独立 Cargo workspace；工具代码重复（TempGuard、默认输出名推导、错误枚举、zip 封装、图片下载）；依赖版本漂移（clap 4.6.1/4.6.7、serde 1.0.228/1.0.229） |
| C2 | schema 三处手工同步（Rust 模型 / skill/references/schema.json / ai-ppt 前端 TS 类型），ppt Fill 类型已落后于模型 4 种填充 |
| C3 | 文档漂移普遍：README 命令表缺 merge/mcp/serve/import/raw 等、命令数与测试数多处矛盾（ai-word 8/7 vs 实际 11；ai-ppt 31/23/39 vs 实际约 52）、ai-ppt README.zh 落后 README、ai-excel SKILL.md"不支持 URL 图片"与实现相反、ppt cli.md 引用 `../schema.json` 相对路径解析不到 |
| C4 | ~~项目命名混乱：目录 `ai-world`（笔误）/ crate `json2docx` / bin `aiworld-cli` / clap `aiworld` / gitee 仓库 `ai-word`——一个项目 5 个名字~~ **已修正**：目录更名 `ai-word`，bin 与 clap 统一为 `json2docx` |

## 三、已确认的方向（2026-10-02 决策记录）

1. **整合形态**：根 Cargo workspace + 共享 core crate（`office-core`），三个 CLI 二进制保留，各自命令树保留格式特有扩展。
2. **命名**：bin 与 clap name 统一为 `json2docx` / `json2xlsx` / `json2pptx`（与核心库 crate 同名，功能自描述，三者成体系）。
3. **输出契约**：状态行改走 stderr；新增全局 `--json/--quiet/--verbose`；stdout 永远只有数据，可被 jq 直接解析。
4. **Schema 单一来源**：引入 schemars 从 serde 类型自动生成 JSON Schema；ai-ppt 前端 TS 类型从生成的 schema 派生。

## 四、实施阶段概要

- **Phase 0**：落盘本报告；修 A1–A5 正确性缺陷（低风险先行）。
- **Phase 1**：根 workspace 收编三项目；新建 `office-core`（TempGuard / 默认输出名 / OPC 封装 / 统一输出层 OutputCtx / 退出码约定）；bin 改名并全仓替换引用；依赖版本统一。
- **Phase 2**：CLI 契约统一——全局 flag 与 stdout/stderr/退出码契约（`docs/cli-conventions.md`）；子命令集对齐（公共核心 `unpack/repack/view/edit/render/validate/dump/schema/templates/extract/raw/raw-set`，格式特有保留）；view 模式统一 `text|layout`；`--prop` 别名收敛 snake_case；ppt 元素类型 snake_case 化（serde alias 保旧兼容）；`templates` 读机器可读索引。
- **Phase 3**：schemars 接入，`schema --json` 运行时生成，schema.json 变为生成物；ai-ppt 前端 TS 类型生成；文档/skill 全面同步；根 CI（fmt/clippy/test/schema 一致性检查）+ 根 README。

**退出码约定（统一后）**：`0` 成功；`1` 运行时错误；`2` clap 用法错误（框架默认）；`3` 产物有 validate issues（repack 非阻塞告警）。

**验收标准**：workspace 全量测试通过（三项目 roundtrip/corpus/CLI e2e 不改语义）；`--json` 下每条命令 stdout 可被 `jq` 解析、错误含结构化 code/hint；schema 命令输出与提交的 schema.json 零漂移（CI 强制）；skill 文档中每条命令/flag 均能在 `--help` 中找到。
