# 场景配方：财务模型（financial-model）

三表联动模型（假设 → 利润表 → 资产负债/现金流）。接到"做个财务模型/预算表/盈利预测"时按本配方组织，其余命令细节见 [`../references/cli.md`](../references/cli.md)。

## 结构约定

| 工作表 | 内容 | 要点 |
|--------|------|------|
| `假设` | 驱动参数：增长率、毛利率、税率、营运资本天数 | 唯一手工输入区；淡黄底纹提示可编辑（`fill=FFF2CC`） |
| `利润表` | 收入 → 毛利 → 费用 → EBITDA → 净利润 | 全部公式引用假设表，不写死数字 |
| `资产负债表` | 资产 = 负债 + 权益 | 校验行：`资产-负债-权益` 应为 0 |
| `现金流` | 经营/投资/筹资 | 期末现金链接到资产负债表货币资金 |

## 生成要点

1. **公式链**：收入 `=上期收入*(1+假设!$B$2)`；成本 `=收入*假设!$B$3`。跨表引用用 `假设!B2` 形式。
2. **数字格式**：金额 `#,##0`；比例 `0.0%`；倍数 `0.0x`。单元格 `type=number` + `number_format`。
3. **冻结窗格**：每张表 `set /sheet[N] freeze=B2`（首列表头 + 首行）。
4. **表头样式**：深色底白字 `fill=1F4E79 color=FFFFFF bold=true`。
5. **汇总行**：`SUM` 范围上方留一行；合计行加 `border.top=thin` + `bold=true`。
6. **迭代**：多步修改用 `batch`（dump 一份当前状态做备份，改错可回放还原）：
   ```bash
   json2xlsx dump model.xlsx > backup.json
   json2xlsx batch model.xlsx --input-file fix.json --force
   ```
7. **校验**：交付前 `validate` + `view '/sheet[利润表]' text` 抽查公式回显。

## 属性速查

不确定某个属性是否支持时先查 `json2xlsx help cell --json`（`--json` 输出机器可读能力表，与实现同源，勿凭记忆猜属性名）。
