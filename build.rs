use std::process::Command;

fn main() {
    // 1. Git Commit Hash
    let commit_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .or_else(|| std::env::var("AINA_GIT_COMMIT").ok())
        .or_else(|| std::env::var("SOURCE_COMMIT").ok())
        .unwrap_or_else(|| "unknown".to_string());

    // 2. Git Branch
    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .or_else(|| std::env::var("AINA_GIT_BRANCH").ok())
        .unwrap_or_else(|| "main".to_string());

    // 3. Build Timestamp
    let build_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            // Basic RFC3339 representation
            format_timestamp_rfc3339(secs)
        })
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

    println!("cargo:rustc-env=AINA_GIT_COMMIT={}", commit_hash);
    println!("cargo:rustc-env=AINA_GIT_BRANCH={}", git_branch);
    println!("cargo:rustc-env=AINA_BUILD_TIME={}", build_time);

    // Re-run build script if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}

fn format_timestamp_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;

    // Simple civil date calculation
    let z = (days as i64) + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", final_y, m, d, hh, mm, ss)
}
