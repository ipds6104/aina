//! OAuth URL parsing, session directory resolution, and PKCE authorization code sanitization.

use std::path::PathBuf;

pub fn get_oauth_session_dir(session_id: &str) -> PathBuf {
    let candidate_app = PathBuf::from(format!("/app/data/oauth_sessions/{}", session_id));
    let candidate_rel = PathBuf::from(format!("data/oauth_sessions/{}", session_id));
    let candidate_tmp = PathBuf::from(format!("/tmp/aina_oauth_{}", session_id));

    if candidate_app.exists() {
        candidate_app
    } else if candidate_rel.exists() {
        candidate_rel
    } else if candidate_tmp.exists() {
        candidate_tmp
    } else if std::path::Path::new("/app/data").is_dir() {
        candidate_app
    } else if std::path::Path::new("data").is_dir() || std::path::Path::new("Cargo.toml").exists() {
        candidate_rel
    } else {
        candidate_tmp
    }
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
                if let Some(h1) = h1 { res.push(h1); }
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
    let base_dir = get_oauth_session_dir(&session_id);
    let _ = tokio::fs::create_dir_all(&base_dir).await;

    let script_candidates = [
        PathBuf::from("scripts/oauth_helper.py"),
        PathBuf::from("/root/projects/aina/scripts/oauth_helper.py"),
        PathBuf::from("/app/scripts/oauth_helper.py"),
    ];
    let script_path = script_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("scripts/oauth_helper.py"));

    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&script_path)
        .arg("init")
        .arg(&session_id);

    let output = match tokio::time::timeout(std::time::Duration::from_secs(10), cmd.output()).await {
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = tokio::fs::remove_dir_all(&base_dir).await;
    anyhow::bail!("Gagal menginisialisasi sesi login OAuth: {}", stderr);
}

/// Exchanges authorization code for credentials using oauth_helper.py.
pub async fn exchange_oauth_code(session_id: &str, code: &str) -> anyhow::Result<String> {
    let base_dir = get_oauth_session_dir(session_id);
    let verifier_file = base_dir.join("code_verifier.txt");
    if !tokio::fs::try_exists(&verifier_file).await.unwrap_or(false) {
        anyhow::bail!(
            "Sesi login tidak valid atau sudah kedaluwarsa. Silakan klik tombol 'Mulai Login Baru'."
        );
    }

    let clean_code = sanitize_oauth_code(code);

    let script_candidates = [
        PathBuf::from("scripts/oauth_helper.py"),
        PathBuf::from("/root/projects/aina/scripts/oauth_helper.py"),
        PathBuf::from("/app/scripts/oauth_helper.py"),
    ];
    let script_path = script_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("scripts/oauth_helper.py"));

    // Direct sub-second PKCE token exchange
    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&script_path)
        .arg("exchange")
        .arg(session_id)
        .arg(&clean_code);

    let output = match tokio::time::timeout(std::time::Duration::from_secs(15), cmd.output()).await {
        Ok(res) => res?,
        Err(_) => {
            let _ = tokio::fs::remove_dir_all(&base_dir).await;
            anyhow::bail!("Timeout saat menukar kode otorisasi ke Google (15 detik). Silakan coba lagi.");
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
