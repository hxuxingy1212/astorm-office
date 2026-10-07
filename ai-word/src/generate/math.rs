//! LaTeX（子集）→ OMML（Office Math Markup Language）
//!
//! 支持：上下标 `_` `^`、`\frac`、`\sqrt`、`\left...\right` 定界符、
//! 希腊字母与常用符号、`\sum` / `\int` 等 n 元算子、`\text{...}`、函数名。
//! 未覆盖的复杂命令会降级为文本。

use crate::utils::xml::{esc, esc_attr};

/// 生成 OMML（`m:oMath` 的内层内容）
pub fn latex_to_omml(latex: &str) -> String {
    let toks = tokenize(latex);
    let mut p = Parser { toks, pos: 0 };
    let node = p.parse_row(&[]);
    emit(&node)
}

/// 判定 LaTeX 是否为“简单文本”（无需结构化转换）
pub fn is_plain(text: &str) -> bool {
    !text.contains('\\') && !text.contains('^') && !text.contains('_') && !text.contains('{')
}

// ---------------- 词法 ----------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Cmd(String),
    Ident(char),
    Num(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Caret,
    Underscore,
    Other(char),
}

fn tokenize(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '{' => {
                out.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                out.push(Tok::RBrace);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
            }
            '^' => {
                out.push(Tok::Caret);
                i += 1;
            }
            '_' => {
                out.push(Tok::Underscore);
                i += 1;
            }
            '\\' => {
                // 命令：\name
                let start = i + 1;
                let mut j = start;
                if j < chars.len() && chars[j].is_ascii_alphabetic() {
                    while j < chars.len() && chars[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                } else if j < chars.len() {
                    j += 1; // 单字符命令，如 \{ \}
                }
                let name: String = chars[start..j].iter().collect();
                if !name.is_empty() {
                    out.push(Tok::Cmd(name));
                }
                i = j;
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                    j += 1;
                }
                out.push(Tok::Num(chars[i..j].iter().collect()));
                i = j;
            }
            _ if c.is_ascii_alphabetic() => {
                out.push(Tok::Ident(c));
                i += 1;
            }
            _ => {
                out.push(Tok::Other(c));
                i += 1;
            }
        }
    }
    out
}

// ---------------- 语法树 ----------------

#[derive(Debug, Clone)]
enum Node {
    Row(Vec<Node>),
    Sym(String),
    Num(String),
    Text(String),
    Sup(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    SubSup(Box<Node>, Box<Node>, Box<Node>),
    Frac(Box<Node>, Box<Node>),
    Sqrt(Option<Box<Node>>, Box<Node>),
    Delim(String, String, Box<Node>),
    Nary {
        chr: String,
        sub: Option<Box<Node>>,
        sup: Option<Box<Node>>,
        body: Box<Node>,
    },
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    /// 解析一行，遇到 stop 中的记号停止
    fn parse_row(&mut self, stop: &[Tok]) -> Node {
        let mut items = Vec::new();
        while let Some(t) = self.peek().cloned() {
            if stop.contains(&t) {
                break;
            }
            let atom = self.parse_atom();
            match atom {
                Some(mut node) => {
                    // 处理上下标后缀
                    loop {
                        match self.peek().cloned() {
                            Some(Tok::Caret) => {
                                self.next();
                                let e = self.parse_atom().unwrap_or(Node::Row(vec![]));
                                node = match node {
                                    Node::Sub(b, s) => Node::SubSup(b, s, Box::new(e)),
                                    other => Node::Sup(Box::new(other), Box::new(e)),
                                };
                            }
                            Some(Tok::Underscore) => {
                                self.next();
                                let s = self.parse_atom().unwrap_or(Node::Row(vec![]));
                                node = match node {
                                    Node::Sup(b, e) => Node::SubSup(b, Box::new(s), e),
                                    other => Node::Sub(Box::new(other), Box::new(s)),
                                };
                            }
                            _ => break,
                        }
                    }
                    items.push(node);
                }
                None => break,
            }
        }
        if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Node::Row(items)
        }
    }

    fn parse_group(&mut self) -> Node {
        // 消费 { ... }
        if self.peek() == Some(&Tok::LBrace) {
            self.next();
            let inner = self.parse_row(&[Tok::RBrace]);
            if self.peek() == Some(&Tok::RBrace) {
                self.next();
            }
            inner
        } else {
            self.parse_atom().unwrap_or(Node::Row(vec![]))
        }
    }

