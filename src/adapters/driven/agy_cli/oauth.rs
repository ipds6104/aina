//! OAuth URL parsing, canonical session directory resolution, and PKCE authorization code sanitization.

use std::path::{Component, Path, PathBuf};

/// Strict whitelist for OAuth session identifiers (`^[a-zA-Z0-9_-]{16,64}$`).
/// Matches generated `<13-digit millis>_<8-hex>` IDs (22 chars) and blocks all path traversal sequences.
pub fn is_safe_session_id(session_id: &str) -> bool {
    let len = session_id.len();
    (16..=64).contains(&len)
        && session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn resolve_oauth_sessions_root() -> PathBuf {
    if Path::new("/app/data").is_dir() {
        PathBuf::from("/app/data/oauth_sessions")
    } else if Path::new("data").is_dir() || Path::new("Cargo.toml").exists() {
        PathBuf::from("data/oauth_sessions")
    } else {
        std::env::temp_dir().join("aina_oauth_sessions")
    }
}

/// Resolves and canonicalizes a sandboxed session directory strictly contained under the OAuth root.
/// Fails fast on invalid characters, path traversal, or symlink escapes.
pub fn get_oauth_session_dir(session_id: &str) -> anyhow::Result<PathBuf> {
    if !is_safe_session_id(session_id) {
        anyhow::bail!(
            "Invalid session_id: must be 16-64 ASCII alphanumeric, '_' or '-' characters without path separators"
        );
    }

    // Confirm session_id parses as a single normal path component
    let mut comps = Path::new(session_id).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => anyhow::bail!("Invalid session_id: unsafe path component"),
    }

    let root = resolve_oauth_sessions_root();
    std::fs::create_dir_all(&root)?;
    let canonical_root = std::fs::canonicalize(&root)?;
    let target = canonical_root.join(session_id);

    if target.parent() != Some(canonical_root.as_path()) {
        anyhow::bail!("Unsafe OAuth session directory outside canonical root");
    }

    if target.exists() {
        let canonical_target = std::fs::canonicalize(&target)?;
        if !canonical_target.starts_with(&canonical_root) || canonical_target == canonical_root {
            anyhow::bail!("Symlink or path escape detected in OAuth session directory");
        }
        return Ok(canonical_target);
    }

    Ok(target)
}

fn simple_urldecode(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = format!("{}{}", h1, h2);
                if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                    res.push(byte as char);
                    continue;
                } else {
                    res.push('%');
                    res.push(h1);
                    res.push(h2);
                    continue;
                }
            } else {
                res.push('%');
                if let Some(h1) = h1 {
                    res.push(h1);
                }
                continue;
            }
        }
        res.push(c);
    }
    res
}

pub fn sanitize_oauth_code(raw: &str) -> String {
    let mut s = simple_urldecode(raw.trim());
    if let Some(idx) = s.find("code=") {
        s = s[idx + 5..].to_string();
    }
    for delim in &["&", "+http", " http", "userinfo.", "rinfo.", ".profile", "+", " "] {
        if let Some(idx) = s.find(delim) {
            s.truncate(idx);
        }
    }
    s.trim().to_string()
}

fn find_oauth_helper_script() -> PathBuf {
    let script_candidates = [
        PathBuf::from("scripts/oauth_helper.py"),
        PathBuf::from("/app/scripts/oauth_helper.py"),
    ];
    script_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("scripts/oauth_helper.py"))
}

fn inject_oauth_env(cmd: &mut tokio::process::Command) {
    if let Ok(val) = std::env::var("GOOGLE_CLIENT_ID")
        .or_else(|_| std::env::var("AINA_GOOGLE_CLIENT_ID"))
    {
        if !val.trim().is_empty() {
            cmd.env("GOOGLE_CLIENT_ID", val.trim());
        }
    }
    if let Ok(val) = std::env::var("GOOGLE_CLIENT_SECRET")
        .or_else(|_| std::env::var("AINA_GOOGLE_CLIENT_SECRET"))
    {
        if !val.trim().is_empty() {
            cmd.env("GOOGLE_CLIENT_SECRET", val.trim());
        }
    }
}

/// Spawns python oauth_helper.py to generate PKCE challenge and authorization URL.
pub async fn init_oauth_session() -> anyhow::Result<(String, String)> {
    let session_id = format!(
        "{}_{:08x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        rand::random::<u32>()
    );
    let base_dir = get_oauth_session_dir(&session_id)?;
    let _ = tokio::fs::create_dir_all(&base_dir).await;

    let script_path = find_oauth_helper_script();

    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&script_path).arg("init").arg(&session_id);
    inject_oauth_env(&mut cmd);

    let output = match tokio::time::timeout(std::time::Duration::from_secs(10), cmd.output()).await
    {
        Ok(res) => res?,
        Err(_) => {
            let _ = tokio::fs::remove_dir_all(&base_dir).await;
            anyhow::bail!("Timeout saat menginisialisasi sesi OAuth (10 detik).");
        }
    };

    let url_file = base_dir.join("auth_url.txt");
    if output.status.success() && tokio::fs::try_exists(&url_file).await.unwrap_or(false) {
        let auth_url = tokio::fs::read_to_string(&url_file).await?;
        return Ok((session_id, auth_url.trim().to_string()));
    }

    // Fallback: parse from stdout if url_file check didn't catch it
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(url) = line.strip_prefix("AUTH_URL:") {
            let clean_url = url.trim();
            let _ = tokio::fs::write(&url_file, clean_url).await;
            return Ok((session_id, clean_url.to_string()));
        }
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = tokio::fs::remove_dir_all(&base_dir).await;
    anyhow::bail!("Gagal menginisialisasi sesi login OAuth: {}", stderr.trim());
}

