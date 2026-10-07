#!/usr/bin/env python3
"""批量 round-trip 评测：对每个 PDF 运行 ai-pdf unpack → repack，记录结果。"""
import concurrent.futures as cf
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

BIN = "/Users/xuxin/Desktop/workspace/ai-pdf/target/release/ai-pdf"
CORPUS = Path("/tmp/pdf-eval/corpus")
WORK = Path("/tmp/pdf-eval/work")
RESULTS = Path("/tmp/pdf-eval/results.jsonl")
MAX_SIZE = 25 * 1024 * 1024
TIMEOUT = 90


def err_type(msg: str) -> str:
    m = msg.lower()
    if "encrypted" in m:
        return "encrypted"
    if "no xref" in m or "startxref" in m or "unrecognized" in m or "not a pdf" in m or "failed to parse" in m:
        return "malformed"
    if "timed out" in m or "timeout" in m:
        return "timeout"
    if "out of range" in m or "object not found" in m or "invalid object" in m:
        return "broken-object"
    if "font" in m:
        return "font"
    if "page" in m:
        return "page-error"
    return "other"


def run_one(pdf: Path) -> dict:
    rel = str(pdf.relative_to(CORPUS))
    source = rel.split(os.sep)[0]
    size = pdf.stat().st_size
    rec = {"file": rel, "source": source, "size": size,
           "unpack": "error", "repack": "-", "error": None, "images": 0,
           "elements": 0, "warnings": 0, "secs": 0.0}
    t0 = time.time()
    tag = hashlib.sha1(rel.encode()).hexdigest()[:12]
    art = WORK / tag / "art"
    try:
        r = subprocess.run([BIN, "unpack", str(pdf), "-o", str(art)],
                           capture_output=True, text=True, timeout=TIMEOUT)
        secs = time.time() - t0
        if r.returncode != 0:
            msg = (r.stdout + r.stderr).strip()
            m = re.search(r'"message":\s*"([^"]+)"', msg)
            rec["error"] = m.group(1)[:200] if m else msg[:200]
            rec["error_type"] = err_type(msg)
            rec["secs"] = round(secs, 2)
            cleanup(tag)
            return rec
        rec["unpack"] = "ok"
        # 统计元素数
        for pj in art.rglob("page-*.json"):
            try:
                data = json.loads(pj.read_text())
                rec["elements"] += len(data.get("elements", []))
            except Exception:
                pass
        summary = art / ".." / "summary.json"
        rebuilt = WORK / tag / "rebuilt.pdf"
        r2 = subprocess.run([BIN, "repack", str(art), "-o", str(rebuilt)],
                            capture_output=True, text=True, timeout=TIMEOUT)
        if r2.returncode != 0:
            msg = (r2.stdout + r2.stderr).strip()
            m = re.search(r'"message":\s*"([^"]+)"', msg)
            rec["repack"] = "error"
            rec["error"] = m.group(1)[:200] if m else msg[:200]
            rec["error_type"] = err_type(msg)
        else:
            try:
                s = json.loads(r2.stdout)
                rec["images"] = s.get("images", 0)
                rec["warnings"] = len(s.get("warnings", []))
                rec["repack_fonts"] = len(s.get("fonts", []))
            except Exception:
                pass
            rec["repack"] = "ok"
            rec["rebuilt"] = str(rebuilt)
        rec["secs"] = round(time.time() - t0, 2)
    except subprocess.TimeoutExpired:
        rec["error"] = f"timeout {TIMEOUT}s"
        rec["error_type"] = "timeout"
        rec["secs"] = TIMEOUT
    except Exception as e:
        rec["error"] = str(e)[:200]
        rec["error_type"] = "other"
    cleanup(tag)
    return rec


def cleanup(tag: str):
    import shutil
    shutil.rmtree(WORK / tag, ignore_errors=True)


def main():
    WORK.mkdir(parents=True, exist_ok=True)
    pdfs = sorted([p for p in CORPUS.rglob("*") if p.suffix.lower() == ".pdf"
                   and p.stat().st_size <= MAX_SIZE and p.stat().st_size > 100])
    print(f"corpus: {len(pdfs)} PDFs", flush=True)
    done = 0
    with open(RESULTS, "w") as out, cf.ThreadPoolExecutor(max_workers=8) as pool:
        futures = {pool.submit(run_one, p): p for p in pdfs}
        for fut in cf.as_completed(futures):
            rec = fut.result()
            out.write(json.dumps(rec, ensure_ascii=False) + "\n")
            out.flush()
            done += 1
            if done % 50 == 0:
                print(f"{done}/{len(pdfs)}", flush=True)
    print("done", flush=True)


if __name__ == "__main__":
    main()
