use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::oracle::configure_child_process_group;
use crate::source_patch::SourcePatchTask;
use crate::{MutationCategory, MutationError, MutationResult};

/// Per-operation timeouts for git subprocess calls invoked from source-prep.
/// Calibrated empirically against the SWE-bench Lite Python repos and the
/// observed p99 latencies on WSL2 → D-drive in this workspace.
///
/// REGRESSION HISTORY:
/// - 2026-05-11 #1: corpus generator hung 5h on `django__django-15790`
///   because `Command::output()` had no timeout. Fix: route every git
///   subprocess through `run_git_timed`.
/// - 2026-05-11 #2 (this file): tree-enumeration ops on `astropy/astropy`
///   legitimately take 120-300s on cold cache; the original 120s
///   GIT_QUERY_TIMEOUT was too tight. Fix: split enumeration off as its
///   own GIT_ENUMERATION_TIMEOUT = 600s. The original 120s still applies
///   to single-blob queries (cat-file -e, show), where 120s leaves
///   plenty of margin while still catching a true hang.
const GIT_QUERY_TIMEOUT: Duration = Duration::from_secs(120);
const GIT_ENUMERATION_TIMEOUT: Duration = Duration::from_secs(600);
const GIT_WRITE_TIMEOUT: Duration = Duration::from_secs(300);
const GIT_NETWORK_TIMEOUT: Duration = Duration::from_secs(900);

pub(crate) fn ensure_repo_cache(
    repo: &str,
    base_commit: &str,
    repo_cache_dir: &Path,
) -> MutationResult<PathBuf> {
    validate_repo_slug(repo)?;
    fs::create_dir_all(repo_cache_dir).map_err(|err| {
        MutationError::invalid(
            "repo_cache_dir",
            format!("failed to create file:{}: {err}", repo_cache_dir.display()),
            "choose a writable repository cache directory",
        )
    })?;
    let cache = repo_cache_dir.join(format!("{}.git", repo.replace('/', "__")));
    if !cache.exists() {
        let url = format!("https://github.com/{repo}.git");
        run_git(
            &[],
            &[
                "clone",
                "--quiet",
                "--bare",
                &url,
                cache.to_str().ok_or_else(|| {
                    MutationError::invalid(
                        "repo_cache_dir",
                        "repository cache path is not valid UTF-8",
                        "use a UTF-8 filesystem path for the repository cache",
                    )
                })?,
            ],
            "clone repository cache",
        )?;
    }
    if !git_commit_exists(&cache, base_commit)? {
        run_git(
            &[cache.as_path()],
            &["fetch", "--quiet", "origin"],
            "fetch missing base commit",
        )?;
    }
    if !git_commit_exists(&cache, base_commit)? {
        return Err(MutationError::invalid(
            "base_commit",
            format!(
                "git:{repo} base commit {base_commit} is not present in file:{}",
                cache.display()
            ),
            "verify the task manifest base_commit against the official SWE-bench dataset",
        ));
    }
    Ok(cache)
}

pub(crate) fn candidate_python_paths(
    repo_cache: &Path,
    base_commit: &str,
    changed_paths: &[String],
) -> MutationResult<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for path in changed_paths.iter().filter(|path| is_python_path(path)) {
        if seen.insert(path.clone()) {
            out.push(path.clone());
        }
    }
    let mut tracked_python = list_python_files_at_commit(repo_cache, base_commit)?;
    tracked_python.sort_by(|left, right| {
        candidate_python_priority(left)
            .cmp(&candidate_python_priority(right))
            .then_with(|| left.cmp(right))
    });
    for path in tracked_python {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    Ok(out)
}

pub(crate) fn list_python_files_at_commit(
    repo_cache: &Path,
    base_commit: &str,
) -> MutationResult<Vec<String>> {
    let output = run_git_capture(
        repo_cache,
        &["ls-tree", "-r", "--name-only", base_commit],
        "list Python files at base commit",
    )?;
    Ok(output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter(|line| is_python_path(line))
        .map(ToString::to_string)
        .collect())
}

