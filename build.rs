use std::process::Command;

fn main() {
    // 1. Git Commit Hash
    let commit_hash = get_git_commit();

    // 2. Git Branch
    let git_branch = get_git_branch();

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

fn get_git_commit() -> String {
    // 1. Env vars
    if let Ok(c) = std::env::var("AINA_GIT_COMMIT") {
        if !c.trim().is_empty() {
            return c.trim().to_string();
        }
    }
    if let Ok(c) = std::env::var("SOURCE_COMMIT") {
        if !c.trim().is_empty() {
            return c.trim().to_string();
        }
    }

    // 2. Try git CLI
    if let Ok(output) = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output() {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    // 3. Try reading .git/HEAD directly from disk
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        let head = head.trim();
        if let Some(ref_path) = head.strip_prefix("ref: ") {
            if let Ok(content) = std::fs::read_to_string(format!(".git/{}", ref_path)) {
                let trimmed = content.trim();
                let short_len = trimmed.len().min(7);
                return trimmed[..short_len].to_string();
            }
            if let Ok(packed) = std::fs::read_to_string(".git/packed-refs") {
                for line in packed.lines() {
                    let parts: Vec<&str> = line.trim().split_whitespace().collect();
                    if parts.len() == 2 && parts[1] == ref_path {
                        let short_len = parts[0].len().min(7);
                        return parts[0][..short_len].to_string();
                    }
                }
            }
        } else if !head.is_empty() && head.len() >= 7 {
            let short_len = head.len().min(7);
            return head[..short_len].to_string();
        }
    }

    // 4. Try reading .git/FETCH_HEAD directly
    if let Ok(fetch_head) = std::fs::read_to_string(".git/FETCH_HEAD") {
        if let Some(first_line) = fetch_head.lines().next() {
            let hash = first_line.split_whitespace().next().unwrap_or("");
            if !hash.is_empty() && hash.len() >= 7 {
                let short_len = hash.len().min(7);
                return hash[..short_len].to_string();
            }
        }
    }

    "unknown".to_string()
}

fn get_git_branch() -> String {
    // 1. Env vars
    if let Ok(b) = std::env::var("AINA_GIT_BRANCH") {
        if !b.trim().is_empty() {
            return b.trim().to_string();
        }
    }
    if let Ok(b) = std::env::var("COOLIFY_BRANCH") {
        if !b.trim().is_empty() {
            return b.trim().to_string();
        }
    }

    // 2. Try git CLI
    if let Ok(output) = Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).output() {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let trimmed = s.trim();
                if !trimmed.is_empty() && trimmed != "HEAD" {
                    return trimmed.to_string();
                }
            }
        }
    }

    // 3. Try reading .git/HEAD directly
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        let head = head.trim();
        if let Some(ref_path) = head.strip_prefix("ref: refs/heads/") {
            return ref_path.to_string();
        }
    }

    "main".to_string()
}