/// Exchanges authorization code for credentials using oauth_helper.py.
pub async fn exchange_oauth_code(session_id: &str, code: &str) -> anyhow::Result<String> {
    let base_dir = get_oauth_session_dir(session_id)?;
    let verifier_file = base_dir.join("code_verifier.txt");
    let alt_verifier_file = base_dir.join("verifier.txt");
    if !tokio::fs::try_exists(&verifier_file).await.unwrap_or(false)
        && !tokio::fs::try_exists(&alt_verifier_file).await.unwrap_or(false)
    {
        anyhow::bail!(
            "Sesi login tidak valid atau sudah kedaluwarsa. Silakan klik tombol 'Mulai Login Baru'."
        );
    }

    let clean_code = sanitize_oauth_code(code);
    let script_path = find_oauth_helper_script();

    // Direct sub-second PKCE token exchange
    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&script_path)
        .arg("exchange")
        .arg(session_id)
        .arg(&clean_code);
    inject_oauth_env(&mut cmd);

    let output = match tokio::time::timeout(std::time::Duration::from_secs(15), cmd.output()).await
    {
        Ok(res) => res?,
        Err(_) => {
            let _ = tokio::fs::remove_dir_all(&base_dir).await;
            anyhow::bail!(
                "Timeout saat menukar kode otorisasi ke Google (15 detik). Silakan coba lagi."
            );
        }
    };
    let token_file = base_dir.join("token.json");
    let error_file = base_dir.join("error.txt");

    if output.status.success() && tokio::fs::try_exists(&token_file).await.unwrap_or(false) {
        let token_content = tokio::fs::read_to_string(&token_file).await?;
        let _ = tokio::fs::remove_dir_all(&base_dir).await;
        return Ok(token_content);
    }

    let err = if let Ok(err_str) = tokio::fs::read_to_string(&error_file).await {
        err_str
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.is_empty() {
            "Verifikasi kode otorisasi gagal atau ditolak oleh Google.".to_string()
        } else {
            stderr
        }
    };
    let _ = tokio::fs::remove_dir_all(&base_dir).await;
    anyhow::bail!("{}", err.trim());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_session_id_and_path_traversal_rejection() {
        // Valid generated session IDs (16..=64 chars)
        assert!(is_safe_session_id("1732150000000_a1b2c3d4"));
        assert!(is_safe_session_id("session-abc_123456"));

        // Invalid / traversal payloads MUST be rejected
        assert!(!is_safe_session_id(""));
        assert!(!is_safe_session_id("short"));
        assert!(!is_safe_session_id(".."));
        assert!(!is_safe_session_id("../evil_session_1234"));
        assert!(!is_safe_session_id("../../etc/passwd"));
        assert!(!is_safe_session_id("1732150000000_a1/b2"));
        assert!(!is_safe_session_id("1732150000000_a1\\b2"));
        assert!(!is_safe_session_id("1732150000000_a1..b2"));
        assert!(!is_safe_session_id(&"a".repeat(65)));

        assert!(get_oauth_session_dir("../../etc/passwd").is_err());
        assert!(get_oauth_session_dir("..").is_err());
        assert!(get_oauth_session_dir("../evil_session_1234").is_err());
    }

    #[test]
    fn test_get_oauth_session_dir_canonical_containment() {
        let sid = "1732150000000_deadbeef";
        let resolved = get_oauth_session_dir(sid).expect("Valid session_id must resolve");
        assert_eq!(resolved.file_name().and_then(|s| s.to_str()), Some(sid));
        let root = std::fs::canonicalize(resolve_oauth_sessions_root()).unwrap();
        assert!(resolved.starts_with(&root));
    }

    #[test]
    fn test_sanitize_oauth_code() {
        // 1. Clean code
        assert_eq!(
            sanitize_oauth_code("4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"
        );

        // 2. User contaminated string with query parameters
        let contaminated = "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw-WjiV4m2f1zhXeJu_iPCcCRmmHdVMAdwaRKCBg7Jbs9Arinfo.profile+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcclog+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fexperimentsandconfigs+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform&authuser=0&prompt=consent";
        assert_eq!(
            sanitize_oauth_code(contaminated),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw-WjiV4m2f1zhXeJu_iPCcCRmmHdVMAdwaRKCBg7Jbs9A"
        );

        // 3. Full callback URL
        let url = "https://antigravity.google/oauth-callback?code=4/0ATsMZqCyxR6mxx8ph9vz1TH9kw&scope=email+profile";
        assert_eq!(
            sanitize_oauth_code(url),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"
        );

        // 4. URL encoded code parameter
        let url_encoded = "code=4%2F0ATsMZqCyxR6mxx8ph9vz1TH9kw&state=xyz";
        assert_eq!(
            sanitize_oauth_code(url_encoded),
            "4/0ATsMZqCyxR6mxx8ph9vz1TH9kw"
        );
    }
}
