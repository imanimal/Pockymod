//! Nexus rate limits, read from response headers.
//!
//! From the spec (section 4, verified there): 20,000 requests per 24 h, resetting 00:00 GMT, then 500 per hour once
//! the daily cap is hit. Responses carry `x-rl-daily-*` and `x-rl-hourly-*` headers; 429 means throttled.
//! UNVERIFIED here: the exact header names (`x-rl-daily-limit`, `x-rl-daily-remaining`, `x-rl-daily-reset`, and the
//! same for hourly) and the reset format. They follow Vortex's client. The live probe found no `x-rl-*` headers on
//! anonymous v2 GraphQL responses, so v2 budget is only known when a keyed request returns them.
//!
//! Rules: read the headers, slow down early, and never retry blindly.

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Timelike, Utc};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// No headers seen yet.
    Unknown,
    Ok,
    /// Budget is low: the app spaces requests out. Amber on the LCD.
    Low,
    /// A 429, or no budget left: no requests until the reset. Red on the LCD.
    Throttled,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    pub daily_limit: Option<u32>,
    pub daily_remaining: Option<u32>,
    pub daily_reset: Option<DateTime<Utc>>,
    pub hourly_limit: Option<u32>,
    pub hourly_remaining: Option<u32>,
    pub hourly_reset: Option<DateTime<Utc>>,
    /// Set by a 429 until this time.
    pub throttled_until: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// What to do before sending the next request.
#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    Go,
    /// Budget is low: wait this long first.
    Wait(std::time::Duration),
    /// Out of budget: do not send until this time.
    Stop(DateTime<Utc>),
}

/// Below these the app slows down.
pub const LOW_HOURLY: u32 = 50;
pub const LOW_DAILY: u32 = 1000;
const LOW_SPACING_MS: u64 = 1200;

impl RateLimit {
    /// Update from one response. `header` looks a header up by lower-case name.
    pub fn observe(&mut self, status: u16, header: impl Fn(&str) -> Option<String>, now: DateTime<Utc>) {
        let num = |n: &str| header(n).and_then(|v| v.trim().parse::<u32>().ok());
        let mut seen = false;
        if let Some(v) = num("x-rl-daily-limit") { self.daily_limit = Some(v); seen = true; }
        if let Some(v) = num("x-rl-daily-remaining") { self.daily_remaining = Some(v); seen = true; }
        if let Some(v) = header("x-rl-daily-reset").and_then(|v| parse_reset(&v)) { self.daily_reset = Some(v); }
        if let Some(v) = num("x-rl-hourly-limit") { self.hourly_limit = Some(v); seen = true; }
        if let Some(v) = num("x-rl-hourly-remaining") { self.hourly_remaining = Some(v); seen = true; }
        if let Some(v) = header("x-rl-hourly-reset").and_then(|v| parse_reset(&v)) { self.hourly_reset = Some(v); }
        if seen { self.updated_at = Some(now); }
        if status == 429 {
            let retry_after = header("retry-after").and_then(|v| v.trim().parse::<i64>().ok()).map(|s| now + Duration::seconds(s));
            // without a stated reset, assume the hourly window: the next full hour
            let until = retry_after.or(self.hourly_reset.filter(|t| *t > now)).unwrap_or_else(|| next_hour(now));
            self.throttled_until = Some(until);
            self.updated_at = Some(now);
        } else if self.throttled_until.is_some_and(|t| t <= now) {
            self.throttled_until = None;
        }
    }

    pub fn level(&self, now: DateTime<Utc>) -> Level {
        if self.throttled_until.is_some_and(|t| t > now) { return Level::Throttled; }
        if self.updated_at.is_none() { return Level::Unknown; }
        let daily_out = self.daily_remaining == Some(0);
        let hourly_out = self.hourly_remaining == Some(0);
        // the hourly allowance applies once the daily cap is spent
        if daily_out && hourly_out { return Level::Throttled; }
        let low = self.hourly_remaining.is_some_and(|h| h <= LOW_HOURLY) && (daily_out || self.daily_remaining.is_none())
            || self.daily_remaining.is_some_and(|d| d <= LOW_DAILY && d > 0);
        if low { Level::Low } else { Level::Ok }
    }

    pub fn gate(&self, now: DateTime<Utc>) -> Gate {
        match self.level(now) {
            Level::Throttled => Gate::Stop(self.throttled_until.filter(|t| *t > now)
                .or(self.hourly_reset.filter(|t| *t > now))
                .unwrap_or_else(|| next_hour(now))),
            Level::Low => Gate::Wait(std::time::Duration::from_millis(LOW_SPACING_MS)),
            _ => Gate::Go,
        }
    }

