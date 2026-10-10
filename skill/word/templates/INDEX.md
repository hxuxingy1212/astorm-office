# 存量风格模板库

风格样式模板（`template.json` + `TEMPLATE.md`）：多数由 `json2docx extract` 从**真实 Word 文档**提取；
`resume` / `contract` / `meeting-minutes` 为**内置场景模板**（按体裁约定编写）。
每个模板含 `page`（页面）、`theme`（字体/配色）、`styles`（Normal/HeadingN/Title… 段落样式）。

## 用法

1. 选定模板，读取其 `template.json`；
2. 把 `page` / `theme` / `styles` 合并进新文档的 `document.json`（用户显式字段优先）；
3. `json2docx repack mydoc/ -o mydoc.docx` 生成；`render` 预览核对。

```jsonc
// mydoc/document.json
{
  "page":   { /* template.json.page */ },
  "theme":  { /* template.json.theme */ },
  "styles": { /* template.json.styles */ },
  "parts":  [ { "blocks": [ { "type": "heading", "level": 1, "text": "标题" } ] } ]
}
```

> 也可对**用户上传的任意 docx** 运行 `json2docx extract that.docx -o tpl/` 即时得到其风格模板再套用。


## 功能样例（samples）

面向**具体功能**的最小可复现蓝图（单文件 `document.json`，内联 `parts`），位于 `../samples/`：

| 样例 | 内容 | 生成 |
|------|------|------|
| `samples/sdt.json` | 内容控件（SDT）：下拉框/富文本/行内 | `json2docx repack <dir> -o sdt.docx` |
| `samples/form.json` | 表单域（text/checkbox/dropdown）+ 域（DATE） | 同上 |
| `samples/chart.json` | 富图表：堆叠柱状图（图例/轴标题/数据标签/系列色）+ 多系列折线 | 同上 |
| `samples/merge.json` | 模板占位符 `{{key}}`（段落/表格/形状） | `json2docx merge <dir> -d '{"key":"v"}' -o out.docx` |

```bash
# 示例：合并填充
mkdir -p /tmp/merge && cp skill/samples/merge.json /tmp/merge/document.json
json2docx merge /tmp/merge -d '{"title":"Q4 对账单","client":"Acme","total":"5,200","item":"服务费","amount":"5,200","date":"2026-01-01","note":"已开票"}' -o out.docx
```

> 另见 `scripts/gen_template_samples.py` 生成的 `feature-showcase.docx`（综合演示盒式边框/制表位前导/富图表/表格宽度/SDT/表单域/域/注音/RTL）。

## 模板清单


| 名称 | 领域/风格 | 纸张 | 正文 | 标题 |
|------|-----------|------|------|------|
| `chinese-sci-numbered` | 中文 SCI 论文（标题编号） | A4 | Times/宋体 12，2 倍行距，首行缩进 | H1 18 粗居中/黑体，H2 16，H3 14 |
| `chinese-sci` | 中文 SCI 论文（不编号） | A4 | Times/宋体 12，2 倍行距 | H1 18 粗居中/黑体 |
| `academic-cambria` | 英文学术 / 报告（Cambria） | Letter | Cambria 12 | H1 16 粗 #345A8A，H2–H6 #4F81BD |
| `modern-aptos` | 现代默认（Aptos，Office 新默认） | - | Aptos 12 | H1 20 #0F4761，H2 16，H3 14… |
| `academic-arial` | 英文学术 / essay（Arial） | Letter | Arial 11 | Heading1 14 粗 |
| `classic-times` | 学术 / 通用（Times New Roman） | A4 | Times New Roman 11 | - |
| `classic-palatino` | 书籍 / 经典报告（Palatino） | - | Times New Roman 12 | Heading1 Palatino 居中 |
| `sans-trebuchet` | 现代友好（Trebuchet MS） | - | Verdana 9 | H1 Trebuchet 24 / H2 16 |
| `chinese-modern` | 中文现代报告（微软雅黑） | A4 | 微软雅黑 10.5 | H1 22，H2 16，H3 15… |
| `chinese-official` | 中文公文（仿宋_GB2312） | A4 | 仿宋_GB2312 16，2.3 倍行距 | - |
| `resume` | 简历 / CV | A4 | Calibri/微软雅黑 10.5 | H1 姓名 22 粗居中 #2E74B5 |
| `contract` | 商业合同 / 协议 | A4 | Times/宋体 12，1.5× | H1 标题 16 粗居中 |
| `meeting-minutes` | 会议纪要 | A4 | Calibri/微软雅黑 11 | H1 18 粗居中 #1F4E79 |
| `chinese-letter` | 中文信函（明朝体） | Letter | MS Mincho 11 | - |
| `japanese-serif` | 日文（游明朝 / 游ゴシック） | A4 | 游明朝 | 游ゴシック Light |
| `design-helvetica` | 设计 / 现代（Helvetica） | A4 | Helvetica 13.5 #333 | H1 13.5 粗 #5B9BD5 |
| `report-cambria` | 商务报告（Cambria） | A4 | Cambria 11 | - |
| `corporate-arial` | 企业文档（Arial 12） | A4 | Arial 12 | - |
| `technical-arial` | IT / 技术文档（Arial 10） | A4 | Arial 10 | - |
| `modern-calibri` | 现代通用（Calibri） | Letter | Calibri 11 | - |
| `french-letter` | 欧洲商务信函（Calibri） | Letter | Calibri 11 | - |

## 建议场景映射

| 用户意图 | 推荐模板 |
|----------|----------|
| 中文 SCI 论文 / 学位论文 | `chinese-sci-numbered` |
| 英文学术论文 / essay | `academic-cambria` 或 `academic-arial` / `classic-times` |
| 中文报告 / 方案 | `chinese-modern` |
| 中文公文 / 通知 | `chinese-official` |
| 日文文档 | `japanese-serif` |
| 商务报告 / 分析 | `report-cambria` 或 `modern-calibri` |
| 企业制度 / 方案 | `corporate-arial` |
| 技术 / 接口文档 | `technical-arial` |
| 设计 / 品牌文档 | `design-helvetica` |
| 正式信函 | `french-letter` |

## 语料来源（模板提取自）

共采集 **1431 份**真实 docx（去重；覆盖中/英/日文，学术/公文/商务/技术/设计等场景），全部通过
`unpack → repack` 回环与 XML 合法性校验。主要来源：

| 来源 | 约计数 | 说明 |
|------|--------|------|
| `open-xml-templating/docxtemplater` | 168 | Word 模板引擎测试文档 |
| `jgm/pandoc`（含 Gitee 镜像） | 266 | Pandoc docx 读写测试套件 |
| `microsoft/Open-XML-SDK`（Gitee 镜像） | 419 | OpenXML SDK 测试文档（多字体/表格/图表/批注等） |
| `Sayi/poi-tl`（含 Gitee 镜像） | 234 | Java Word 模板引擎测试（中文字体/公文风格） |
| `plutext/docx4j`（含 Gitee 镜像） | 226 | 文档处理样例 |
| `python-openxml/python-docx`（含 Gitee 镜像） | 91 | python-docx 测试夹具 |
| `bokuweb/docx-rs` | 73 | docx-rs 测试夹具（含日文） |
| `elapouya/python-docx-template` | 40 | 模板引擎测试 |
| `Achuan-2/pandoc_docx_template` | 1 | 中文 SCI 论文样式 |

> 重新采集：`./scripts/collect_docs.sh`（仓库 zip / Gitee 镜像，去重汇集）→
> `./scripts/collect_templates.sh`（批量提取）。语料为第三方测试文档，仅本地用于风格提取，不纳入版本库。
