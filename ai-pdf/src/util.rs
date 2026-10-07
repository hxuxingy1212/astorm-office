//! 通用工具：页码区间解析、PDF 日期、可复现随机数。

use anyhow::{bail, Result};
use std::time::{SystemTime, UNIX_EPOCH};

/// 解析人类可读页码串 "1,3,5-8" → 去重升序的 1-based 页码列表。
/// 与原版 pdf.py 的页码解析行为一致：越界报错。
pub fn parse_page_spec(spec: &str, max_page: u32) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((lo, hi)) = part.split_once('-') {
            let lo: u32 = lo
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid page range: {part}"))?;
            let hi: u32 = hi
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid page range: {part}"))?;
            if lo == 0 || hi == 0 || lo > hi {
                bail!("invalid page range: {part}");
            }
            for p in lo..=hi {
                out.push(p);
            }
        } else {
            let p: u32 = part
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid page number: {part}"))?;
            if p == 0 {
                bail!("page numbers are 1-based: {part}");
            }
            out.push(p);
        }
    }
    out.sort_unstable();
    out.dedup();
    for &p in &out {
        if p > max_page {
            bail!("page {p} out of range (document has {max_page} pages)");
        }
    }
    Ok(out)
}

/// epoch 天数 → 公历年月日（Howard Hinnant civil_from_days 算法）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn utc_now() -> (i64, u32, u32, u32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    (
        y,
        mo,
        d,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    )
}

/// PDF 日期串 "D:YYYYMMDDHHmmSS+00'00'"（UTC）。
pub fn pdf_date_now() -> String {
    let (y, mo, d, h, mi, s) = utc_now();
    format!("D:{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}+00'00'")
}

/// xorshift64* — 复刻 Python random.seed(n) 的确定性用途（非逐位兼容，仅保证可复现）。
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: Option<u64>) -> Self {
        let s = seed.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E37_79B9_7F4A_7C15)
        });
        Rng(if s == 0 { 0x9E37_79B9_7F4A_7C15 } else { s })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// [lo, hi) 均匀浮点。
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.f64() * (hi - lo)
    }

    /// [lo, hi] 均匀整数（含两端）。
    pub fn randint(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.f64() * ((hi - lo + 1) as f64)).floor() as i64
    }

    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.randint(0, items.len() as i64 - 1) as usize]
    }

    pub fn bool(&mut self) -> bool {
        self.f64() < 0.5
    }
}

/// 截断到指定小数位（避免 -0.0 显示）。
pub fn round_f(x: f64, digits: usize) -> f64 {
    let m = 10f64.powi(digits as i32);
    let r = (x * m).round() / m;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_spec() {
        assert_eq!(
            parse_page_spec("1,3,5-8", 10).unwrap(),
            vec![1, 3, 5, 6, 7, 8]
        );
        assert!(parse_page_spec("5-3", 10).is_err());
        assert_eq!(parse_page_spec("1", 1).unwrap(), vec![1]);
        assert!(parse_page_spec("2", 1).is_err());
        assert!(parse_page_spec("0", 5).is_err());
    }

    #[test]
    fn rng_deterministic() {
        let mut a = Rng::new(Some(42));
        let mut b = Rng::new(Some(42));
        for _ in 0..8 {
            assert!((a.f64() - b.f64()).abs() < 1e-12);
        }
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // 2024-01-01
    }
}
