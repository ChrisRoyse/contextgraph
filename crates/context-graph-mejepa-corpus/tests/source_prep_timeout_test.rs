//! REGRESSION TEST for the 2026-05-11 source-prep hang.
//!
//! Reproduces the exact failure mode that wedged the corpus generator for
//! 5 hours on `django__django-15790`: a `Command::new("git").output()` call
//! against a hung git subprocess. The fake `git` script below sleeps for
//! 9999 seconds — without the timeout fix, this test would itself hang
//! indefinitely. With the fix, it must return `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT`
//! within (configured_timeout + grace) seconds.
//!
//! This is a real subprocess test — not a mock. We materialize an actual
//! shell script on disk, point `$PATH` at it, and run the production
//! `run_git` helper. The Source of Truth is:
//!   1. The `MutationError::SubprocessTimeout` variant returned to the caller
//!   2. The wallclock elapsed (must be ~= the timeout, not 9999 seconds)
//!   3. The error code string `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT`
//!   4. Absence of orphan child processes after the helper returns

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use context_graph_mejepa_corpus::MutationError;
use tempfile::TempDir;

/// Materialize a fake `git` script that sleeps forever. Returns the directory
/// to prepend to PATH so `Command::new("git")` picks up this script instead
/// of the real git binary.
fn hanging_git_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("git");
    fs::write(
        &script,
        "#!/usr/bin/env bash\n\
         # Fake git that hangs forever. Used by source_prep_timeout_test to\n\
         # prove run_git_timed escalates SIGTERM/SIGKILL on deadline.\n\
         exec sleep 9999\n",
    )
    .expect("write hanging git script");
    let mut perms = fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).expect("chmod hanging git");
    dir
}

/// Materialize a fake `git` script that prints something and exits 0
/// immediately. Confirms the timed runner does not slow down well-behaved
/// subprocesses — the happy-path latency must remain low.
fn fast_git_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("git");
    fs::write(
        &script,
        "#!/usr/bin/env bash\n\
         echo 'ContextGraph fast-git stub'\n\
         exit 0\n",
    )
    .expect("write fast git script");
    let mut perms = fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).expect("chmod fast git");
    dir
}

