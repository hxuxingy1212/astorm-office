//! Minimal formula evaluator: computes cached values for formula cells.
//!
//! 支持：算术（+ - * / ^ %）、字符串连接（&）、比较、括号、单元格/区域引用
//! （含跨表 `Sheet1!A1`、绝对引用 `$A$1`）以及一组常用函数。用于在生成时写入
//! 缓存值，使非重算阅读器（预览、脚本）也能看到结果。
//!
//! 复杂度定位：覆盖常见报表/模型公式；不追求与 Excel 完全一致（数组公式、
//! 易失函数、迭代求解等不在此列）。

#![allow(clippy::excessive_precision)]

use std::collections::HashMap;

use serde_json::Value;

use chrono::{Datelike, Timelike};

use crate::model::workbook::Workbook;

#[derive(Clone, Debug)]
pub struct LambdaDef {
    params: Vec<String>,
    body: String,
    scope: HashMap<String, V>,
}

#[derive(Clone, Debug)]
enum V {
    Empty,
    Num(f64),
    Str(String),
    Bool(bool),
    Grid { cols: usize, data: Vec<V> },
    Lambda(Box<LambdaDef>),
    Err(String),
}

impl V {
    fn num(&self) -> Result<f64, String> {
        match self {
            V::Num(n) => Ok(*n),
            V::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            V::Empty => Ok(0.0),
            V::Str(s) => s.trim().parse::<f64>().map_err(|_| "#VALUE!".into()),
            V::Err(e) => Err(e.clone()),
            V::Grid { .. } | V::Lambda(_) => Err("#VALUE!".into()),
        }
    }
    fn truthy(&self) -> Result<bool, String> {
        match self {
            V::Bool(b) => Ok(*b),
            V::Num(n) => Ok(*n != 0.0),
            V::Empty => Ok(false),
            V::Str(s) => Ok(!s.is_empty()),
            V::Err(e) => Err(e.clone()),
            V::Grid { .. } | V::Lambda(_) => Err("#VALUE!".into()),
        }
    }
    fn to_json(&self) -> Option<Value> {
        match self {
            V::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 9.0e15 {
                    Some(Value::Number((*n as i64).into()))
                } else {
                    serde_json::Number::from_f64(*n).map(Value::Number)
                }
            }
            V::Str(s) => Some(Value::String(s.clone())),
            V::Bool(b) => Some(Value::Bool(*b)),
            _ => None,
        }
    }
}

fn cell_to_v(v: &Value) -> V {
    match v {
        Value::Number(n) => V::Num(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => V::Str(s.clone()),
        Value::Bool(b) => V::Bool(*b),
        Value::Null => V::Empty,
        _ => V::Empty,
    }
}

type Key = (String, String);

struct Ctx {
    cells: HashMap<Key, V>,
    cur_sheet: String,
    cur_row: Option<u32>,
    cur_col: Option<u32>,
    vars: HashMap<String, V>,
}

fn norm_ref(r: &str) -> String {
    r.chars()
        .filter(|c| *c != '$')
        .collect::<String>()
        .to_uppercase()
}

fn parse_cell_ref_local(r: &str) -> Option<(u32, u32)> {
    crate::utils::a1::parse_cell_ref(r).ok()
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    ctx: &'a Ctx,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str, ctx: &'a Ctx) -> Self {
        Parser {
            s: s.as_bytes(),
            i: 0,
            ctx,
        }
    }
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn eval(&mut self) -> Result<V, String> {
        let v = self.cmp()?;
        self.ws();
        if self.i < self.s.len() {
            return Err("#VALUE!".into());
        }
        Ok(v)
    }
    fn cmp(&mut self) -> Result<V, String> {
        let mut lhs = self.concat()?;
        loop {
            self.ws();
            let (op, len) = match self.s.get(self.i..self.i + 2) {
                Some(b"<>") => ("<>", 2),
                Some(b"<=") => ("<=", 2),
                Some(b">=") => (">=", 2),
                _ => match self.peek() {
                    Some(b'=') => ("=", 1),
                    Some(b'<') => ("<", 1),
                    Some(b'>') => (">", 1),
                    _ => break,
                },
            };
            self.i += len;
            let rhs = self.concat()?;
            lhs = V::Bool(compare(&lhs, &rhs, op)?);
        }
        Ok(lhs)
    }
    fn concat(&mut self) -> Result<V, String> {
        let mut lhs = self.addsub()?;
        while self.eat(b'&') {
            let rhs = self.addsub()?;
            lhs = V::Str(format!("{}{}", as_str(&lhs), as_str(&rhs)));
        }
        Ok(lhs)
    }
    fn addsub(&mut self) -> Result<V, String> {
        let mut lhs = self.muldiv()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.i += 1;
                    let r = self.muldiv()?;
                    lhs = V::Num(lhs.num()? + r.num()?);
                }
                Some(b'-') => {
                    self.i += 1;
                    let r = self.muldiv()?;
                    lhs = V::Num(lhs.num()? - r.num()?);
                }
                _ => break,
            }
        }
        Ok(lhs)
    }
    fn muldiv(&mut self) -> Result<V, String> {
        let mut lhs = self.power()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.i += 1;
                    let r = self.power()?;
                    lhs = V::Num(lhs.num()? * r.num()?);
                }
                Some(b'/') => {
                    self.i += 1;
                    let r = self.power()?;
                    let d = r.num()?;
                    lhs = if d == 0.0 {
                        V::Err("#DIV/0!".into())
                    } else {
                        V::Num(lhs.num()? / d)
                    };
                }
                _ => break,
            }
        }
        Ok(lhs)
    }
    fn power(&mut self) -> Result<V, String> {
        let base = self.unary()?;
        if self.eat(b'^') {
            let exp = self.power()?;
            return Ok(V::Num(base.num()?.powf(exp.num()?)));
        }
        Ok(base)
    }
    fn unary(&mut self) -> Result<V, String> {
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                Ok(V::Num(-self.unary()?.num()?))
            }
            Some(b'+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.postfix(),
        }
    }
    fn postfix(&mut self) -> Result<V, String> {
        let v = self.primary()?;
        if self.peek() == Some(b'%') {
            self.i += 1;
            return Ok(V::Num(v.num()? / 100.0));
        }
        Ok(v)
    }
    fn primary(&mut self) -> Result<V, String> {
        self.ws();
        let c = match self.peek() {
            Some(c) => c,
            None => return Ok(V::Empty),
        };
        if c == b'(' {
            self.i += 1;
            let v = self.cmp()?;
            if !self.eat(b')') {
                return Err("#VALUE!".into());
            }
            return Ok(v);
        }
        if c == b'"' {
            self.i += 1;
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != b'"' {
                self.i += 1;
            }
            let s = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
            self.i += 1; // closing quote
            return Ok(V::Str(s));
        }
        if c.is_ascii_digit() || c == b'.' {
            let start = self.i;
            while self.i < self.s.len()
                && ((self.s[self.i] as char).is_ascii_digit()
                    || self.s[self.i] == b'.'
                    || self.s[self.i] == b'e'
                    || self.s[self.i] == b'E'
                    || self.s[self.i] == b'+'
                    || self.s[self.i] == b'-')
            {
                self.i += 1;
            }
            let t = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
            return t.parse::<f64>().map(V::Num).map_err(|_| "#VALUE!".into());
        }
        // identifier: function / ref / sheet-qualified ref / TRUE / FALSE
        self.ident_like()
    }

    fn ident_like(&mut self) -> Result<V, String> {
        // Optional sheet qualifier: Name! or 'Name'!
        let start = self.i;
        let mut sheet: Option<String> = None;
        if self.peek() == Some(b'\'') {
            self.i += 1;
            let s0 = self.i;
            while self.i < self.s.len() && self.s[self.i] != b'\'' {
                self.i += 1;
            }
            sheet = Some(String::from_utf8_lossy(&self.s[s0..self.i]).into_owned());
            self.i += 1;
        } else {
            let s0 = self.i;
            while self.i < self.s.len()
                && ((self.s[self.i] as char).is_alphanumeric()
                    || self.s[self.i] == b'_'
                    || self.s[self.i] == b'.')
            {
                self.i += 1;
            }
            if self.s.get(self.i) == Some(&b'!') {
                sheet = Some(String::from_utf8_lossy(&self.s[s0..self.i]).into_owned());
            } else {
                self.i = start;
            }
        }
        if sheet.is_some() {
            if self.s.get(self.i) == Some(&b'!') {
                self.i += 1;
            } else {
                self.i = start;
                sheet = None;
            }
        }

        // name token
        let s0 = self.i;
        let mut dollar_prefix = false;
        while self.i < self.s.len() && self.s[self.i] == b'$' {
            dollar_prefix = true;
            self.i += 1;
        }
        let _ = dollar_prefix;
        let start2 = self.i;
        while self.i < self.s.len()
            && ((self.s[self.i] as char).is_ascii_alphanumeric()
                || self.s[self.i] == b'_'
                || self.s[self.i] == b'.'
                || self.s[self.i] == b'$')
        {
            self.i += 1;
        }
        let name = String::from_utf8_lossy(&self.s[start2..self.i]).into_owned();

        // function call
        if self.s.get(self.i) == Some(&b'(') && sheet.is_none() {
            self.i += 1;
            let mut args = Vec::new();
            let mut raw: Vec<String> = Vec::new();
            if self.peek() != Some(b')') {
                loop {
                    let s = self.i;
                    let v = match self.cmp() {
                        Ok(v) => v,
                        Err(e) => V::Err(e),
                    };
                    args.push(v);
                    let e = self.i;
                    raw.push(String::from_utf8_lossy(&self.s[s..e]).trim().to_string());
                    if !self.eat(b',') {
                        break;
                    }
                }
            }
            if !self.eat(b')') {
                return Err("#VALUE!".into());
            }
            if let Some(r) = special_func(&name, &raw, self.ctx) {
                return r;
            }
            if let Some(r) = ref_func(&name, &raw, self.ctx) {
                return r;
            }
            return call_func(&name, &args, self.ctx);
        }

        // range: A1:B2
        if self.s.get(self.i) == Some(&b':') {
            self.i += 1;
            let mut d2 = false;
            while self.i < self.s.len() && self.s[self.i] == b'$' {
                d2 = true;
                self.i += 1;
            }
            let _ = d2;
            let r0 = self.i;
            while self.i < self.s.len()
                && ((self.s[self.i] as char).is_ascii_alphanumeric() || self.s[self.i] == b'$')
            {
                self.i += 1;
            }
            let name2 = String::from_utf8_lossy(&self.s[r0..self.i]).into_owned();
            let sh = sheet.clone().unwrap_or_else(|| self.ctx.cur_sheet.clone());
            return Ok(range_values(&sh, &name, &name2, self.ctx));
        }

        // cell reference
        if parse_cell_ref_local(&name).is_some() {
            let sh = sheet.clone().unwrap_or_else(|| self.ctx.cur_sheet.clone());
            return Ok(lookup(&sh, &name, self.ctx));
        }
        // TRUE / FALSE / named
        match name.to_ascii_uppercase().as_str() {
            "TRUE" => Ok(V::Bool(true)),
            "FALSE" => Ok(V::Bool(false)),
            _ => {
                let _ = s0;
                // LET/LAMBDA 绑定的变量优先
                if let Some(v) = self.ctx.vars.get(&name) {
                    return Ok(v.clone());
                }
                Ok(V::Str(name))
            }
        }
    }
}

fn lookup(sheet: &str, r: &str, ctx: &Ctx) -> V {
    let key = (sheet.to_lowercase(), norm_ref(r));
    ctx.cells.get(&key).cloned().unwrap_or(V::Empty)
}

fn range_values(sh: &str, a: &str, b: &str, ctx: &Ctx) -> V {
    let (Some((c1, r1)), Some((c2, r2))) = (parse_cell_ref_local(a), parse_cell_ref_local(b))
    else {
        return V::Grid {
            cols: 0,
            data: Vec::new(),
        };
    };
    let (c1, c2) = (c1.min(c2), c1.max(c2));
    let (r1, r2) = (r1.min(r2), r1.max(r2));
    let cols = (c2 - c1 + 1) as usize;
    let mut data = Vec::new();
    for r in r1..=r2 {
        for c in c1..=c2 {
            let name = crate::utils::a1::make_cell_ref(c, r);
            data.push(lookup(sh, &name, ctx));
        }
    }
    V::Grid { cols, data }
}

fn flatten(args: &[V]) -> Vec<(V, bool)> {
    // (value, from_range)
    let mut out = Vec::new();
    for a in args {
        match a {
            V::Grid { data, .. } => {
                for v in data {
                    out.push((v.clone(), true));
                }
            }
            v => out.push((v.clone(), false)),
        }
    }
    out
}