pub(crate) fn parse_patch_file_paths(patch: &str) -> MutationResult<Vec<String>> {
    let mut paths = Vec::new();
    for line in patch.lines() {
        if !line.starts_with("diff --git ") {
            continue;
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 {
            return Err(MutationError::invalid(
                "patch.diff_header",
                format!("invalid diff header: {line}"),
                "pass a complete git-format unified diff",
            ));
        }
        for raw in [parts[2], parts[3]] {
            if raw == "/dev/null" {
                continue;
            }
            let path = raw
                .strip_prefix("a/")
                .or_else(|| raw.strip_prefix("b/"))
                .unwrap_or(raw);
            validate_relative_repo_path(path)?;
            if !paths.iter().any(|existing| existing == path) {
                paths.push(path.to_string());
            }
        }
    }
    if paths.is_empty() {
        return Err(MutationError::invalid(
            "patch.diff_header",
            "patch has no diff --git file headers",
            "pass a complete git-format unified diff with changed file paths",
        ));
    }
    Ok(paths)
}

pub(crate) fn parse_changed_paths(patch: &str) -> MutationResult<Vec<String>> {
    let mut paths = Vec::new();
    for line in patch.lines() {
        if !line.starts_with("diff --git ") {
            continue;
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 {
            return Err(MutationError::invalid(
                "patch.diff_header",
                format!("invalid diff header: {line}"),
                "pass a complete git-format unified diff",
            ));
        }
        let path = parts[3].strip_prefix("b/").unwrap_or(parts[3]);
        validate_relative_repo_path(path)?;
        if !paths.iter().any(|existing| existing == path) {
            paths.push(path.to_string());
        }
    }
    if paths.is_empty() {
        return Err(MutationError::invalid(
            "patch.diff_header",
            "patch has no diff --git file headers",
            "pass a complete git-format unified diff with changed file paths",
        ));
    }
    Ok(paths)
}

pub(crate) fn read_git_blob(
    repo_cache: &Path,
    base_commit: &str,
    path: &str,
) -> MutationResult<Option<Vec<u8>>> {
    validate_relative_repo_path(path)?;
    let object = format!("{base_commit}:{path}");
    let exists = run_git_timed(
        Some(repo_cache),
        &["cat-file", "-e", &object],
        GIT_QUERY_TIMEOUT,
        "git cat-file blob exists",
    )?;
    if !exists.status.success() {
        return Ok(None);
    }
    let output = run_git_timed(
        Some(repo_cache),
        &["show", &object],
        GIT_QUERY_TIMEOUT,
        "read git blob",
    )?;
    if !output.status.success() {
        return Err(git_error(
            "read git blob",
            format!(
                "exit_code={:?}; stdout_tail={}; stderr_tail={}",
                output.status.code(),
                tail(&output.stdout),
                tail(&output.stderr)
            ),
        ));
    }
    Ok(Some(output.stdout))
}

pub(crate) fn run_git(
    current_dir: &[&Path],
    args: &[&str],
    operation: &'static str,
) -> MutationResult<()> {
    let timeout = classify_git_timeout(args);
    let output = run_git_timed(current_dir.first().copied(), args, timeout, operation)?;
    if output.status.success() {
        return Ok(());
    }
    Err(git_error(
        operation,
        format!(
            "exit_code={:?}; stdout_tail={}; stderr_tail={}",
            output.status.code(),
            tail(&output.stdout),
            tail(&output.stderr)
        ),
    ))
}

pub(crate) fn run_git_capture(
    work_dir: &Path,
    args: &[&str],
    operation: &'static str,
) -> MutationResult<String> {
    let timeout = classify_git_timeout(args);
    let output = run_git_timed(Some(work_dir), args, timeout, operation)?;
    if !output.status.success() {
        return Err(git_error(
            operation,
            format!(
                "exit_code={:?}; stdout_tail={}; stderr_tail={}",
                output.status.code(),
                tail(&output.stdout),
                tail(&output.stderr)
            ),
        ));
    }
    String::from_utf8(output.stdout).map_err(|err| {
        MutationError::invalid(
            "git.stdout",
            format!("git {operation} emitted non-UTF-8 stdout: {err}"),
            "inspect the repository path encoding and git output",
        )
    })
}

/// REGRESSION FIX (2026-05-11): every git subprocess spawned from source-prep
/// flows through this helper. Three properties matter:
///
/// 1. **Per-operation timeout (no infinite hang).** Built on
///    `oracle::wait_with_timeout`, which polls `try_wait()` then escalates
///    SIGTERM → SIGKILL via `terminate_child_process_group`. A hung git
///    subprocess will be killed at the deadline; the parent never blocks
///    forever in `do_sys_poll`.
///
/// 2. **Process-group isolation.** `configure_child_process_group` puts the
///    child in its own group so SIGTERM/SIGKILL reach grandchildren that
///    git may have spawned (e.g., `git-remote-https`).
///
/// 3. **Structured timeout error.** On timeout we return
///    `MutationError::SubprocessTimeout` with stdout/stderr tails so the
///    operator sees `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT` plus the partial
///    output, instead of silent hang.
///
/// Call sites must NOT use `Command::new("git").output()` directly — use
/// this helper or `run_git` / `run_git_capture` instead.
/// Explicit-timeout entrypoint exposed for integration tests via
/// `crate::test_support`. Production code paths use `run_git` /
/// `run_git_capture` (which classify the timeout from the verb).
pub(crate) fn run_git_with_timeout_for_test(
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
    operation: &'static str,
) -> MutationResult<()> {
    let output = run_git_timed(Some(cwd), args, timeout, operation)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_error(
            operation,
            format!("exit_code={:?}", output.status.code()),
        ))
    }
}

