#!/usr/bin/env bash
# Full-feature end-to-end test for json2xlsx.
# Builds the CLI, generates a kitchen-sink workbook via the JSON-first product
# directory, repacks it, validates structurally (parse roundtrip, openpyxl,
# xmllint), then renders an HTML preview and screenshots it with headless
# Chrome for visual (image-recognition) verification.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/debug/json2xlsx"
WORK="${TMPDIR:-/tmp}/json2xlsx_e2e"
VENV_PY="/var/folders/j_/k9qc2jcs6tx95938m619pgrc0000gn/T/opencode/xlsxvenv/bin/python"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

pass() { echo "  PASS  $1"; }
fail() { echo "  FAIL  $1"; exit 1; }

echo "== build =="
cargo build -q -p json2xlsx

echo "== prepare product dir =="
rm -rf "$WORK"
mkdir -p "$WORK/xl/worksheets" "$WORK/assets"
# recognizable logo (PNG via Pillow, else tiny GIF fallback)
PYBIN="$VENV_PY"; [ -x "$PYBIN" ] || PYBIN=python3
IMG="assets/logo.png"
if ! "$PYBIN" - "$WORK/assets/logo.png" <<'PY' 2>/dev/null
import sys
from PIL import Image, ImageDraw
img = Image.new("RGB", (160, 64), (68, 114, 196))
d = ImageDraw.Draw(img)
d.rectangle([2, 2, 157, 61], outline=(255, 255, 255), width=3)
d.text((58, 26), "LOGO", fill=(255, 255, 255))
img.save(sys.argv[1])
PY
then
  IMG="assets/logo.gif"
  "$PYBIN" - "$WORK/assets/logo.gif" <<'PY'
import sys
data=bytes([0x47,0x49,0x46,0x38,0x39,0x61,0x01,0x00,0x01,0x00,0x80,0x00,0x00,0x00,0x00,0x00,0xFF,0xFF,0xFF,0x21,0xF9,0x04,0x00,0x00,0x00,0x00,0x00,0x2C,0x00,0x00,0x00,0x00,0x01,0x00,0x01,0x00,0x00,0x02,0x02,0x44,0x01,0x00,0x3B])
open(sys.argv[1],'wb').write(data)
PY
fi

cat > "$WORK/workbook.json" <<'JSON'
{
  "meta": { "title": "Kitchen Sink", "author": "ai-excel" },
  "date1904": false,
  "styles": {
    "header": { "font": { "bold": true, "color": "FFFFFF" }, "fill": "4472C4", "alignment": { "horizontal": "center" } },
    "money":  { "number_format": "#,##0" }
  },
  "named_ranges": [ { "name": "Rate", "ref": "Data!$C$1" } ],
  "sheets": [ "xl/worksheets/sheet1.json", "xl/worksheets/sheet2.json" ]
}
JSON

cat > "$WORK/xl/worksheets/sheet1.json" <<JSON
{
  "name": "Data",
  "tab_color": "22AA22",
  "freeze": "A2",
  "gridlines": false,
  "zoom": 120,
  "auto_filter": "A1:C4",
  "filter_columns": [
    { "col_id": 0, "values": ["Q1", "Q2"] },
    { "col_id": 2, "operator": "greaterThan", "criteria": "100" }
  ],
  "columns": [ { "width": 18 }, { "width": 12 }, { "width": 12 } ],
  "print_area": "A1:C4",
  "print_title_rows": "1:1",
  "protect": true,
  "validations": [ { "range": "A2:A4", "type": "list", "values": ["Q1", "Q2", "Q3"], "allow_blank": true } ],
  "conditional_formats": [
    { "range": "C2:C4", "type": "cellIs", "operator": "greaterThan", "formulas": ["5"], "font": { "bold": true, "color": "FF0000" } },
    { "range": "B2:B4", "type": "colorScale", "colors": ["63BE7B", "FFEB84", "F8696B"] },
    { "range": "B2:B4", "type": "iconSet", "icon_style": "3TrafficLights1", "show_value": true }
  ],
  "tables": [ { "range": "A1:C4", "name": "Sales", "style": "TableStyleMedium9", "show_banded_rows": true, "columns": ["Quarter", "Amount", "Growth"] } ],
  "images": [ { "src": "$IMG", "anchor": "E2", "size": { "w": 2.5, "h": 1 } } ],
  "charts": [
    { "type": "column", "data_range": "B2:B4", "categories": "A2:A4", "title": "Amount", "legend": "none", "show_values": true, "colors": ["4472C4"], "anchor": "E6", "size": { "w": 6, "h": 4 } },
    { "type": "pie", "data_range": "B2:B4", "categories": "A2:A4", "title": "份额", "anchor": "K6", "size": { "w": 5, "h": 4 } }
  ],
  "rows": [
    { "index": 1, "cells": [
      { "ref": "A1", "value": "Quarter", "style": "header" },
      { "ref": "B1", "value": "Amount", "style": "header" },
      { "ref": "C1", "value": "Growth", "style": "header" } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "Q1" },
      { "ref": "B2", "value": 120, "number_format": "#,##0" },
      { "ref": "C2", "value": 5.5, "number_format": "0.0%" } ] },
    { "index": 3, "cells": [
      { "ref": "A3", "value": "Q2" },
      { "ref": "B3", "value": 180, "number_format": "#,##0" },
      { "ref": "C3", "value": -2.1, "number_format": "0.0%" } ] },
    { "index": 4, "cells": [
      { "ref": "A4", "value": "Q3", "comment": "下一季度" },
      { "ref": "B4", "value": 260, "number_format": "#,##0" },
      { "ref": "C4", "value": 9.0, "number_format": "0.0%" },
      { "ref": "D4", "value": "官网", "link": "https://example.com", "link_tooltip": "打开官网" },
      { "ref": "D5", "type": "richtext", "value": "重点 说明", "runs": [ { "text": "重点 ", "bold": true, "color": "FF0000", "underline": "single" }, { "text": "说明", "vert_align": "superscript" } ] } ] }
  ]
}
JSON

