# office-cli —— `astorm-office` 统一 CLI

`astorm-office` 是本仓库**四个格式 CLI 的统一入口**：按格式参数选路，把命令原样转发给
`json2docx` / `json2xlsx` / `json2pptx` / `json2pdf`，并提供批量执行。

它**不重新实现任何文档能力**——没有文档解析、排版或转换代码；只做三件事：
格式 → 二进制的映射、二进制的定位、命令的转发与批处理编排。
因此它的行为与直接调用底层 CLI 完全等价（含 `--json`、`-o`、退出码）。

## 用法

```text
astorm-office <格式> <命令...>        # 转发给对应底层 CLI
astorm-office <命令...>               # 省略格式：从参数里的文件扩展名/目录名推断
astorm-office formats [--json]        # 格式别名表 + 各 CLI 的解析路径（诊断）
astorm-office batch <脚本|-> [...]     # 批量执行多条命令（文本或 JSON 脚本）
```

格式别名（大小写不敏感）：

| 格式 | 别名 | 底层 CLI | 覆盖扩展名 |
|---|---|---|---|
| word | `word` `docx` `doc` `docs` | `json2docx` | `.docx` `.doc` `.docm` |
| excel | `excel` `xlsx` `xls` `sheet` `sheets` | `json2xlsx` | `.xlsx` `.xls` `.xlsm` `.csv` `.tsv` |
| ppt | `ppt` `pptx` `slides` `presentation` | `json2pptx` | `.pptx` `.ppt` |
| pdf | `pdf` | `json2pdf` | `.pdf` |

也接受底层二进制名当格式写（`astorm-office json2pdf …`）。

```bash
astorm-office word unpack thesis.docx -o prod/     # → json2docx unpack …
astorm-office word view prod/ / text               # → 整篇 Markdown（view text）
astorm-office excel import data.csv -o book.xlsx   # → json2xlsx import …
astorm-office ppt view deck.pptx / layout          # → json2pptx view …
astorm-office pdf extract text r.pdf --format markdown
astorm-office view thesis.docx / text              # 省略格式：.docx → json2docx
astorm-office view samples/xlsx / text             # 产物目录以格式命名时也能推断
astorm-office -f ppt view deck.pptx / text         # 显式指定（--format/-f）
```

## 批量执行

```bash
astorm-office batch steps.txt                      # 文本脚本：一行一条，# 注释
astorm-office batch steps.json                     # JSON 脚本
astorm-office batch - < steps.txt                  # stdin
astorm-office batch --commands '<json 或文本串>'
```

```text
# steps.txt —— 一个脚本里可以混排四种格式（每行就是一条完整命令）
word repack samples/docx -o out.docx
word view out.docx / text
excel import data.csv -o book.xlsx
pdf extract text report.pdf --format markdown
```

```json
{ "version": 1, "commands": [
  { "format": "word", "args": ["unpack", "thesis.docx", "-o", "prod/"] },
  { "args": ["view", "prod/", "/", "text"] },
  { "format": "ppt", "args": ["view", "deck.pptx", "/", "text"], "continue_on_error": true },
  "pdf qa report.pdf"
]}
```

- 每步的 `format` 可省（从 `args` 推断）；字符串步骤按 shell 风格切词（支持引号）。
- 默认**停在第 1 个失败处**；`--force` 或步骤级 `"continue_on_error": true` 可继续。
- `--dry-run` 只解析与定位，不执行；`--json` 捕获各步输出并输出一份报告。
- 不做隐式依赖：步骤之间通过文件/产物目录传递结果，与逐条手敲命令完全等价。

报告（`--json`）：

```json
{
  "ok": true, "total": 2, "succeeded": 2, "failed": 0, "duration_ms": 42,
  "steps": [
    { "index": 1, "format": "word", "bin": "/abs/bin/json2docx", "source": "sibling",
      "args": ["unpack", "thesis.docx", "-o", "prod/"], "ok": true, "exit_code": 0,
      "stdout": "…", "stderr": "…" }
  ]
}
```

默认模式（不加 `--json`）：子命令的 stdout/stderr **直通**你的终端，逐步状态与汇总走 stderr。

## 二进制定位

按顺序：`ASTORM_JSON2DOCX` / `ASTORM_JSON2XLSX` / `ASTORM_JSON2PPTX` / `ASTORM_JSON2PDF`
环境变量 → **与自己同目录**（官方 `office-kit` 的 `bin/` 布局）→ `PATH`。
全都找不到时报错并给出三条出路（装 kit / 设环境变量 / 本地 `cargo build --release -p <x>-cli`）。
`astorm-office formats` 会显示每个格式解析到的实际路径与来源。

## 退出码

- 转发模式：**原样透传**子进程退出码（`0/1/2/3`，其中 3 = 产物有校验问题）。
- 批量模式：全部成功 `0`；任一步失败 `1`；用法错误 `2`（clap）。

## 开发

```bash
cargo test -p astorm-office-cli        # 单元测试（别名/推断/脚本解析）+ 集成测试（桩 CLI 验证分发与批处理）
cargo build --release -p astorm-office-cli
```