/// Spawn `git $args` with a deadline. Drains stdout/stderr in background
/// threads so commands producing >64 KB of output (e.g., `git ls-tree -r`
/// on a 30k-file repo like astropy) don't deadlock on a full pipe.
///
/// This was the second sub-bug in the 2026-05-11 source-prep regression:
/// the initial timeout helper relied on `Command::output()`'s naïve
/// post-exit `wait_with_output()` collection. For large-output children,
/// the OS pipe buffer (typically 64 KB on Linux) fills, git blocks on the
/// next `write()`, the parent never sees the child exit, and the
/// "timeout" fires on a child that was actually progressing — just
/// blocked on a pipe the parent wasn't reading.
///
/// The pattern below is the canonical Rust subprocess hardening idiom:
///   1. Spawn with stdout/stderr piped.
///   2. Move the pipe handles into two background threads that read to
///      EOF, parking the bytes in heap-owned Vec<u8> buffers.
///   3. Poll `try_wait()` with the deadline.
///   4. On natural exit: join the drain threads to recover the bytes.
///   5. On timeout: send SIGTERM to the process group (catches grandchildren
///      like `git-remote-https`), escalate to SIGKILL after 2 s, then
///      collect whatever the drain threads captured before the kill.
///
/// The Source of Truth for "did the timeout work?" is the wallclock
/// elapsed at return: it must be `≤ timeout + 6 s` even when the child
/// hangs. The regression test in `tests/source_prep_timeout_test.rs`
/// covers this with a fake `git` that sleeps forever.
fn run_git_timed(
    cwd: Option<&Path>,
    args: &[&str],
    timeout: Duration,
    operation: &'static str,
) -> MutationResult<Output> {
    let mut cmd = Command::new("git");
    if let Some(dir) = cwd {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    configure_child_process_group(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|err| git_error(operation, format!("spawn failed: {err}")))?;

    // Hand the pipe handles to drain threads so the OS pipe buffer never
    // fills up while we're polling try_wait().
    let stdout_handle = child.stdout.take().map(spawn_drain);
    let stderr_handle = child.stderr.take().map(spawn_drain);

    let started = Instant::now();
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let (stdout, stderr) = terminate_child_and_collect(
                        &mut child,
                        stdout_handle,
                        stderr_handle,
                        operation,
                    )?;
                    return Err(MutationError::SubprocessTimeout {
                        program: "git".to_string(),
                        operation,
                        elapsed_secs: started.elapsed().as_secs(),
                        stdout_tail: tail(&stdout),
                        stderr_tail: tail(&stderr),
                        remediation: "increase the per-operation git timeout only after profiling, \
                                      or investigate why the git subprocess wedged (filesystem stall, \
                                      lockfile deadlock, malformed repo)",
                    });
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                return Err(git_error(
                    operation,
                    format!("try_wait failed: {err}"),
                ));
            }
        }
    };

    let stdout = stdout_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    Ok(Output {
        status: exit_status,
        stdout,
        stderr,
    })
}

fn spawn_drain<R>(mut reader: R) -> thread::JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = Vec::new();
        // Best-effort read; if the pipe errors we still return what was
        // captured. The caller uses tail() so partial reads are useful.
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

