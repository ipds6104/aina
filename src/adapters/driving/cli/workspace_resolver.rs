use std::path::{Path, PathBuf};

pub fn resolve_workspace(args: &[String]) -> PathBuf {
    let mut idx = 0;
    while idx < args.len() {
        if (args[idx] == "--workspace" || args[idx] == "-w") && idx + 1 < args.len() {
            let val = &args[idx + 1];
            let p = PathBuf::from(val);
            if p.is_dir() {
                return p;
            }
            // Check if relative to AINA_WORKSPACES_DIR
            if let Ok(root_env) = std::env::var("AINA_WORKSPACES_DIR") {
                let cand = Path::new(&root_env).join(val);
                if cand.is_dir() {
                    return cand;
                }
            }
            // Check if relative to workspaces/
            let candidate = Path::new("workspaces").join(val);
            if candidate.is_dir() {
                return candidate;
            }
            return p;
        }
        idx += 1;
    }

    // 1. Check AGENT_WORKSPACE / AINA_WORKSPACE environment variable
    for env_key in &["AGENT_WORKSPACE", "AINA_WORKSPACE"] {
        if let Ok(val) = std::env::var(env_key) {
            if !val.trim().is_empty() {
                let p = PathBuf::from(val);
                if p.is_dir() {
                    return p;
                }
            }
        }
    }

    // 2. Check AINA_WORKSPACES_DIR/default
    if let Ok(root_env) = std::env::var("AINA_WORKSPACES_DIR") {
        if !root_env.trim().is_empty() {
            let p = Path::new(&root_env).join("default");
            if p.is_dir() {
                return p;
            }
        }
    }

    // 3. Check if current working directory (CWD) is a workspace (has knowledge/)
    let cwd = PathBuf::from(".");
    if cwd.join("knowledge").is_dir() {
        return cwd;
    }

    // 4. Check repo's workspaces/default
    if Path::new("workspaces/default").is_dir() {
        return PathBuf::from("workspaces/default");
    }

    PathBuf::from("workspaces/default")
}
