#!/usr/bin/env bash
# 收集更多开源项目的 .xlsx 测试夹具（“案例”），用于解析/回环回归。
# 网络不稳时走 codeload tarball（比 git 协议稳），只解包需要的子路径。
# 产物：/tmp/xlsx_more/<source>/*.xlsx（扁平化）
# 用法：./scripts/collect_corpus.sh [source...]   （默认全部）
set -uo pipefail
DEST=/tmp/xlsx_more
STAGE=/tmp/xlsx_more_src
mkdir -p "$DEST" "$STAGE"

extract() { # <tgz> <dst> <subpath...>  (bash 3.2 安全：不使用空数组)
  local tgz="$1" dst="$2"; shift 2
  rm -rf "$dst"; mkdir -p "$dst"
  if [ $# -gt 0 ]; then
    local pats=""
    local p; for p in "$@"; do pats="$pats */$p */$p/*"; done
    # shellcheck disable=SC2086
    tar -xzf "$tgz" -C "$dst" --strip-components=1 --wildcards $pats 2>/dev/null \
      || tar -xzf "$tgz" -C "$dst" --strip-components=1 2>/dev/null
  else
    tar -xzf "$tgz" -C "$dst" --strip-components=1 2>/dev/null
  fi
}

# fetch_tarball <name> <owner/repo> <branch...> [-- subpath...]
fetch_tarball() {
  local name="$1" repo="$2"; shift 2
  local brs=""
  while [ $# -gt 0 ] && [ "$1" != "--" ]; do brs="$brs $1"; shift; done
  [ $# -gt 0 ] && shift  # drop --
  local dst="$STAGE/$name"
  [ -f "$dst/.done" ] && { echo "  have  $name"; return 0; }
  local tgz="/tmp/_dl_${name}.tgz" br code
  for br in $brs; do
    code=$(curl -sL --retry 3 --retry-delay 3 --retry-all-errors --max-time 240 \
           -w '%{http_code}' -o "$tgz" \
           "https://codeload.github.com/$repo/tar.gz/refs/heads/$br" 2>/dev/null)
    [ "$code" = "200" ] && break
    echo "  ..    $name/$br http=$code"
  done
  if [ "${code:-}" != "200" ]; then echo "  FAIL  $name"; return 1; fi
  extract "$tgz" "$dst" "$@"
  rm -f "$tgz"; touch "$dst/.done"
}

harvest() { # <name>
  local name="${1:-}" dir="$STAGE/${1:-}" out="$DEST/${1:-}"
  [ -n "$name" ] && [ -d "$dir" ] || return 0
  mkdir -p "$out"; local n=0
  while IFS= read -r f; do
    cp "$f" "$out/$(printf '%s' "$f" | sed "s|$dir/||; s|/|__|g")" && n=$((n+1))
  done < <(find "$dir" -type f \( -iname '*.xlsx' -o -iname '*.xlsm' \) -size -3M 2>/dev/null)
  echo "  ok    $name: $n files"
}

run() { # <source>
  case "$1" in
  phpspreadsheet) fetch_tarball phpspreadsheet   PHPOffice/PhpSpreadsheet  master -- tests/data; harvest phpspreadsheet ;;
  pandas)         fetch_tarball pandas           pandas-dev/pandas         main -- pandas/tests/io/data/excel; harvest pandas ;;
  calamine)       fetch_tarball calamine         tafia/calamine            main master -- tests; harvest calamine ;;
  umya)           fetch_tarball umya             MathNya/umya-spreadsheet  master -- tests; harvest umya ;;
  sheetjs)        fetch_tarball sheetjs          SheetJS/test_files        master -- test_files; harvest sheetjs ;;
  poi)            fetch_tarball poi              apache/poi                trunk master -- test-data/spreadsheet; harvest poi ;;
  libxlsxwriter)  fetch_tarball libxlsxwriter    jmcnamara/libxlsxwriter   main -- test; harvest libxlsxwriter ;;
  exceljs)        fetch_tarball exceljs          exceljs/exceljs           master -- spec; harvest exceljs ;;
  xlsxwriter)     fetch_tarball xlsxwriter       jmcnamara/XlsxWriter      main -- xlsxwriter/test; harvest xlsxwriter ;;
  poiji)          fetch_tarball poiji            ozlerhakan/poiji          master -- src/test/resources; harvest poiji ;;
  openpyxl)       fetch_tarball openpyxl         theorchard/openpyxl       main master -- openpyxl/tests; harvest openpyxl ;;
  rust_xlsxwriter) fetch_tarball rust_xlsxwriter jmcnamara/rust_xlsxwriter main -- tests; harvest rust_xlsxwriter ;;
  epplus)         fetch_tarball epplus           EPPlusSoftware/EPPlus     develop8 develop -- sample; harvest epplus ;;
  *) echo "  ?     unknown source $1" ;;
  esac
}

if [ $# -eq 0 ]; then
  set -- phpspreadsheet pandas calamine umya sheetjs poi libxlsxwriter exceljs \
         xlsxwriter poiji openpyxl rust_xlsxwriter epplus
fi
for s in "$@"; do run "$s"; done

echo "=== totals ==="
for d in "$DEST"/*/; do [ -d "$d" ] && printf "%-18s %s\n" "$(basename "$d")" "$(find "$d" -type f | wc -l | tr -d ' ')"; done
echo "TOTAL: $(find "$DEST" -type f \( -iname '*.xlsx' -o -iname '*.xlsm' \) | wc -l | tr -d ' ')"
