use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC time formatted as an ISO-8601 timestamp (second precision).
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    format_iso8601(secs)
}

/// Format a UNIX timestamp as ISO-8601 UTC, e.g. `2026-11-09T13:04:07Z`.
pub fn format_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Cryptographically-flavoured random hex string (no external RNG dependency).
pub fn random_hex(bytes: usize) -> String {
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        ^ (std::process::id() as u64) << 32;
    let mut out = String::with_capacity(bytes * 2);
    for _ in 0..bytes {
        // xorshift64*
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let value = seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 24;
        out.push_str(&format!("{:02x}", (value & 0xff) as u8));
    }
    out
}

/// Filesystem-safe slug derived from a project title.
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "game".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// PascalCase identifier for a file stem, used for script names in Studio.
pub fn pascal_case(input: &str) -> String {
    let mut out = String::new();
    for word in input.split(|c: char| !c.is_alphanumeric()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars);
        }
    }
    if out.is_empty() {
        "Script".to_string()
    } else if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("N{out}")
    } else {
        out
    }
}

/// Truncate a string for logs and diagnostics without breaking UTF-8.
pub fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out: String = input.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// Resolve a command on `PATH` without shelling out.
pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{program}.exe"));
            if is_executable(&exe) {
                return Some(exe);
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Run a command with a hidden console window on Windows.
pub fn hidden_command(program: &Path) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = std::process::Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(program)
    }
}

/// Byte length helper for size labels.
pub fn human_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_is_filesystem_safe() {
        assert_eq!(slugify("Neon Tycoon!"), "neon-tycoon");
        assert_eq!(slugify("  ***  "), "game");
        assert_eq!(slugify("Cave/Quest 2"), "cave-quest-2");
    }

    #[test]
    fn pascal_case_handles_separators() {
        assert_eq!(pascal_case("main"), "Main");
        assert_eq!(pascal_case("player_data"), "PlayerData");
        assert_eq!(pascal_case("2cool"), "N2cool");
    }

    #[test]
    fn timestamps_are_iso8601() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
        let now = now_iso8601();
        assert_eq!(now.len(), 20);
        assert!(now.ends_with('Z'));
    }

    #[test]
    fn random_hex_has_expected_shape() {
        let token = random_hex(16);
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, random_hex(16));
    }

    #[test]
    fn truncate_respects_utf8() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello…");
    }
}
