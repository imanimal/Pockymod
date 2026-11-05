//! Small date helpers: RFC 3339 timestamps from the Nexus v2 API into Unix seconds, and "now".

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Days from 1970-01-01 to the given civil date (proleptic Gregorian). Howard Hinnant's algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parses `2024-05-01T12:30:00Z`, `2024-05-01T12:30:00.123+02:00`, `2024-05-01 12:30:00` or a bare date.
/// Also accepts a number (seconds since the epoch) given as a string.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.bytes().all(|b| b.is_ascii_digit()) {
        return s.parse().ok();
    }
    let b = s.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let mut secs = days_from_civil(y, mo, d) * 86400;
    if b.len() == 10 {
        return Some(secs);
    }
    if b[10] != b'T' && b[10] != b't' && b[10] != b' ' {
        return None;
    }
    let (h, mi) = (num(11..13)?, num(14..16)?);
    let sec = if b.len() >= 19 && b[16] == b':' { num(17..19)? } else { 0 };
    secs += h * 3600 + mi * 60 + sec;
    // skip fractional seconds, then read the offset
    let mut i = if b.len() >= 19 && b[16] == b':' { 19 } else { 16 };
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i >= b.len() {
        return Some(secs); // no offset: treat as UTC
    }
    match b[i] {
        b'Z' | b'z' => Some(secs),
        b'+' | b'-' => {
            let sign = if b[i] == b'+' { 1 } else { -1 };
            let oh: i64 = s.get(i + 1..i + 3)?.parse().ok()?;
            let om: i64 = s.get(i + 4..i + 6).and_then(|x| x.parse().ok()).unwrap_or(0);
            Some(secs - sign * (oh * 3600 + om * 60))
        }
        _ => None,
    }
}

/// Formats Unix seconds as `YYYY-MM-DD` (UTC).
pub fn ymd(ts: i64) -> String {
    let days = ts.div_euclid(86400);
    // inverse of days_from_civil
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_shapes() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2024-05-01T12:30:00Z"), Some(1714566600));
        assert_eq!(parse_rfc3339("2024-05-01T14:30:00+02:00"), Some(1714566600));
        assert_eq!(parse_rfc3339("2024-05-01T12:30:00.456Z"), Some(1714566600));
        assert_eq!(parse_rfc3339("2024-05-01"), Some(1714521600));
        assert_eq!(parse_rfc3339("1714566600"), Some(1714566600));
        assert_eq!(parse_rfc3339("yesterday"), None);
        assert_eq!(parse_rfc3339(""), None);
    }

    #[test]
    fn ymd_round_trips() {
        assert_eq!(ymd(1714566600), "2024-05-01");
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(parse_rfc3339("2026-10-22").unwrap()), "2026-10-22");
        assert_eq!(ymd(parse_rfc3339("2000-02-29").unwrap()), "2000-02-29");
    }
}
