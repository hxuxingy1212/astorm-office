#!/usr/bin/env bash
# 收集真实世界数据集（CSV/JSON），用于场景报表与回归。
# 产物：/tmp/datasets_raw/<name>.<ext>
# 用法：./scripts/collect_datasets.sh
set -uo pipefail
OUT=/tmp/datasets_raw
mkdir -p "$OUT"

# get <name> <url>
get() {
  local name="$1"
  local url="$2"
  local ext="${3:-csv}"
  local f="$OUT/$name.$ext"
  [ -s "$f" ] && { echo "  have  $name"; return 0; }
  local code
  code=$(curl -sL --retry 3 --retry-delay 2 --retry-all-errors --max-time 90 \
         -w '%{http_code}' -o "$f" "$url" 2>/dev/null)
  if [ "$code" = "200" ] && [ -s "$f" ]; then
    lines=$(wc -l < "$f" | tr -d ' ')
    echo "  ok    $name ($(wc -c <"$f" | tr -d ' ') bytes, $lines lines)"
  else
    rm -f "$f"; echo "  FAIL  $name (http $code)"
  fi
}

echo "== seaborn-data =="
SB=https://raw.githubusercontent.com/mwaskom/seaborn-data/master
get tips            $SB/tips.csv
get flights         $SB/flights.csv
get titanic         $SB/titanic.csv
get iris            $SB/iris.csv
get diamonds        $SB/diamonds.csv
get penguins        $SB/penguins.csv
get planets         $SB/planets.csv
get fmri            $SB/fmri.csv

echo "== vega-datasets =="
VG=https://raw.githubusercontent.com/vega/vega-datasets/main/data
get cars            $VG/cars.json json
get stocks          $VG/stocks.csv
get gapminder       $VG/gapminder.json json
get seattle-weather $VG/seattle-weather.csv
get movies          $VG/movies.json json
get population      $VG/population.json json
get sp500           $VG/sp500.csv
get weather         $VG/weather.csv

echo "== fiveThirtyEight =="
FT=https://raw.githubusercontent.com/fivethirtyeight/data/master
get college-majors  $FT/college-majors/recent-grads.csv
get airline-safety  $FT/airline-safety/airline-safety.csv
get nba-salaries    $FT/nba-salaries/nba_salaries.csv

echo "== UCI =="
get uci-iris        https://archive.ics.uci.edu/ml/machine-learning-databases/iris/iris.data
get uci-wine        https://archive.ics.uci.edu/ml/machine-learning-databases/wine/wine.data

echo "== World Bank (JSON) =="
WB=https://api.worldbank.org/v2
get wb-gdp          "$WB/country/CN;US;JP;DE;IN;BR/indicator/NY.GDP.MKTP.CD?format=json&per_page=1000" json
get wb-pop          "$WB/country/CN;US;JP;DE;IN;BR/indicator/SP.POP.TOTL?format=json&per_page=1000" json
get wb-life         "$WB/country/CN;US;JP;DE;IN;BR/indicator/SP.DYN.LE00.IN?format=json&per_page=1000" json

echo "== done -> $OUT =="
ls -la "$OUT" 2>/dev/null | tail -n +2