fn as_str(v: &V) -> String {
    match v {
        V::Str(s) => s.clone(),
        V::Num(n) => fmt_num(*n),
        V::Bool(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        V::Empty => String::new(),
        V::Err(e) => e.clone(),
        V::Grid { .. } | V::Lambda(_) => String::new(),
    }
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn compare(a: &V, b: &V, op: &str) -> Result<bool, String> {
    // 数值优先，否则按字符串
    let (na, nb) = (as_num_opt(a), as_num_opt(b));
    if let (Some(x), Some(y)) = (na, nb) {
        return Ok(match op {
            "=" => x == y,
            "<>" => x != y,
            "<" => x < y,
            ">" => x > y,
            "<=" => x <= y,
            ">=" => x >= y,
            _ => false,
        });
    }
    let (x, y) = (as_str(a).to_uppercase(), as_str(b).to_uppercase());
    Ok(match op {
        "=" => x == y,
        "<>" => x != y,
        "<" => x < y,
        ">" => x > y,
        "<=" => x <= y,
        ">=" => x >= y,
        _ => false,
    })
}

fn as_num_opt(v: &V) -> Option<f64> {
    match v {
        V::Num(n) => Some(*n),
        V::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        V::Empty => None,
        V::Str(_) | V::Err(_) | V::Grid { .. } | V::Lambda(_) => None,
    }
}

fn nums(args: &[V]) -> Vec<f64> {
    flatten(args)
        .into_iter()
        .filter_map(|(v, _)| match v {
            V::Num(n) => Some(n),
            V::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
            _ => None,
        })
        .collect()
}

/// 用给定变量作用域求值一个表达式字符串。
fn eval_expr(expr: &str, ctx: &Ctx, scope: &HashMap<String, V>) -> Result<V, String> {
    let c = Ctx {
        cells: ctx.cells.clone(),
        cur_sheet: ctx.cur_sheet.clone(),
        cur_row: ctx.cur_row,
        cur_col: ctx.cur_col,
        vars: scope.clone(),
    };
    Parser::new(expr, &c).eval()
}

/// 应用一个 LAMBDA：绑定参数后求值函数体。
fn apply_lambda(l: &LambdaDef, vals: &[V], ctx: &Ctx) -> Result<V, String> {
    let mut scope = l.scope.clone();
    for (p, v) in l.params.iter().zip(vals.iter()) {
        scope.insert(p.clone(), v.clone());
    }
    for p in l.params.iter().skip(vals.len()) {
        scope.entry(p.clone()).or_insert(V::Empty);
    }
    eval_expr(&l.body, ctx, &scope)
}

/// LET / LAMBDA 等需要捕获原始实参的“特殊形式”。
fn special_func(name: &str, raw: &[String], ctx: &Ctx) -> Option<Result<V, String>> {
    let up = name.to_ascii_uppercase();
    match up.as_str() {
        "LET" => {
            if raw.len() < 3 || raw.len().is_multiple_of(2) {
                return Some(Err("#VALUE!".into()));
            }
            let mut scope = ctx.vars.clone();
            let mut i = 0;
            while i + 1 < raw.len() - 1 {
                let var = raw[i].trim().to_string();
                match eval_expr(&raw[i + 1], ctx, &scope) {
                    Ok(v) => {
                        scope.insert(var, v);
                    }
                    Err(e) => return Some(Err(e)),
                }
                i += 2;
            }
            Some(eval_expr(
                raw.last().map(|s| s.as_str()).unwrap_or(""),
                ctx,
                &scope,
            ))
        }
        "LAMBDA" => {
            if raw.is_empty() {
                return Some(Err("#VALUE!".into()));
            }
            let body = raw.last().unwrap().clone();
            let params: Vec<String> = raw[..raw.len() - 1]
                .iter()
                .map(|s| s.trim().to_string())
                .collect();
            Some(Ok(V::Lambda(Box::new(LambdaDef {
                params,
                body,
                scope: ctx.vars.clone(),
            }))))
        }
        _ => None,
    }
}

/// 解析引用字符串：可带表名（`Sheet1!` 或 `'Sheet 1'!`），返回 (sheet, c1,r1,c2,r2)。
fn parse_ref(s: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    let s = s.trim();
    let (sheet, rest) = if let Some(i) = s.rfind('!') {
        let sh = s[..i].trim_matches(|c| c == '\'' || c == '"').to_string();
        (Some(sh), s[i + 1..].to_string())
    } else {
        (None, s.to_string())
    };
    let rest = rest.replace('$', "");
    if let Some((a, b)) = rest.split_once(':') {
        let (c1, r1) = crate::utils::a1::parse_cell_ref(a.trim()).ok()?;
        let (c2, r2) = crate::utils::a1::parse_cell_ref(b.trim()).ok()?;
        Some((sheet, c1.min(c2), r1.min(r2), c1.max(c2), r1.max(r2)))
    } else {
        let (c, r) = crate::utils::a1::parse_cell_ref(rest.trim()).ok()?;
        Some((sheet, c, r, c, r))
    }
}

fn fetch(ctx: &Ctx, sheet: &str, c: u32, r: u32) -> V {
    let name = format!("{}{}", col_name(c), r);
    lookup(sheet, &name, ctx)
}

fn grid_of_ref(ctx: &Ctx, sheet: &str, c1: u32, r1: u32, c2: u32, r2: u32) -> V {
    let cols = (c2 - c1 + 1) as usize;
    let mut data = Vec::new();
    for r in r1..=r2 {
        for c in c1..=c2 {
            data.push(fetch(ctx, sheet, c, r));
        }
    }
    V::Grid { cols, data }
}

fn as_f64(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// 引用语义函数；命中返回 Some(结果)，否则 None（交回普通函数）。
fn ref_func(name: &str, raw: &[String], ctx: &Ctx) -> Option<Result<V, String>> {
    let up = name.to_ascii_uppercase();
    let err = |e: &str| Some(Err(e.to_string()));
    let r0 = raw.first().map(|s| s.as_str()).unwrap_or("");
    match up.as_str() {
        "ROW" => {
            if raw.is_empty() || r0.is_empty() {
                return ctx
                    .cur_row
                    .map(|r| Ok(V::Num(r as f64)))
                    .or_else(|| err("#VALUE!"));
            }
            parse_ref(r0)
                .map(|(_, _, r1, _, _)| Ok(V::Num(r1 as f64)))
                .or_else(|| err("#REF!"))
        }
        "COLUMN" => {
            if raw.is_empty() || r0.is_empty() {
                return ctx
                    .cur_col
                    .map(|c| Ok(V::Num(c as f64)))
                    .or_else(|| err("#VALUE!"));
            }
            parse_ref(r0)
                .map(|(_, c1, _, _, _)| Ok(V::Num(c1 as f64)))
                .or_else(|| err("#REF!"))
        }
        "ROWS" => parse_ref(r0)
            .map(|(_, _, r1, _, r2)| Ok(V::Num((r2 - r1 + 1) as f64)))
            .or_else(|| err("#REF!")),
        "COLUMNS" => parse_ref(r0)
            .map(|(_, c1, _, c2, _)| Ok(V::Num((c2 - c1 + 1) as f64)))
            .or_else(|| err("#REF!")),
        "AREAS" => Some(Ok(V::Num(1.0))),
        "ISREF" => Some(Ok(V::Bool(parse_ref(r0).is_some()))),
        "INDIRECT" => {
            let inner = r0.trim().trim_matches('"');
            match parse_ref(inner) {
                Some((sh, c1, r1, c2, r2)) => {
                    let sheet = sh.unwrap_or_else(|| ctx.cur_sheet.clone());
                    if c1 == c2 && r1 == r2 {
                        Some(Ok(fetch(ctx, &sheet, c1, r1)))
                    } else {
                        Some(Ok(grid_of_ref(ctx, &sheet, c1, r1, c2, r2)))
                    }
                }
                None => err("#REF!"),
            }
        }
        "OFFSET" => {
            let Some((sh, c1, r1, c2, r2)) = parse_ref(r0) else {
                return err("#REF!");
            };
            let sheet = sh.unwrap_or_else(|| ctx.cur_sheet.clone());
            let rows = as_f64(raw.get(1).map(|s| s.as_str()).unwrap_or("0")) as i64;
            let cols = as_f64(raw.get(2).map(|s| s.as_str()).unwrap_or("0")) as i64;
            let nrows = (r2 as i64 - r1 as i64 + 1).max(1);
            let ncols = (c2 as i64 - c1 as i64 + 1).max(1);
            let h = raw
                .get(3)
                .filter(|s| !s.is_empty())
                .map(|s| as_f64(s) as i64)
                .unwrap_or(nrows);
            let w = raw
                .get(4)
                .filter(|s| !s.is_empty())
                .map(|s| as_f64(s) as i64)
                .unwrap_or(ncols);
            let nr1 = (r1 as i64 + rows).max(1);
            let nc1 = (c1 as i64 + cols).max(1);
            let nr2 = nr1 + h - 1;
            let nc2 = nc1 + w - 1;
            if h == 1 && w == 1 {
                Some(Ok(fetch(ctx, &sheet, nc1 as u32, nr1 as u32)))
            } else {
                Some(Ok(grid_of_ref(
                    ctx, &sheet, nc1 as u32, nr1 as u32, nc2 as u32, nr2 as u32,
                )))
            }
        }
        "ADDRESS" => {
            let r = as_f64(raw.first().map(|s| s.as_str()).unwrap_or("1")) as u32;
            let c = as_f64(raw.get(1).map(|s| s.as_str()).unwrap_or("1")) as u32;
            let abs = as_f64(raw.get(2).map(|s| s.as_str()).unwrap_or("1")) as i32;
            let col = col_name(c);
            let a1 = match abs {
                1 => format!("${}${}", col, r),
                2 => format!("{}${}", col, r),
                3 => format!("${}{}", col, r),
                _ => format!("{}{}", col, r),
            };
            let sheet = raw.get(4).map(|s| s.trim_matches('"')).unwrap_or("");
            Some(Ok(V::Str(if sheet.is_empty() {
                a1
            } else {
                format!("{}!{}", sheet, a1)
            })))
        }
        _ => None,
    }
}

fn call_func(name: &str, args: &[V], ctx: &Ctx) -> Result<V, String> {
    let up = name.to_ascii_uppercase();
    let err = |e: &str| Err(e.to_string());
    match up.as_str() {
        "SUM" => Ok(V::Num(nums(args).iter().sum())),
        "PRODUCT" => Ok(V::Num(nums(args).iter().product())),
        "AVERAGE" => {
            let v = nums(args);
            if v.is_empty() {
                err("#DIV/0!")
            } else {
                Ok(V::Num(v.iter().sum::<f64>() / v.len() as f64))
            }
        }
        "COUNT" => Ok(V::Num(nums(args).len() as f64)),
        "COUNTA" => Ok(V::Num(
            flatten(args)
                .iter()
                .filter(|(v, _)| !matches!(v, V::Empty))
                .count() as f64,
        )),
        "COUNTBLANK" => Ok(V::Num(
            flatten(args)
                .iter()
                .filter(|(v, _)| matches!(v, V::Empty))
                .count() as f64,
        )),
        "MIN" => {
            let v = nums(args);
            Ok(V::Num(v.iter().cloned().fold(f64::INFINITY, f64::min)))
        }
        "MAX" => {
            let v = nums(args);
            Ok(V::Num(v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)))
        }
        "ABS" => Ok(V::Num(arg(args, 0)?.num()?.abs())),
        "INT" => Ok(V::Num(arg(args, 0)?.num()?.floor())),
        "SQRT" => Ok(V::Num(arg(args, 0)?.num()?.sqrt())),
        "POWER" => Ok(V::Num(arg(args, 0)?.num()?.powf(arg(args, 1)?.num()?))),
        "MOD" => {
            let a = arg(args, 0)?.num()?;
            let b = arg(args, 1)?.num()?;
            Ok(V::Num(a - b * (a / b).floor()))
        }
        "ROUND" => {
            let a = arg(args, 0)?.num()?;
            let d = arg(args, 1)?.num().unwrap_or(0.0) as i32;
            let f = 10f64.powi(d);
            Ok(V::Num((a * f).round() / f))
        }
        "ROUNDUP" => {
            let a = arg(args, 0)?.num()?;
            let d = arg(args, 1)?.num().unwrap_or(0.0) as i32;
            let f = 10f64.powi(d);
            Ok(V::Num((a * f).ceil() / f))
        }
        "ROUNDDOWN" => {
            let a = arg(args, 0)?.num()?;
            let d = arg(args, 1)?.num().unwrap_or(0.0) as i32;
            let f = 10f64.powi(d);
            Ok(V::Num((a * f).floor() / f))
        }
        "IF" => {
            if arg(args, 0)?.truthy()? {
                Ok(arg(args, 1).cloned().unwrap_or(V::Bool(true)))
            } else {
                Ok(arg(args, 2).cloned().unwrap_or(V::Bool(false)))
            }
        }
        "IFERROR" => {
            let v = arg(args, 0)?.clone();
            if matches!(v, V::Err(_)) {
                Ok(arg(args, 1).cloned().unwrap_or(V::Empty))
            } else {
                Ok(v)
            }
        }
        "AND" => {
            for (v, _) in flatten(args) {
                if !v.truthy()? {
                    return Ok(V::Bool(false));
                }
            }
            Ok(V::Bool(true))
        }
        "OR" => {
            for (v, _) in flatten(args) {
                if v.truthy()? {
                    return Ok(V::Bool(true));
                }
            }
            Ok(V::Bool(false))
        }
        "NOT" => Ok(V::Bool(!arg(args, 0)?.truthy()?)),
        "CONCATENATE" | "CONCAT" => {
            let mut s = String::new();
            for (v, _) in flatten(args) {
                s.push_str(&as_str(&v));
            }
            Ok(V::Str(s))
        }
        "LEN" => Ok(V::Num(as_str(arg(args, 0)?).chars().count() as f64)),
        "UPPER" => Ok(V::Str(as_str(arg(args, 0)?).to_uppercase())),
        "LOWER" => Ok(V::Str(as_str(arg(args, 0)?).to_lowercase())),
        "TRIM" => Ok(V::Str(as_str(arg(args, 0)?).trim().to_string())),
        "LEFT" => {
            let s = as_str(arg(args, 0)?);
            let n = args.get(1).and_then(|v| v.num().ok()).unwrap_or(1.0) as usize;
            Ok(V::Str(s.chars().take(n).collect()))
        }
        "RIGHT" => {
            let s = as_str(arg(args, 0)?);
            let n = args.get(1).and_then(|v| v.num().ok()).unwrap_or(1.0) as usize;
            let len = s.chars().count();
            Ok(V::Str(s.chars().skip(len.saturating_sub(n)).collect()))
        }
        "SUMIF" => {
            let (_, range) = grid_of(arg(args, 0)?);
            let crit = arg(args, 1)?.clone();
            let (_, sum_range) = args.get(2).map(grid_of).unwrap_or((0, range.clone()));
            let mut total = 0.0;
            for (i, v) in range.iter().enumerate() {
                if matches_crit(v, &crit)? {
                    if let Some(sv) = sum_range.get(i) {
                        total += sv.num().unwrap_or(0.0);
                    }
                }
            }
            Ok(V::Num(total))
        }
        "COUNTIF" => {
            let (_, range) = grid_of(arg(args, 0)?);
            let crit = arg(args, 1)?.clone();
            let mut c = 0.0;
            for v in &range {
                if matches_crit(v, &crit)? {
                    c += 1.0;
                }
            }
            Ok(V::Num(c))
        }
        "AVERAGEIF" => {
            let (_, range) = grid_of(arg(args, 0)?);
            let crit = arg(args, 1)?.clone();
            let (_, sum_range) = args.get(2).map(grid_of).unwrap_or((0, range.clone()));
            let mut total = 0.0;
            let mut n = 0.0;
            for (i, v) in range.iter().enumerate() {
                if matches_crit(v, &crit)? {
                    if let Some(sv) = sum_range.get(i) {
                        total += sv.num()?;
                        n += 1.0;
                    }
                }
            }
            if n == 0.0 {
                err("#DIV/0!")
            } else {
                Ok(V::Num(total / n))
            }
        }
        "SUMIFS" | "COUNTIFS" | "MAXIFS" | "MINIFS" => {
            let (_, target) = grid_of(arg(args, 0)?);
            let mut mask = vec![true; target.len()];
            let mut k = 1;
            while k + 1 < args.len() {
                let (_, rng) = grid_of(&args[k]);
                let crit = args[k + 1].clone();
                for (i, v) in rng.iter().enumerate() {
                    if i < mask.len() && !matches_crit(v, &crit).unwrap_or(false) {
                        mask[i] = false;
                    }
                }
                k += 2;
            }
            let sel: Vec<f64> = target
                .iter()
                .enumerate()
                .filter(|(i, _)| mask.get(*i).copied().unwrap_or(false))
                .filter_map(|(_, v)| v.num().ok())
                .collect();
            match up.as_str() {
                "COUNTIFS" => Ok(V::Num(sel.len() as f64)),
                "SUMIFS" => Ok(V::Num(sel.iter().sum())),
                "MAXIFS" => Ok(V::Num(
                    sel.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                )),
                _ => Ok(V::Num(sel.iter().cloned().fold(f64::INFINITY, f64::min))),
            }
        }
        "SUMPRODUCT" => {
            let grids: Vec<(usize, Vec<V>)> = args.iter().map(grid_of).collect();
            let len = grids.iter().map(|(_, d)| d.len()).min().unwrap_or(0);
            let mut total = 0.0;
            for i in 0..len {
                let mut p = 1.0;
                for (_, d) in &grids {
                    p *= d.get(i).and_then(|v| v.num().ok()).unwrap_or(0.0);
                }
                total += p;
            }
            Ok(V::Num(total))
        }
        "INDEX" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let row = arg(args, 1)?.num()? as usize;
            let col = args.get(2).and_then(|v| v.num().ok()).unwrap_or(1.0) as usize;
            let cols = cols.max(1);
            let idx = (row.saturating_sub(1)) * cols + (col.saturating_sub(1));
            Ok(data.get(idx).cloned().unwrap_or(V::Err("#REF!".into())))
        }
        "MATCH" => {
            let key = arg(args, 0)?;
            let (_, data) = grid_of(arg(args, 1)?);
            for (i, v) in data.iter().enumerate() {
                if compare(v, key, "=").unwrap_or(false) {
                    return Ok(V::Num((i + 1) as f64));
                }
            }
            err("#N/A")
        }
        "VLOOKUP" => {
            let key = arg(args, 0)?.clone();
            let (cols, data) = grid_of(arg(args, 1)?);
            let col = arg(args, 2)?.num()? as usize;
            let cols = cols.max(1);
            let rows = data.len() / cols;
            for r in 0..rows {
                if compare(&data[r * cols], &key, "=").unwrap_or(false) {
                    let idx = r * cols + (col.saturating_sub(1));
                    return Ok(data.get(idx).cloned().unwrap_or(V::Err("#REF!".into())));
                }
            }
            err("#N/A")
        }
        "HLOOKUP" => {
            let key = arg(args, 0)?.clone();
            let (cols, data) = grid_of(arg(args, 1)?);
            let row = arg(args, 2)?.num()? as usize;
            let cols = cols.max(1);
            let rows = data.len() / cols;
            for c in 0..cols {
                if compare(&data[c], &key, "=").unwrap_or(false) {
                    let idx = (row.saturating_sub(1)) * cols + c;
                    return Ok(data.get(idx).cloned().unwrap_or(V::Err("#REF!".into())));
                }
            }
            let _ = rows;
            err("#N/A")
        }
        "CHOOSE" => {
            let i = arg(args, 0)?.num()? as usize;
            Ok(args.get(i).cloned().unwrap_or(V::Err("#VALUE!".into())))
        }
        "IFNA" => {
            let v = arg(args, 0)?.clone();
            match &v {
                V::Err(e) if e == "#N/A" => Ok(arg(args, 1).cloned().unwrap_or(V::Empty)),
                _ => Ok(v),
            }
        }
        "MEDIAN" => {
            let mut v = nums(args);
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if v.is_empty() {
                err("#NUM!")
            } else if v.len() % 2 == 1 {
                Ok(V::Num(v[v.len() / 2]))
            } else {
                Ok(V::Num((v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0))
            }
        }
        "LARGE" | "SMALL" => {
            let mut v = nums(args);
            let k = arg(args, 1)?.num()? as usize;
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if k == 0 || k > v.len() {
                return err("#NUM!");
            }
            Ok(V::Num(if up == "LARGE" {
                v[v.len() - k]
            } else {
                v[k - 1]
            }))
        }
        "STDEV" | "STDEV.S" | "STDEV.P" | "VAR" | "VAR.S" | "VAR.P" => {
            let v = nums(args);
            let n = v.len();
            if n < 2 {
                return err("#DIV/0!");
            }
            let mean = v.iter().sum::<f64>() / n as f64;
            let ss: f64 = v.iter().map(|x| (x - mean).powi(2)).sum();
            let pop = matches!(up.as_str(), "STDEV.P" | "VAR.P");
            let denom = if pop { n as f64 } else { (n - 1) as f64 };
            let var = ss / denom;
            Ok(V::Num(if up.starts_with("STDEV") {
                var.sqrt()
            } else {
                var
            }))
        }
        "SIGN" => Ok(V::Num(arg(args, 0)?.num()?.signum())),
        "EXP" => Ok(V::Num(arg(args, 0)?.num()?.exp())),
        "LN" => Ok(V::Num(arg(args, 0)?.num()?.ln())),
        "LOG10" => Ok(V::Num(arg(args, 0)?.num()?.log10())),
        "LOG" => {
            let x = arg(args, 0)?.num()?;
            let base = args.get(1).and_then(|v| v.num().ok()).unwrap_or(10.0);
            Ok(V::Num(x.log(base)))
        }
        "PI" => Ok(V::Num(std::f64::consts::PI)),
        "TRUNC" => {
            let a = arg(args, 0)?.num()?;
            let d = args.get(1).and_then(|v| v.num().ok()).unwrap_or(0.0) as i32;
            let f = 10f64.powi(d);
            Ok(V::Num((a * f).trunc() / f))
        }
        "VALUE" => {
            let s = as_str(arg(args, 0)?);
            let cleaned: String = s.chars().filter(|c| *c != ',' && *c != '$').collect();
            cleaned
                .trim()
                .parse::<f64>()
                .map(V::Num)
                .map_err(|_| "#VALUE!".into())
        }
        "TEXT" => {
            let n = arg(args, 0)?.num()?;
            let f = as_str(arg(args, 1)?);
            Ok(V::Str(fmt_pattern(n, &f)))
        }
        "DATE" => {
            let y = arg(args, 0)?.num()? as i32;
            let m = arg(args, 1)?.num()? as u32;
            let d = arg(args, 2)?.num()? as u32;
            match chrono::NaiveDate::from_ymd_opt(y, m.clamp(1, 12), d.clamp(1, 31)) {
                Some(dt) => Ok(V::Num(crate::utils::date::date_to_serial(dt, false) as f64)),
                None => err("#VALUE!"),
            }
        }
        "YEAR" | "MONTH" | "DAY" => {
            use chrono::Datelike;
            let serial = arg(args, 0)?.num()? as i64;
            match crate::utils::date::serial_to_date(serial, false) {
                Some(dt) => Ok(V::Num(match up.as_str() {
                    "YEAR" => dt.year() as f64,
                    "MONTH" => dt.month() as f64,
                    _ => dt.day() as f64,
                })),
                None => err("#VALUE!"),
            }
        }
        "NPV" => {
            let rate = arg(args, 0)?.num()?;
            let mut total = 0.0;
            for (i, v) in flatten(&args[1..]).iter().enumerate() {
                total += v.0.num().unwrap_or(0.0) / (1.0 + rate).powi(i as i32 + 1);
            }
            Ok(V::Num(total))
        }
        "PMT" => {
            let rate = arg(args, 0)?.num()?;
            let nper = arg(args, 1)?.num()?;
            let pv = arg(args, 2)?.num()?;
            let fv = args.get(3).and_then(|v| v.num().ok()).unwrap_or(0.0);
            if rate == 0.0 {
                Ok(V::Num(-(pv + fv) / nper))
            } else {
                Ok(V::Num(
                    -rate * (pv * (1.0 + rate).powf(nper) + fv) / ((1.0 + rate).powf(nper) - 1.0),
                ))
            }
        }
        "FV" => {
            let rate = arg(args, 0)?.num()?;
            let nper = arg(args, 1)?.num()?;
            let pmt = arg(args, 2)?.num()?;
            let pv = args.get(3).and_then(|v| v.num().ok()).unwrap_or(0.0);
            if rate == 0.0 {
                Ok(V::Num(-(pv + pmt * nper)))
            } else {
                Ok(V::Num(
                    -(pv * (1.0 + rate).powf(nper) + pmt * ((1.0 + rate).powf(nper) - 1.0) / rate),
                ))
            }
        }
        "PV" => {
            let rate = arg(args, 0)?.num()?;
            let nper = arg(args, 1)?.num()?;
            let pmt = arg(args, 2)?.num()?;
            let fv = args.get(3).and_then(|v| v.num().ok()).unwrap_or(0.0);
            if rate == 0.0 {
                Ok(V::Num(-(fv + pmt * nper)))
            } else {
                Ok(V::Num(
                    -(fv + pmt * ((1.0 + rate).powf(nper) - 1.0) / rate) / (1.0 + rate).powf(nper),
                ))
            }
        }
        "IRR" => {
            let (_, data) = grid_of(arg(args, 0)?);
            let cf: Vec<f64> = data.iter().filter_map(|v| v.num().ok()).collect();
            match irr(&cf) {
                Some(r) => Ok(V::Num(r)),
                None => err("#NUM!"),
            }
        }
        // ---- 三角 / 数学 ----
        "SIN" => Ok(V::Num(arg(args, 0)?.num()?.sin())),
        "COS" => Ok(V::Num(arg(args, 0)?.num()?.cos())),
        "TAN" => Ok(V::Num(arg(args, 0)?.num()?.tan())),
        "ASIN" => Ok(V::Num(arg(args, 0)?.num()?.asin())),
        "ACOS" => Ok(V::Num(arg(args, 0)?.num()?.acos())),
        "ATAN" => Ok(V::Num(arg(args, 0)?.num()?.atan())),
        "ATAN2" => Ok(V::Num(arg(args, 0)?.num()?.atan2(arg(args, 1)?.num()?))),
        "SINH" => Ok(V::Num(arg(args, 0)?.num()?.sinh())),
        "COSH" => Ok(V::Num(arg(args, 0)?.num()?.cosh())),
        "TANH" => Ok(V::Num(arg(args, 0)?.num()?.tanh())),
        "DEGREES" => Ok(V::Num(arg(args, 0)?.num()?.to_degrees())),
        "RADIANS" => Ok(V::Num(arg(args, 0)?.num()?.to_radians())),
        "CEILING" | "CEILING.MATH" => {
            let a = arg(args, 0)?.num()?;
            let s = args.get(1).and_then(|v| v.num().ok()).unwrap_or(1.0);
            if s == 0.0 {
                return err("#DIV/0!");
            }
            Ok(V::Num((a / s).ceil() * s))
        }
        "FLOOR" | "FLOOR.MATH" => {
            let a = arg(args, 0)?.num()?;
            let s = args.get(1).and_then(|v| v.num().ok()).unwrap_or(1.0);
            if s == 0.0 {
                return err("#DIV/0!");
            }
            Ok(V::Num((a / s).floor() * s))
        }
        "MROUND" => {
            let a = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            if m == 0.0 {
                return Ok(V::Num(0.0));
            }
            Ok(V::Num((a / m).round() * m))
        }
        "EVEN" => {
            let a = arg(args, 0)?.num()?.ceil();
            Ok(V::Num(if (a as i64) % 2 == 0 { a } else { a + 1.0 }))
        }
        "ODD" => {
            let a = arg(args, 0)?.num()?.ceil();
            let i = a as i64;
            let up = if i % 2 != 0 { i } else { i + 1 };
            Ok(V::Num(up as f64))
        }
        "FACT" => {
            let n = arg(args, 0)?.num()?.floor().max(0.0) as u64;
            Ok(V::Num((1..=n).fold(1f64, |a, b| a * b as f64)))
        }
        "GCD" => {
            let mut g = 0i64;
            for v in nums(args) {
                g = gcd(g, v.abs() as i64);
            }
            Ok(V::Num(g as f64))
        }
        "LCM" => {
            let mut l = 1i64;
            for v in nums(args) {
                let n = v.abs() as i64;
                if n == 0 {
                    return Ok(V::Num(0.0));
                }
                l = (l / gcd(l, n)).abs() * n;
            }
            Ok(V::Num(l as f64))
        }
        "SUMSQ" => Ok(V::Num(nums(args).iter().map(|x| x * x).sum())),
        "SQRTPI" => Ok(V::Num((arg(args, 0)?.num()? * std::f64::consts::PI).sqrt())),
        // ---- 文本 ----
        "MID" => {
            let s = as_str(arg(args, 0)?);
            let start = arg(args, 1)?.num()?.max(1.0) as usize;
            let len = arg(args, 2)?.num()?.max(0.0) as usize;
            Ok(V::Str(s.chars().skip(start - 1).take(len).collect()))
        }
        "FIND" => {
            let needle = as_str(arg(args, 0)?);
            let hay = as_str(arg(args, 1)?);
            let start = args
                .get(2)
                .and_then(|v| v.num().ok())
                .unwrap_or(1.0)
                .max(1.0) as usize;
            let tail: String = hay.chars().skip(start - 1).collect();
            match tail.find(&needle) {
                Some(bi) => Ok(V::Num(
                    ((start - 1) + tail[..bi].chars().count() + 1) as f64,
                )),
                None => err("#VALUE!"),
            }
        }
        "SEARCH" => {
            let needle = as_str(arg(args, 0)?).to_lowercase();
            let hay = as_str(arg(args, 1)?).to_lowercase();
            match hay.find(&needle) {
                Some(bi) => Ok(V::Num((hay[..bi].chars().count() + 1) as f64)),
                None => err("#VALUE!"),
            }
        }
        "SUBSTITUTE" => {
            let s = as_str(arg(args, 0)?);
            let old = as_str(arg(args, 1)?);
            let new = as_str(arg(args, 2)?);
            match args.get(3).and_then(|v| v.num().ok()) {
                Some(n) if n >= 1.0 => {
                    let mut out = String::new();
                    let mut count = 0.0;
                    let mut rest = s.as_str();
                    if old.is_empty() {
                        return Ok(V::Str(s));
                    }
                    while let Some(i) = rest.find(&old) {
                        count += 1.0;
                        if count == n {
                            out.push_str(&rest[..i]);
                            out.push_str(&new);
                            out.push_str(&rest[i + old.len()..]);
                            return Ok(V::Str(out));
                        }
                        out.push_str(&rest[..i + old.len()]);
                        rest = &rest[i + old.len()..];
                    }
                    Ok(V::Str(out))
                }
                _ => Ok(V::Str(s.replace(&old, &new))),
            }
        }
        "REPLACE" => {
            let s = as_str(arg(args, 0)?);
            let start = arg(args, 1)?.num()?.max(1.0) as usize;
            let len = arg(args, 2)?.num()?.max(0.0) as usize;
            let new = as_str(arg(args, 3)?);
            let chars: Vec<char> = s.chars().collect();
            let mut out: String = chars.iter().take(start - 1).collect();
            out.push_str(&new);
            out.extend(chars.iter().skip(start - 1 + len));
            Ok(V::Str(out))
        }
        "REPT" => {
            let s = as_str(arg(args, 0)?);
            let n = arg(args, 1)?.num()?.max(0.0) as usize;
            Ok(V::Str(s.repeat(n)))
        }
        "PROPER" => Ok(V::Str(proper(&as_str(arg(args, 0)?)))),
        "CHAR" => Ok(V::Str(
            char::from_u32(arg(args, 0)?.num()? as u32)
                .map(|c| c.to_string())
                .unwrap_or_default(),
        )),
        "CLEAN" => Ok(V::Str(
            as_str(arg(args, 0)?)
                .chars()
                .filter(|c| !c.is_control())
                .collect(),
        )),
        "EXACT" => Ok(V::Bool(as_str(arg(args, 0)?) == as_str(arg(args, 1)?))),
        "TEXTJOIN" => {
            let delim = as_str(arg(args, 0)?);
            let ignore_empty = arg(args, 1)?.truthy().unwrap_or(false);
            let mut parts: Vec<String> = Vec::new();
            for (v, _) in flatten(&args[2..]) {
                let s = as_str(&v);
                if !(ignore_empty && s.is_empty()) {
                    parts.push(s);
                }
            }
            Ok(V::Str(parts.join(&delim)))
        }
        // ---- 逻辑 ----
        "TRUE" => Ok(V::Bool(true)),
        "FALSE" => Ok(V::Bool(false)),
        "XOR" => {
            let mut t = false;
            for (v, _) in flatten(args) {
                if v.truthy()? {
                    t = !t;
                }
            }
            Ok(V::Bool(t))
        }
        "IFS" => {
            let mut i = 0;
            while i + 1 < args.len() {
                if args[i].truthy()? {
                    return Ok(args[i + 1].clone());
                }
                i += 2;
            }
            err("#N/A")
        }
        "SWITCH" => {
            let expr = arg(args, 0)?;
            let mut i = 1;
            while i + 1 < args.len() {
                if compare(expr, &args[i], "=").unwrap_or(false) {
                    return Ok(args[i + 1].clone());
                }
                i += 2;
            }
            Ok(args.get(i).cloned().unwrap_or(V::Err("#N/A".into())))
        }
        // ---- 引用/查找（补充） ----
        "ROWS" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            Ok(V::Num((data.len() / cols.max(1)) as f64))
        }
        "COLUMNS" => {
            let (cols, _) = grid_of(arg(args, 0)?);
            Ok(V::Num(cols as f64))
        }
        "XLOOKUP" => {
            let key = arg(args, 0)?.clone();
            let (_, look) = grid_of(arg(args, 1)?);
            let (_, ret) = grid_of(arg(args, 2)?);
            for (i, v) in look.iter().enumerate() {
                if compare(v, &key, "=").unwrap_or(false) {
                    return Ok(ret.get(i).cloned().unwrap_or(V::Err("#REF!".into())));
                }
            }
            Ok(args.get(3).cloned().unwrap_or(V::Err("#N/A".into())))
        }
        "XMATCH" => {
            let key = arg(args, 0)?.clone();
            let (_, look) = grid_of(arg(args, 1)?);
            for (i, v) in look.iter().enumerate() {
                if compare(v, &key, "=").unwrap_or(false) {
                    return Ok(V::Num((i + 1) as f64));
                }
            }
            err("#N/A")
        }
        "LOOKUP" => {
            let key = arg(args, 0)?.clone();
            let (_, look) = grid_of(arg(args, 1)?);
            let (_, ret) = grid_of(args.get(2).unwrap_or(arg(args, 1)?));
            let mut best: Option<(usize, f64)> = None;
            for (i, v) in look.iter().enumerate() {
                if let (Ok(a), Ok(b)) = (v.num(), key.num()) {
                    if a <= b && best.map(|(_, bv)| a > bv).unwrap_or(true) {
                        best = Some((i, a));
                    }
                }
            }
            match best {
                Some((i, _)) => Ok(ret.get(i).cloned().unwrap_or(V::Err("#N/A".into()))),
                None => err("#N/A"),
            }
        }
        // ---- 日期 ----
        "TODAY" => Ok(V::Num(crate::utils::date::date_to_serial(
            chrono::Local::now().date_naive(),
            false,
        ) as f64)),
        "NOW" => {
            let now = chrono::Local::now();
            let days = crate::utils::date::date_to_serial(now.date_naive(), false) as f64;
            let secs = now.time().num_seconds_from_midnight() as f64;
            Ok(V::Num(days + secs / 86400.0))
        }
        "TIME" => {
            let h = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            let s = arg(args, 2)?.num()?;
            Ok(V::Num((h * 3600.0 + m * 60.0 + s) / 86400.0))
        }
        "HOUR" | "MINUTE" | "SECOND" => {
            let serial = arg(args, 0)?.num()?;
            let frac = serial - serial.floor();
            let total = (frac * 86400.0).round() as i64;
            Ok(V::Num(match up.as_str() {
                "HOUR" => (total / 3600) as f64,
                "MINUTE" => ((total % 3600) / 60) as f64,
                _ => (total % 60) as f64,
            }))
        }
        "DATEVALUE" => {
            let s = as_str(arg(args, 0)?);
            match chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
                Ok(d) => Ok(V::Num(crate::utils::date::date_to_serial(d, false) as f64)),
                Err(_) => err("#VALUE!"),
            }
        }
        "EDATE" => {
            let serial = arg(args, 0)?.num()? as i64;
            let months = arg(args, 1)?.num()? as i64;
            match crate::utils::date::serial_to_date(serial, false)
                .and_then(|d| add_months(d, months))
            {
                Some(d) => Ok(V::Num(crate::utils::date::date_to_serial(d, false) as f64)),
                None => err("#VALUE!"),
            }
        }
        "EOMONTH" => {
            let serial = arg(args, 0)?.num()? as i64;
            let months = arg(args, 1)?.num()? as i64;
            match crate::utils::date::serial_to_date(serial, false)
                .and_then(|d| add_months(d, months + 1))
                .and_then(|d| d.with_day(1))
                .map(|d| d - chrono::Days::new(1))
            {
                Some(d) => Ok(V::Num(crate::utils::date::date_to_serial(d, false) as f64)),
                None => err("#VALUE!"),
            }
        }
        "WEEKDAY" => {
            use chrono::Datelike;
            let serial = arg(args, 0)?.num()? as i64;
            match crate::utils::date::serial_to_date(serial, false) {
                Some(d) => {
                    let wd = d.weekday().num_days_from_sunday() as f64; // 0=Sun
                    let t = args.get(1).and_then(|v| v.num().ok()).unwrap_or(1.0) as i64;
                    let v = match t {
                        2 => (wd + 6.0) % 7.0 + 1.0, // Mon=1
                        3 => (wd + 6.0) % 7.0,       // Mon=0
                        _ => wd + 1.0,               // Sun=1
                    };
                    Ok(V::Num(v))
                }
                None => err("#VALUE!"),
            }
        }
        "DAYS" => Ok(V::Num(arg(args, 0)?.num()? - arg(args, 1)?.num()?)),
        // ---- 财务（补充） ----
        "NPER" => {
            let rate = arg(args, 0)?.num()?;
            let pmt = arg(args, 1)?.num()?;
            let pv = arg(args, 2)?.num()?;
            let fv = args.get(3).and_then(|v| v.num().ok()).unwrap_or(0.0);
            if rate == 0.0 {
                Ok(V::Num(-(pv + fv) / pmt))
            } else {
                Ok(V::Num(
                    (pmt - fv * rate).ln() / (1.0 + rate).ln()
                        - (pmt + pv * rate).ln() / (1.0 + rate).ln()
                        + 0.0,
                ))
            }
        }
        "SLN" => {
            let c = arg(args, 0)?.num()?;
            let s = arg(args, 1)?.num()?;
            let l = arg(args, 2)?.num()?;
            Ok(V::Num((c - s) / l))
        }
        "RATE" => {
            let nper = arg(args, 0)?.num()?;
            let pmt = arg(args, 1)?.num()?;
            let pv = arg(args, 2)?.num()?;
            let fv = args.get(3).and_then(|v| v.num().ok()).unwrap_or(0.0);
            match rate_solve(nper, pmt, pv, fv) {
                Some(r) => Ok(V::Num(r)),
                None => err("#NUM!"),
            }
        }
        "XNPV" => {
            let rate = arg(args, 0)?.num()?;
            let (_, vals) = grid_of(arg(args, 1)?);
            let (_, dates) = grid_of(arg(args, 2)?);
            let cf: Vec<(f64, f64)> = vals
                .iter()
                .zip(dates.iter())
                .filter_map(|(v, d)| Some((v.num().ok()?, d.num().ok()?)))
                .collect();
            Ok(V::Num(xnpv(rate, &cf)))
        }
        "XIRR" => {
            let (_, vals) = grid_of(arg(args, 0)?);
            let (_, dates) = grid_of(arg(args, 1)?);
            let cf: Vec<(f64, f64)> = vals
                .iter()
                .zip(dates.iter())
                .filter_map(|(v, d)| Some((v.num().ok()?, d.num().ok()?)))
                .collect();
            match xirr(&cf) {
                Some(r) => Ok(V::Num(r)),
                None => err("#NUM!"),
            }
        }
        // ---- 统计（补充） ----
        "AVERAGEIFS" => {
            let (_, target) = grid_of(arg(args, 0)?);
            let mut mask = vec![true; target.len()];
            let mut k = 1;
            while k + 1 < args.len() {
                let (_, rng) = grid_of(&args[k]);
                let crit = args[k + 1].clone();
                for (i, v) in rng.iter().enumerate() {
                    if i < mask.len() && !matches_crit(v, &crit).unwrap_or(false) {
                        mask[i] = false;
                    }
                }
                k += 2;
            }
            let sel: Vec<f64> = target
                .iter()
                .enumerate()
                .filter(|(i, _)| mask.get(*i).copied().unwrap_or(false))
                .filter_map(|(_, v)| v.num().ok())
                .collect();
            if sel.is_empty() {
                err("#DIV/0!")
            } else {
                Ok(V::Num(sel.iter().sum::<f64>() / sel.len() as f64))
            }
        }
        "MAXA" => Ok(V::Num(
            nums(args).iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )),
        "MINA" => Ok(V::Num(
            nums(args).iter().cloned().fold(f64::INFINITY, f64::min),
        )),
        "GEOMEAN" => {
            let v = nums(args);
            if v.is_empty() || v.iter().any(|x| *x <= 0.0) {
                return err("#NUM!");
            }
            Ok(V::Num(
                (v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp(),
            ))
        }
        "HARMEAN" => {
            let v = nums(args);
            if v.is_empty() || v.iter().any(|x| *x <= 0.0) {
                return err("#NUM!");
            }
            Ok(V::Num(
                v.len() as f64 / v.iter().map(|x| 1.0 / x).sum::<f64>(),
            ))
        }
        "AVEDEV" => {
            let v = nums(args);
            if v.is_empty() {
                return err("#NUM!");
            }
            let m = v.iter().sum::<f64>() / v.len() as f64;
            Ok(V::Num(
                v.iter().map(|x| (x - m).abs()).sum::<f64>() / v.len() as f64,
            ))
        }
        "DEVSQ" => {
            let v = nums(args);
            let m = v.iter().sum::<f64>() / v.len().max(1) as f64;
            Ok(V::Num(v.iter().map(|x| (x - m).powi(2)).sum()))
        }
        "RANK" | "RANK.EQ" => {
            let x = arg(args, 0)?.num()?;
            let v = {
                let (_, d) = grid_of(arg(args, 1)?);
                d.iter().filter_map(|v| v.num().ok()).collect::<Vec<f64>>()
            };
            let asc = args.get(2).and_then(|v| v.num().ok()).unwrap_or(0.0) != 0.0;
            let mut rank = 1.0;
            for y in &v {
                if (!asc && *y > x) || (asc && *y < x) {
                    rank += 1.0;
                }
            }
            Ok(V::Num(rank))
        }
        "PERCENTILE.INC" | "PERCENTILE" | "QUARTILE.INC" | "QUARTILE" => {
            let mut v = {
                let (_, d) = grid_of(arg(args, 0)?);
                d.iter().filter_map(|v| v.num().ok()).collect::<Vec<f64>>()
            };
            if v.is_empty() {
                return err("#NUM!");
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let k = if up.starts_with("QUARTILE") {
                arg(args, 1)?.num()? / 4.0
            } else {
                arg(args, 1)?.num()?
            };
            if !(0.0..=1.0).contains(&k) {
                return err("#NUM!");
            }
            let pos = k * (v.len() - 1) as f64;
            let lo = pos.floor() as usize;
            let hi = pos.ceil() as usize;
            let frac = pos - lo as f64;
            Ok(V::Num(v[lo] + (v[hi] - v[lo]) * frac))
        }
        "MODE.SNGL" | "MODE" => {
            let v = nums(args);
            let mut best = None;
            let mut bestc = 1;
            for (i, x) in v.iter().enumerate() {
                let c = v.iter().filter(|y| (*y - x).abs() < 1e-12).count();
                if c > bestc {
                    bestc = c;
                    best = Some(*x);
                }
                let _ = i;
            }
            best.map(V::Num).ok_or_else(|| "#N/A".to_string())
        }
        "CORREL" | "PEARSON" => {
            let xs = nums_of(arg(args, 0)?);
            let ys = nums_of(arg(args, 1)?);
            Ok(V::Num(correl(&xs, &ys)))
        }
        "COVAR.P" | "COVAR" => {
            let xs = nums_of(arg(args, 0)?);
            let ys = nums_of(arg(args, 1)?);
            let n = xs.len().min(ys.len());
            if n == 0 {
                return err("#DIV/0!");
            }
            let mx = xs.iter().take(n).sum::<f64>() / n as f64;
            let my = ys.iter().take(n).sum::<f64>() / n as f64;
            let c = xs
                .iter()
                .zip(ys.iter())
                .take(n)
                .map(|(a, b)| (a - mx) * (b - my))
                .sum::<f64>()
                / n as f64;
            Ok(V::Num(c))
        }
        "SLOPE" => {
            let ys = nums_of(arg(args, 0)?);
            let xs = nums_of(arg(args, 1)?);
            let n = xs.len().min(ys.len());
            let (mx, my) = (
                xs.iter().sum::<f64>() / n as f64,
                ys.iter().sum::<f64>() / n as f64,
            );
            let num: f64 = xs
                .iter()
                .zip(ys.iter())
                .map(|(x, y)| (x - mx) * (y - my))
                .sum();
            let den: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
            Ok(V::Num(num / den))
        }
        "INTERCEPT" => {
            let ys = nums_of(arg(args, 0)?);
            let xs = nums_of(arg(args, 1)?);
            let n = xs.len().min(ys.len());
            let (mx, my) = (
                xs.iter().sum::<f64>() / n as f64,
                ys.iter().sum::<f64>() / n as f64,
            );
            let num: f64 = xs
                .iter()
                .zip(ys.iter())
                .map(|(x, y)| (x - mx) * (y - my))
                .sum();
            let den: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
            Ok(V::Num(my - (num / den) * mx))
        }
        "RSQ" => {
            let xs = nums_of(arg(args, 0)?);
            let ys = nums_of(arg(args, 1)?);
            let c = correl(&xs, &ys);
            Ok(V::Num(c * c))
        }
        // ---- 数学（补充） ----
        "COMBIN" => {
            let n = arg(args, 0)?.num()?.floor().max(0.0) as u64;
            let k = arg(args, 1)?.num()?.floor().max(0.0) as u64;
            if k > n {
                return Ok(V::Num(0.0));
            }
            Ok(V::Num((fact(n) / (fact(k) * fact(n - k))).round()))
        }
        "PERMUT" => {
            let n = arg(args, 0)?.num()?.floor().max(0.0) as u64;
            let k = arg(args, 1)?.num()?.floor().max(0.0) as u64;
            if k > n {
                return Ok(V::Num(0.0));
            }
            Ok(V::Num((fact(n) / fact(n - k)).round()))
        }
        "MULTINOMIAL" => {
            let v = nums(args);
            let s: f64 = v.iter().sum::<f64>().max(0.0);
            Ok(V::Num(
                (fact(s as u64) / v.iter().map(|x| fact(x.max(0.0) as u64)).product::<f64>())
                    .round(),
            ))
        }
        "SEC" => Ok(V::Num(1.0 / arg(args, 0)?.num()?.cos())),
        "CSC" => Ok(V::Num(1.0 / arg(args, 0)?.num()?.sin())),
        "COT" => Ok(V::Num(1.0 / arg(args, 0)?.num()?.tan())),
        "ACOT" => Ok(V::Num(
            std::f64::consts::FRAC_PI_2 - arg(args, 0)?.num()?.atan(),
        )),
        "ASEC" => Ok(V::Num((1.0 / arg(args, 0)?.num()?).acos())),
        "ACSC" => Ok(V::Num((1.0 / arg(args, 0)?.num()?).asin())),
        "SUMXMY2" => {
            let xs = nums_of(arg(args, 0)?);
            let ys = nums_of(arg(args, 1)?);
            Ok(V::Num(
                xs.iter().zip(ys.iter()).map(|(a, b)| (a - b).powi(2)).sum(),
            ))
        }
        "SUMX2MY2" => {
            let xs = nums_of(arg(args, 0)?);
            let ys = nums_of(arg(args, 1)?);
            Ok(V::Num(
                xs.iter().zip(ys.iter()).map(|(a, b)| a * a - b * b).sum(),
            ))
        }
        "SUMX2PY2" => {
            let xs = nums_of(arg(args, 0)?);
            let ys = nums_of(arg(args, 1)?);
            Ok(V::Num(
                xs.iter().zip(ys.iter()).map(|(a, b)| a * a + b * b).sum(),
            ))
        }
        "ROMAN" => Ok(V::Str(to_roman(arg(args, 0)?.num()? as i64))),
        "ARABIC" => Ok(V::Num(from_roman(&as_str(arg(args, 0)?)) as f64)),
        "BASE" => {
            let n = arg(args, 0)?.num()?.max(0.0) as u64;
            let radix = arg(args, 1)?.num()? as u32;
            if !(2..=36).contains(&radix) {
                return err("#NUM!");
            }
            Ok(V::Str(base_repr(n, radix)))
        }
        "DECIMAL" => {
            let s = as_str(arg(args, 0)?);
            let radix = arg(args, 1)?.num()? as u32;
            Ok(V::Num(
                u64::from_str_radix(s.trim(), radix)
                    .map(|v| v as f64)
                    .unwrap_or(0.0),
            ))
        }
        "BITAND" => Ok(V::Num(
            (arg(args, 0)?.num()? as i64 & arg(args, 1)?.num()? as i64) as f64,
        )),
        "BITOR" => Ok(V::Num(
            (arg(args, 0)?.num()? as i64 | arg(args, 1)?.num()? as i64) as f64,
        )),
        "BITXOR" => Ok(V::Num(
            (arg(args, 0)?.num()? as i64 ^ arg(args, 1)?.num()? as i64) as f64,
        )),
        "BITLSHIFT" => Ok(V::Num(
            ((arg(args, 0)?.num()? as i64) << (arg(args, 1)?.num()? as i64)) as f64,
        )),
        "BITRSHIFT" => Ok(V::Num(
            ((arg(args, 0)?.num()? as i64) >> (arg(args, 1)?.num()? as i64)) as f64,
        )),
        // ---- 信息 ----
        "ISBLANK" => Ok(V::Bool(matches!(arg(args, 0)?, V::Empty))),
        "ISNUMBER" => Ok(V::Bool(matches!(arg(args, 0)?, V::Num(_)))),
        "ISTEXT" => Ok(V::Bool(matches!(arg(args, 0)?, V::Str(_)))),
        "ISLOGICAL" => Ok(V::Bool(matches!(arg(args, 0)?, V::Bool(_)))),
        "ISERROR" | "ISERR" => Ok(V::Bool(matches!(arg(args, 0)?, V::Err(_)))),
        "ISNA" => Ok(V::Bool(matches!(arg(args, 0)?, V::Err(e) if e == "#N/A"))),
        "ISEVEN" => Ok(V::Bool((arg(args, 0)?.num()?.floor() as i64) % 2 == 0)),
        "ISODD" => Ok(V::Bool((arg(args, 0)?.num()?.floor() as i64) % 2 != 0)),
        "NA" => err("#N/A"),
        "N" => Ok(V::Num(match arg(args, 0)? {
            V::Num(n) => *n,
            V::Bool(b) => *b as i32 as f64,
            _ => 0.0,
        })),
        "T" => Ok(V::Str(match arg(args, 0)? {
            V::Str(s) => s.clone(),
            _ => String::new(),
        })),
        // ---- 文本（补充） ----
        "NUMBERVALUE" => {
            let s = as_str(arg(args, 0)?);
            let dsep = args.get(1).map(as_str).unwrap_or_else(|| ".".into());
            let dch = dsep.chars().next().unwrap_or('.');
            let cleaned: String = s
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '-' || *c == '.' || *c == dch)
                .collect();
            let normalized = cleaned.replace(&dsep, ".");
            normalized
                .trim()
                .parse::<f64>()
                .map(V::Num)
                .map_err(|_| "#VALUE!".into())
        }
        "DOLLAR" => {
            let n = arg(args, 0)?.num()?;
            let d = args.get(1).and_then(|v| v.num().ok()).unwrap_or(2.0) as usize;
            Ok(V::Str(format!("${:.*}", d, n)))
        }
        "UNICHAR" => Ok(V::Str(
            char::from_u32(arg(args, 0)?.num()? as u32)
                .map(|c| c.to_string())
                .unwrap_or_default(),
        )),
        "UNICODE" => Ok(V::Num(
            as_str(arg(args, 0)?)
                .chars()
                .next()
                .map(|c| c as u32 as f64)
                .unwrap_or(0.0),
        )),
        "TEXTBEFORE" => {
            let s = as_str(arg(args, 0)?);
            let d = as_str(arg(args, 1)?);
            Ok(V::Str(s.split(&d).next().unwrap_or("").to_string()))
        }
        "TEXTAFTER" => {
            let s = as_str(arg(args, 0)?);
            let d = as_str(arg(args, 1)?);
            Ok(V::Str(
                s.split_once(&d).map(|x| x.1).unwrap_or("").to_string(),
            ))
        }
        // ---- 引用（补充） ----
        "ADDRESS" => {
            let r = arg(args, 0)?.num()? as u32;
            let c = arg(args, 1)?.num()? as u32;
            Ok(V::Str(crate::utils::a1::make_cell_ref(c, r)))
        }
        // ---- 日期（补充） ----
        "WEEKNUM" | "ISOWEEKNUM" => {
            use chrono::Datelike;
            let serial = arg(args, 0)?.num()? as i64;
            match crate::utils::date::serial_to_date(serial, false) {
                Some(d) => {
                    let _ = &up;
                    Ok(V::Num(d.iso_week().week() as f64))
                }
                None => err("#VALUE!"),
            }
        }
        "NETWORKDAYS" => {
            let a = arg(args, 0)?.num()? as i64;
            let b = arg(args, 1)?.num()? as i64;
            let (lo, hi) = (a.min(b), a.max(b));
            let mut c = 0.0;
            for s in lo..=hi {
                if let Some(d) = crate::utils::date::serial_to_date(s, false) {
                    let wd = d.weekday().num_days_from_monday();
                    if wd < 5 {
                        c += 1.0;
                    }
                }
            }
            Ok(V::Num(c))
        }
        "WORKDAY" => {
            let mut s = arg(args, 0)?.num()? as i64;
            let mut days = arg(args, 1)?.num()? as i64;
            let step = if days >= 0 { 1 } else { -1 };
            days = days.abs();
            while days > 0 {
                s += step;
                if let Some(d) = crate::utils::date::serial_to_date(s, false) {
                    if d.weekday().num_days_from_monday() < 5 {
                        days -= 1;
                    }
                }
            }
            Ok(V::Num(s as f64))
        }
        // ---- 财务（补充） ----
        "IPMT" | "PPMT" => {
            let rate = arg(args, 0)?.num()?;
            let per = arg(args, 1)?.num()?;
            let nper = arg(args, 2)?.num()?;
            let pv = arg(args, 3)?.num()?;
            let fv = args.get(4).and_then(|v| v.num().ok()).unwrap_or(0.0);
            let pmt = if rate == 0.0 {
                -(pv + fv) / nper
            } else {
                -rate * (pv * (1.0 + rate).powf(nper) + fv) / ((1.0 + rate).powf(nper) - 1.0)
            };
            let balance = if rate == 0.0 {
                pv
            } else {
                pv * (1.0 + rate).powf(per - 1.0)
                    + pmt * ((1.0 + rate).powf(per - 1.0) - 1.0) / rate
            };
            let interest = balance * rate;
            Ok(V::Num(if up == "IPMT" {
                interest
            } else {
                pmt - interest
            }))
        }
        "EFFECT" => {
            let nom = arg(args, 0)?.num()?;
            let n = arg(args, 1)?.num()?;
            Ok(V::Num((1.0 + nom / n).powf(n) - 1.0))
        }
        "NOMINAL" => {
            let eff = arg(args, 0)?.num()?;
            let n = arg(args, 1)?.num()?;
            Ok(V::Num(n * ((1.0 + eff).powf(1.0 / n) - 1.0)))
        }
        // ---- 统计（第三批） ----
        "STDEVA" | "STDEVPA" | "VARA" | "VARPA" => {
            let v: Vec<f64> = flatten(args)
                .into_iter()
                .map(|(x, _)| match x {
                    V::Num(n) => n,
                    V::Bool(b) => b as i32 as f64,
                    _ => 0.0,
                })
                .collect();
            let n = v.len();
            if n < 2 {
                return err("#DIV/0!");
            }
            let m = v.iter().sum::<f64>() / n as f64;
            let ss: f64 = v.iter().map(|x| (x - m).powi(2)).sum();
            let pop = up.ends_with('A') && up.starts_with("STDEV")
                || up.starts_with("VAR") && up.ends_with("PA")
                || up == "STDEVPA"
                || up == "VARPA";
            let denom = if pop { n as f64 } else { (n - 1) as f64 };
            let var = ss / denom;
            Ok(V::Num(if up.starts_with("STDEV") {
                var.sqrt()
            } else {
                var
            }))
        }
        "TYPE" => Ok(V::Num(match arg(args, 0)? {
            V::Num(_) => 1.0,
            V::Str(_) => 2.0,
            V::Bool(_) => 4.0,
            V::Err(_) => 16.0,
            V::Grid { .. } => 64.0,
            V::Lambda(_) => 128.0,
            V::Empty => 1.0,
        })),
        "ERROR.TYPE" => Ok(V::Num(match arg(args, 0)? {
            V::Err(e) => match e.as_str() {
                "#NULL!" => 1.0,
                "#DIV/0!" => 2.0,
                "#VALUE!" => 3.0,
                "#REF!" => 4.0,
                "#NAME?" => 5.0,
                "#NUM!" => 6.0,
                "#N/A" => 7.0,
                _ => 0.0,
            },
            _ => return err("#N/A"),
        })),
        "VALUETOTEXT" | "FORMULATOTEXT" => Ok(V::Str(as_str(arg(args, 0)?))),
        // ---- 日期（第三批） ----
        "DAYS360" => {
            let (s, e) = (arg(args, 0)?.num()? as i64, arg(args, 1)?.num()? as i64);
            Ok(V::Num(days360(s, e) as f64))
        }
        "YEARFRAC" => {
            let s = arg(args, 0)?.num()? as i64;
            let e = arg(args, 1)?.num()? as i64;
            let basis = args.get(2).and_then(|v| v.num().ok()).unwrap_or(0.0) as i32;
            Ok(V::Num(yearfrac(s, e, basis)))
        }
        // ---- 财务（第三批） ----
        "DDB" => {
            let cost = arg(args, 0)?.num()?;
            let salvage = arg(args, 1)?.num()?;
            let life = arg(args, 2)?.num()?;
            let period = arg(args, 3)?.num()?;
            let factor = args.get(4).and_then(|v| v.num().ok()).unwrap_or(2.0);
            Ok(V::Num(ddb(cost, salvage, life, period, factor)))
        }
        "DB" => {
            let cost = arg(args, 0)?.num()?;
            let salvage = arg(args, 1)?.num()?;
            let life = arg(args, 2)?.num()?;
            let period = arg(args, 3)?.num()?;
            let month = args.get(4).and_then(|v| v.num().ok()).unwrap_or(12.0);
            Ok(V::Num(db(cost, salvage, life, period, month)))
        }
        "FVSCHEDULE" => {
            let p = arg(args, 0)?.num()?;
            let rates = nums_of(arg(args, 1)?);
            Ok(V::Num(rates.iter().fold(p, |a, r| a * (1.0 + r))))
        }
        "CUMIPMT" | "CUMPRINC" => {
            let rate = arg(args, 0)?.num()?;
            let nper = arg(args, 1)?.num()?;
            let pv = arg(args, 2)?.num()?;
            let start = arg(args, 3)?.num()?;
            let end = arg(args, 4)?.num()?;
            let typ = args.get(5).and_then(|v| v.num().ok()).unwrap_or(0.0);
            let pmt = if rate == 0.0 {
                -(pv) / nper
            } else {
                -rate * (pv * (1.0 + rate).powf(nper)) / ((1.0 + rate).powf(nper) - 1.0)
            };
            let mut acc = 0.0;
            let s = start as i64;
            let e = end as i64;
            for per in s..=e {
                let balance = if rate == 0.0 {
                    pv
                } else {
                    pv * (1.0 + rate).powf(per as f64 - 1.0 + typ * 0.0)
                        + pmt * ((1.0 + rate).powf(per as f64 - 1.0) - 1.0) / rate
                };
                let interest = balance * rate;
                acc += if up == "CUMIPMT" {
                    interest
                } else {
                    pmt - interest
                };
            }
            Ok(V::Num(acc))
        }
        "DOLLARDE" => {
            let d = arg(args, 0)?.num()?;
            let f = arg(args, 1)?.num()?;
            let int = d.trunc();
            let frac = d - int;
            Ok(V::Num(int + frac * 100.0 / f))
        }
        "DOLLARFR" => {
            let d = arg(args, 0)?.num()?;
            let f = arg(args, 1)?.num()?;
            let int = d.trunc();
            let frac = d - int;
            Ok(V::Num(int + frac * f / 100.0))
        }
        // ---- 工程（数制/逻辑） ----
        "HEX2DEC" => Ok(V::Num(
            u64::from_str_radix(as_str(arg(args, 0)?).trim(), 16)
                .map(|v| v as f64)
                .unwrap_or(0.0),
        )),
        "BIN2DEC" => Ok(V::Num(
            u64::from_str_radix(as_str(arg(args, 0)?).trim(), 2)
                .map(|v| v as f64)
                .unwrap_or(0.0),
        )),
        "OCT2DEC" => Ok(V::Num(
            u64::from_str_radix(as_str(arg(args, 0)?).trim(), 8)
                .map(|v| v as f64)
                .unwrap_or(0.0),
        )),
        "DEC2HEX" => Ok(V::Str(base_repr(arg(args, 0)?.num()?.max(0.0) as u64, 16))),
        "DEC2BIN" => Ok(V::Str(base_repr(arg(args, 0)?.num()?.max(0.0) as u64, 2))),
        "DEC2OCT" => Ok(V::Str(base_repr(arg(args, 0)?.num()?.max(0.0) as u64, 8))),
        "DELTA" => Ok(V::Num(
            if arg(args, 0)?.num()? == args.get(1).and_then(|v| v.num().ok()).unwrap_or(0.0) {
                1.0
            } else {
                0.0
            },
        )),
        "GESTEP" => {
            let a = arg(args, 0)?.num()?;
            let s = args.get(1).and_then(|v| v.num().ok()).unwrap_or(0.0);
            Ok(V::Num(if a >= s { 1.0 } else { 0.0 }))
        }
        // ---- 汇总（SUBTOTAL） ----
        "SUBTOTAL" => {
            let fnum = arg(args, 0)?.num()? as i64;
            let body = &args[1..];
            let code = ((fnum - 1) % 100) + 1;
            let v = nums(body);
            match code {
                1 => {
                    if v.is_empty() {
                        err("#DIV/0!")
                    } else {
                        Ok(V::Num(v.iter().sum::<f64>() / v.len() as f64))
                    }
                }
                2 | 3 => Ok(V::Num(v.len() as f64)),
                4 => Ok(V::Num(v.iter().cloned().fold(f64::NEG_INFINITY, f64::max))),
                5 => Ok(V::Num(v.iter().cloned().fold(f64::INFINITY, f64::min))),
                6 => Ok(V::Num(v.iter().product())),
                7 | 8 => {
                    if v.len() < 2 {
                        err("#DIV/0!")
                    } else {
                        let m = v.iter().sum::<f64>() / v.len() as f64;
                        let ss: f64 = v.iter().map(|x| (x - m).powi(2)).sum();
                        let var = ss / (v.len() - 1) as f64;
                        Ok(V::Num(if code == 7 { var.sqrt() } else { var }))
                    }
                }
                9 => Ok(V::Num(v.iter().sum())),
                _ => err("#VALUE!"),
            }
        }
        // ---- 分布函数（第四批） ----
        "NORM.DIST" | "NORMDIST" => {
            let x = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            let sd = arg(args, 2)?.num()?;
            let cum = arg(args, 3)?.truthy()?;
            let z = (x - m) / sd;
            Ok(V::Num(if cum {
                norm_cdf(z)
            } else {
                (-0.5 * z * z).exp() / (sd * (2.0 * std::f64::consts::PI).sqrt())
            }))
        }
        "NORM.S.DIST" | "NORMSDIST" => {
            let z = arg(args, 0)?.num()?;
            let cum = arg(args, 1)?.truthy()?;
            Ok(V::Num(if cum {
                norm_cdf(z)
            } else {
                (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
            }))
        }
        "NORM.INV" | "NORMINV" => {
            let p = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            let sd = arg(args, 2)?.num()?;
            if !(0.0..1.0).contains(&p) {
                return err("#NUM!");
            }
            Ok(V::Num(m + sd * norm_s_inv(p)))
        }
        "NORM.S.INV" | "NORMSINV" => {
            let p = arg(args, 0)?.num()?;
            if !(0.0..1.0).contains(&p) {
                return err("#NUM!");
            }
            Ok(V::Num(norm_s_inv(p)))
        }
        "LOGNORM.DIST" | "LOGNORMDIST" => {
            let x = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            let sd = arg(args, 2)?.num()?;
            let cum = args.get(3).map(|v| v.truthy()).transpose()?.unwrap_or(true);
            if x <= 0.0 {
                return err("#NUM!");
            }
            let z = (x.ln() - m) / sd;
            Ok(V::Num(if cum {
                norm_cdf(z)
            } else {
                (-0.5 * z * z).exp() / (x * sd * (2.0 * std::f64::consts::PI).sqrt())
            }))
        }
        "LOGNORM.INV" | "LOGINV" => {
            let p = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            let sd = arg(args, 2)?.num()?;
            if !(0.0..1.0).contains(&p) {
                return err("#NUM!");
            }
            Ok(V::Num((m + sd * norm_s_inv(p)).exp()))
        }
        "EXPON.DIST" | "EXPONDIST" => {
            let x = arg(args, 0)?.num()?;
            let l = arg(args, 1)?.num()?;
            let cum = arg(args, 2)?.truthy()?;
            Ok(V::Num(if cum {
                1.0 - (-l * x).exp()
            } else {
                l * (-l * x).exp()
            }))
        }
        "POISSON.DIST" | "POISSON" => {
            let x = arg(args, 0)?.num()? as i64;
            let m = arg(args, 1)?.num()?;
            let cum = arg(args, 2)?.truthy()?;
            if cum {
                Ok(V::Num(1.0 - reg_lower_gamma((x + 1) as f64, m)))
            } else {
                Ok(V::Num(
                    (-m).exp() * m.powi(x as i32) / fact(x.max(0) as u64),
                ))
            }
        }
        "BINOM.DIST" | "BINOMDIST" => {
            let k = arg(args, 0)?.num()? as u64;
            let n = arg(args, 1)?.num()? as u64;
            let p = arg(args, 2)?.num()?;
            let cum = arg(args, 3)?.truthy()?;
            if k > n {
                return Ok(V::Num(if cum { 1.0 } else { 0.0 }));
            }
            if cum {
                Ok(V::Num(reg_lower_beta(
                    (n - k) as f64,
                    (k + 1) as f64,
                    1.0 - p,
                )))
            } else {
                let c = fact(n) / (fact(k) * fact(n - k));
                Ok(V::Num(
                    c * p.powi(k as i32) * (1.0 - p).powi((n - k) as i32),
                ))
            }
        }
        "GAMMA" => Ok(V::Num(gamma_fn(arg(args, 0)?.num()?))),
        "GAMMALN" => Ok(V::Num(gamma_ln(arg(args, 0)?.num()?))),
        "GAMMA.DIST" | "GAMMADIST" => {
            let x = arg(args, 0)?.num()?;
            let a = arg(args, 1)?.num()?;
            let b = arg(args, 2)?.num()?;
            let cum = arg(args, 3)?.truthy()?;
            if cum {
                Ok(V::Num(reg_lower_gamma(a, x / b)))
            } else {
                Ok(V::Num(
                    x.powf(a - 1.0) * (-x / b).exp() / (gamma_fn(a) * b.powf(a)),
                ))
            }
        }
        "GAMMA.INV" | "GAMMAINV" => {
            let p = arg(args, 0)?.num()?;
            let a = arg(args, 1)?.num()?;
            let b = arg(args, 2)?.num()?;
            match invert(p, 1e-9, |x| reg_lower_gamma(a, x)) {
                Some(x) => Ok(V::Num(x * b)),
                None => err("#NUM!"),
            }
        }
        "BETA.DIST" | "BETADIST" => {
            let x = arg(args, 0)?.num()?;
            let a = arg(args, 1)?.num()?;
            let b = arg(args, 2)?.num()?;
            let cum = args.get(3).map(|v| v.truthy()).transpose()?.unwrap_or(true);
            if cum {
                Ok(V::Num(reg_lower_beta(a, b, x)))
            } else {
                Ok(V::Num(
                    x.powf(a - 1.0) * (1.0 - x).powf(b - 1.0) * gamma_fn(a + b)
                        / (gamma_fn(a) * gamma_fn(b)),
                ))
            }
        }
        "BETA.INV" | "BETAINV" => {
            let p = arg(args, 0)?.num()?;
            let a = arg(args, 1)?.num()?;
            let b = arg(args, 2)?.num()?;
            match invert(p, 1e-10, |x| reg_lower_beta(a, b, x)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "CHISQ.DIST" | "CHISQDIST" => {
            let x = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            let cum = args.get(2).map(|v| v.truthy()).transpose()?.unwrap_or(true);
            if cum {
                Ok(V::Num(reg_lower_gamma(df / 2.0, x / 2.0)))
            } else {
                Ok(V::Num(
                    x.powf(df / 2.0 - 1.0) * (-x / 2.0).exp()
                        / (2f64.powf(df / 2.0) * gamma_fn(df / 2.0)),
                ))
            }
        }
        "CHISQ.INV" | "CHIINV" => {
            let p = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            match invert(p, 1e-9, |x| reg_lower_gamma(df / 2.0, x / 2.0)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "T.DIST" | "TDIST" => {
            let x = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            let cum = args.get(2).map(|v| v.truthy()).transpose()?.unwrap_or(true);
            let t = df / (df + x * x);
            let ib = 0.5 * reg_lower_beta(df / 2.0, 0.5, t);
            if cum {
                Ok(V::Num(if x >= 0.0 { 1.0 - ib } else { ib }))
            } else {
                Ok(V::Num(
                    gamma_fn((df + 1.0) / 2.0)
                        / ((df * std::f64::consts::PI).sqrt() * gamma_fn(df / 2.0))
                        * (1.0 + x * x / df).powf(-(df + 1.0) / 2.0),
                ))
            }
        }
        "F.DIST" | "FDIST" => {
            let x = arg(args, 0)?.num()?;
            let d1 = arg(args, 1)?.num()?;
            let d2 = arg(args, 2)?.num()?;
            let cum = args.get(3).map(|v| v.truthy()).transpose()?.unwrap_or(true);
            if cum {
                Ok(V::Num(reg_lower_beta(
                    d1 / 2.0,
                    d2 / 2.0,
                    d1 * x / (d1 * x + d2),
                )))
            } else {
                let num = (d1 * x).powf(d1) * d2.powf(d2);
                let den = (d1 * x + d2).powf(d1 + d2);
                Ok(V::Num(
                    (num / den).sqrt() / (x * beta_fn(d1 / 2.0, d2 / 2.0)),
                ))
            }
        }
        "CONFIDENCE.NORM" | "CONFIDENCE" => {
            let alpha = arg(args, 0)?.num()?;
            let sd = arg(args, 1)?.num()?;
            let n = arg(args, 2)?.num()?;
            Ok(V::Num(norm_s_inv(1.0 - alpha / 2.0) * sd / n.sqrt()))
        }
        "WEIBULL.DIST" | "WEIBULL" => {
            let x = arg(args, 0)?.num()?;
            let a = arg(args, 1)?.num()?;
            let b = arg(args, 2)?.num()?;
            let cum = arg(args, 3)?.truthy()?;
            Ok(V::Num(if cum {
                1.0 - (-(x / b).powf(a)).exp()
            } else {
                (a / b.powf(a)) * x.powf(a - 1.0) * (-(x / b).powf(a)).exp()
            }))
        }
        "PERCENTRANK.INC" | "PERCENTRANK" => {
            let (_, d) = grid_of(arg(args, 0)?);
            let mut v: Vec<f64> = d.iter().filter_map(|x| x.num().ok()).collect();
            let x = arg(args, 1)?.num()?;
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let below = v.iter().filter(|y| **y < x).count();
            let n = v.len();
            Ok(V::Num(if n <= 1 {
                0.0
            } else {
                below as f64 / (n - 1) as f64
            }))
        }
        "TRIMMEAN" => {
            let (_, d) = grid_of(arg(args, 0)?);
            let mut v: Vec<f64> = d.iter().filter_map(|x| x.num().ok()).collect();
            let frac = arg(args, 1)?.num()?;
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let k = ((v.len() as f64 * frac) / 2.0).floor() as usize;
            let n = v.len();
            if 2 * k >= n {
                return err("#NUM!");
            }
            let trimmed = &v[k..n - k];
            Ok(V::Num(trimmed.iter().sum::<f64>() / trimmed.len() as f64))
        }
        // ---- 分布（第五批：反函数/检验/补充） ----
        "STANDARDIZE" => {
            let x = arg(args, 0)?.num()?;
            let m = arg(args, 1)?.num()?;
            let sd = arg(args, 2)?.num()?;
            Ok(V::Num((x - m) / sd))
        }
        "GAUSS" => Ok(V::Num(norm_cdf(arg(args, 0)?.num()?) - 0.5)),
        "PHI" => {
            let z = arg(args, 0)?.num()?;
            Ok(V::Num(
                (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt(),
            ))
        }
        "T.DIST.RT" => {
            let x = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            Ok(V::Num(1.0 - t_cdf(x, df)))
        }
        "T.DIST.2T" => {
            let x = arg(args, 0)?.num()?.abs();
            let df = arg(args, 1)?.num()?;
            Ok(V::Num(2.0 * (1.0 - t_cdf(x, df))))
        }
        "T.INV" => {
            let p = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            match invert_signed(p, 1e-9, |x| t_cdf(x, df)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "T.INV.2T" => {
            let p = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            let target = 1.0 - p / 2.0;
            match invert_signed(target, 1e-9, |x| t_cdf(x, df)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "F.DIST.RT" => {
            let x = arg(args, 0)?.num()?;
            let d1 = arg(args, 1)?.num()?;
            let d2 = arg(args, 2)?.num()?;
            Ok(V::Num(1.0 - f_cdf(x, d1, d2)))
        }
        "F.INV" => {
            let p = arg(args, 0)?.num()?;
            let d1 = arg(args, 1)?.num()?;
            let d2 = arg(args, 2)?.num()?;
            match invert(p, 1e-9, |x| f_cdf(x, d1, d2)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "F.INV.RT" => {
            let p = arg(args, 0)?.num()?;
            let d1 = arg(args, 1)?.num()?;
            let d2 = arg(args, 2)?.num()?;
            match invert(1.0 - p, 1e-9, |x| f_cdf(x, d1, d2)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "CHISQ.DIST.RT" => {
            let x = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            Ok(V::Num(1.0 - reg_lower_gamma(df / 2.0, x / 2.0)))
        }
        "CHISQ.INV.RT" => {
            let p = arg(args, 0)?.num()?;
            let df = arg(args, 1)?.num()?;
            match invert(1.0 - p, 1e-9, |x| reg_lower_gamma(df / 2.0, x / 2.0)) {
                Some(x) => Ok(V::Num(x)),
                None => err("#NUM!"),
            }
        }
        "Z.TEST" | "ZTEST" => {
            let (_, d) = grid_of(arg(args, 0)?);
            let v: Vec<f64> = d.iter().filter_map(|x| x.num().ok()).collect();
            let x = arg(args, 1)?.num()?;
            let sigma = args
                .get(2)
                .and_then(|v| v.num().ok())
                .unwrap_or_else(|| stdev_s(&v));
            let n = v.len() as f64;
            if n == 0.0 || sigma == 0.0 {
                return err("#DIV/0!");
            }
            let mean = v.iter().sum::<f64>() / n;
            Ok(V::Num(1.0 - norm_cdf((mean - x) / (sigma / n.sqrt()))))
        }
        "T.TEST" | "TTEST" => {
            let a = nums_of(arg(args, 0)?);
            let b = nums_of(arg(args, 1)?);
            let tails = arg(args, 2)?.num()?;
            let typ = arg(args, 3)?.num()? as i32;
            Ok(V::Num(ttest(&a, &b, tails, typ)))
        }
        "F.TEST" | "FTEST" => {
            let a = nums_of(arg(args, 0)?);
            let b = nums_of(arg(args, 1)?);
            let (v1, v2) = (var_s(&a), var_s(&b));
            let (f, d1, d2) = if v1 >= v2 {
                (v1 / v2, (a.len() - 1) as f64, (b.len() - 1) as f64)
            } else {
                (v2 / v1, (b.len() - 1) as f64, (a.len() - 1) as f64)
            };
            let p = 2.0 * (1.0 - f_cdf(f, d1, d2));
            Ok(V::Num(p.min(1.0)))
        }
        "CHISQ.TEST" | "CHITEST" => {
            let (cols, obs) = grid_of(arg(args, 0)?);
            let (_, exp) = grid_of(arg(args, 1)?);
            if cols == 0 {
                return err("#DIV/0!");
            }
            let rows = obs.len() / cols;
            let mut chi = 0.0;
            for i in 0..obs.len().min(exp.len()) {
                let o = obs[i].num()?;
                let e = exp[i].num()?;
                if e != 0.0 {
                    chi += (o - e).powi(2) / e;
                }
            }
            let df = ((rows - 1) * (cols - 1)) as f64;
            if df <= 0.0 {
                return err("#N/A");
            }
            Ok(V::Num(1.0 - reg_lower_gamma(df / 2.0, chi / 2.0)))
        }
        "HYPGEOM.DIST" | "HYPGEOMDIST" => {
            let x = arg(args, 0)?.num()? as u64;
            let n = arg(args, 1)?.num()? as u64;
            let k = arg(args, 2)?.num()? as u64;
            let nn = arg(args, 3)?.num()? as u64;
            let cum = args
                .get(4)
                .map(|v| v.truthy())
                .transpose()?
                .unwrap_or(false);
            let pmf = |x: u64| comb_u(k, x) * comb_u(nn - k, n - x) / comb_u(nn, n);
            if cum {
                let mut s = 0.0;
                for i in 0..=x {
                    s += pmf(i);
                }
                Ok(V::Num(s))
            } else {
                Ok(V::Num(pmf(x)))
            }
        }
        "NEGBINOM.DIST" | "NEGBINOMDIST" => {
            let f = arg(args, 0)?.num()? as u64;
            let s = arg(args, 1)?.num()? as u64;
            let p = arg(args, 2)?.num()?;
            let cum = args
                .get(3)
                .map(|v| v.truthy())
                .transpose()?
                .unwrap_or(false);
            let pmf =
                |f: u64| comb_u(f + s - 1, s - 1) * p.powi(s as i32) * (1.0 - p).powi(f as i32);
            if cum {
                let mut sum = 0.0;
                for i in 0..=f {
                    sum += pmf(i);
                }
                Ok(V::Num(sum))
            } else {
                Ok(V::Num(pmf(f)))
            }
        }
        "RANK.AVG" => {
            let x = arg(args, 0)?.num()?;
            let (_, d) = grid_of(arg(args, 1)?);
            let v: Vec<f64> = d.iter().filter_map(|v| v.num().ok()).collect();
            let asc = args.get(2).and_then(|v| v.num().ok()).unwrap_or(0.0) != 0.0;
            let mut rank = 1.0;
            let mut ties = 0.0;
            for y in &v {
                if (!asc && *y > x) || (asc && *y < x) {
                    rank += 1.0;
                } else if (*y - x).abs() < 1e-12 {
                    ties += 1.0;
                }
            }
            Ok(V::Num(rank + (ties - 1.0) / 2.0))
        }
        // ---- 财务/日期（补充） ----
        "PDURATION" => {
            let rate = arg(args, 0)?.num()?;
            let pv = arg(args, 1)?.num()?;
            let fv = arg(args, 2)?.num()?;
            Ok(V::Num((fv / pv).ln() / (1.0 + rate).ln()))
        }
        "RRI" => {
            let n = arg(args, 0)?.num()?;
            let pv = arg(args, 1)?.num()?;
            let fv = arg(args, 2)?.num()?;
            Ok(V::Num((fv / pv).powf(1.0 / n) - 1.0))
        }
        "NETWORKDAYS.INTL" => {
            let a = arg(args, 0)?.num()? as i64;
            let b = arg(args, 1)?.num()? as i64;
            let wk = args.get(2).and_then(|v| v.num().ok()).unwrap_or(1.0) as i64;
            let (lo, hi) = (a.min(b), a.max(b));
            let mut c = 0.0;
            for s in lo..=hi {
                if let Some(d) = crate::utils::date::serial_to_date(s, false) {
                    if is_workday(d.weekday().num_days_from_monday() as i64, wk) {
                        c += 1.0;
                    }
                }
            }
            Ok(V::Num(c))
        }
        "WORKDAY.INTL" => {
            let mut s = arg(args, 0)?.num()? as i64;
            let mut days = arg(args, 1)?.num()? as i64;
            let wk = args.get(2).and_then(|v| v.num().ok()).unwrap_or(1.0) as i64;
            let step = if days >= 0 { 1 } else { -1 };
            days = days.abs();
            while days > 0 {
                s += step;
                if let Some(d) = crate::utils::date::serial_to_date(s, false) {
                    if is_workday(d.weekday().num_days_from_monday() as i64, wk) {
                        days -= 1;
                    }
                }
            }
            Ok(V::Num(s as f64))
        }
        "DATESTRING" => {
            use chrono::Datelike;
            let serial = arg(args, 0)?.num()? as i64;
            match crate::utils::date::serial_to_date(serial, false) {
                Some(d) => Ok(V::Str(format!(
                    "{:02}/{:02}/{:02}",
                    d.year() % 100,
                    d.month(),
                    d.day()
                ))),
                None => err("#VALUE!"),
            }
        }
        // ---- 数组/动态数组（第六批） ----
        "TRANSPOSE" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            if cols == 0 {
                return Ok(V::Grid {
                    cols: 0,
                    data: vec![],
                });
            }
            let rows = data.len() / cols;
            let mut out = Vec::with_capacity(data.len());
            for c in 0..cols {
                for r in 0..rows {
                    out.push(data[r * cols + c].clone());
                }
            }
            Ok(V::Grid {
                cols: rows,
                data: out,
            })
        }
        "SEQUENCE" => {
            let rows = arg(args, 0)?.num()? as usize;
            let cols = args.get(1).and_then(|v| v.num().ok()).unwrap_or(1.0) as usize;
            let start = args.get(2).and_then(|v| v.num().ok()).unwrap_or(1.0);
            let step = args.get(3).and_then(|v| v.num().ok()).unwrap_or(1.0);
            let mut data = Vec::with_capacity(rows * cols);
            for i in 0..(rows * cols) {
                data.push(V::Num(start + i as f64 * step));
            }
            Ok(V::Grid {
                cols: cols.max(1),
                data,
            })
        }
        "MUNIT" => {
            let n = arg(args, 0)?.num()? as usize;
            let mut data = Vec::with_capacity(n * n);
            for i in 0..n {
                for j in 0..n {
                    data.push(V::Num(if i == j { 1.0 } else { 0.0 }));
                }
            }
            Ok(V::Grid {
                cols: n.max(1),
                data,
            })
        }
        "MDETERM" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let m: Vec<f64> = data.iter().map(|v| v.num().unwrap_or(0.0)).collect();
            Ok(V::Num(determinant(&m, rows, cols)))
        }
        "MMULT" => {
            let (ca, a) = grid_of(arg(args, 0)?);
            let (cb, b) = grid_of(arg(args, 1)?);
            let ra = a.len() / ca.max(1);
            let rb = b.len() / cb.max(1);
            let (an, bn): (Vec<f64>, Vec<f64>) = (
                a.iter().map(|v| v.num().unwrap_or(0.0)).collect(),
                b.iter().map(|v| v.num().unwrap_or(0.0)).collect(),
            );
            let mut out = Vec::new();
            for i in 0..ra {
                for j in 0..cb {
                    let mut s = 0.0;
                    for k in 0..rb.min(ca) {
                        s += an[i * ca + k] * bn[k * cb + j];
                    }
                    out.push(V::Num(s));
                }
            }
            Ok(V::Grid {
                cols: cb.max(1),
                data: out,
            })
        }
        "MINVERSE" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let m: Vec<f64> = data.iter().map(|v| v.num().unwrap_or(0.0)).collect();
            match inverse(&m, rows, cols) {
                Some(inv) => Ok(V::Grid {
                    cols,
                    data: inv.into_iter().map(V::Num).collect(),
                }),
                None => err("#NUM!"),
            }
        }
        "FREQUENCY" => {
            let (_, data) = grid_of(arg(args, 0)?);
            let (_, bins) = grid_of(arg(args, 1)?);
            let vals: Vec<f64> = data.iter().filter_map(|v| v.num().ok()).collect();
            let bs: Vec<f64> = bins.iter().filter_map(|v| v.num().ok()).collect();
            let mut out = vec![V::Num(0.0); bs.len() + 1];
            for x in vals {
                let mut idx = bs.len();
                for (i, b) in bs.iter().enumerate() {
                    if x <= *b {
                        idx = i;
                        break;
                    }
                }
                if let V::Num(n) = &mut out[idx] {
                    *n += 1.0;
                }
            }
            Ok(V::Grid { cols: 1, data: out })
        }
        "HSTACK" | "VSTACK" => {
            let grids: Vec<(usize, Vec<V>)> = args.iter().map(grid_of).collect();
            if up == "HSTACK" {
                let rows = grids
                    .iter()
                    .map(|(c, d)| if *c == 0 { 0 } else { d.len() / c })
                    .max()
                    .unwrap_or(0);
                let tcols: usize = grids.iter().map(|(c, _)| *c).sum();
                let mut data = Vec::with_capacity(rows * tcols);
                for r in 0..rows {
                    for (c, d) in &grids {
                        let grim = if *c == 0 { 0 } else { d.len() / c };
                        for cc in 0..*c {
                            data.push(if r < grim {
                                d.get(r * c + cc).cloned().unwrap_or(V::Empty)
                            } else {
                                V::Empty
                            });
                        }
                    }
                }
                Ok(V::Grid {
                    cols: tcols.max(1),
                    data,
                })
            } else {
                let cols = grids.iter().map(|(c, _)| *c).max().unwrap_or(1);
                let mut data = Vec::new();
                for (c, d) in &grids {
                    let rows = if *c == 0 { 0 } else { d.len() / c };
                    for r in 0..rows {
                        for cc in 0..cols {
                            data.push(if cc < *c {
                                d.get(r * c + cc).cloned().unwrap_or(V::Empty)
                            } else {
                                V::Empty
                            });
                        }
                    }
                }
                Ok(V::Grid {
                    cols: cols.max(1),
                    data,
                })
            }
        }
        "TAKE" | "DROP" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let nrows = arg(args, 1)?.num()? as i64;
            let ncols = args
                .get(2)
                .and_then(|v| v.num().ok())
                .unwrap_or(cols as f64) as i64;
            let (r0, rn, c0, cn) = if up == "TAKE" {
                take_range(rows as i64, nrows, cols as i64, ncols)
            } else {
                drop_range(rows as i64, nrows, cols as i64, ncols)
            };
            let mut out = Vec::new();
            for r in r0..rn {
                for c in c0..cn {
                    out.push(
                        data.get((r * cols as i64 + c) as usize)
                            .cloned()
                            .unwrap_or(V::Empty),
                    );
                }
            }
            Ok(V::Grid {
                cols: (cn - c0).max(1) as usize,
                data: out,
            })
        }
        "FILTER" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let (ic, inc) = grid_of(arg(args, 1)?);
            let mut out = Vec::new();
            for r in 0..rows {
                let keep = if ic == 1 {
                    inc.get(r)
                        .map(|v| v.truthy().unwrap_or(false))
                        .unwrap_or(false)
                } else {
                    inc.iter()
                        .skip(r * ic)
                        .take(ic)
                        .any(|v| v.truthy().unwrap_or(false))
                };
                if keep {
                    for c in 0..cols {
                        out.push(data[r * cols + c].clone());
                    }
                }
            }
            if out.is_empty() {
                Ok(args.get(2).cloned().unwrap_or(V::Err("#CALC!".into())))
            } else {
                Ok(V::Grid {
                    cols: cols.max(1),
                    data: out,
                })
            }
        }
        "UNIQUE" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let mut seen: Vec<Vec<String>> = Vec::new();
            let mut out = Vec::new();
            for r in 0..rows {
                let row: Vec<String> = (0..cols).map(|c| as_str(&data[r * cols + c])).collect();
                if !seen.contains(&row) {
                    seen.push(row);
                    for c in 0..cols {
                        out.push(data[r * cols + c].clone());
                    }
                }
            }
            Ok(V::Grid {
                cols: cols.max(1),
                data: out,
            })
        }
        "SORT" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let idx = args
                .get(1)
                .and_then(|v| v.num().ok())
                .unwrap_or(1.0)
                .max(1.0) as usize
                - 1;
            let desc = args.get(2).and_then(|v| v.num().ok()).unwrap_or(1.0) < 0.0;
            let mut rowsv: Vec<Vec<V>> = (0..rows)
                .map(|r| (0..cols).map(|c| data[r * cols + c].clone()).collect())
                .collect();
            rowsv.sort_by(|a, b| {
                let (x, y) = (
                    a.get(idx).and_then(|v| v.num().ok()).unwrap_or(0.0),
                    b.get(idx).and_then(|v| v.num().ok()).unwrap_or(0.0),
                );
                if desc {
                    y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
                }
            });
            let mut out = Vec::new();
            for row in rowsv {
                out.extend(row);
            }
            Ok(V::Grid {
                cols: cols.max(1),
                data: out,
            })
        }
        "CHOOSEROWS" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let mut out = Vec::new();
            for a in &args[1..] {
                let i = a.num()? as i64;
                let r = if i < 0 { rows as i64 + i } else { i - 1 };
                if (0..rows as i64).contains(&r) {
                    for c in 0..cols {
                        out.push(data[(r as usize) * cols + c].clone());
                    }
                }
            }
            Ok(V::Grid {
                cols: cols.max(1),
                data: out,
            })
        }
        "CHOOSECOLS" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let rows = data.len() / cols.max(1);
            let mut sel = Vec::new();
            for a in &args[1..] {
                let i = a.num()? as i64;
                sel.push(if i < 0 { cols as i64 + i } else { i - 1 });
            }
            let mut out = Vec::new();
            for r in 0..rows {
                for &c in &sel {
                    if (0..cols as i64).contains(&c) {
                        out.push(data[r * cols + c as usize].clone());
                    }
                }
            }
            Ok(V::Grid {
                cols: sel.len().max(1),
                data: out,
            })
        }
        "MAP" => {
            let lam = match args.last() {
                Some(V::Lambda(l)) => (**l).clone(),
                _ => return err("#VALUE!"),
            };
            let arrays: Vec<(usize, Vec<V>)> = args[..args.len() - 1].iter().map(grid_of).collect();
            if arrays.is_empty() {
                return err("#VALUE!");
            }
            let len = arrays.iter().map(|(_, d)| d.len()).min().unwrap_or(0);
            let cols = arrays.first().map(|(c, _)| *c).unwrap_or(1).max(1);
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                let vals: Vec<V> = arrays
                    .iter()
                    .map(|(_, d)| d.get(i).cloned().unwrap_or(V::Empty))
                    .collect();
                out.push(apply_lambda(&lam, &vals, ctx)?);
            }
            Ok(V::Grid { cols, data: out })
        }
        "REDUCE" => {
            let init = arg(args, 0)?.clone();
            let (_, arr) = grid_of(arg(args, 1)?);
            let lam = match arg(args, 2)? {
                V::Lambda(l) => (**l).clone(),
                _ => return err("#VALUE!"),
            };
            let mut acc = init;
            for v in arr {
                acc = apply_lambda(&lam, &[acc, v], ctx)?;
            }
            Ok(acc)
        }
        "SCAN" => {
            let init = arg(args, 0)?.clone();
            let (cols, arr) = grid_of(arg(args, 1)?);
            let lam = match arg(args, 2)? {
                V::Lambda(l) => (**l).clone(),
                _ => return err("#VALUE!"),
            };
            let mut acc = init;
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                acc = apply_lambda(&lam, &[acc, v], ctx)?;
                out.push(acc.clone());
            }
            Ok(V::Grid {
                cols: cols.max(1),
                data: out,
            })
        }
        "BYROW" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let lam = match arg(args, 1)? {
                V::Lambda(l) => (**l).clone(),
                _ => return err("#VALUE!"),
            };
            let cols = cols.max(1);
            let rows = data.len() / cols;
            let mut out = Vec::with_capacity(rows);
            for r in 0..rows {
                let row: Vec<V> = (0..cols).map(|c| data[r * cols + c].clone()).collect();
                out.push(apply_lambda(&lam, &[V::Grid { cols, data: row }], ctx)?);
            }
            Ok(V::Grid { cols: 1, data: out })
        }
        "BYCOL" => {
            let (cols, data) = grid_of(arg(args, 0)?);
            let lam = match arg(args, 1)? {
                V::Lambda(l) => (**l).clone(),
                _ => return err("#VALUE!"),
            };
            let cols = cols.max(1);
            let rows = data.len() / cols;
            let mut out = Vec::with_capacity(cols);
            for c in 0..cols {
                let col: Vec<V> = (0..rows).map(|r| data[r * cols + c].clone()).collect();
                out.push(apply_lambda(&lam, &[V::Grid { cols: 1, data: col }], ctx)?);
            }
            Ok(V::Grid { cols: 1, data: out })
        }
        "MAKEARRAY" => {
            let rows = arg(args, 0)?.num()? as i64;
            let cols = arg(args, 1)?.num()? as i64;
            let lam = match arg(args, 2)? {
                V::Lambda(l) => (**l).clone(),
                _ => return err("#VALUE!"),
            };
            let mut out = Vec::new();
            for r in 1..=rows {
                for c in 1..=cols {
                    out.push(apply_lambda(
                        &lam,
                        &[V::Num(r as f64), V::Num(c as f64)],
                        ctx,
                    )?);
                }
            }
            Ok(V::Grid {
                cols: cols.max(1) as usize,
                data: out,
            })
        }
        "ISOMITTED" => Ok(V::Bool(matches!(arg(args, 0)?, V::Empty))),

        _ => err("#NAME?"),
    }
}

fn days360(s: i64, e: i64) -> i64 {
    let d = |x: i64| crate::utils::date::serial_to_date(x, false);
    let (Some(a), Some(b)) = (d(s), d(e)) else {
        return 0;
    };
    let (mut y1, mut m1, mut d1) = (a.year() as i64, a.month() as i64, a.day() as i64);
    let (y2, m2, d2) = (b.year() as i64, b.month() as i64, b.day() as i64);
    if d1 == 31 {
        d1 = 30;
    }
    let mut d2v = d2;
    if d2v == 31 {
        d2v = 30;
    }
    let _ = &mut y1;
    let _ = &mut m1;
    (y2 - y1) * 360 + (m2 - m1) * 30 + (d2v - d1)
}

fn yearfrac(s: i64, e: i64, basis: i32) -> f64 {
    let d = |x: i64| crate::utils::date::serial_to_date(x, false);
    match basis {
        1 => {
            if let (Some(a), Some(b)) = (d(s), d(e)) {
                let a2 = chrono::NaiveDate::from_ymd_opt(a.year(), 1, 1).unwrap_or(a);
                let b2 = chrono::NaiveDate::from_ymd_opt(b.year() + 1, 1, 1).unwrap_or(b);
                (b - a).num_days() as f64 / (b2 - a2).num_days() as f64
            } else {
                0.0
            }
        }
        2 => (e - s) as f64 / 360.0,
        3 => (e - s) as f64 / 365.0,
        _ => days360(s, e) as f64 / 360.0,
    }
}

fn ddb(cost: f64, salvage: f64, life: f64, period: f64, factor: f64) -> f64 {
    let rate = (factor / life).min(1.0);
    let mut book = cost;
    let mut dep = 0.0;
    for p in 1..=(period as i64) {
        dep = (book * rate).min(book - salvage).max(0.0);
        book -= dep;
        let _ = p;
    }
    dep
}

fn db(cost: f64, salvage: f64, life: f64, period: f64, month: f64) -> f64 {
    let rate = (1.0 - (salvage / cost).powf(1.0 / life)).round_to(3);
    let mut book = cost;
    let mut dep = 0.0;
    for p in 1..=(period as i64) {
        let d = if p == 1 {
            book * rate * month / 12.0
        } else {
            book * rate
        };
        book -= d;
        dep = d;
    }
    dep
}

trait Round3 {
    fn round_to(self, n: i32) -> f64;
}
impl Round3 for f64 {
    fn round_to(self, n: i32) -> f64 {
        let f = 10f64.powi(n);
        (self * f).round() / f
    }
}

fn proper(s: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for ch in s.chars() {
        if ch.is_alphabetic() {
            if cap {
                out.extend(ch.to_uppercase());
            } else {
                out.extend(ch.to_lowercase());
            }
            cap = false;
        } else {
            out.push(ch);
            cap = ch == ' ';
        }
    }
    out
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn add_months(d: chrono::NaiveDate, months: i64) -> Option<chrono::NaiveDate> {
    use chrono::Datelike;
    let total = d.year() as i64 * 12 + (d.month0() as i64) + months;
    let y = total.div_euclid(12) as i32;
    let m = (total.rem_euclid(12) as u32) + 1;
    let day = d.day();
    for cand in (1..=day).rev() {
        if let Some(nd) = chrono::NaiveDate::from_ymd_opt(y, m, cand) {
            return Some(nd);
        }
    }
    None
}

fn rate_solve(nper: f64, pmt: f64, pv: f64, fv: f64) -> Option<f64> {
    let f = |r: f64| {
        if r == 0.0 {
            pv + pmt * nper + fv
        } else {
            pv * (1.0 + r).powf(nper) + pmt * ((1.0 + r).powf(nper) - 1.0) / r + fv
        }
    };
    let (mut lo, mut hi) = (-0.9999, 10.0);
    if f(lo) * f(hi) > 0.0 {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if f(lo) * f(mid) <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some((lo + hi) / 2.0)
}

fn xnpv(rate: f64, cf: &[(f64, f64)]) -> f64 {
    let d0 = cf.first().map(|(_, d)| *d).unwrap_or(0.0);
    cf.iter()
        .map(|(v, d)| v / (1.0 + rate).powf((d - d0) / 365.0))
        .sum()
}

fn nums_of(v: &V) -> Vec<f64> {
    let (_, data) = grid_of(v);
    data.iter().filter_map(|x| x.num().ok()).collect()
}

fn correl(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len().min(ys.len());
    if n == 0 {
        return 0.0;
    }
    let (mx, my) = (
        xs.iter().sum::<f64>() / n as f64,
        ys.iter().sum::<f64>() / n as f64,
    );
    let mut num = 0.0;
    let mut dx = 0.0;
    let mut dy = 0.0;
    for i in 0..n {
        num += (xs[i] - mx) * (ys[i] - my);
        dx += (xs[i] - mx).powi(2);
        dy += (ys[i] - my).powi(2);
    }
    if dx == 0.0 || dy == 0.0 {
        0.0
    } else {
        num / (dx.sqrt() * dy.sqrt())
    }
}

fn fact(n: u64) -> f64 {
    (1..=n).fold(1f64, |a, b| a * b as f64)
}

fn to_roman(mut n: i64) -> String {
    if n <= 0 {
        return String::new();
    }
    let table = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (v, sym) in table {
        while n >= v {
            out.push_str(sym);
            n -= v;
        }
    }
    out
}

fn from_roman(s: &str) -> i64 {
    let val = |c: char| match c.to_ascii_uppercase() {
        'I' => 1,
        'V' => 5,
        'X' => 10,
        'L' => 50,
        'C' => 100,
        'D' => 500,
        'M' => 1000,
        _ => 0,
    };
    let mut total = 0;
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        let v = val(*c);
        if i + 1 < chars.len() && v < val(chars[i + 1]) {
            total -= v;
        } else {
            total += v;
        }
    }
    total
}

fn base_repr(mut n: u64, radix: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let digits = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut out = Vec::new();
    while n > 0 {
        out.push(digits[(n % radix as u64) as usize]);
        n /= radix as u64;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    if x >= 0.0 {
        y
    } else {
        -y
    }
}

fn norm_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn norm_s_inv(p: f64) -> f64 {
    // Acklam 反正态近似
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    let pl = 0.02425;
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

fn gamma_ln(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];
    if x < 0.5 {
        (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - gamma_ln(1.0 - x)
    } else {
        let x = x - 1.0;
        let mut a = G[0];
        let t = x + 7.5;
        for (i, g) in G.iter().enumerate().skip(1) {
            a += g / (x + i as f64);
        }
        0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}

fn gamma_fn(x: f64) -> f64 {
    if x < 0.5 {
        std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * gamma_fn(1.0 - x))
    } else {
        gamma_ln(x).exp()
    }
}

fn beta_fn(a: f64, b: f64) -> f64 {
    (gamma_ln(a) + gamma_ln(b) - gamma_ln(a + b)).exp()
}

fn reg_lower_gamma(a: f64, x: f64) -> f64 {
    if x < 0.0 || a <= 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // 级数展开
        let mut ap = a;
        let mut sum = 1.0 / a;
        let mut del = sum;
        for _ in 0..200 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        sum * (-x + a * x.ln() - gamma_ln(a)).exp()
    } else {
        1.0 - reg_upper_gamma_cf(a, x)
    }
}

fn reg_upper_gamma_cf(a: f64, x: f64) -> f64 {
    // 连分式（Lentz）
    let tiny = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / tiny;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..200 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < tiny {
            d = tiny;
        }
        c = b + an / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    (-x + a * x.ln() - gamma_ln(a)).exp() * h
}

fn betacf(a: f64, b: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..200 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

fn reg_lower_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (gamma_ln(a + b) - gamma_ln(a) - gamma_ln(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(a, b, x) / a
    } else {
        1.0 - bt * betacf(b, a, 1.0 - x) / b
    }
}

fn invert(target: f64, tol: f64, f: impl Fn(f64) -> f64) -> Option<f64> {
    let (mut lo, mut hi) = (0.0, 1.0e9);
    if f(lo) > target {
        return None;
    }
    while f(hi) < target && hi < 1e12 {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
        if (hi - lo).abs() < tol * (1.0 + hi.abs()) {
            break;
        }
    }
    Some((lo + hi) / 2.0)
}

fn t_cdf(x: f64, df: f64) -> f64 {
    let t = df / (df + x * x);
    let ib = 0.5 * reg_lower_beta(df / 2.0, 0.5, t);
    if x >= 0.0 {
        1.0 - ib
    } else {
        ib
    }
}

fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    reg_lower_beta(d1 / 2.0, d2 / 2.0, d1 * x / (d1 * x + d2))
}

fn invert_signed(target: f64, tol: f64, f: impl Fn(f64) -> f64) -> Option<f64> {
    let (mut lo, mut hi) = (-1.0e6, 1.0e6);
    if f(lo) > target || f(hi) < target {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
        if (hi - lo).abs() < tol * (1.0 + hi.abs()) {
            break;
        }
    }
    Some((lo + hi) / 2.0)
}

fn comb_u(n: u64, k: u64) -> f64 {
    if k > n {
        return 0.0;
    }
    fact(n) / (fact(k) * fact(n - k))
}

fn var_s(v: &[f64]) -> f64 {
    let n = v.len();
    if n < 2 {
        return 0.0;
    }
    let m = v.iter().sum::<f64>() / n as f64;
    v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64
}
fn stdev_s(v: &[f64]) -> f64 {
    var_s(v).sqrt()
}

fn ttest(a: &[f64], b: &[f64], tails: f64, typ: i32) -> f64 {
    let (t, df) = match typ {
        1 => {
            let d: Vec<f64> = a.iter().zip(b.iter()).map(|(x, y)| x - y).collect();
            let n = d.len() as f64;
            let m = d.iter().sum::<f64>() / n;
            let sd = var_s(&d).sqrt();
            (m / (sd / n.sqrt()), n - 1.0)
        }
        2 => {
            let (n1, n2) = (a.len() as f64, b.len() as f64);
            let (m1, m2) = (a.iter().sum::<f64>() / n1, b.iter().sum::<f64>() / n2);
            let sp = ((n1 - 1.0) * var_s(a) + (n2 - 1.0) * var_s(b)) / (n1 + n2 - 2.0);
            (
                (m1 - m2) / (sp * (1.0 / n1 + 1.0 / n2)).sqrt(),
                n1 + n2 - 2.0,
            )
        }
        _ => {
            let (n1, n2) = (a.len() as f64, b.len() as f64);
            let (m1, m2) = (a.iter().sum::<f64>() / n1, b.iter().sum::<f64>() / n2);
            let (v1, v2) = (var_s(a), var_s(b));
            let se = (v1 / n1 + v2 / n2).sqrt();
            let df = (v1 / n1 + v2 / n2).powi(2)
                / ((v1 / n1).powi(2) / (n1 - 1.0) + (v2 / n2).powi(2) / (n2 - 1.0));
            ((m1 - m2) / se, df)
        }
    };
    tails * (1.0 - t_cdf(t.abs(), df))
}

fn is_workday(dow: i64, wk: i64) -> bool {
    // wk: 1=Sat,Sun（默认）；简化为工作日=周一..周五（对默认）
    match wk {
        1 => dow < 5,
        2 => dow != 6 && dow != 0,
        7 => dow != 5 && dow != 6,
        _ => dow < 5,
    }
}

fn determinant(m: &[f64], rows: usize, cols: usize) -> f64 {
    if rows != cols || rows == 0 {
        return f64::NAN;
    }
    let n = rows;
    let mut a = m.to_vec();
    let mut det = 1.0;
    for i in 0..n {
        let mut p = i;
        for r in i + 1..n {
            if a[r * n + i].abs() > a[p * n + i].abs() {
                p = r;
            }
        }
        if a[p * n + i].abs() < 1e-12 {
            return 0.0;
        }
        if p != i {
            for c in 0..n {
                a.swap(p * n + c, i * n + c);
            }
            det = -det;
        }
        det *= a[i * n + i];
        for r in i + 1..n {
            let f = a[r * n + i] / a[i * n + i];
            for c in i..n {
                a[r * n + c] -= f * a[i * n + c];
            }
        }
    }
    let _ = cols;
    det
}

fn inverse(m: &[f64], rows: usize, cols: usize) -> Option<Vec<f64>> {
    if rows != cols || rows == 0 {
        return None;
    }
    let n = rows;
    let mut a = vec![0.0; n * 2 * n];
    for i in 0..n {
        for j in 0..n {
            a[i * 2 * n + j] = m[i * n + j];
        }
        a[i * 2 * n + n + i] = 1.0;
    }
    for i in 0..n {
        let mut p = i;
        for r in i + 1..n {
            if a[r * 2 * n + i].abs() > a[p * 2 * n + i].abs() {
                p = r;
            }
        }
        if a[p * 2 * n + i].abs() < 1e-12 {
            return None;
        }
        if p != i {
            for c in 0..2 * n {
                a.swap(p * 2 * n + c, i * 2 * n + c);
            }
        }
        let d = a[i * 2 * n + i];
        for c in 0..2 * n {
            a[i * 2 * n + c] /= d;
        }
        for r in 0..n {
            if r != i {
                let f = a[r * 2 * n + i];
                for c in 0..2 * n {
                    a[r * 2 * n + c] -= f * a[i * 2 * n + c];
                }
            }
        }
    }
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = a[i * 2 * n + n + j];
        }
    }
    Some(out)
}

fn take_range(rows: i64, nrows: i64, cols: i64, ncols: i64) -> (i64, i64, i64, i64) {
    let (r0, rn) = if nrows >= 0 {
        (0, nrows.min(rows))
    } else {
        (rows + nrows, rows)
    };
    let (c0, cn) = if ncols >= 0 {
        (0, ncols.min(cols))
    } else {
        (cols + ncols, cols)
    };
    (r0.max(0), rn.max(0), c0.max(0), cn.max(0))
}
fn drop_range(rows: i64, nrows: i64, cols: i64, ncols: i64) -> (i64, i64, i64, i64) {
    let (r0, rn) = if nrows >= 0 {
        (nrows.min(rows), rows)
    } else {
        (0, rows + nrows)
    };
    let (c0, cn) = if ncols >= 0 {
        (ncols.min(cols), cols)
    } else {
        (0, cols + ncols)
    };
    (r0.max(0), rn.max(0), c0.max(0), cn.max(0))
}

fn xirr(cf: &[(f64, f64)]) -> Option<f64> {
    let (mut lo, mut hi) = (-0.9999, 10.0);
    if xnpv(lo, cf) * xnpv(hi, cf) > 0.0 {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if xnpv(lo, cf) * xnpv(mid, cf) <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some((lo + hi) / 2.0)
}

fn grid_of(v: &V) -> (usize, Vec<V>) {
    match v {
        V::Grid { cols, data } => (*cols, data.clone()),
        other => (1, vec![other.clone()]),
    }
}

fn fmt_pattern(n: f64, pat: &str) -> String {
    if pat.contains('%') {
        let dec = pat.matches('0').count().saturating_sub(1);
        return format!("{:.*}%", dec, n * 100.0);
    }
    let dec = pat
        .split('.')
        .nth(1)
        .map(|d| d.matches('0').count())
        .unwrap_or(0);
    let s = format!("{:.*}", dec, n);
    if pat.contains("#,##0") {
        // 简单千分位
        let (int_part, frac) = s
            .split_once('.')
            .map(|(a, b)| (a.to_string(), Some(b.to_string())))
            .unwrap_or((s.clone(), None));
        let neg = int_part.starts_with('-');
        let digits = int_part.trim_start_matches('-');
        let mut grouped = String::new();
        for (i, ch) in digits.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        let grouped: String = grouped.chars().rev().collect();
        let sign = if neg { "-" } else { "" };
        return match frac {
            Some(f) => format!("{sign}{grouped}.{f}"),
            None => format!("{sign}{grouped}"),
        };
    }
    s
}

fn irr(cf: &[f64]) -> Option<f64> {
    let f = |r: f64| {
        cf.iter()
            .enumerate()
            .map(|(i, c)| c / (1.0 + r).powi(i as i32))
            .sum::<f64>()
    };
    let (mut lo, mut hi) = (-0.9999, 10.0);
    if f(lo) * f(hi) > 0.0 {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if f(lo) * f(mid) <= 0.0 {
            hi = mid
        } else {
            lo = mid
        }
    }
    Some((lo + hi) / 2.0)
}

fn matches_crit(v: &V, crit: &V) -> Result<bool, String> {
    // 支持 ">10"、"<=5"、"<>x" 等比较文本
    if let V::Str(c) = crit {
        for (op, rest) in [(">=", &c[2.min(c.len())..]), ("<=", &c[2.min(c.len())..])] {
            if c.starts_with(op) {
                let rhs = V::Str(rest.to_string());
                return compare(v, &rhs, op);
            }
        }
        for op in ["<>", ">", "<", "="] {
            if let Some(rest) = c.strip_prefix(op) {
                let rhs = parse_scalar(rest);
                return compare(v, &rhs, op);
            }
        }
        return compare(v, crit, "=");
    }
    compare(v, crit, "=")
}

fn parse_scalar(s: &str) -> V {
    if let Ok(n) = s.trim().parse::<f64>() {
        V::Num(n)
    } else {
        V::Str(s.to_string())
    }
}

fn arg(args: &[V], i: usize) -> Result<&V, String> {
    args.get(i).ok_or_else(|| "#VALUE!".to_string())
}

/// 计算工作簿中公式单元格的缓存值；就地写回 `cell.value`。
/// 支持动态数组溢出（spill）：数组结果写入以锚点为左上角的矩形区域。
pub fn evaluate(wb: &mut Workbook) {
    let mut cells: HashMap<Key, V> = HashMap::new();
    let mut base: std::collections::HashSet<Key> = std::collections::HashSet::new();
    struct Pending {
        sheet: String,
        sheet_i: usize,
        row: usize,
        cell: usize,
        formula: String,
        reference: String,
    }
    let mut pending = Vec::new();
    for (si, sheet) in wb.sheets.iter().enumerate() {
        let sname = sheet.name.clone();
        for (ri, row) in sheet.rows.iter().enumerate() {
            for (ci, cell) in row.cells.iter().enumerate() {
                let Some(r) = cell.reference.as_deref() else {
                    continue;
                };
                if let Some(f) = &cell.formula {
                    if cell.value.is_none() {
                        pending.push(Pending {
                            sheet: sname.clone(),
                            sheet_i: si,
                            row: ri,
                            cell: ci,
                            formula: f.clone(),
                            reference: r.to_string(),
                        });
                    }
                } else if let Some(v) = &cell.value {
                    let k = (sname.to_lowercase(), norm_ref(r));
                    cells.insert(k.clone(), cell_to_v(v));
                    base.insert(k);
                }
            }
        }
    }

    // 溢出的额外单元格：(sheet_i, row, col, V)
    let mut spilled: Vec<(usize, u32, u32, V)> = Vec::new();

    let mut resolved = vec![false; pending.len()];
    for _pass in 0..12 {
        let mut progressed = false;
        for (pi, p) in pending.iter().enumerate() {
            if resolved[pi] {
                continue;
            }
            let (ccol, crow) = crate::utils::a1::parse_cell_ref(&p.reference).unwrap_or((0, 0));
            let ctx = Ctx {
                cells: cells.clone(),
                cur_sheet: p.sheet.clone(),
                cur_row: Some(crow),
                cur_col: Some(ccol),
                vars: HashMap::new(),
            };
            let mut parser = Parser::new(&p.formula, &ctx);
            if let Ok(v) = parser.eval() {
                let key = (p.sheet.to_lowercase(), norm_ref(&p.reference));
                let anchor_val = match &v {
                    V::Grid { data, .. } => data.first().cloned().unwrap_or(V::Empty),
                    other => other.clone(),
                };
                cells.insert(key, anchor_val);
                // 溢出写入
                if let V::Grid { cols, data } = &v {
                    if *cols > 0 && data.len() > 1 {
                        let blocked = (0..data.len()).any(|i| {
                            let (r, c) = (crow + (i / cols) as u32, ccol + (i % cols) as u32);
                            i > 0
                                && base.contains(&(
                                    p.sheet.to_lowercase(),
                                    format!("{}{}", col_name(c), r),
                                ))
                        });
                        if blocked {
                            cells.insert(
                                (p.sheet.to_lowercase(), norm_ref(&p.reference)),
                                V::Err("#SPILL!".into()),
                            );
                        } else {
                            for (i, val) in data.iter().enumerate().skip(1) {
                                let (r, c) = (crow + (i / cols) as u32, ccol + (i % cols) as u32);
                                cells.insert(
                                    (p.sheet.to_lowercase(), format!("{}{}", col_name(c), r)),
                                    val.clone(),
                                );
                                spilled.push((p.sheet_i, r, c, val.clone()));
                            }
                        }
                    }
                }
                resolved[pi] = true;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }

    // 写回公式单元格（锚点 = 数组左上角）
    for (pi, p) in pending.iter().enumerate() {
        if !resolved[pi] {
            continue;
        }
        if let Some(v) = cells.get(&(p.sheet.to_lowercase(), norm_ref(&p.reference))) {
            if let Some(j) = v.to_json() {
                wb.sheets[p.sheet_i].rows[p.row].cells[p.cell].value = Some(j);
            }
        }
    }

    // 物化溢出单元格为普通值单元格
    for (si, row, col, v) in spilled {
        let Some(j) = v.to_json() else { continue };
        let reference = format!("{}{}", col_name(col), row);
        let rows = &mut wb.sheets[si].rows;
        let ri = match rows.iter().position(|r| r.index == Some(row)) {
            Some(i) => i,
            None => {
                rows.push(crate::model::sheet::Row {
                    index: Some(row),
                    ..Default::default()
                });
                rows.len() - 1
            }
        };
        if rows[ri]
            .cells
            .iter()
            .any(|c| c.reference.as_deref() == Some(reference.as_str()))
        {
            continue;
        }
        rows[ri].cells.push(crate::model::cell::Cell {
            reference: Some(reference),
            value: Some(j),
            ..Default::default()
        });
    }
}

fn col_name(mut c: u32) -> String {
    let mut s = String::new();
    while c > 0 {
        let rem = ((c - 1) % 26) as u8;
        s.insert(0, (b'A' + rem) as char);
        c = (c - 1) / 26;
    }
    s
}