cat > "$WORK/xl/worksheets/sheet2.json" <<'JSON'
{
  "name": "Summary",
  "gridlines": false,
  "zoom": 120,
  "rows": [
    { "index": 1, "cells": [
      { "ref": "A1", "value": "Total" },
      { "ref": "B1", "formula": "SUM(Data!B2:B4)", "number_format": "#,##0" } ] },
    { "index": 3, "cells": [
      { "ref": "A3", "type": "date", "value": "2026-01-31" },
      { "ref": "B3", "value": true } ] }
  ]
}
JSON

echo "== repack (product -> xlsx) =="
"$BIN" repack "$WORK" -o "$WORK/book.xlsx" | sed 's/^/  /'
[ -f "$WORK/book.xlsx" ] && pass "xlsx generated"

echo "== unpack -> repack roundtrip =="
"$BIN" unpack "$WORK/book.xlsx" -o "$WORK/rt" >/dev/null
"$BIN" repack "$WORK/rt" -o "$WORK/rt.xlsx" >/dev/null
"$BIN" view "$WORK/rt" /sheet[1] structure | grep -q '"tables"' && pass "structure view works"

echo "== xmllint well-formedness =="
TMPX="$(mktemp -d)"; unzip -qq "$WORK/book.xlsx" -d "$TMPX"
bad=0
while IFS= read -r f; do xmllint --noout "$f" 2>/dev/null || { echo "  BAD $f"; bad=1; }; done \
  < <(find "$TMPX" -name '*.xml' -o -name '*.rels' -o -name '*.vml')
rm -rf "$TMPX"
[ $bad -eq 0 ] && pass "all OOXML parts well-formed"

echo "== structural validation (openpyxl) =="
PYBIN="$VENV_PY"; [ -x "$PYBIN" ] || PYBIN=python3
if "$PYBIN" -c "import openpyxl" 2>/dev/null; then
"$PYBIN" - "$WORK/book.xlsx" <<'PY' || fail "openpyxl assertions"
import sys, warnings, openpyxl
warnings.simplefilter("error")
wb = openpyxl.load_workbook(sys.argv[1])
assert wb.sheetnames == ["Data", "Summary"], wb.sheetnames
ws = wb["Data"]
assert ws["B2"].value == 120
assert ws["A4"].comment.text == "下一季度"
assert ws["D4"].hyperlink.target == "https://example.com"
assert list(ws.tables.keys()) == ["Sales"]
assert len(ws.data_validations.dataValidation) == 1
assert len(list(ws.conditional_formatting)) == 2
assert len(ws._images) == 1 and len(ws._charts) == 2
assert ws.auto_filter.ref == "A1:C4"
assert ws.protection.sheet is True
assert wb["Summary"]["B1"].value == "=SUM(Data!B2:B4)"
print("  PASS  openpyxl read all features")
PY
else
  echo "  SKIP  openpyxl not available"
fi

echo "== render HTML + screenshot =="
"$BIN" render "$WORK" -o "$WORK/preview.html" >/dev/null
[ -f "$WORK/preview.html" ] && pass "preview.html written"
PNG="$WORK/preview.png"
if [ -x "$CHROME" ]; then
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars \
    --window-size=1200,1500 --screenshot="$PNG" "file://$WORK/preview.html" >/dev/null 2>&1
  [ -s "$PNG" ] && pass "screenshot: $PNG"
  ls -la "$PNG"
else
  echo "  SKIP  Chrome not found"
fi

echo
echo "E2E OK. Artifacts in $WORK"
