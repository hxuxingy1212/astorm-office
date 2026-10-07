//! 学术论文示例：Agent 风格——写一份内联 document.json，再 repack 成 docx
//!
//! 运行：cargo run --example paper

use serde_json::json;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new("examples/out/paper_inline");
    std::fs::create_dir_all(dir)?;

    let document = json!({
      "meta": {
        "title": "基于大模型的多模态检索方法研究",
        "author": "张三",
        "subject": "计算机科学",
        "keywords": ["大模型", "多模态", "信息检索"]
      },
      "page": { "size": "A4", "margins": { "top": 72, "bottom": 72, "left": 90, "right": 90 } },
      "theme": {
        "major_font": "Times New Roman", "minor_font": "Times New Roman",
        "east_asia_font": "宋体", "heading_font": "黑体"
      },
      "template": "academic-paper",
      "parts": [
        { "blocks": [
          { "type": "heading", "level": 1, "text": "摘要" },
          { "type": "paragraph", "text": "本文提出了一种基于大模型的多模态检索方法，通过统一表征与跨模态对齐，在多个基准上取得了领先效果。" },
          { "type": "paragraph", "runs": [
            { "text": "关键词：", "bold": true },
            { "text": "大模型；多模态；信息检索" }
          ] },
          { "type": "toc", "title": "目录", "levels": [1, 2, 3] }
        ] },
        { "blocks": [
          { "type": "heading", "level": 1, "text": "第一章 绪论" },
          { "type": "paragraph", "runs": [
            { "text": "随着大语言模型的发展，跨模态信息检索成为研究热点。" },
            { "text": "（相关综述见脚注）", "footnote": "Zhang et al. 2024 综述了多模态检索的进展。" }
          ] },
          { "type": "heading", "level": 2, "text": "1.1 研究背景" },
          { "type": "paragraph", "text": "传统方法难以统一建模文本、图像与音频等异构模态。" },
          { "type": "list", "ordered": true, "items": [
            { "blocks": [ { "type": "paragraph", "text": "提出统一的多模态表征框架" } ] },
            { "blocks": [ { "type": "paragraph", "text": "设计跨模态对齐损失" } ] },
            { "blocks": [ { "type": "paragraph", "text": "在多个基准上验证有效性" } ] }
          ] },
          { "type": "formula", "latex": "L = L_ret + \\lambda L_align", "display": true, "number": "(1)" },
          { "type": "paragraph", "text": "其中第一项为检索损失，第二项为对齐损失。" }
        ] },
        { "blocks": [
          { "type": "heading", "level": 1, "text": "第二章 方法" },
          { "type": "table", "header_row": true, "widths": [150, 150, 150],
            "caption": "各方法在基准上的表现",
            "rows": [
              { "cells": [
                { "blocks": [ { "type": "paragraph", "text": "方法", "align": "center" } ] },
                { "blocks": [ { "type": "paragraph", "text": "R@1", "align": "center" } ] },
                { "blocks": [ { "type": "paragraph", "text": "R@5", "align": "center" } ] }
              ] },
              { "cells": [
                { "blocks": [ { "type": "paragraph", "text": "Baseline" } ] },
                { "blocks": [ { "type": "paragraph", "text": "72.1" } ] },
                { "blocks": [ { "type": "paragraph", "text": "88.4" } ] }
              ] },
              { "cells": [
                { "blocks": [ { "type": "paragraph", "text": "Ours" } ] },
                { "blocks": [ { "type": "paragraph", "text": "79.6" } ] },
                { "blocks": [ { "type": "paragraph", "text": "92.3" } ] }
              ] }
            ] },
          { "type": "code", "lang": "python", "text": "def align(x, y):\n    return cosine_similarity(x, y).mean()" }
        ] },
        { "blocks": [
          { "type": "bibliography", "style": "GB/T 7714", "title": "参考文献", "entries": [
            { "kind": "article", "authors": ["张三", "李四"], "title": "多模态信息检索综述", "container": "计算机学报", "year": 2024 },
            { "kind": "inproceedings", "authors": ["Wang, L."], "title": "Unified Multimodal Retrieval", "container": "NeurIPS", "year": 2023 }
          ] }
        ] }
      ]
    });

    let doc_path = dir.join("document.json");
    std::fs::write(&doc_path, serde_json::to_string_pretty(&document)?)?;

    let out = "examples/out/paper.docx";
    let r = json2docx::repack(dir.to_str().unwrap(), out)?;
    println!("已生成 {} ({} 个分片)", r.path, r.parts);
    Ok(())
}
