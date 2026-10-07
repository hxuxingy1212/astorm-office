# 学术写作规范（writing-guide）

写论文内容时遵循本规范；排版细节见 [design-guide.md](design-guide.md)。

## 1. 结构骨架

```
标题（Title）
作者 / 单位
摘要（Abstract）        —— 独立成段，200–300 字，无首行缩进
关键词（Keywords）      —— 3–6 个，分号或逗号分隔
目录（toc）             —— 3 个以上一级标题时添加
1 引言
2 相关工作
3 方法
4 实验
5 结论
参考文献（bibliography）
```

- 标题层级：`heading level 1` 为章，`level 2` 为节，一般不超过 3 级。
- 摘要不写引用、不放公式；关键词用名词短语。
- 每章开头一句话点明本章要解决的问题。

## 2. 引用格式

| 格式 | 正文 | 文献表 | 常见领域 |
|------|------|--------|----------|
| GB/T 7714 | 顺序编码 `[1]` | 引用顺序 | 中文期刊/学位论文 |
| IEEE | `[1]` | 首次引用顺序 | 工程 |
| APA 7 | `(Smith, 2024)` | 作者字母序 | 社科 |
| Chicago | 上标脚注 | 作者字母序 | 人文 |
| MLA 9 | `(Smith 412)` | 作者字母序 | 文学 |

统一用 `bibliography` 块生成，`style` 指定（如 `"GB/T 7714"`）。条目按作者、题名、出处、年填写：

```json
{ "type": "bibliography", "style": "GB/T 7714", "entries": [
  { "kind": "article", "authors": ["张三", "李四"], "title": "多模态检索综述",
    "container": "计算机学报", "year": 2024 },
  { "kind": "inproceedings", "authors": ["Wang, L."], "title": "Unified Multimodal Retrieval",
    "container": "NeurIPS", "year": 2023 }
] }
```

**规则**
- 正文每处引用都要在文献表中出现；顺序编码下编号与首次出现顺序一致。
- 图表引用写「如图 1 所示」「见表 2」，编号与题注一致。

## 3. 图、表与题注

- **题注位置铁律**：图题在图**下方**，表题在表**上方**。
- 编号连续：图 1、图 2…；表 1、表 2…。
- 表格用 `table` 块的 `caption`（自动置于表上方）；图片用 `image.caption`（置于图下方）或紧随的 `caption` 块。
- 图片必须给 `alt`（可访问性）。
- 表格首行用 `header_row: true`；数值列右对齐，文字列左对齐，表头居中。

## 4. 公式

- 行内变量、希腊字母、上下标统一用 `formula` 块，**不要**写成纯文本（如 `lambda_1`、`x_{t+1}`）。
- 独立公式用 `"display": true`，可给编号：

```json
{ "type": "formula", "latex": "L = L_{ret} + \\lambda L_{align}", "display": true, "number": "(1)" }
```

- 公式后接一句「其中…表示…」解释符号。

## 5. 语言与标点

- 中文正文用全角标点；中英文之间不空格；数字与单位间留半角空格（`12 pt` 或写在一起按期刊要求）。
- 首次出现的缩写给全称：多模态检索（Multimodal Retrieval, MMR）。
- 避免口语与情绪化表达；结论只陈述结果与局限。

## 5.5 可用块与字段

- 块类型（18 种）与字段以 `SKILL.md` 为准；常用：`heading`/`paragraph`(可 `runs` 行内混排)/`list`/`table`/`image`/`formula`/`chart`/`caption`/`toc`/`bibliography`/`code`/`quote`/`textbox`/`shape`/`attachment`/`sdt`/`raw`/`page_break`。
- 行内可用：通用域 `run.field`（`PAGE`/`REF`/`STYLEREF`/`MERGEFIELD`…）、表单域 `run.form_field`、注音 `run.ruby`、符号 `run.symbol`、RTL `run.rtl`/`lang`。
- 模板数据填充：正文/表格/形状/页眉页脚中的 `{{key}}` 可用 `json2docx merge` 批量替换。

## 6. 交付前自查

1. 结构完整（摘要/关键词/目录/正文/参考文献）。
2. 标题层级无跳级（不出现 H1 直接到 H3）。
3. 图表编号连续且与正文引用一致。
4. 公式非纯文本。
5. 每个正文引用都能在参考文献中找到。
6. `render` 后逐页确认无占位符、无溢出、图表题注位置正确。