    /// The number the rate-limit LCD shows: requests left this hour once the daily cap is spent, otherwise today's.
    pub fn display(&self, now: DateTime<Utc>) -> RateDisplay {
        let daily_out = self.daily_remaining == Some(0);
        let (remaining, limit, window) = if daily_out || self.daily_remaining.is_none() {
            (self.hourly_remaining, self.hourly_limit, "hour")
        } else {
            (self.daily_remaining, self.daily_limit, "day")
        };
        RateDisplay { level: self.level(now), remaining, limit, window, resets_at: if window == "hour" { self.hourly_reset } else { self.daily_reset }, throttled_until: self.throttled_until }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateDisplay {
    pub level: Level,
    pub remaining: Option<u32>,
    pub limit: Option<u32>,
    pub window: &'static str,
    pub resets_at: Option<DateTime<Utc>>,
    pub throttled_until: Option<DateTime<Utc>>,
}

fn next_hour(now: DateTime<Utc>) -> DateTime<Utc> {
    let base = now.with_minute(0).and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0)).unwrap_or(now);
    base + Duration::hours(1)
}

/// Reset values seen in the wild: RFC 3339, "2026-11-05 00:00:00 +0000", or unix seconds. UNVERIFIED which one Nexus
/// sends today, so all are accepted.
pub fn parse_reset(v: &str) -> Option<DateTime<Utc>> {
    let v = v.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(v) { return Some(t.with_timezone(&Utc)); }
    if let Ok(t) = DateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S %z") { return Some(t.with_timezone(&Utc)); }
    if let Ok(t) = NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S") { return Some(Utc.from_utc_datetime(&t)); }
    if let Ok(s) = v.parse::<i64>() { return Utc.timestamp_opt(s, 0).single(); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn hdr(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k: &str| m.get(k).cloned()
    }
    fn now() -> DateTime<Utc> { Utc.with_ymd_and_hms(2026, 11, 4, 9, 30, 0).unwrap() }

    #[test]
    fn unknown_until_headers_seen() {
        let mut r = RateLimit::default();
        assert_eq!(r.level(now()), Level::Unknown);
        r.observe(200, hdr(&[]), now());
        assert_eq!(r.level(now()), Level::Unknown);
        assert_eq!(r.gate(now()), Gate::Go);
    }

    #[test]
    fn ok_then_low_then_out() {
        let mut r = RateLimit::default();
        r.observe(200, hdr(&[("x-rl-daily-limit", "20000"), ("x-rl-daily-remaining", "19990"), ("x-rl-hourly-limit", "500"), ("x-rl-hourly-remaining", "500"),
            ("x-rl-daily-reset", "2026-11-05 00:00:00 +0000")]), now());
        assert_eq!(r.level(now()), Level::Ok);
        assert_eq!(r.display(now()).window, "day");
        assert_eq!(r.display(now()).remaining, Some(19990));
        assert_eq!(r.daily_reset, Some(Utc.with_ymd_and_hms(2026, 11, 5, 0, 0, 0).unwrap()));

        r.observe(200, hdr(&[("x-rl-daily-remaining", "800")]), now());
        assert_eq!(r.level(now()), Level::Low);
        assert!(matches!(r.gate(now()), Gate::Wait(_)));

        // daily cap spent: the hourly allowance is what's left
        r.observe(200, hdr(&[("x-rl-daily-remaining", "0"), ("x-rl-hourly-remaining", "40")]), now());
        assert_eq!(r.level(now()), Level::Low);
        let d = r.display(now());
        assert_eq!((d.window, d.remaining, d.limit), ("hour", Some(40), Some(500)));

        r.observe(200, hdr(&[("x-rl-hourly-remaining", "0")]), now());
        assert_eq!(r.level(now()), Level::Throttled);
        assert_eq!(r.gate(now()), Gate::Stop(Utc.with_ymd_and_hms(2026, 11, 4, 10, 0, 0).unwrap()));
    }

    #[test]
    fn a_429_stops_until_retry_after_and_then_clears() {
        let mut r = RateLimit::default();
        r.observe(429, hdr(&[("retry-after", "120")]), now());
        assert_eq!(r.level(now()), Level::Throttled);
        assert_eq!(r.gate(now()), Gate::Stop(now() + Duration::seconds(120)));
        let later = now() + Duration::seconds(121);
        assert_ne!(r.level(later), Level::Throttled);
        r.observe(200, hdr(&[]), later);
        assert!(r.throttled_until.is_none());
    }

    #[test]
    fn a_429_without_retry_after_waits_for_the_hour() {
        let mut r = RateLimit::default();
        r.observe(429, hdr(&[]), now());
        assert_eq!(r.throttled_until, Some(Utc.with_ymd_and_hms(2026, 11, 4, 10, 0, 0).unwrap()));
    }

    #[test]
    fn reset_formats() {
        let t = Utc.with_ymd_and_hms(2026, 11, 5, 0, 0, 0).unwrap();
        assert_eq!(parse_reset("2026-11-05T00:00:00Z"), Some(t));
        assert_eq!(parse_reset("2026-11-05 00:00:00 +0000"), Some(t));
        assert_eq!(parse_reset("2026-11-05 00:00:00"), Some(t));
        assert_eq!(parse_reset(&t.timestamp().to_string()), Some(t));
        assert_eq!(parse_reset("soon"), None);
    }
}
