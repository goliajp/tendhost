//! Runs one command the server asked for.

use std::process::Stdio;
use std::time::Duration;

use chrono::Utc;
use tendhost_protocol::RunResult;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// How much of the end of the output is kept.
const KEEP_BYTES: usize = 64 * 1024;

/// Runs `command` with `sh -c`, stdout and stderr together, in a process
/// group of its own. At the timeout the whole group is killed, so nothing
/// the command started outlives it.
pub async fn execute(run_id: String, command: &str, timeout: Duration) -> RunResult {
    let started_at = Utc::now();
    let failed = |output: String| RunResult {
        run_id: run_id.clone(),
        exit_code: None,
        timed_out: false,
        output,
        started_at,
        finished_at: Utc::now(),
    };

    let spawned = Command::new("sh")
        .arg("-c")
        .arg(format!("exec 2>&1\n{command}"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return failed(format!("tendhostd could not start the command: {error}")),
    };
    let group = child.id();
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let reader = tokio::spawn(async move {
        let mut tail = Vec::new();
        let mut chunk = [0u8; 8192];
        while let Ok(n) = stdout.read(&mut chunk).await
            && n > 0
        {
            tail.extend_from_slice(&chunk[..n]);
            if tail.len() > 2 * KEEP_BYTES {
                tail.drain(..tail.len() - KEEP_BYTES);
            }
        }
        if tail.len() > KEEP_BYTES {
            tail.drain(..tail.len() - KEEP_BYTES);
        }
        String::from_utf8_lossy(&tail).into_owned()
    });

    let (status, timed_out) = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(status) => (status.ok(), false),
        Err(_) => {
            if let Some(group) = group {
                // SAFETY: killpg only sends a signal; `group` is the id of
                // the process group created for this child above
                unsafe { libc::killpg(group as libc::pid_t, libc::SIGKILL) };
            }
            (child.wait().await.ok(), true)
        }
    };
    RunResult {
        run_id: run_id.clone(),
        exit_code: status.and_then(|s| s.code()).filter(|_| !timed_out),
        timed_out,
        output: reader.await.unwrap_or_default(),
        started_at,
        finished_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run(command: &str, timeout_ms: u64) -> RunResult {
        execute("run_1".into(), command, Duration::from_millis(timeout_ms)).await
    }

    #[tokio::test]
    async fn the_exit_code_and_both_output_streams_come_back() {
        let ok = run("echo out; echo err >&2", 5000).await;
        assert_eq!((ok.exit_code, ok.timed_out), (Some(0), false));
        assert!(ok.output.contains("out") && ok.output.contains("err"));
        assert_eq!(run("exit 3", 5000).await.exit_code, Some(3));
        assert!(ok.finished_at >= ok.started_at);
    }

    #[tokio::test]
    async fn a_command_past_its_timeout_is_killed_with_what_it_started() {
        let marker = std::env::temp_dir().join(format!("tendhostd-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let command = format!(
            "echo begun; (sleep 1; touch {}) & sleep 30",
            marker.display()
        );
        let started = std::time::Instant::now();
        let result = run(&command, 300).await;
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!((result.exit_code, result.timed_out), (None, true));
        assert!(result.output.contains("begun"));
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(
            !marker.exists(),
            "a background child of the command outlived the timeout"
        );
    }

    #[tokio::test]
    async fn only_the_end_of_long_output_is_kept() {
        let result = run(
            "i=0; while [ $i -lt 30000 ]; do echo line-$i; i=$((i+1)); done; echo last",
            20000,
        )
        .await;
        assert!(result.output.len() <= KEEP_BYTES);
        assert!(result.output.ends_with("last\n"));
        assert!(!result.output.contains("line-0\n"));
    }
}