    fn parse_atom(&mut self) -> Option<Node> {
        let t = self.next()?;
        Some(match t {
            Tok::Ident(c) => Node::Sym(c.to_string()),
            Tok::Num(n) => Node::Num(n),
            Tok::Other(c) => Node::Sym(c.to_string()),
            Tok::LBrace => {
                let inner = self.parse_row(&[Tok::RBrace]);
                if self.peek() == Some(&Tok::RBrace) {
                    self.next();
                }
                inner
            }
            Tok::LParen => {
                let inner = self.parse_row(&[Tok::RParen]);
                if self.peek() == Some(&Tok::RParen) {
                    self.next();
                }
                Node::Delim("(".to_string(), ")".to_string(), Box::new(inner))
            }
            Tok::Cmd(name) => self.parse_command(&name),
            // 未预期的记号 → 文本
            other => Node::Sym(format!("{other:?}")),
        })
    }

    fn parse_command(&mut self, name: &str) -> Node {
        match name {
            "frac" => {
                let num = self.parse_group();
                let den = self.parse_group();
                Node::Frac(Box::new(num), Box::new(den))
            }
            "sqrt" => {
                // 可选次数 [n]
                let deg = if self.peek() == Some(&Tok::LBracket) {
                    self.next();
                    let d = self.parse_row(&[Tok::RBracket]);
                    if self.peek() == Some(&Tok::RBracket) {
                        self.next();
                    }
                    Some(Box::new(d))
                } else {
                    None
                };
                let body = self.parse_group();
                Node::Sqrt(deg, Box::new(body))
            }
            "text" | "mathrm" | "operatorname" => {
                // 读取原始文本组（token 拼接）
                let s = if self.peek() == Some(&Tok::LBrace) {
                    self.next();
                    let mut s = String::new();
                    while let Some(tok) = self.peek().cloned() {
                        if tok == Tok::RBrace {
                            break;
                        }
                        self.next();
                        s.push_str(&tok_text(&tok));
                    }
                    if self.peek() == Some(&Tok::RBrace) {
                        self.next();
                    }
                    s
                } else {
                    String::new()
                };
                Node::Text(s)
            }
            "left" => {
                let (beg, _) = self.parse_delim_char();
                let inner = self.parse_row(&[Tok::Cmd("right".to_string())]);
                // 消费 \right 与闭合符
                let mut end = ")".to_string();
                if self.peek() == Some(&Tok::Cmd("right".to_string())) {
                    self.next();
                    let (e, _) = self.parse_delim_char();
                    end = e;
                }
                Node::Delim(beg, end, Box::new(inner))
            }
            "sum" | "int" | "prod" | "iint" | "oint" => {
                let chr = match name {
                    "sum" => "∑".to_string(),
                    "int" => "∫".to_string(),
                    "iint" => "∬".to_string(),
                    "oint" => "∮".to_string(),
                    _ => "∏".to_string(),
                };
                let (sub, sup) = self.parse_scripts();
                let body = self.parse_atom().unwrap_or(Node::Row(vec![]));
                Node::Nary {
                    chr,
                    sub,
                    sup,
                    body: Box::new(body),
                }
            }
            "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "lim" | "max" | "min" | "det"
            | "dim" | "arg" | "gcd" | "deg" => Node::Text(name.to_string()),
            _ => {
                let s = symbol(name);
                Node::Sym(s)
            }
        }
    }

    /// 解析一组可选的上下标（用于 n 元算子）
    fn parse_scripts(&mut self) -> (Option<Box<Node>>, Option<Box<Node>>) {
        let mut sub = None;
        let mut sup = None;
        loop {
            match self.peek().cloned() {
                Some(Tok::Underscore) => {
                    self.next();
                    sub = Some(Box::new(self.parse_atom().unwrap_or(Node::Row(vec![]))));
                }
                Some(Tok::Caret) => {
                    self.next();
                    sup = Some(Box::new(self.parse_atom().unwrap_or(Node::Row(vec![]))));
                }
                _ => break,
            }
        }
        (sub, sup)
    }

    fn parse_delim_char(&mut self) -> (String, ()) {
        match self.next() {
            Some(Tok::Other(c)) => (c.to_string(), ()),
            Some(Tok::LParen) => ("(".to_string(), ()),
            Some(Tok::RParen) => (")".to_string(), ()),
            Some(Tok::LBracket) => ("[".to_string(), ()),
            Some(Tok::RBracket) => ("]".to_string(), ()),
            Some(Tok::Cmd(n)) => (symbol(&n), ()),
            _ => (".".to_string(), ()),
        }
    }
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Ident(c) => c.to_string(),
        Tok::Num(n) => n.clone(),
        Tok::Other(c) => c.to_string(),
        Tok::Cmd(n) => format!("\\{n}"),
        Tok::LBrace => "{".to_string(),
        Tok::RBrace => "}".to_string(),
        _ => String::new(),
    }
}

