#!/usr/bin/env bash
# 真实语料回归：从 GitHub 下载 docx 语料（Apache POI / python-docx 测试文件），
# 逐个 unpack → repack，校验 XML 合法性与文本回环保真度。
# 需要网络；语料下载到 /tmp/docx_corpus（不纳入版本库）。
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="$(pwd)/target/release/json2docx"
cargo build --release --quiet

CORPUS=/tmp/docx_corpus
if [ ! -d "$CORPUS/poi" ]; then
python3 - <<'PY'
import json, urllib.request, os
def api(url):
    req=urllib.request.Request(url, headers={'User-Agent':'curl'})
    with urllib.request.urlopen(req, timeout=60) as r: return json.load(r)

# python-docx
base="https://api.github.com/repos/python-openxml/python-docx/contents"
os.makedirs('/tmp/docx_corpus/pythondocx', exist_ok=True)
files=[]
def walk(path):
    items=api(f"{base}/{path}")
    for it in items:
        if it['type']=='file' and it['name'].lower().endswith('.docx'): files.append(it['download_url'])
        elif it['type']=='dir': walk(it['path'])
walk("tests/test_files")
for u in files:
    try: open(f"/tmp/docx_corpus/pythondocx/{u.split('/')[-1]}",'wb').write(urllib.request.urlopen(u,timeout=30).read())
    except Exception as e: print("dl err",e)

# Apache POI（抽样）
items=api("https://api.github.com/repos/apache/poi/contents/test-data/document")
docx=[x for x in items if x['type']=='file' and x['name'].lower().endswith('.docx')]
kw=['table','image','footnote','header','toc','list','style','hyperlink','merge','column','chart','comment','revision','field','nested','sample','simple']
picked=[]; seen=set()
for k in kw:
    for x in docx:
        if k in x['name'].lower() and x['name'] not in seen: picked.append(x); seen.add(x['name']); break
for x in docx:
    if len(picked)>=40: break
    if x['name'] not in seen: picked.append(x); seen.add(x['name'])
os.makedirs('/tmp/docx_corpus/poi', exist_ok=True)
for x in picked:
    try: open(f"/tmp/docx_corpus/poi/{x['name']}",'wb').write(urllib.request.urlopen(x['download_url'],timeout=60).read())
    except Exception as e: print("dl err",x['name'],e)
print("corpus ready")
PY
fi

python3 - "$BIN" <<'PY'
import os, subprocess, sys, glob, zipfile, difflib, re
import xml.etree.ElementTree as ET
BIN=sys.argv[1]
def text(p):
    try:
        r=subprocess.run(['textutil','-convert','txt','-stdout',p],capture_output=True,text=True,timeout=60)
        return r.stdout if r.returncode==0 else None
    except Exception: return None
def xml_ok(p):
    try:
        z=zipfile.ZipFile(p)
        for n in z.namelist():
            if n.endswith(('.xml','.rels')): ET.fromstring(z.read(n))
        return True
    except Exception as e: return str(e)
files=sorted(glob.glob('/tmp/docx_corpus/*/*.docx'))
bad=0
for f in files:
    out='/tmp/co_prod'; outp='/tmp/co_out.docx'
    subprocess.run(['rm','-rf',out])
    u=subprocess.run([BIN,'unpack',f,'-o',out],capture_output=True,text=True)
    if u.returncode!=0: print("UNPACK-ERR",os.path.basename(f),u.stderr[:100]); bad+=1; continue
    r=subprocess.run([BIN,'repack',out,'-o',outp],capture_output=True,text=True)
    if r.returncode!=0: print("REPACK-ERR",os.path.basename(f),r.stderr[:100]); bad+=1; continue
    xo=xml_ok(outp)
    if xo is not True: print("XML-BAD",os.path.basename(f),xo); bad+=1; continue
    t0=text(f); t1=text(outp)
    if t0 is not None and t1 is not None:
        a=re.sub(r'\s+','',t0); b=re.sub(r'\s+','',t1)
        ratio=difflib.SequenceMatcher(None,a,b).ratio() if (a or b) else 1.0
        if ratio<0.9: print(f"LOW {ratio:.2f}", os.path.basename(f))
print(f"corpus={len(files)} problems={bad}")
PY
