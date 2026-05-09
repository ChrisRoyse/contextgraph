use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::source_patch::SourcePatchTask;
use crate::{MutationCategory, MutationError, MutationResult};

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
    work_dir: &Path,
    changed_paths: &[String],
) -> MutationResult<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for path in changed_paths.iter().filter(|path| is_python_path(path)) {
        if seen.insert(path.clone()) {
            out.push(path.clone());
        }
    }
    for path in list_python_files(work_dir)? {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    Ok(out)
}

pub(crate) fn list_python_files(work_dir: &Path) -> MutationResult<Vec<String>> {
    let output = run_git_capture(work_dir, &["ls-files", "--", "*.py"], "list Python files")?;
    Ok(output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToString::to_string)
        .collect())
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

pub(crate) fn run_git(
    current_dir: &[&Path],
    args: &[&str],
    operation: &'static str,
) -> MutationResult<()> {
    let mut cmd = Command::new("git");
    if let Some(dir) = current_dir.first() {
        cmd.arg("-C").arg(dir);
    }
    let output = cmd
        .args(args)
        .output()
        .map_err(|err| git_error(operation, err.to_string()))?;
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
    let output = Command::new("git")
        .arg("-C")
        .arg(work_dir)
        .args(args)
        .output()
        .map_err(|err| git_error(operation, err.to_string()))?;
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

pub(crate) fn write_readback(path: &Path, text: &str) -> MutationResult<()> {
    fs::write(path, text).map_err(|err| {
        MutationError::op_failed(
            "source.write",
            format!("failed writing file:{}: {err}", path.display()),
            "ensure the worktree is writable",
        )
    })?;
    let readback = fs::read_to_string(path).map_err(|err| {
        MutationError::op_failed(
            "source.readback",
            format!("failed reading file:{} after write: {err}", path.display()),
            "ensure the worktree is readable",
        )
    })?;
    if readback != text {
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
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{commit}^{{commit}}"))
        .status()
        .map_err(|err| git_error("git cat-file spawn", err.to_string()))?;
    Ok(status.success())
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

    pub(crate) fn path_str(&self) -> MutationResult<&str> {
        self.path.to_str().ok_or_else(|| {
            MutationError::invalid(
                "work_dir",
                "work directory path is not valid UTF-8",
                "use a UTF-8 work root for source-backed patch generation",
            )
        })
    }
}

impl Drop for WorkDirGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