/// LaTeX 命令 → Unicode 符号
fn symbol(name: &str) -> String {
    let s = match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ε",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "φ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "cdot" => "⋅",
        "times" => "×",
        "div" => "÷",
        "pm" => "±",
        "mp" => "∓",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "infty" => "∞",
        "to" | "rightarrow" => "→",
        "leftarrow" => "←",
        "partial" => "∂",
        "nabla" => "∇",
        "in" => "∈",
        "notin" => "∉",
        "subset" => "⊂",
        "cup" => "∪",
        "cap" => "∩",
        "forall" => "∀",
        "exists" => "∃",
        "neg" => "¬",
        "wedge" => "∧",
        "vee" => "∨",
        "degree" => "°",
        "angle" => "∠",
        "perp" => "⊥",
        "sim" => "∼",
        "propto" => "∝",
        "prime" => "′",
        "ldots" => "…",
        "cdots" => "⋯",
        "langle" => "⟨",
        "rangle" => "⟩",
        "lvert" | "rvert" | "vert" => "|",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lbrace" => "{",
        "rbrace" => "}",
        _ => return name.to_string(),
    };
    s.to_string()
}

// ---------------- 代码生成 ----------------

fn emit(n: &Node) -> String {
    match n {
        Node::Row(items) => items.iter().map(emit).collect(),
        Node::Sym(s) => run(s),
        Node::Num(s) => run(s),
        Node::Text(s) => format!("<m:r><m:t xml:space=\"preserve\">{}</m:t></m:r>", esc(s)),
        Node::Sup(b, e) => wrap("m:sSup", &[("m:e", emit(b)), ("m:sup", emit(e))]),
        Node::Sub(b, s) => wrap("m:sSub", &[("m:e", emit(b)), ("m:sub", emit(s))]),
        Node::SubSup(b, s, e) => wrap(
            "m:sSubSup",
            &[("m:e", emit(b)), ("m:sub", emit(s)), ("m:sup", emit(e))],
        ),
        Node::Frac(a, b) => wrap("m:f", &[("m:num", emit(a)), ("m:den", emit(b))]),
        Node::Sqrt(deg, body) => {
            let pr = match deg {
                Some(d) => format!("<m:radPr/>{}", elem("m:deg", &emit(d))),
                None => "<m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/>".to_string(),
            };
            format!("<m:rad>{pr}{}</m:rad>", elem("m:e", &emit(body)))
        }
        Node::Delim(beg, end, inner) => {
            let pr = format!(
                "<m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/></m:dPr>",
                esc_attr(beg),
                esc_attr(end)
            );
            format!("<m:d>{pr}{}</m:d>", elem("m:e", &emit(inner)))
        }
        Node::Nary {
            chr,
            sub,
            sup,
            body,
        } => {
            let pr = format!(
                "<m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"undOvr\"/><m:subHide m:val=\"{}\"/><m:supHide m:val=\"{}\"/></m:naryPr>",
                esc_attr(chr),
                if sub.is_some() { "0" } else { "1" },
                if sup.is_some() { "0" } else { "1" }
            );
            let sub_xml = sub
                .as_ref()
                .map(|s| elem("m:sub", &emit(s)))
                .unwrap_or_else(|| "<m:sub/>".to_string());
            let sup_xml = sup
                .as_ref()
                .map(|s| elem("m:sup", &emit(s)))
                .unwrap_or_else(|| "<m:sup/>".to_string());
            format!(
                "<m:nary>{pr}{sub_xml}{sup_xml}{}</m:nary>",
                elem("m:e", &emit(body))
            )
        }
    }
}

fn run(text: &str) -> String {
    format!("<m:r><m:t>{}</m:t></m:r>", esc(text))
}

fn elem(name: &str, body: &str) -> String {
    format!("<{name}>{body}</{name}>")
}

fn wrap(name: &str, parts: &[(&str, String)]) -> String {
    let mut s = format!("<{name}>");
    for (tag, body) in parts {
        s.push('<');
        s.push_str(tag);
        s.push('>');
        s.push_str(body);
        s.push_str("</");
        s.push_str(tag);
        s.push('>');
    }
    s.push_str("</");
    s.push_str(name);
    s.push('>');
    s
}
