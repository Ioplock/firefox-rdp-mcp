//! Thin typed wrapper around the `ff-rdp` CLI.
//!
//! Design (per architecture decision): this MCP server implements NO
//! Firefox RDP itself. Every tool shells out to `ff-rdp --format json`,
//! parses the JSON envelope (`results` / `total` / `meta`) and returns it
//! as structured MCP content. No human-readable output is regex-parsed.
//!
//! `ff-rdp` is stateless: each invocation opens its own RDP connection,
//! does its work and disconnects. There is no daemon to manage.

use std::process::Stdio;
use tokio::time::{Duration, Instant};
tokio::task_local! { pub static DEADLINE: Instant; }

/// Connection settings + path to the backend binary.
#[derive(Clone, Debug)]
pub struct Backend {
    /// `ff-rdp` executable. Defaults to `ff-rdp` on PATH (provided by
    /// `nix develop`); override with `FF_RDP_BIN`.
    pub exe: String,
    /// RDP host. Always defaults to loopback; override with `FF_RDP_HOST`.
    pub host: String,
    /// RDP port. Defaults to 6000; override with `FF_RDP_PORT`.
    pub port: u16,
    /// Per-command operation deadline in MILLISECONDS (ff-rdp `--timeout`
    /// is ms, default 10000). Defaults to 8000; `FF_RDP_TIMEOUT_MS`.
    pub timeout_ms: u64,
}

impl Backend {
    pub fn from_env() -> Self {
        Self {
            exe: std::env::var("FF_RDP_BIN").unwrap_or_else(|_| "ff-rdp".to_owned()),
            host: std::env::var("FF_RDP_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned()),
            port: std::env::var("FF_RDP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6000),
            timeout_ms: std::env::var("FF_RDP_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8_000),
        }
    }

    /// Run `ff-rdp` with the given subcommand args.
    /// Returns the parsed JSON envelope on success.
    pub async fn run(&self, args: Vec<String>) -> Result<serde_json::Value, rmcp::ErrorData> {
        let mut cmd = tokio::process::Command::new(&self.exe);
        cmd.args([
            "--host",
            &self.host,
            "--port",
            &self.port.to_string(),
            "--timeout",
            &self.timeout_ms.to_string(),
            "--format",
            "json",
        ]);
        cmd.args(&args);
        cmd.stdin(Stdio::null());

        cmd.kill_on_drop(true);
        let budget = DEADLINE
            .try_with(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_millis(self.timeout_ms));
        let started = Instant::now();
        let operation = args.first().map(String::as_str).unwrap_or("status");
        let output = tokio::time::timeout(budget.min(Duration::from_millis(self.timeout_ms)), cmd.output())
            .await.map_err(|_| crate::error::internal("backend deadline exceeded; action may already have occurred — inspect before retrying"))?
            .map_err(|e| {
            crate::error::internal(format!(
                "failed to spawn `{}` (is ff-rdp on PATH? run inside `nix develop`): {e}",
                self.exe
            ))
        })?;

        tracing::info!(
            operation,
            elapsed_ms = started.elapsed().as_millis() as u64,
            success = output.status.success(),
            output_bytes = output.stdout.len(),
            "backend finished"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

        match serde_json::from_str::<serde_json::Value>(&stdout) {
            Ok(json) => {
                if output.status.success() {
                    Ok(json)
                } else {
                    // ff-rdp prints a `hint:` line naming the next command.
                    Err(crate::error::internal(format!(
                        "ff-rdp failed (exit {}): {}",
                        output.status.code().unwrap_or(-1),
                        if stderr.is_empty() {
                            json.to_string()
                        } else {
                            stderr
                        }
                    )))
                }
            }
            Err(_) => Err(crate::error::internal(format!(
                "ff-rdp returned non-JSON output (exit {}). stdout: {:?} stderr: {:?}",
                output.status.code().unwrap_or(-1),
                stdout.chars().take(500).collect::<String>(),
                stderr.chars().take(500).collect::<String>(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn deadline_kills_backend_instead_of_leaving_orphan() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ff-mcp-deadline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("backend");
        let pidfile = dir.join("pid");
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        std::fs::write(
            &script,
            format!(
                "#!{shell}\necho $$ > '{}'\nexec sleep 30\n",
                pidfile.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let backend = Backend {
            exe: script.to_string_lossy().into(),
            host: "127.0.0.1".into(),
            port: 6000,
            timeout_ms: 30000,
        };
        let start = Instant::now();
        let result = DEADLINE
            .scope(
                Instant::now() + Duration::from_millis(200),
                backend.run(vec!["tabs".into()]),
            )
            .await;
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        let pid = std::fs::read_to_string(&pidfile).unwrap();
        let proc = std::path::PathBuf::from(format!("/proc/{}", pid.trim()));
        for _ in 0..50 {
            if !proc.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!proc.exists(), "timed-out backend was not reaped");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