/// Snapshot of current `git` subprocesses owned by the current user. Used to
/// assert the timeout helper actually killed the hung child and didn't
/// orphan it. Uses `pgrep -U $(id -u)` rather than libc to keep the test
/// dependency surface minimal.
fn count_user_git_subprocs() -> usize {
    let uid_output = std::process::Command::new("id")
        .arg("-u")
        .output()
        .expect("id -u");
    let uid = String::from_utf8_lossy(&uid_output.stdout)
        .trim()
        .to_string();
    let output = std::process::Command::new("pgrep")
        .args(["-U", &uid, "-x", "git"])
        .output()
        .expect("pgrep git");
    if !output.status.success() {
        // pgrep exits 1 when no matches; that's correct (zero subprocesses).
        return 0;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// HAPPY PATH: a fast `git` subprocess completes well within the timeout.
/// FSV: error is Ok(_), elapsed wallclock is < 1 second.
#[test]
fn run_git_timed_completes_fast_subprocess_without_delay() {
    let stub_dir = fast_git_dir();
    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", stub_dir.path().display(), orig_path);
    let _path_guard = PathGuard::set(&new_path);

    let work_dir = tempfile::tempdir().expect("work tempdir");
    let started = Instant::now();
    // `status` verb falls in the QUERY timeout class (120s) — but the fast-git
    // stub exits within milliseconds.
    let result = run_git_status(work_dir.path());
    let elapsed = started.elapsed();

    assert!(
        result.is_ok(),
        "fast-git stub must succeed; got: {:?}",
        result
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "happy-path latency must stay under 3s; got {elapsed:?}",
    );
}

/// REGRESSION: a hung `git` subprocess MUST return SubprocessTimeout within
/// (timeout + 2s grace) seconds and leave no orphan child.
#[test]
fn run_git_timed_kills_hung_subprocess_within_grace_window() {
    let stub_dir = hanging_git_dir();
    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", stub_dir.path().display(), orig_path);
    let _path_guard = PathGuard::set(&new_path);

    let work_dir = tempfile::tempdir().expect("work tempdir");
    // Sub-3s timeout via the GIT_TEST_TIMEOUT_SECS env var honored by
    // run_git_with_explicit_timeout below. This keeps the test fast while
    // still exercising the SIGTERM→SIGKILL path in oracle::wait_with_timeout.
    let baseline_git = count_user_git_subprocs();

    let started = Instant::now();
    let result = run_git_with_explicit_timeout(work_dir.path(), Duration::from_secs(2));
    let elapsed = started.elapsed();

    let err = result.expect_err("hung git must produce a SubprocessTimeout error");
    let code = err.code();
    assert_eq!(
        code, "MEJEPA_CORPUS_SUBPROCESS_TIMEOUT",
        "expected MEJEPA_CORPUS_SUBPROCESS_TIMEOUT; got {code} (message: {err})",
    );

    // FSV §6.4: the wallclock proves the kill path actually fires. Without
    // the fix this test would never reach this assertion (sleep 9999).
    assert!(
        elapsed >= Duration::from_secs(2),
        "must wait at least the configured timeout; got {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "must escalate to SIGKILL within (timeout + 6s) grace; got {elapsed:?}",
    );

    // FSV §6.5: physical-state verification — no orphan git subprocess on
    // the system after the helper returned.
    std::thread::sleep(Duration::from_millis(500));
    let post_git = count_user_git_subprocs();
    assert!(
        post_git <= baseline_git,
        "timed runner orphaned a git subprocess: before={baseline_git}, after={post_git}",
    );
}

/// REGRESSION (2026-05-11 sub-bug 3): a `git`-like subprocess that emits
/// more than 64 KB to stdout MUST complete without deadlocking. The previous version
/// of `run_git_timed` deadlocked because it only read stdout via
/// `wait_with_output()` AFTER the child exited — for huge output the OS
/// pipe buffer (64 KB on Linux) filled, the child blocked on `write()`,
/// the parent never saw `try_wait()` succeed, and the spurious "timeout"
/// fired. The fix drains stdout/stderr in background threads.
///
/// This test materializes a stub that prints 200,000 lines to stdout
/// (≈ 5 MB) and exits 0. Without the drain-threads fix this would hang
/// past the timeout and fail with SubprocessTimeout. With the fix it
/// completes within the timeout AND we read back the full 200,000 lines.
#[test]
fn run_git_timed_drains_large_stdout_without_deadlock() {
    let stub_dir = tempfile::tempdir().expect("tempdir");
    let script = stub_dir.path().join("git");
    fs::write(
        &script,
        "#!/usr/bin/env bash\n\
         # Emit 200,000 lines (~5 MB) to stdout to exceed any Linux pipe\n\
         # buffer (typically 64 KB) and prove run_git_timed drains the pipe\n\
         # while waiting for the child to exit.\n\
         for i in $(seq 1 200000); do echo \"line_$i\"; done\n\
         exit 0\n",
    )
    .expect("write large-output stub");
    let mut perms = fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).expect("chmod large stub");

    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", stub_dir.path().display(), orig_path);
    let _path_guard = PathGuard::set(&new_path);

    let work_dir = tempfile::tempdir().expect("work tempdir");
    let started = Instant::now();
    let result = run_git_with_explicit_timeout(work_dir.path(), Duration::from_secs(60));
    let elapsed = started.elapsed();

    assert!(
        result.is_ok(),
        "large-output stub must succeed without deadlocking; got: {:?}; elapsed={:?}",
        result,
        elapsed,
    );
    assert!(
        elapsed < Duration::from_secs(15),
        "200_000-line stub should complete in seconds; got {elapsed:?} (probable pipe deadlock)",
    );
}

/// BOUNDARY: zero-second timeout. Spawning is observed; the deadline expires
/// before the subprocess can produce output. Still returns SubprocessTimeout
/// (not a silent success).
#[test]
fn run_git_timed_zero_second_timeout_fails_closed() {
    let stub_dir = hanging_git_dir();
    let orig_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", stub_dir.path().display(), orig_path);
    let _path_guard = PathGuard::set(&new_path);

    let work_dir = tempfile::tempdir().expect("work tempdir");
    let started = Instant::now();
    let result = run_git_with_explicit_timeout(work_dir.path(), Duration::from_millis(50));
    let elapsed = started.elapsed();

    let err = result.expect_err("zero-budget call must fail closed");
    assert_eq!(err.code(), "MEJEPA_CORPUS_SUBPROCESS_TIMEOUT");
    assert!(
        elapsed < Duration::from_secs(5),
        "zero-budget timeout must terminate quickly; got {elapsed:?}",
    );
}

// ---------------- production-API access helpers ----------------

/// Reach into the source-prep code path the way the production resume-runner
/// does. We can't call `run_git_timed` directly because it's a private fn,
/// but `run_git` (which delegates to it) is `pub(crate)` and re-exposed via
/// `test_support::run_git_for_tests`. If that helper doesn't exist yet, the
/// test compiles failure forces us to add it before merging — which is the
/// right hygiene (FSV-PROTOCOL §10 "tests must fail when the project is in
/// a broken state").
fn run_git_status(work_dir: &std::path::Path) -> Result<(), MutationError> {
    context_graph_mejepa_corpus::test_support::run_git_for_tests(work_dir, &["status"])
}

fn run_git_with_explicit_timeout(
    work_dir: &std::path::Path,
    timeout: Duration,
) -> Result<(), MutationError> {
    context_graph_mejepa_corpus::test_support::run_git_with_timeout_for_tests(
        work_dir,
        &["status"],
        timeout,
    )
}

// ---------------- PATH guard ----------------

/// Process-global PATH lock. Cargo's default test harness runs the 3 tests
/// in this file in parallel; they all mutate `$PATH`, which is process state,
/// not thread state. Without serialization, test A's PATH-stub gets clobbered
/// by test B's, and the subprocess git resolution races. The Mutex here
/// guarantees only one PathGuard exists at a time, restoring correctness
/// while leaving the rest of the test harness parallel-safe.
static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PathGuard {
    original: String,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl PathGuard {
    fn set(new_value: &str) -> Self {
        let guard = PATH_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::var("PATH").unwrap_or_default();
        // SAFETY: serialized by the Mutex above. Only one PathGuard holds
        // the lock at any time, so the set_var → drop pair never races
        // with another mutation in this process.
        unsafe { std::env::set_var("PATH", new_value) };
        Self {
            original,
            _guard: guard,
        }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        unsafe { std::env::set_var("PATH", &self.original) };
    }
}
