//! Rotating user agent for outbound HTTP from `fetch_url` and
//! `internet_search`.
//!
//! Instead of a fresh random user agent per request, this module picks a
//! new profile on a weekly cadence (ISO week number). Same week -> same
//! UA so caches stay warm, requests stay coherent, and tests are
//! deterministic.

use std::time::{SystemTime, UNIX_EPOCH};

const CHROMIUM_BROWSERS: &[&str] = &["Chrome", "Edg"];

struct OsConfig {
    os_token: &'static str,
    engines: &'static [&'static str],
}

const OS_CONFIGS: &[OsConfig] = &[
    OsConfig {
        os_token: "Windows NT 10.0; Win64; x64",
        engines: &["537.36"],
    },
    OsConfig {
        os_token: "Macintosh; Intel Mac OS X 10_15_7",
        engines: &["537.36", "605.1.15"],
    },
    OsConfig {
        os_token: "X11; Linux x86_64",
        engines: &["537.36"],
    },
];

const MAJOR_VERSIONS: &[u32] = &[146, 147, 148, 149, 150];
const MINOR_VERSIONS: &[u32] = &[0, 1, 2, 3, 4, 5];
const PATCH_VERSIONS: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
];

#[derive(Debug, Clone)]
pub struct UserAgentSelection {
    pub browser: &'static str,
    pub os: &'static str,
    pub version: String,
    pub engine: &'static str,
}

/// Pick a user agent based on the ISO week of `now`. Same week -> same UA,
/// different week -> different UA. The selection is fully deterministic
/// from a single u64 seed, so unit tests can pin `now`.
pub fn user_agent_for_week(now: SystemTime) -> String {
    let seed = weekly_seed(now);
    let selection = pick(seed);
    let UserAgentSelection {
        browser,
        os,
        version,
        engine,
    } = selection;
    if browser == "Edg" {
        format!(
            "Mozilla/5.0 ({}) AppleWebKit/{} (KHTML, like Gecko) Chrome/{} Safari/{} Edg/{}",
            os, engine, version, engine, version
        )
    } else {
        format!(
            "Mozilla/5.0 ({}) AppleWebKit/{} (KHTML, like Gecko) Chrome/{} Safari/{}",
            os, engine, version, engine
        )
    }
}

fn pick(seed: u64) -> UserAgentSelection {
    let mut s = seed;
    let browser_idx = (s as usize) % CHROMIUM_BROWSERS.len();
    s = s.wrapping_mul(2_654_435_761).wrapping_add(1);
    let os_index = (s as usize) % OS_CONFIGS.len();
    let os = &OS_CONFIGS[os_index];
    s = s.wrapping_mul(2_654_435_761).wrapping_add(1);
    let engine = os.engines[(s as usize) % os.engines.len()];
    s = s.wrapping_mul(2_654_435_761).wrapping_add(1);
    let major = MAJOR_VERSIONS[(s as usize) % MAJOR_VERSIONS.len()];
    s = s.wrapping_mul(2_654_435_761).wrapping_add(1);
    let minor = MINOR_VERSIONS[(s as usize) % MINOR_VERSIONS.len()];
    s = s.wrapping_mul(2_654_435_761).wrapping_add(1);
    let patch = PATCH_VERSIONS[(s as usize) % PATCH_VERSIONS.len()];
    s = s.wrapping_mul(2_654_435_761).wrapping_add(1);
    let build = ((s as u32) % 100) as u32;
    UserAgentSelection {
        browser: CHROMIUM_BROWSERS[browser_idx],
        os: os.os_token,
        version: format!("{major}.{minor}.{patch}.{build}"),
        engine,
    }
}

/// ISO week index: `(year_since_1970, week_of_year) -> u64`. Trivial
/// implementation - we only need a value that changes once per 7-day
/// window starting from the Unix epoch.
fn weekly_seed(now: SystemTime) -> u64 {
    let secs = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs / (7 * 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_within_a_week() {
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(86_400 * 100);
        let ua1 = user_agent_for_week(now);
        let ua2 = user_agent_for_week(now);
        assert_eq!(ua1, ua2);
    }

    #[test]
    fn rotates_across_weeks() {
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(86_400 * 100);
        let mut seen = std::collections::HashSet::new();
        for offset in 0..30 {
            let now = base + std::time::Duration::from_secs(offset * 86_400 * 7);
            seen.insert(user_agent_for_week(now));
        }
        // Across 30 weeks we should see at least 3 distinct user agents
        // (some weeks will repeat with the seed pattern; we just want to
        // confirm the rotation is not stuck on one value).
        assert!(
            seen.len() >= 3,
            "only {} distinct UAs across 30 weeks",
            seen.len()
        );
    }

    #[test]
    fn ua_matches_chrome_or_edge_layout() {
        let ua = user_agent_for_week(SystemTime::now());
        assert!(ua.starts_with("Mozilla/5.0 ("), "got: {ua}");
        assert!(
            ua.contains("AppleWebKit/") && ua.contains("Chrome/"),
            "got: {ua}"
        );
    }
}