fn terminate_child_and_collect(
    child: &mut std::process::Child,
    stdout_handle: Option<thread::JoinHandle<Vec<u8>>>,
    stderr_handle: Option<thread::JoinHandle<Vec<u8>>>,
    operation: &'static str,
) -> MutationResult<(Vec<u8>, Vec<u8>)> {
    // Send SIGTERM to the process group (catches grandchildren).
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = Command::new("kill")
            .args(["-TERM", "--", &process_group])
            .status();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(err) => {
                    return Err(git_error(operation, format!("try_wait after SIGTERM: {err}")));
                }
            }
        }
        let _ = Command::new("kill")
            .args(["-KILL", "--", &process_group])
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
    let stdout = stdout_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    Ok((stdout, stderr))
}

/// Choose a timeout class based on the first git verb in `args`. Defaults to
/// the conservative GIT_QUERY_TIMEOUT.
///
/// Enumeration verbs (`ls-tree`, `ls-files`, `log`, `rev-list`, `diff-tree`)
/// walk the full tree/history and legitimately take minutes on large repos
/// like astropy / sympy / django on cold cache. They go to the longer
/// GIT_ENUMERATION_TIMEOUT bucket. Single-blob queries (`cat-file`, `show`,
/// `rev-parse`) stay on the tighter GIT_QUERY_TIMEOUT so a wedged blob
/// access surfaces quickly.
fn classify_git_timeout(args: &[&str]) -> Duration {
    match args.first().copied() {
        Some("clone") | Some("fetch") | Some("push") | Some("pull") => GIT_NETWORK_TIMEOUT,
        Some("apply") | Some("am") | Some("checkout") | Some("reset") | Some("commit")
        | Some("add") | Some("rm") | Some("merge") | Some("rebase") => GIT_WRITE_TIMEOUT,
        Some("ls-tree") | Some("ls-files") | Some("log") | Some("rev-list")
        | Some("diff-tree") => GIT_ENUMERATION_TIMEOUT,
        _ => GIT_QUERY_TIMEOUT,
    }
}


pub(crate) fn write_readback(path: &Path, text: &str) -> MutationResult<()> {
    write_readback_bytes(path, text.as_bytes())
}

pub(crate) fn write_readback_bytes(path: &Path, bytes: &[u8]) -> MutationResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            MutationError::op_failed(
                "source.mkdir",
                format!("failed creating file:{} parent: {err}", path.display()),
                "ensure the worktree path is writable",
            )
        })?;
    }
    fs::write(path, bytes).map_err(|err| {
        MutationError::op_failed(
            "source.write",
            format!("failed writing file:{}: {err}", path.display()),
            "ensure the worktree is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        MutationError::op_failed(
            "source.readback",
            format!("failed reading file:{} after write: {err}", path.display()),
            "ensure the worktree is readable",
        )
    })?;
    if readback != bytes {
        return Err(MutationError::op_failed(
            "source.readback",
            format!("readback mismatch after writing file:{}", path.display()),
            "inspect filesystem corruption or concurrent mutation",
        ));
    }
    Ok(())
}

pub(crate) fn validate_task(task: &SourcePatchTask) -> MutationResult<()> {
    validate_repo_slug(&task.repo)?;
    validate_token("instance_id", &task.instance_id)?;
    validate_token("base_commit", &task.base_commit)?;
    if task.base_commit.len() != 40 || !task.base_commit.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(MutationError::invalid(
            "base_commit",
            format!(
                "base_commit must be a 40-character git SHA: {}",
                task.base_commit
            ),
            "load base_commit from the official SWE-bench task manifest",
        ));
    }
    Ok(())
}

pub(crate) fn validate_patch_text(patch: &str) -> MutationResult<()> {
    if patch.trim().is_empty() || !patch.lines().any(|line| line.starts_with("diff --git ")) {
        return Err(MutationError::invalid(
            "official_patch",
            "official patch is empty or missing diff --git headers",
            "load the official SWE-bench patch before source-backed mutation",
        ));
    }
    Ok(())
}

fn git_commit_exists(repo: &Path, commit: &str) -> MutationResult<bool> {
    let object = format!("{commit}^{{commit}}");
    let output = run_git_timed(
        Some(repo),
        &["cat-file", "-e", &object],
        GIT_QUERY_TIMEOUT,
        "git cat-file commit exists",
    )?;
    Ok(output.status.success())
}

