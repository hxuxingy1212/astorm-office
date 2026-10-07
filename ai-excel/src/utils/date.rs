//! Excel date <-> serial number conversion.

use chrono::{Days, NaiveDate};

fn epoch(date1904: bool) -> NaiveDate {
    if date1904 {
        NaiveDate::from_ymd_opt(1904, 1, 1).unwrap()
    } else {
        // 1899-12-30 absorbs Excel's 1900 leap-year bug for dates >= 1900-03-01.
        NaiveDate::from_ymd_opt(1899, 12, 30).unwrap()
    }
}

/// Convert a date to an Excel serial number, replicating Excel's 1900 system
/// (serial 1 = 1900-01-01, serial 60 = the fictitious 1900-02-29).
pub fn date_to_serial(d: NaiveDate, date1904: bool) -> i64 {
    if !date1904 {
        let cut = NaiveDate::from_ymd_opt(1900, 2, 28).unwrap();
        if d <= cut {
            let base = NaiveDate::from_ymd_opt(1899, 12, 31).unwrap();
            return (d - base).num_days();
        }
    }
    (d - epoch(date1904)).num_days()
}

/// Convert an Excel serial number to a date, or `None` when it falls outside
/// the representable date range (avoids panics on corrupt/overflowing cells).
pub fn serial_to_date(serial: i64, date1904: bool) -> Option<NaiveDate> {
    if !date1904 {
        // Serial 60 is Excel's non-existent 1900-02-29; map it to 1900-02-28.
        if serial == 60 {
            return NaiveDate::from_ymd_opt(1900, 2, 28);
        }
        if (1..=59).contains(&serial) {
            let base = NaiveDate::from_ymd_opt(1899, 12, 31).unwrap();
            return base.checked_add_days(Days::new(serial as u64));
        }
    }
    let e = epoch(date1904);
    if serial >= 0 {
        e.checked_add_days(Days::new(serial as u64))
    } else {
        e.checked_sub_days(Days::new(serial.unsigned_abs()))
    }
}

pub fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

/// Heuristic: does an Excel number-format code display a date? Ignores quoted
/// literals, `[...]` tokens (colors/conditions/locale) and `\`-escapes so that
/// e.g. `0.00;[Red]\-0.00` is not mistaken for a date because of the `d`.
pub fn is_date_format(code: &str) -> bool {
    let mut in_quotes = false;
    let mut in_bracket = false;
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                chars.next();
            }
            '[' if !in_quotes => in_bracket = true,
            ']' if !in_quotes => in_bracket = false,
            _ if in_quotes || in_bracket => {}
            _ => {
                let l = c.to_ascii_lowercase();
                if l == 'y' || l == 'd' {
                    return true;
                }
            }
        }
    }
    false
}

/// Heuristic: does the format display a time component (`h`/`s` or `:`)?
/// 用于区分纯日期与日期时间：日期时间保留原始序列值，避免丢失时分秒。
pub fn has_time_format(code: &str) -> bool {
    let mut in_quotes = false;
    let mut in_bracket = false;
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                chars.next();
            }
            '[' if !in_quotes => in_bracket = true,
            ']' if !in_quotes => in_bracket = false,
            _ if in_quotes || in_bracket => {}
            _ => {
                let l = c.to_ascii_lowercase();
                if l == 'h' || l == 's' || c == ':' {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format_detected() {
        assert!(has_time_format("yyyy-mm-dd hh:mm"));
        assert!(has_time_format("h:mm:ss"));
        assert!(has_time_format("[$-409]hh:mm AM/PM"));
        assert!(!has_time_format("yyyy-mm-dd"));
        assert!(!has_time_format("#,##0"));
        assert!(!has_time_format("\"h\"0.0"));
    }

    #[test]
    fn serial_roundtrip() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let s = date_to_serial(d, false);
        assert_eq!(serial_to_date(s, false), Some(d));
    }

    #[test]
    fn excel_1900_system() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd).unwrap();
        assert_eq!(date_to_serial(d(1900, 1, 1), false), 1);
        assert_eq!(date_to_serial(d(1900, 2, 28), false), 59);
        assert_eq!(date_to_serial(d(1900, 3, 1), false), 61);
        assert_eq!(serial_to_date(1, false), Some(d(1900, 1, 1)));
        assert_eq!(serial_to_date(59, false), Some(d(1900, 2, 28)));
        assert_eq!(serial_to_date(60, false), Some(d(1900, 2, 28)));
        assert_eq!(serial_to_date(61, false), Some(d(1900, 3, 1)));
        assert_eq!(date_to_serial(d(2026, 1, 31), false), 46053);
    }

    #[test]
    fn out_of_range_serial_is_none() {
        assert_eq!(serial_to_date(i64::MAX, false), None);
        assert_eq!(serial_to_date(i64::MIN, false), None);
    }

    #[test]
    fn date_format_detection() {
        assert!(is_date_format("yyyy-mm-dd"));
        assert!(is_date_format("mm-dd-yy"));
        assert!(is_date_format("[$-409]d-mmm-yy"));
        assert!(!is_date_format("0.00;[Red]\\-0.00"));
        assert!(!is_date_format("#,##0"));
        assert!(!is_date_format("@"));
        assert!(!is_date_format("0.0\"days\""));
    }
}
