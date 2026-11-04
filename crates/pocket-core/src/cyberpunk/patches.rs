//! Game patches and their release dates, for the "updated since my patch" filter and for health checks.
//! UNVERIFIED: dates are from public patch notes as remembered when this was written; 2.5's date is from the spec.
//! Executable file versions are not mapped here; the installed patch is read from the game (see game.rs) or picked
//! in Settings.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch { pub version: &'static str, pub released: &'static str }

pub const PATCHES: &[Patch] = &[
    Patch { version: "2.0", released: "2023-09-21" },
    Patch { version: "2.1", released: "2023-12-05" },
    Patch { version: "2.12", released: "2024-03-01" },
    Patch { version: "2.2", released: "2024-12-10" },
    Patch { version: "2.3", released: "2025-07-17" },
    Patch { version: "2.5", released: "2026-10-22" },
];

/// The release time (Unix seconds, midnight UTC) of a patch, or of the newest patch at or below `version`.
pub fn released_at(version: &str) -> Option<i64> {
    let want = parse(version)?;
    PATCHES.iter().filter(|p| parse(p.version).is_some_and(|v| v <= want)).max_by(|a, b| parse(a.version).cmp(&parse(b.version)))
        .and_then(|p| chrono::NaiveDate::parse_from_str(p.released, "%Y-%m-%d").ok())
        .and_then(|d| d.and_hms_opt(0, 0, 0)).map(|t| t.and_utc().timestamp())
}

/// Cyberpunk patch numbers are decimals: 2.1 < 2.12 < 2.2 < 2.3 < 2.31 < 2.5. The key pads the minor part on the
/// right ("2.12" → (2, "1200"), "2.2" → (2, "2000")) so it compares as a decimal.
pub fn parse(v: &str) -> Option<(u32, String)> {
    let v = v.trim().trim_start_matches('v');
    let (major, minor) = v.split_once('.').unwrap_or((v, "0"));
    let major: u32 = major.parse().ok()?;
    let minor: String = minor.chars().take_while(|c| c.is_ascii_digit()).collect();
    Some((major, format!("{:0<6}", if minor.is_empty() { "0" } else { &minor })))
}

pub fn latest() -> &'static Patch { PATCHES.last().expect("patch table is not empty") }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_the_patch_at_or_below() {
        assert_eq!(released_at("2.5"), Some(1792627200));
        assert_eq!(released_at("2.31"), released_at("2.3"));
        assert!(released_at("1.6").is_none());
        assert!(parse("2.31").unwrap() > parse("2.3").unwrap());
        assert!(parse("2.12").unwrap() < parse("2.2").unwrap());
        assert!(parse("2.31").unwrap() < parse("2.5").unwrap());
        assert_eq!(released_at("2.21"), released_at("2.2"));
    }
}
