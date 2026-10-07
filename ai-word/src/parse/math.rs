//! OMML → LaTeX（用于公式回环）

use crate::parse::xmltree::XmlNode;

/// 将 OMML 节点（m:oMath / m:oMathPara）转为 LaTeX
pub fn omml_to_latex(node: &XmlNode) -> String {
    let mut s = String::new();
    emit(node, &mut s);
    s.trim().to_string()
}

fn emit(n: &XmlNode, out: &mut String) {
    match n.name.as_str() {
        "m:oMath" | "m:oMathPara" | "m:box" | "m:groupChr" => {
            for c in &n.children {
                emit(c, out);
            }
        }
        "m:e" | "m:num" | "m:den" | "m:sub" | "m:sup" | "m:deg" => {
            for c in &n.children {
                emit(c, out);
            }
        }
        "m:r" => out.push_str(&run_text(n)),
        "m:t" => out.push_str(&n.text_content()),
        "m:sSup" => {
            let (base, sup) = scripts(n);
            out.push_str(&format!("{}^{{{}}}", base, sup));
        }
        "m:sSub" => {
            let (base, sub) = scripts(n);
            out.push_str(&format!("{}_{{{}}}", base, sub));
        }
        "m:sSubSup" => {
            let base = child_str(n, "m:e");
            let sub = child_str(n, "m:sub");
            let sup = child_str(n, "m:sup");
            out.push_str(&format!("{}_{{{}}}^{{{}}}", base, sub, sup));
        }
        "m:f" => {
            let num = child_str(n, "m:num");
            let den = child_str(n, "m:den");
            out.push_str(&format!("\\frac{{{}}}{{{}}}", num, den));
        }
        "m:rad" => {
            let body = child_str(n, "m:e");
            out.push_str(&format!("\\sqrt{{{}}}", body));
        }
        "m:d" => {
            let beg = n
                .child("m:dPr")
                .and_then(|p| p.child("m:begChr"))
                .and_then(|c| c.attr("m:val"))
                .unwrap_or("(")
                .to_string();
            let end = n
                .child("m:dPr")
                .and_then(|p| p.child("m:endChr"))
                .and_then(|c| c.attr("m:val"))
                .unwrap_or(")")
                .to_string();
            let inner = child_str(n, "m:e");
            out.push_str(&format!("{}{}{}", beg, inner, end));
        }
        "m:nary" => {
            let chr = n
                .child("m:naryPr")
                .and_then(|p| p.child("m:chr"))
                .and_then(|c| c.attr("m:val"))
                .unwrap_or("∑")
                .to_string();
            let cmd = match chr.as_str() {
                "∫" => "\\int",
                "∏" => "\\prod",
                "∬" => "\\iint",
                "∮" => "\\oint",
                _ => "\\sum",
            };
            let sub = child_str(n, "m:sub");
            let sup = child_str(n, "m:sup");
            let body = child_str(n, "m:e");
            out.push_str(&format!("{}_{{{}}}^{{{}}} {}", cmd, sub, sup, body));
        }
        _ => {
            for c in &n.children {
                emit(c, out);
            }
        }
    }
}

fn child_str(n: &XmlNode, name: &str) -> String {
    n.child(name)
        .map(|c| {
            let mut s = String::new();
            emit(c, &mut s);
            s
        })
        .unwrap_or_default()
}

/// sSup：返回 (base, script)；sSub：返回 (base, script)
fn scripts(n: &XmlNode) -> (String, String) {
    let base = child_str(n, "m:e");
    let script = child_str(n, "m:sup") + &child_str(n, "m:sub");
    (base, script)
}

fn run_text(r: &XmlNode) -> String {
    let mut out = String::new();
    for c in &r.children {
        if c.name == "m:t" {
            out.push_str(&c.text_content());
        }
    }
    out
}