fn validate_repo_slug(repo: &str) -> MutationResult<()> {
    let mut parts = repo.split('/');
    let Some(owner) = parts.next() else {
        return Err(invalid_repo(repo));
    };
    let Some(name) = parts.next() else {
        return Err(invalid_repo(repo));
    };
    if parts.next().is_some()
        || owner.is_empty()
        || name.is_empty()
        || !owner.chars().all(is_repo_char)
        || !name.chars().all(is_repo_char)
    {
        return Err(invalid_repo(repo));
    }
    Ok(())
}

fn invalid_repo(repo: &str) -> MutationError {
    MutationError::invalid(
        "repo",
        format!("repo must be a GitHub owner/name slug with safe characters: {repo}"),
        "load repo from the official SWE-bench task manifest",
    )
}

fn validate_token(field: &'static str, value: &str) -> MutationResult<()> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(MutationError::invalid(
            field,
            format!("{field} is empty or contains a control character"),
            "load a stable single-line value from the SWE-bench manifest",
        ));
    }
    Ok(())
}

fn validate_relative_repo_path(path: &str) -> MutationResult<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path.split('/').any(|part| part == "..")
    {
        return Err(MutationError::invalid(
            "patch.path",
            format!("unsafe repository-relative path in patch: {path}"),
            "use paths emitted by git-format unified diffs",
        ));
    }
    Ok(())
}

fn is_repo_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

fn is_python_path(path: &str) -> bool {
    path.ends_with(".py")
}

fn candidate_python_priority(path: &str) -> u8 {
    let first = path.split('/').next().unwrap_or(path);
    if first.starts_with('.') {
        return 6;
    }
    if matches!(
        first,
        "bench"
            | "benchmark"
            | "benchmarks"
            | "bin"
            | "ci"
            | "dev"
            | "doc"
            | "docs"
            | "example"
            | "examples"
            | "maint"
            | "release"
            | "script"
            | "scripts"
            | "tool"
            | "tools"
    ) {
        return 5;
    }
    if first == "tests"
        || first == "test"
        || path.contains("/tests/")
        || path.contains("/test/")
        || path.ends_with("_test.py")
        || path.ends_with("_tests.py")
        || path.ends_with("test.py")
    {
        return 4;
    }
    if !path.contains('/') {
        return 3;
    }
    1
}

fn git_error(operation: &'static str, message: impl Into<String>) -> MutationError {
    MutationError::op_failed(
        "git",
        format!("{operation} failed: {}", message.into()),
        "inspect git, network, repository cache, and task manifest state",
    )
}

fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars().rev().take(2000).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|ch| if is_repo_char(ch) { ch } else { '_' })
        .collect()
}

pub(crate) struct WorkDirGuard {
    path: PathBuf,
}

impl WorkDirGuard {
    pub(crate) fn create(
        root: &Path,
        task: &SourcePatchTask,
        category: MutationCategory,
        seed: u64,
    ) -> MutationResult<Self> {
        let dir = root.join(format!(
            "{}-{}-{}-{seed:016x}",
            std::process::id(),
            sanitize_component(&task.instance_id),
            category.slug()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|err| {
                MutationError::op_failed(
                    "work_dir",
                    format!("failed to remove stale file:{}: {err}", dir.display()),
                    "inspect stale source-backed patch work directories",
                )
            })?;
        }
        Ok(Self { path: dir })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkDirGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::candidate_python_priority;

    #[test]
    fn production_python_paths_rank_before_ci_docs_and_tests() {
        let mut paths = [
            ".ci/parse_durations_log.py",
            "bin/authors_update.py",
            "tests/test_api.py",
            "docs/conf.py",
            "setup.py",
            "sympy/core/basic.py",
        ];
        paths.sort_by(|left, right| {
            candidate_python_priority(left)
                .cmp(&candidate_python_priority(right))
                .then_with(|| left.cmp(right))
        });
        assert_eq!(paths[0], "sympy/core/basic.py");
        assert_eq!(paths[1], "setup.py");
        assert_eq!(paths[2], "tests/test_api.py");
        assert_eq!(paths[3], "bin/authors_update.py");
        assert_eq!(paths[4], "docs/conf.py");
        assert_eq!(paths[5], ".ci/parse_durations_log.py");
    }
}
