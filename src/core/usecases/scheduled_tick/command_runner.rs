//! Hermetic subprocess execution with process-group isolation and hard timeouts.
//!
//! Used by both `DirectCommand` scheduled tasks and `AntigravityCliAdapter` so that
//! timed-out child processes (including background Chromium/Python subtrees) are
//! terminated cleanly via POSIX process-group SIGKILL without leaving orphans or zombies.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Environment credentials forwarded to Admin-authorized `DirectCommand` scripts.
#[derive(Debug, Clone)]
pub struct CommandEnvConfig {
    pub whatsmeow_base_url: String,
    pub whatsmeow_api_key: String,
    pub companion_base_url: Option<String>,
    pub companion_api_key: Option<String>,
}

/// Captured subprocess execution outcome.
#[derive(Debug)]
pub struct SubprocessOutput {
    pub status: std::process::ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Result of running a subprocess with an optional hard timeout.
#[derive(Debug)]
pub enum SubprocessOutcome {
    Completed(SubprocessOutput),
    TimedOut {
        elapsed: Duration,
        #[allow(dead_code)]
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

/// Terminates the entire Unix process group whose PGID equals `pid`.
#[cfg(unix)]
pub fn kill_process_group(pid: u32) {
    // SAFETY: POSIX `kill(-pgid, SIGKILL)` terminates every process in the
    // dedicated process group created via `Command::process_group(0)`.
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if let Ok(pgid) = i32::try_from(pid) {
        if pgid > 0 {
            unsafe {
                let _ = kill(-pgid, 9);
            }
        }
    }
}

#[cfg(not(unix))]
pub fn kill_process_group(_pid: u32) {}

/// Spawns `cmd` in its own process group (`pgid == pid`) and awaits completion
/// with an optional hard timeout. On timeout, kills the entire process group and
/// reaps the direct child so no zombie or orphaned descendants remain.
pub async fn run_command_with_pgroup_timeout(
    cmd: &mut Command,
    timeout: Option<Duration>,
) -> std::io::Result<SubprocessOutcome> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd.spawn()?;
    let child_pid = child.id();

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut r) = stdout_pipe {
            let _ = r.read_to_end(&mut buf).await;
        }
        buf
    });
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut r) = stderr_pipe {
            let _ = r.read_to_end(&mut buf).await;
        }
        buf
    });

    let wait_res = match timeout {
        Some(limit) if !limit.is_zero() => {
            match tokio::time::timeout(limit, child.wait()).await {
                Ok(res) => Ok(res?),
                Err(_) => Err(limit),
            }
        }
        _ => Ok(child.wait().await?),
    };

    match wait_res {
        Ok(status) => {
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            Ok(SubprocessOutcome::Completed(SubprocessOutput {
                status,
                stdout,
                stderr,
            }))
        }
        Err(limit) => {
            if let Some(pid) = child_pid {
                kill_process_group(pid);
            }
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
            let stdout = tokio::time::timeout(Duration::from_secs(1), stdout_task)
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();
            let stderr = tokio::time::timeout(Duration::from_secs(1), stderr_task)
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();
            Ok(SubprocessOutcome::TimedOut {
                elapsed: limit,
                stdout,
                stderr,
            })
        }
    }
}

/// Executes a deterministic shell command (`DirectCommand`) inside `workspace_dir`
/// without invoking any LLM session.
pub async fn execute_direct_shell_command(
    payload: &str,
    workspace_dir: Option<&Path>,
    env_cfg: Option<&CommandEnvConfig>,
    turn_token: Option<&str>,
    turn_started_epoch: Option<i64>,
    timeout: Duration,
) -> anyhow::Result<String> {
    let script = payload.trim();
    if script.is_empty() {
        anyhow::bail!("Payload DirectCommand kosong.");
    }

    let mut cmd = Command::new("bash");
    cmd.arg("-lc").arg(script);

    if let Some(dir) = workspace_dir {
        if dir.is_dir() {
            cmd.current_dir(dir);
        }
    }

    if let Some(cfg) = env_cfg {
        cmd.env("WHATSMEOW_BASE_URL", &cfg.whatsmeow_base_url);
        cmd.env("WHATSMEOW_API_KEY", &cfg.whatsmeow_api_key);
        if let Some(ref comp_url) = cfg.companion_base_url {
            cmd.env("WHATSMEOW_COMPANION_URL", comp_url);
        }
        if let Some(ref comp_key) = cfg.companion_api_key {
            cmd.env("WHATSMEOW_COMPANION_KEY", comp_key);
        }
    }
    if let Some(tok) = turn_token {
        cmd.env("AINA_TURN_TOKEN", tok);
    }
    if let Some(epoch) = turn_started_epoch {
        cmd.env("AINA_TURN_STARTED_EPOCH", epoch.to_string());
    }

    match run_command_with_pgroup_timeout(&mut cmd, Some(timeout)).await? {
        SubprocessOutcome::Completed(out) => {
            let stdout_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr_str = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if out.status.success() {
                Ok(stdout_str)
            } else {
                let detail = if !stderr_str.is_empty() {
                    stderr_str
                } else if !stdout_str.is_empty() {
                    stdout_str
                } else {
                    format!("Command exited with code {:?}", out.status.code())
                };
                anyhow::bail!(
                    "DirectCommand gagal (exit code {:?}): {}",
                    out.status.code(),
                    detail
                );
            }
        }
        SubprocessOutcome::TimedOut { elapsed, stderr, .. } => {
            let err_tail = String::from_utf8_lossy(&stderr).trim().to_string();
            if err_tail.is_empty() {
                anyhow::bail!(
                    "DirectCommand melebihi batas waktu {} detik (process group dihentikan).",
                    elapsed.as_secs()
                );
            } else {
                anyhow::bail!(
                    "DirectCommand melebihi batas waktu {} detik (process group dihentikan). Stderr: {}",
                    elapsed.as_secs(),
                    err_tail
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn direct_command_succeeds_and_returns_stdout() {
        let out = execute_direct_shell_command(
            "echo hello-direct-command",
            None,
            None,
            None,
            None,
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(out, "hello-direct-command");
    }

    #[tokio::test]
    async fn direct_command_nonzero_exit_fails_fast() {
        let err = execute_direct_shell_command(
            "echo boom >&2; exit 42",
            None,
            None,
            None,
            None,
            Duration::from_secs(10),
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("42"));
        assert!(msg.contains("boom"));
    }

    #[tokio::test]
    async fn direct_command_timeout_kills_process_group_including_grandchildren() {
        let marker = format!("aina_pgroup_test_{}", rand::random::<u32>());
        let pid_file = std::env::temp_dir().join(format!("{}.pid", marker));
        let script = format!(
            "sleep 30 & echo $! > '{}' ; wait",
            pid_file.display()
        );

        let start = std::time::Instant::now();
        let err = execute_direct_shell_command(
            &script,
            None,
            None,
            None,
            None,
            Duration::from_millis(600),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("melebihi batas waktu"));
        assert!(start.elapsed() < Duration::from_secs(5));

        // Verify background grandchild was terminated via process-group SIGKILL
        if let Ok(pid_txt) = std::fs::read_to_string(&pid_file) {
            if let Ok(grandchild_pid) = pid_txt.trim().parse::<u32>() {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let proc_dir = std::path::PathBuf::from(format!("/proc/{}", grandchild_pid));
                assert!(
                    !proc_dir.exists(),
                    "Grandchild process {} survived process-group kill!",
                    grandchild_pid
                );
            }
        }
        let _ = std::fs::remove_file(&pid_file);
    }
}
