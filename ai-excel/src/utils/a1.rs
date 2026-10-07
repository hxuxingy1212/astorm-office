use crate::error::{Error, Result};

/// Maximum columns (XFD) and rows in an xlsx sheet.
pub const MAX_COL: u32 = 16384;
pub const MAX_ROW: u32 = 1_048_576;

/// "A" -> 1, "Z" -> 26, "AA" -> 27.
pub fn col_to_index(letters: &str) -> Result<u32> {
    if letters.is_empty() {
        return Err(Error::InvalidInput("空列引用".into()));
    }
    let mut n: u32 = 0;
    for c in letters.chars() {
        let up = c.to_ascii_uppercase();
        if !up.is_ascii_uppercase() {
            return Err(Error::InvalidInput(format!("非法列字母: {letters}")));
        }
        n = n.saturating_mul(26) + (up as u32 - 'A' as u32 + 1);
    }
    if n == 0 || n > MAX_COL {
        return Err(Error::InvalidInput(format!("列越界: {letters}")));
    }
    Ok(n)
}

/// 1 -> "A", 26 -> "Z", 27 -> "AA".
pub fn index_to_col(mut n: u32) -> String {
    let mut s = String::new();
    while n > 0 {
        let rem = ((n - 1) % 26) as u8;
        s.insert(0, (b'A' + rem) as char);
        n = (n - 1) / 26;
    }
    s
}

/// Parse "B2" -> (2, 2).
pub fn parse_cell_ref(r: &str) -> Result<(u32, u32)> {
    let r = r.trim().trim_start_matches('$');
    let split = r
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| Error::InvalidInput(format!("非法单元格引用: {r}")))?;
    let (letters, digits) = r.split_at(split);
    let letters = letters.trim_end_matches('$');
    let col = col_to_index(letters)?;
    let row: u32 = digits
        .parse()
        .map_err(|_| Error::InvalidInput(format!("非法单元格引用: {r}")))?;
    if row == 0 || row > MAX_ROW {
        return Err(Error::InvalidInput(format!("行越界: {r}")));
    }
    Ok((col, row))
}

pub fn make_cell_ref(col: u32, row: u32) -> String {
    format!("{}{}", index_to_col(col), row)
}

/// Parse "A1:C10" -> ((1,1),(3,10)). A single ref yields a 1x1 range.
pub fn parse_range(r: &str) -> Result<((u32, u32), (u32, u32))> {
    let r = r.trim();
    match r.split_once(':') {
        Some((a, b)) => Ok((parse_cell_ref(a)?, parse_cell_ref(b)?)),
        None => {
            let p = parse_cell_ref(r)?;
            Ok((p, p))
        }
    }
}

/// Shift relative A1 references in a formula by (drow, dcol), preserving `$`
/// absolute markers. Used to expand shared formulas.
pub fn shift_formula(formula: &str, drow: i64, dcol: i64) -> String {
    if drow == 0 && dcol == 0 {
        return formula.to_string();
    }
    let bytes = formula.as_bytes();
    let mut out = String::with_capacity(formula.len());
    let mut i = 0usize;
    let mut in_quotes = false;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'"' {
            in_quotes = !in_quotes;
            out.push('"');
            i += 1;
            continue;
        }
        if !in_quotes && (ch == b'$' || ch.is_ascii_alphabetic()) {
            let prev = if i == 0 { 0u8 } else { bytes[i - 1] };
            let prev_ok =
                !(prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'!' || prev == b'.');
            if prev_ok {
                if let Some((len, shifted)) = try_shift_ref(&formula[i..], drow, dcol) {
                    out.push_str(&shifted);
                    i += len;
                    continue;
                }
            }
        }
        out.push(ch as char);
        i += 1;
    }
    out
}

fn try_shift_ref(s: &str, drow: i64, dcol: i64) -> Option<(usize, String)> {
    let b = s.as_bytes();
    let mut j = 0usize;
    let mut col_abs = false;
    if b.get(j) == Some(&b'$') {
        col_abs = true;
        j += 1;
    }
    let cs = j;
    while j < b.len() && b[j].is_ascii_alphabetic() {
        j += 1;
    }
    if j == cs || j - cs > 3 {
        return None;
    }
    let letters = &s[cs..j];
    let mut row_abs = false;
    if b.get(j) == Some(&b'$') {
        row_abs = true;
        j += 1;
    }
    let rs = j;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    if j == rs || j - rs > 7 {
        return None;
    }
    if let Some(&n) = b.get(j) {
        // reject identifiers/function calls like LOG10( or A1B
        if n.is_ascii_alphanumeric() || n == b'_' || n == b'.' || n == b'(' {
            return None;
        }
    }
    let col = col_to_index(letters).ok()? as i64;
    let row: i64 = s[rs..j].parse().ok()?;
    if row < 1 || row > MAX_ROW as i64 {
        return None;
    }
    let new_col = if col_abs { col } else { col + dcol };
    let new_row = if row_abs { row } else { row + drow };
    if new_col < 1 || new_col > MAX_COL as i64 || new_row < 1 || new_row > MAX_ROW as i64 {
        return None;
    }
    let mut out = String::new();
    if col_abs {
        out.push('$');
    }
    out.push_str(&index_to_col(new_col as u32));
    if row_abs {
        out.push('$');
    }
    out.push_str(&new_row.to_string());
    Some((j, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_roundtrip() {
        assert_eq!(col_to_index("A").unwrap(), 1);
        assert_eq!(col_to_index("Z").unwrap(), 26);
        assert_eq!(col_to_index("AA").unwrap(), 27);
        assert_eq!(col_to_index("XFD").unwrap(), 16384);
        assert_eq!(index_to_col(1), "A");
        assert_eq!(index_to_col(26), "Z");
        assert_eq!(index_to_col(27), "AA");
        assert_eq!(index_to_col(16384), "XFD");
    }

    #[test]
    fn cell_ref() {
        assert_eq!(parse_cell_ref("B2").unwrap(), (2, 2));
        assert_eq!(parse_cell_ref("$AA$10").unwrap(), (27, 10));
        assert_eq!(make_cell_ref(27, 10), "AA10");
    }

    #[test]
    fn shift_formula_refs() {
        assert_eq!(shift_formula("A1+B2", 1, 0), "A2+B3");
        assert_eq!(shift_formula("$A1+B$2", 1, 1), "$A2+C$2");
        assert_eq!(shift_formula("SUM(A1:A3)", 2, 0), "SUM(A3:A5)");
        assert_eq!(shift_formula("LOG10(A1)", 1, 0), "LOG10(A2)");
        assert_eq!(shift_formula("DATE(2020,1,1)", 1, 0), "DATE(2020,1,1)");
    }
}
