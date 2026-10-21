# view text 演示物料（四格式同一份内容）

同一份「2026 年度经营复盘」在四种格式里的物料，用于对照 **`view text` 的结构化输出**：
标题层级、列表嵌套、表格、公式、图表数据、分组层次在纯文本里各自长什么样。

| 物料 | 来源 | 结构看点 |
|---|---|---|
| `demo.docx` | `src/word/` repack | 一级/二级标题、段落、无序列表（含嵌套项）、带题注的表（表头行）、原生图表、显示公式、引用 |
| `demo.pptx` | `src/ppt/` repack | 4 页：封面 / 项目符号列表 / 数据表 / 大字 KPI + 原生图表 |
| `demo.xlsx` | `src/excel/` repack | 合并标题单元格、表头行、月度数据、`=SUM(...)` 公式行、冻结与自动筛选 |
| `demo.pdf` | `demo.docx` 经 `json2pdf convert office` | 版式重建（标题/表格/列表识别）与元素级寻址两套读法 |

`src/` 下是可编辑的产物目录（JSON）：改 JSON 后 `repack` 即可再生成二进制物料。

## 读法

```bash
# Word：整篇 Markdown（分片之间带 <!-- part N --> 锚点）
json2docx view demo.docx / text

# PPT：整本 Markdown（每页一个 ## 幻灯片 N 小节）；单页把 / 换成 /slide[2]
json2pptx view demo.pptx / text

# Excel：整本 Markdown（每表一个 ## 表名 小节）；单表 /sheet[1]，区域 /sheet[1]/range[A1:D4]
json2xlsx view demo.xlsx / text

# PDF：元素级读法（每项带 CLI 路径，供 edit 用；输出 JSON）
json2pdf view demo.pdf '/page[1]' text
# PDF：整篇结构化 Markdown（标题/列表/表格识别；JSON 包，取纯文本见下）
json2pdf extract text demo.pdf --format markdown
```

统一入口等价写法：`astorm-office word view demo.docx / text` 等；
`astorm-office view demo.docx / text` 会按扩展名自动分发。

`view … text` 的 stdout 就是 Markdown 原文，可以直接重定向落盘：

```bash
json2docx view demo.docx / text > demo.md     # 直接得到 .md
json2docx view demo.docx / text --json        # 结构化信封（含 media/issues/warnings）
```

PDF 的 `extract` 输出恒为 JSON 包（每页带 complexity/OCR 路由信息），取纯文本用 `| jq -r '.pages[].text'`。

## 性能提示（实测，release，本机 M 系列）

| 用法 | 耗时 |
|---|---|
| 小件 `view … text`（三格式） | 5~7 ms（其中 ~4 ms 是进程启动） |
| 大 docx（896 KB Markdown 输出） | 61 ms（二进制输入）/ **4.3 ms**（产物目录） |
| 大 pptx（300 页） | 35 ms（二进制输入） |
| 大 xlsx（2 万行） | 233 ms（大头是解析工作簿 160 ms） |

**同一文档要反复读，先 `unpack` 成产物目录再反复 `view`**——实测快 7 倍以上
（二进制输入每次都要重新解析并落临时目录；产物目录只需加载 JSON）。
`astorm-office batch` 里同理：把 `unpack` 放第一步，后面所有步骤都读产物目录。

## 已知行为

- **公式**：Excel 单元格有公式时，纯文本视图显示 `=SUM(B3:B8)`（与 Excel 的公式视图一致），而不是计算值；
  数字格式（如 `0.0%`）不在纯文本里套用，需要"所见即所得"时用 `render -o preview.html`。
- **图表**：原生图表（Word/PPT）输出为「分类 × 系列」数据表；矢量坐标轴文字会混在 PDF 文本流里。
- **PDF 提取**：表格行会同时出现在表格与正文流中（去重是近似策略）；`extract` 的段落/列表/标题是**版式重建**的结果，不是 PDF 里原有的语义。
