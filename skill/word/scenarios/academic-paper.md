# 场景配方：学术论文（academic-paper）

中文学术论文/学位论文结构。接到"写论文/期刊投稿/毕业论文"时按本配方组织，写作规范见 [`../references/writing-guide.md`](../references/writing-guide.md)，版式见 [`../references/design-guide.md`](../references/design-guide.md)。

## 结构约定

| 部分 | 块类型 | 要点 |
|------|--------|------|
| 标题页 | `heading level=1` + `paragraph` | 标题/作者/单位/摘要/关键词 |
| 摘要 | `paragraph style=Abstract` | 200–300 字；关键词 3–5 个分号分隔 |
| 章节 | `heading level=1/2/3` + `numbering=true` | 1 / 1.1 / 1.1.1 三级 |
| 公式 | `formula` | `display=true` + `number=(1)` 按章编号如 (2-1) |
| 图表 | `image` + `caption of=image` | 题注"图 1-1 ×××"，表格用 `table` + 表头行 |
| 参考文献 | `bibliography` | GB/T 7714 风格；正文引用 `[1]` 上标 |
| 附录 | `heading level=1` | 大表格/代码放附录 |

## 生成要点

1. **模板**：优先 `academic-paper` 内置模板（宋体正文/黑体标题/Times New Roman 西文），再按学校要求微调字号行距。
2. **公式**：LaTeX 写法，行内 `display=false`，独立成行 `display=true`：
   ```bash
   json2docx edit paper/ '/part[1]' add --type formula \
     --prop latex='\int_{-\infty}^{+\infty} e^{-x^2}\,dx=\sqrt{\pi}' --prop display=true
   ```
3. **交叉引用**：题注用 `caption of=<图片名>`，正文引用写"如图 1-1 所示"；编号由 caption 顺序保证。
4. **参考文献**：`bibliography` 块的 `entries` 数组（authors/title/container/year…），不要手写编号。
5. **精准修改**：老论文补改用 `@paraId=` 稳定寻址（unpack 自原始 docx 时保留），插删段落不漂移：
   ```bash
   json2docx unpack thesis.docx -o thesis/
   json2docx edit thesis/ '/part[1]/paragraph[@paraId=1A2B3C4D]' set --prop text=修正后的表述
   ```
6. **批量修订**：多处修改（如全部"本文"改"本研究"之外的结构调整）用 `batch` 一次回放；改错可用此前 `dump` 的指令回放还原。
7. **校验交付**：`validate`（悬空样式/缺失图片）→ `render /` 出 HTML 通读 → `repack` 交付 docx。
