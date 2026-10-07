# astorm-office CLI 统一契约

四个 CLI（`json2docx` / `json2xlsx` / `json2pptx` / `json2pdf`）遵循同一套命令风格、I/O 契约与退出码约定。
本文档是 agent 与脚本调用这些 CLI 的权威依据；各 CLI 的格式特有命令见各自的 `skill/references/cli.md`。

## 一、命令集

四个 CLI 的公共核心命令（语义一致）：

| 命令 | 语义 | stdout（默认） | stdout（--json） |
|---|---|---|---|
| `unpack <input>` | 文档 → 产物目录（`--inline` 输出单个自包含 JSON，见下） | 空 | `{"ok":true,"output":...}` |
| `repack <dir>` | 产物目录 → 文档（自动校验） | 空 | `{"ok":true,"output":...,"issues":[...]}` |
| `view <input> <path> <text\|layout>` | 省 token 的两档只读视图 | 数据 JSON | 数据 JSON |
| `edit <input> <path> get\|set\|add\|remove` | 精准修改 | 数据 JSON | 数据 JSON |
| `render <input> [path]` | 目视验证（HTML/PNG/PDF） | 空 | `{"ok":true,"output":...,"format":...}` |
| `validate <input>` | 结构校验 | `{"issues":[...],"count":N}` | 同左 |
| `dump <input>` | 可回放的编辑指令（batch JSON） | `{"version":1,"commands":[...]}` | 同左 |
| `extract <input> [--name X]` | 从存量文档提取风格模板 | 空 | `{"ok":true,"output":...}` |
| `raw <input> <part>` | 读取 OPC 部件（字节原样） | 部件字节 | 部件字节 |
| `raw-set <input> <part> --file F` | 替换 OPC 部件（`-o` 必填，**不覆盖原文件**） | 空 | `{"ok":true,"replaced":...,"output":...}` |
| `schema` | JSON Schema 指引（--json 时输出机器可读 schema） | 指引文本 | schema JSON |
| `templates` | 内置模板/风格列表 | 格式化列表 | `{"templates":[...]}` |

> **`json2pdf` 的覆盖范围**：公共核心命令中实现 `unpack` / `repack` / `view` / `edit` /
> `render` / `validate` / `dump` / `batch` / `serve` / `mcp` / `schema` / `help`
> （语义与契约一致；寻址首段为 `/page[N]`，元素段按类型计数如 `/page[1]/text[2]`，
> group 可下钻；无 @id/@name 稳定寻址——PDF 模型无 name/id 元数据，索引制寻址）。
> 未实现：`extract`（模板提取语义；PDF 的 `extract text/table/image` 是内容提取，见
> 格式特有命令）、`raw` / `raw-set`（OPC 部件概念不适用于 PDF）、`templates`（PDF 暂无
> 内置模板）、`merge`（{{key}} 填充暂不提供，占位符可直接 edit 替换）。

新增公共命令（四 CLI 语义一致）：

| 命令 | 语义 | stdout | 退出码 |
|---|---|---|---|
| `batch <input> --input-file F\|--commands '<json>'` | 回放 dump 风格指令；**默认遇错即停**，`--force` 跳过错误继续 | 报告 JSON `{"ok","applied","failed","steps":[{"index","op","path","ok","result"/"error","suggestion"}]}` | 任一步失败 → 1 |
| `help [元素]` | 能力速查（属性/别名/示例，能力表与实现同源） | 人读表格；`--json` 时机器可读能力 schema | 未知元素 → 1（带建议） |
| `merge <input> --data '<json>'` | `{{key}}` 占位符填充（xlsx/ppt 原地产物目录；docx 输出新文件） | 空；`--json` 结果 | 1（数据非法） |
| `serve <dir>` | 常驻编辑服务：stdin 逐行 JSON op（edit/view/batch/merge/issues/save/quit），逐行回 `{"ok",...}` | 逐行 JSON | 1（加载失败） |
| `mcp` | MCP 服务（stdio JSON-RPC 2.0，tools/list + tools/call） | 逐行 JSON-RPC | — |

> dump → batch 回放是**保真闭环**：对同一文档，`dump` 出的指令逐条回放后语义不变
> （xlsx 的 `add sheet` 同名幂等、cell 按 ref 幂等；ppt 的 `add` 按 `@id` 幂等原位替换）。

格式特有命令：

| CLI | 命令 |
|---|---|
| `json2docx` | `serve`（常驻编辑）、`mcp`（stdio JSON-RPC）、`merge`（{{key}} 填充 → 新 docx） |
| `json2xlsx` | `import`（CSV/TSV → xlsx）、`query`（sheets/sheet[N]/sheet[name=X]/cell:contains）、`serve`、`mcp` |
| `json2pptx` | `query`（类 CSS 选择器）、`serve`、`mcp` |
| `json2pdf` | `extract text/table/image`（内容提取）、`pages merge/split/rotate/crop/clean`（页操作）、`meta get/set/brand`（元数据）、`form info/fill`（AcroForm）、`convert office/html/latex`（Office→PDF 引擎链 auto=本机 LibreOffice→native 自研 / Chromium / tectonic → PDF）、`convert docx/pptx/xlsx`（PDF → Office，产物目录映射）、`palette generate/cascade` 与 `design svg/layout/derive/audit`（调色板与设计引擎）、`code/content sanitize`（清洗）、`check font/toc` 与 `qa`（质检，发现问题退出码 3）、`env check`（依赖探测）；点分写法（`extract.text`）兼容原 pdf.py 习惯 |

## 二、I/O 契约

