use std::ffi::OsStr;
use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::oracle::configure_child_process_group;

#[derive(Debug, Error)]
pub enum TimedSubprocessError {
    #[error("subprocess {program} spawn failed during {operation}: {source}")]
    Spawn {
        program: String,
        operation: &'static str,
        source: io::Error,
    },
    #[error("subprocess {program} poll failed during {operation}: {source}")]
    Poll {
        program: String,
        operation: &'static str,
        source: io::Error,
    },
    #[error("subprocess {program} wait failed during {operation}: {source}")]
    Wait {
        program: String,
        operation: &'static str,
        source: io::Error,
    },
    #[error(
        "subprocess {program} timed out after {elapsed_secs}s during {operation}; timeout_secs={timeout_secs}; stdout_tail={stdout_tail}; stderr_tail={stderr_tail}; remediation: fix the hung subprocess or raise the timeout only after profiling"
    )]
    Timeout {
        program: String,
        operation: &'static str,
        timeout_secs: u64,
        elapsed_secs: u64,
        stdout_tail: String,
        stderr_tail: String,
    },
}

impl TimedSubprocessError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Spawn { .. } => "MEJEPA_CORPUS_SUBPROCESS_SPAWN_FAILED",
            Self::Poll { .. } => "MEJEPA_CORPUS_SUBPROCESS_POLL_FAILED",
            Self::Wait { .. } => "MEJEPA_CORPUS_SUBPROCESS_WAIT_FAILED",
            Self::Timeout { .. } => "MEJEPA_CORPUS_SUBPROCESS_TIMEOUT",
        }
    }
}

pub fn run_capture_timed(
    program: &Path,
    args: &[&str],
    timeout: Duration,
    operation: &'static str,
) -> Result<Output, TimedSubprocessError> {
    let program_display = display_program(program);
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    configure_child_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|source| TimedSubprocessError::Spawn {
            program: program_display.clone(),
            operation,
            source,
        })?;
    let stdout_handle = child.stdout.take().map(spawn_drain);
    let stderr_handle = child.stderr.take().map(spawn_drain);
    let started = Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let (stdout, stderr) = terminate_child_and_collect(
                        &program_display,
                        operation,
                        &mut child,
                        stdout_handle,
                        stderr_handle,
                    )?;
                    return Err(TimedSubprocessError::Timeout {
                        program: program_display,
                        operation,
                        timeout_secs: timeout.as_secs(),
                        elapsed_secs: started.elapsed().as_secs(),
                        stdout_tail: tail_bytes(&stdout),
                        stderr_tail: tail_bytes(&stderr),
                    });
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(source) => {
                return Err(TimedSubprocessError::Poll {
                    program: program_display,
                    operation,
                    source,
                });
            }
        }
    };

    let stdout = stdout_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn terminate_child_and_collect(
    program: &str,
    operation: &'static str,
    child: &mut Child,
    stdout_handle: Option<thread::JoinHandle<Vec<u8>>>,
    stderr_handle: Option<thread::JoinHandle<Vec<u8>>>,
) -> Result<(Vec<u8>, Vec<u8>), TimedSubprocessError> {
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new("kill")
            .args(["-TERM", "--", &process_group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(source) => {
                    return Err(TimedSubprocessError::Poll {
                        program: program.to_string(),
                        operation,
                        source,
                    });
                }
            }
        }
        let _ = Command::new("kill")
            .args(["-KILL", "--", &process_group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    child.wait().map_err(|source| TimedSubprocessError::Wait {
        program: program.to_string(),
        operation,
        source,
    })?;
    let stdout = stdout_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    Ok((stdout, stderr))
}

fn spawn_drain<R>(mut reader: R) -> thread::JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

fn display_program(program: &Path) -> String {
    program
        .as_os_str()
        .to_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DisplayOsStr(program.as_os_str()).to_string())
}

fn tail_bytes(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars().rev().take(2000).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

struct DisplayOsStr<'a>(&'a OsStr);

impl fmt::Display for DisplayOsStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn timed_subprocess_captures_fast_stdout() {
        let output = run_capture_timed(
            Path::new("sh"),
            &["-c", "printf 'timed-subprocess-ok'"],
            Duration::from_secs(5),
            "timed subprocess fast test",
        )
        .expect("fast subprocess should succeed");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "timed-subprocess-ok"
        );
    }

    #[test]
    fn timed_subprocess_times_out_and_returns_structured_error() {
        let started = Instant::now();
        let error = run_capture_timed(
            Path::new("sh"),
            &["-c", "sleep 30"],
            Duration::from_millis(50),
            "timed subprocess timeout test",
        )
        .expect_err("sleeping subprocess must time out");

        assert_eq!(error.code(), "MEJEPA_CORPUS_SUBPROCESS_TIMEOUT");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout helper must terminate quickly; elapsed={:?}",
            started.elapsed()
        );
    }

    #[test]
    fn timed_subprocess_drains_large_stdout() {
        let output = run_capture_timed(
            Path::new("sh"),
            &[
                "-c",
                "i=0; while [ \"$i\" -lt 200000 ]; do echo line_$i; i=$((i+1)); done",
            ],
            Duration::from_secs(30),
            "timed subprocess large stdout test",
        )
        .expect("large stdout subprocess should not deadlock");

        assert!(output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("line_199999"));
    }
}
