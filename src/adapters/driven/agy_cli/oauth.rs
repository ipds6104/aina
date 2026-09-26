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