1. **stdout 永远只有数据**：要么是单个 JSON 文档（可被 `jq` 直接解析），要么是原始字节（`raw`）。写盘类命令（unpack/repack/render/extract/raw-set/import）默认**不产生 stdout 输出**。
2. **stderr 承载状态**：`已解包/已生成/已渲染` 等中文状态行、进度与日志一律走 stderr。
3. **`--json`（全局）**：写盘命令改为向 stdout 输出结果 JSON（含 `ok`/`output` 及格式相关字段）；stderr 的状态变为 JSON 事件行（`{"event":"status","message":...}`）；错误变为 `{"ok":false,"error":{"code":...,"message":...,"hint":...}}`。
4. **`--quiet`（全局）**：抑制 stderr 状态输出（不影响 stdout 数据）。
5. **`--verbose`（全局）**：输出更详细的诊断信息。
6. **错误**：一律走 stderr。默认模式为一行中文（`错误: ...`，可附 `建议: ...`）；`--json` 模式为结构化 JSON `{"ok":false,"error":{"code","message","hint","suggestion"}}`。错误码：`json` / `io` / `zip` / `xml` / `invalid_input` / `image_load` / `encrypted`（密码保护的 OOXML/PDF；损坏的非加密包按 `zip`/`invalid_input` 归码）/ `internal`。
   - **suggestion（自愈建议）**：未知属性/类型/取值/索引越界类错误必须附带——最近匹配（`是否想用 "bold"？`）+ 合法取值/范围（`可选值: text/layout` 或 `索引范围 1~3`）。能力表来自 `help --json`，与实现同源（契约测试保证声明即可用）。
7. **无状态**：命令不共享隐藏状态。对二进制文档的写操作（edit set/add/remove、raw-set、import）**必须** `-o` 显式指定输出，绝不回写输入文件；对产物目录的 edit 则原地修改。

## 三、退出码

| 码 | 含义 |
|---|---|
| `0` | 成功 |
| `1` | 运行时错误（IO/解析/输入非法等） |
| `2` | 用法错误（clap 参数错误，框架默认） |
| `3` | 命令成功但产物存在 **error 级**校验问题（`json2docx`/`json2xlsx` 的 issues 均为 error 级；`json2pptx` 区分 `error`/`warning`/`info`，仅 `error` 影响退出码，warning/info 仍会报告；`json2pdf` 的 `check font/toc` 与 `qa` 发现问题时报告 JSON 仍输出到 stdout，退出码 3） |

## 四、公共约定

- 输入一律为**位置参数**，接受文档文件或产物目录（自动识别）。
- 输出路径一律 `-o/--output`（global，可放在任意位置）。
- `edit` 属性用 `--prop k=v`（可重复）；新增元素用 `--type T`。
- 路径语法为 1 基树形寻址：`/part[1]/paragraph[2]`（docx）、`/sheet[1]/cell[A1]`（xlsx）、`/slide[1]/text[2]`（pptx）。
- **稳定 ID 寻址**（多步编辑防索引漂移）：docx 段落 `paragraph[@paraId=1A2B3C4D]`（w14:paraId，unpack 自原始文档保留、repack 回写）；pptx 元素 `text[@id=5]` / `text[@name=标题]`（cNvPr/@id，`edit add` 回显分配的 id）。
- JSON 字段命名统一 **snake_case**（pptx 元素 `type` 值亦然；历史 camelCase 值仍可读入，但不再输出）。
- 单位：长度/字号 pt，颜色不带 `#`，行距用倍数，页面用 A4/Letter 等别名。
- 产品目录是 CLI 与 agent 的统一载体：`unpack` 产物 = JSON 文档 + 媒体目录，人机均可直接编辑。
- **`unpack --inline`**：输出**单个自包含 JSON**（默认 `<输入名>.inline.json`，`-o` 可指定）——
  顶层数组分片（slides/parts/sheets/pages）内联为对象、媒体引用转 data URI
  （`fonts/`、`embeddings/` 不内联：预览不消费，转了只是体积膨胀）。
  该形态可直接经 IPC/内存传给 `@astorm/office-viewer` 的 `data` prop（loader 与
  media resolver 对内联/data URI 形态直接支持），省去 1+N 次 fetch 与自定义协议；
  适合预览/交付，**编辑流程请继续用目录形态**（repack 只认目录）。
  注意：百万行级大表内联成单文件会抬高内存峰值，超大文档建议仍走目录 + 按需 fetch。
- **外部引擎可用环境变量显式指定**（探测顺序在最前）：
  `ASTORM_CHROME_BIN`（`render png`/`render pdf` 的无头 Chrome，桌面端可传自带
  Chromium/Electron helper）、`ASTORM_SOFFICE_PATH`（`convert` 的 LibreOffice 路径）。

## 五、迁移说明（相对旧版）

- bin 改名：`aiworld-cli` → `json2docx`；`json2xlsx-cli` → `json2xlsx`；`json2pptx-cli` → `json2pptx`；`ai-pdf`（独立仓库）→ `json2pdf`（并入本仓库时统一命名）。
- `json2pdf` 契约迁移：原 pdf.py 风格（所有命令 stdout 输出 `{"status":"success",...}` / 错误 JSON）→ 本契约（stdout 只出数据、状态走 stderr、`--json` 全局开关、`-o` 全局输出、`check/qa` 发现问题退出码 3）。
- 状态行从 stdout 移到 stderr；写盘命令默认不再向 stdout 打印内容（需要机器可读结果时加 `--json`）。
- `json2xlsx` 的 view 模式更名：`values` → `text`、`structure` → `layout`（旧值仍兼容）。
- `json2xlsx` 的 `template extract` 提升为顶层 `extract`（旧写法兼容）。
- `raw-set` 现在严格遵循 `-o`（此前实现会原地覆写输入文件，属缺陷）。
- `repack`/`validate` 发现校验问题时退出码为 3（此前为 0）。
