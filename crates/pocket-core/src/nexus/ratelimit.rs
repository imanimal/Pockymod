//! Nexus rate limits, from the response headers (verified, spec section 4):
//! 20,000 requests per 24 h (reset 00:00 GMT), then 500 per hour once the daily cap is used up.
//! Headers: x-rl-daily-limit, x-rl-daily-remaining, x-rl-daily-reset, x-rl-hourly-limit, x-rl-hourly-remaining,
//! x-rl-hourly-reset. A 429 means throttled. We read them on every response, slow down early, and never retry blind.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// Plenty left (green LCD).
    Ok,
    /// Getting low: requests are spaced out (amber).
    Low,
    /// Throttled or at zero (red).
    Throttled,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    pub daily_limit: Option<u32>,
    pub daily_remaining: Option<u32>,
    pub daily_reset: Option<String>,
    pub hourly_limit: Option<u32>,
    pub hourly_remaining: Option<u32>,
    pub hourly_reset: Option<String>,
    /// Set when Nexus answered 429.
    pub throttled_until: Option<i64>,
    pub updated_at: i64,
}

/// Below this many hourly requests, the LCD goes amber and requests are spaced out.
pub const LOW_HOURLY: u32 = 60;
/// Below this many daily requests (before the hourly regime starts), same.
pub const LOW_DAILY: u32 = 500;

impl RateLimit {
    /// Reads the x-rl-* headers. `get` returns a header value by lower-case name.
    pub fn update_from<'a>(&mut self, get: impl Fn(&str) -> Option<&'a str>) {
        let num = |n: &str| get(n).and_then(|v| v.trim().parse::<u32>().ok());
        let any = num("x-rl-daily-remaining").is_some() || num("x-rl-hourly-remaining").is_some();
        if !any {
            return;
        }
        self.daily_limit = num("x-rl-daily-limit").or(self.daily_limit);
        self.daily_remaining = num("x-rl-daily-remaining").or(self.daily_remaining);
        self.daily_reset = get("x-rl-daily-reset").map(String::from).or(self.daily_reset.take());
        self.hourly_limit = num("x-rl-hourly-limit").or(self.hourly_limit);
        self.hourly_remaining = num("x-rl-hourly-remaining").or(self.hourly_remaining);
        self.hourly_reset = get("x-rl-hourly-reset").map(String::from).or(self.hourly_reset.take());
        self.updated_at = crate::timefmt::now();
        if self.hourly_remaining.unwrap_or(1) > 0 || self.daily_remaining.unwrap_or(1) > 0 {
            self.throttled_until = None;
        }
    }

    /// The number that matters this hour: once the daily budget is gone, only the hourly one counts.
    pub fn remaining_now(&self) -> Option<u32> {
        match (self.daily_remaining, self.hourly_remaining) {
            (Some(0), h) => h,
            (Some(d), Some(h)) => Some(d.max(h)),
            (d, h) => d.or(h),
        }
    }

    pub fn level(&self, now: i64) -> Level {
        if self.throttled_until.is_some_and(|t| t > now) {
            return Level::Throttled;
        }
        let daily_gone = self.daily_remaining == Some(0);
        match (daily_gone, self.hourly_remaining, self.daily_remaining) {
            (true, Some(0), _) => Level::Throttled,
            (true, Some(h), _) if h < LOW_HOURLY => Level::Low,
            (true, None, _) => Level::Low,
            (false, _, Some(d)) if d < LOW_DAILY => Level::Low,
            _ => Level::Ok,
        }
    }

    /// How long to wait before the next request: none normally, spaced out when low, refuse when throttled.
    pub fn pacing(&self, now: i64) -> Pacing {
        match self.level(now) {
            Level::Ok => Pacing::Go,
            Level::Low => Pacing::Wait(std::time::Duration::from_millis(1500)),
            Level::Throttled => Pacing::Stop,
        }
    }

    /// Called on a 429. `retry_after` is the Retry-After header in seconds, when Nexus sends one.
    pub fn mark_throttled(&mut self, now: i64, retry_after: Option<i64>) {
        // without Retry-After, assume the next hour boundary
        let until = retry_after.map(|s| now + s).unwrap_or(now - now.rem_euclid(3600) + 3600);
        self.throttled_until = Some(until);
        self.updated_at = now;
    }

    pub fn resume_text(&self) -> String {
        match (self.throttled_until, &self.hourly_reset) {
            (Some(t), _) => format!("at {} UTC", hhmm(t)),
            (None, Some(r)) => format!("after {r}"),
            _ => "within the hour".into(),
        }
    }
}

fn hhmm(ts: i64) -> String {
    let s = ts.rem_euclid(86400);
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pacing {
    Go,
    Wait(std::time::Duration),
    Stop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn rl(h: &[(&str, &str)]) -> RateLimit {
        let map: HashMap<&str, &str> = h.iter().copied().collect();
        let mut r = RateLimit::default();
        r.update_from(|n| map.get(n).copied());
        r
    }

    #[test]
    fn reads_headers() {
        let r = rl(&[("x-rl-daily-limit", "20000"), ("x-rl-daily-remaining", "19990"), ("x-rl-hourly-limit", "500"), ("x-rl-hourly-remaining", "500")]);
        assert_eq!(r.daily_remaining, Some(19990));
        assert_eq!(r.remaining_now(), Some(19990));
        assert_eq!(r.level(0), Level::Ok);
        assert_eq!(r.pacing(0), Pacing::Go);
    }

    #[test]
    fn hourly_regime_after_daily_cap() {
        let r = rl(&[("x-rl-daily-remaining", "0"), ("x-rl-hourly-remaining", "30")]);
        assert_eq!(r.remaining_now(), Some(30));
        assert_eq!(r.level(0), Level::Low);
        let r = rl(&[("x-rl-daily-remaining", "0"), ("x-rl-hourly-remaining", "0")]);
        assert_eq!(r.level(0), Level::Throttled);
        assert_eq!(r.pacing(0), Pacing::Stop);
    }

    #[test]
    fn low_daily_is_amber() {
        assert_eq!(rl(&[("x-rl-daily-remaining", "120"), ("x-rl-hourly-remaining", "500")]).level(0), Level::Low);
    }

    #[test]
    fn missing_headers_change_nothing() {
        let mut r = rl(&[("x-rl-daily-remaining", "100")]);
        r.update_from(|_| None);
        assert_eq!(r.daily_remaining, Some(100));
    }

    #[test]
    fn throttle_until_next_hour() {
        let mut r = RateLimit::default();
        r.mark_throttled(3600 * 5 + 120, None);
        assert_eq!(r.throttled_until, Some(3600 * 6));
        assert_eq!(r.level(3600 * 5 + 200), Level::Throttled);
        assert_eq!(r.level(3600 * 6 + 1), Level::Ok);
        assert_eq!(r.resume_text(), "at 06:00 UTC");
        r.mark_throttled(100, Some(30));
        assert_eq!(r.throttled_until, Some(130));
    }
}
