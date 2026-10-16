# 资源导出与失败报告

Word、Excel、PPT 的 `view ... text --attachments DIR --media DIR` 共用 `office_core::PayloadSink`。
PDF 使用 `extract attachment ... -o DIR`。加 `--json` 可读取完整导出清单。

```bash
json2docx view contract.docx / text --attachments ./attachments --media ./media --strict --json
json2xlsx view report.xlsx / text --media ./media --strict --json
json2pptx view slides.pptx / text --attachments ./attachments --strict --json
json2pdf extract attachment report.pdf -o ./attachments --strict --json
```

## 容错与严格模式

默认尝试整批资源，失败时保留 Markdown 中的原始引用，清单记录具体错误，命令继续返回成功。
`--strict`（别名 `--strict-exports`）在本批完成后检查错误：有任何失败则返回退出码 **1**，
stdout 仍保留 Markdown 或完整 JSON 清单，成功导出的文件仍然保留。严格模式需指定导出目录。
它不回滚成功文件，也不覆盖目录内已有文件；同名目标自动使用 `name-2.ext` 等名字，创建目标时采用排他创建。

严格失败的 JSON 同时包含 `ok: false` 和 `error.code: "resource_export_failed"`。
`export_summary` 提供本批资源数、成功落盘数、失败数和 `strict` 标志。
HTTP(S) 链接原样保留，不请求远端内容，不算严格失败；其大小与哈希未知。

## 清单字段

Word/Excel/PPT 的附件位于 `attachments[]`，媒体位于 `media_exported[]`；PDF 的附件位于 `attachments[]`。

| 字段 | 含义 |
| --- | --- |
| `resource_id` | `resource-` 加 SHA-256，标识来源文档及载荷身份，不随导出目录或同名避让后的文件名改变；不是内容哈希 |
| `resource_type` | MIME 类型：优先使用格式解析器提供的类型，否则按文件扩展名推断；未知为 `application/octet-stream` |
| `name` / `src` | 资源展示名及原始载荷引用（包内路径、本地路径、URL、PDF 对象标识等） |
| `source_document` | 来源输入文件/产物目录；调用库时未提供上下文则为 `null` |
| `source_locations` | 所有已遇到的模型位置；相同载荷重复引用只导出一次，并累积位置 |
| `kind` | `file` 已落盘；`url` 原样保留；`missing` 载荷不存在；`error` 其他失败 |
| `path` | 成功落盘后的目标路径，保持已有接口的路径表达；失败及 URL 无此字段 |
| `relative_link` | 相对**本次指定导出目录**的文件名 URI，空格、`#`、`%`、非 ASCII 等按 UTF-8 百分号编码；未知为 `null` |
| `size` | 成功时为实际写出的字节数；失败时如已知则为源文件大小，否则 `null` |
| `sha256` | 实际写出内容的完整 SHA-256 小写十六进制；未成功落盘时为 `null` |
| `error` | 成功/URL 时为 `null`，失败时为下述结构化原因 |

`relative_link` 不假定 stdout 将保存到哪里；若 Markdown 文件保存在其他目录，调用方需按实际目录组合链接。
成功清单中的 `size` 和 `sha256` 在复制时计算，不读取另一份缓存或附件声明的大小。

来源位置示例：

- Word：`/part[1]/block[2]`，嵌套表格可为 `/part[1]/block[3]/row[1]/cell[2]/block[1]`，行内图片带 `run[N]`。
- Excel：`/sheet[1]/image[2]/cell[D4]`，OLE 带 `ole[N]` 及可用的锚点单元格。
- PPT：`/slide[2]/element[3]`，分组内元素继续带 `element[N]`，内嵌部件带 `part[N]`。
- PDF：`/attachment[1]/pdf-obj-12-0`，产物目录附件使用 `/attachment[1]`。

库调用未提供模型位置时，`source_locations` 使用载荷原始引用作为回退。

## 错误结构

```json
{
  "code": "create_dir_failed",
  "operation": "create_dir",
  "message": "文件系统返回的错误说明",
  "io_kind": "NotADirectory",
  "os_code": 20,
  "target": "./blocked/attachments"
}
```

| `error.code` | 含义 |
| --- | --- |
| `payload_missing` | 源文件不存在或格式中的载荷流/载荷引用不存在，唯一使用 `kind: missing` 的错误 |
| `source_read_failed` | 源文件无法读取，或引用的源不是普通文件 |
| `create_dir_failed` | 输出目录创建失败 |
| `copy_failed` | 文件载荷的目标创建、复制或写入失败 |
| `write_failed` | 内存字节载荷的目标创建或写入失败 |
| `invalid_name` | 导出名为空、是 `.`/`..` 或包含目录分隔符、驱动器分隔符、NUL |
| `invalid_payload` | PPT base64 或 PDF 附件流解码失败 |

`operation` 标识具体失败步骤；`io_kind`、`os_code`、`target` 在适用时提供，其他情况为 `null`。
`target` 是该操作涉及的路径（源读取操作为源文件，目录/写入操作为输出目标）。
失败的半文件会尝试删除；已有目标不会被截断。权限或文件系统阻止清理时，调用方可根据错误中的目标路径检查。

## 库调用

```rust
use office_core::PayloadSink;

let sink = PayloadSink::new("./attachments")
    .with_root("./product")
    .with_source_document("contract.docx")
    .with_strict(true);
{
    let _location = sink.at_source("part[1]/block[2]");
    let link = sink.link_for("appendix.pdf", "word/embeddings/appendix.pdf");
    // link_for 容错并记录错误；try_link_for 返回 Result<String, ExportError>。
}
let manifest = sink.entries();
sink.check()?; // 严格模式汇总本批错误为 ExportBatchError；默认模式返回 Ok。
```

`try_link_for_bytes[_typed]` 支持内存载荷，`link_for_base64` 支持 PPT 包内 base64 载荷。
`PayloadSinks::at_source` 为嵌套渲染器提供作用域定位，`annotate_report` 将统计及严格失败状态加入现有 JSON 信封。
