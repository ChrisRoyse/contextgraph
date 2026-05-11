use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::patch_mutation::PatchMutationOutcome;
use crate::source_patch_support::{
    candidate_python_paths, ensure_repo_cache, list_python_files_at_commit, parse_changed_paths,
    parse_patch_file_paths, read_git_blob, run_git, run_git_capture, validate_patch_text,
    validate_task, write_readback, write_readback_bytes, WorkDirGuard,
};
use crate::{
    apply_mutation, MutationCategory, MutationConfig, MutationError, MutationResult, SplitMix64,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePatchTask {
    pub instance_id: String,
    pub repo: String,
    pub base_commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePatchConfig {
    pub repo_cache_dir: PathBuf,
    pub work_root: PathBuf,
}

/// Build a SWE-bench `model_patch` by mutating real source blobs.
///
/// The previous diff-only path could mutate only lines added by the official
/// patch. Full SWE-bench Lite coverage needs a source-backed path because many
/// valid fixes do not add a boolean operator, numeric literal, assertion, or
/// swappable local name. This function reads the needed files from the bare
/// repository cache, applies the official patch when appropriate in a sparse
/// temporary Git repository, runs the parser-backed source operator on real
/// repository files, and captures the result with `git diff --binary`.
pub fn mutate_task_source_patch(
    task: &SourcePatchTask,
    category: MutationCategory,
    official_patch: &str,
    seed: u64,
    config: &SourcePatchConfig,
) -> MutationResult<PatchMutationOutcome> {
    validate_task(task)?;
    validate_patch_text(official_patch)?;
    if category == MutationCategory::KnownGood {
        return Ok(PatchMutationOutcome {
            category,
            mutated_patch: official_patch.to_string(),
            seed,
            note: "official SWE-bench patch used unchanged".to_string(),
        });
    }

    let cache = ensure_repo_cache(&task.repo, &task.base_commit, &config.repo_cache_dir)?;
    fs::create_dir_all(&config.work_root).map_err(|err| {
        MutationError::invalid(
            "work_root",
            format!(
                "failed to create file:{}: {err}",
                config.work_root.display()
            ),
            "choose a writable work root for source-backed patch generation",
        )
    })?;
    let changed_paths = parse_changed_paths(official_patch)?;
    let (mutated_patch, source_note) = if category == MutationCategory::WrongFile {
        mutate_wrong_file(task, seed, &cache, &config.work_root, &changed_paths)?
    } else {
        mutate_patched_source(
            task,
            category,
            official_patch,
            seed,
            &cache,
            &config.work_root,
            &changed_paths,
        )?
    };

    if mutated_patch.trim().is_empty() {
        return Err(MutationError::op_failed(
            "git.diff",
            "source-backed mutation produced an empty diff",
            "inspect the source mutation operator and changed-path selection",
        ));
    }
    Ok(PatchMutationOutcome {
        category,
        mutated_patch,
        seed,
        note: format!(
            "source-backed mutation category={} repo={} base_commit={} {}",
            category.slug(),
            task.repo,
            task.base_commit,
            source_note
        ),
    })
}

fn apply_official_patch(official_patch: &str, work_dir: &Path) -> MutationResult<()> {
    let patch_path = work_dir.join(".git/mejepa-official.patch");
    write_readback(&patch_path, official_patch)?;
    run_git(
        &[work_dir],
        &[
            "apply",
            "--whitespace=nowarn",
            patch_path.to_str().ok_or_else(|| {
                MutationError::invalid(
                    "official_patch_path",
                    "official patch path is not valid UTF-8",
                    "use a UTF-8 work root for source-backed patch generation",
                )
            })?,
        ],
        "apply official SWE-bench patch before mutation",
    )
}

fn mutate_patched_source(
    task: &SourcePatchTask,
    category: MutationCategory,
    official_patch: &str,
    seed: u64,
    repo_cache: &Path,
    work_root: &Path,
    changed_paths: &[String],
) -> MutationResult<(String, String)> {
    let candidates = candidate_python_paths(repo_cache, &task.base_commit, changed_paths)?;
    let patch_paths = parse_patch_file_paths(official_patch)?;
    let mut last_error = None;
    for rel in candidates {
        let work_dir = WorkDirGuard::create(work_root, task, category, seed)?;
        let mut required_paths = patch_paths.clone();
        if !required_paths.iter().any(|path| path == &rel) {
            required_paths.push(rel.clone());
        }
        init_sparse_base_repo(
            repo_cache,
            &task.base_commit,
            work_dir.path(),
            &required_paths,
        )?;
        apply_official_patch(official_patch, work_dir.path())?;
        let path = work_dir.path().join(&rel);
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                last_error = Some(format!("file:{}: {err}", path.display()));
                continue;
            }
        };
        let outcome = match apply_mutation(
            category,
            &source,
            MutationConfig {
                seed,
                alternate_source: None,
            },
        ) {
            Ok(outcome) => outcome,
            Err(err) => {
                last_error = Some(format!("file:{rel}: {} ({})", err, err.code()));
                continue;
            }
        };
        let site_note = outcome
            .mutation_site
            .as_ref()
            .map(|site| {
                format!(
                    "source_site_offset={} source_site_len={} source_site_note={}",
                    site.byte_offset, site.byte_length, site.note
                )
            })
            .unwrap_or_else(|| "source_site=none".to_string());
        write_readback(&path, &outcome.mutated_source)?;
        let mutated_patch = capture_sparse_diff(work_dir.path())?;
        return Ok((
            mutated_patch,
            format!(
                "source_path=file:{rel} candidate_scope=changed_paths_then_tracked_python patch_builder=git_blob_sparse {site_note}",
            ),
        ));
    }
    Err(MutationError::no_site(
        "source.python_files",
        format!(
            "no parseable Python source file in task {} could satisfy category {}; last_error={}",
            task.instance_id,
            category.slug(),
            last_error.unwrap_or_else(|| "none".to_string())
        ),
        "choose a task/category pair with a real source-level mutation site, or extend the operator for this language",
    ))
}

fn mutate_wrong_file(
    task: &SourcePatchTask,
    seed: u64,
    repo_cache: &Path,
    work_root: &Path,
    changed_paths: &[String],
) -> MutationResult<(String, String)> {
    let changed = changed_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut candidates = list_python_files_at_commit(repo_cache, &task.base_commit)?
        .into_iter()
        .filter(|path| !changed.contains(path))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "wrong_file.alternate_python_file",
            format!(
                "task {} has no alternate tracked Python file outside the official patch",
                task.instance_id
            ),
            "WrongFile requires a real repository file distinct from the official fix path",
        ));
    }
    candidates.sort();
    let mut rng = SplitMix64::new(seed);
    let start = (rng.next_u64() as usize) % candidates.len();
    let mut last_error = None;
    for offset in 0..candidates.len() {
        let rel = &candidates[(start + offset) % candidates.len()];
        let work_dir = WorkDirGuard::create(work_root, task, MutationCategory::WrongFile, seed)?;
        init_sparse_base_repo(
            repo_cache,
            &task.base_commit,
            work_dir.path(),
            std::slice::from_ref(rel),
        )?;
        let path = work_dir.path().join(rel);
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                last_error = Some(format!("file:{}: {err}", path.display()));
                continue;
            }
        };
        let outcome = match apply_mutation(
            MutationCategory::OverEngineer,
            &source,
            MutationConfig {
                seed,
                alternate_source: None,
            },
        ) {
            Ok(outcome) => outcome,
            Err(err) => {
                last_error = Some(format!("file:{rel}: {} ({})", err, err.code()));
                continue;
            }
        };
        let site_note = outcome
            .mutation_site
            .as_ref()
            .map(|site| {
                format!(
                    "source_site_offset={} source_site_len={} source_site_note={}",
                    site.byte_offset, site.byte_length, site.note
                )
            })
            .unwrap_or_else(|| "source_site=none".to_string());
        write_readback(&path, &outcome.mutated_source)?;
        let mutated_patch = capture_sparse_diff(work_dir.path())?;
        return Ok((
            mutated_patch,
            format!(
                "source_path=file:{rel} candidate_scope=alternate_tracked_python patch_builder=git_blob_sparse {site_note}",
            ),
        ));
    }
    Err(MutationError::no_site(
        "wrong_file.alternate_python_file",
        format!(
            "no alternate Python file in task {} could be mutated; last_error={}",
            task.instance_id,
            last_error.unwrap_or_else(|| "none".to_string())
        ),
        "WrongFile requires a parseable alternate Python source file",
    ))
}

fn init_sparse_base_repo(
    repo_cache: &Path,
    base_commit: &str,
    work_dir: &Path,
    paths: &[String],
) -> MutationResult<()> {
    fs::create_dir_all(work_dir).map_err(|err| {
        MutationError::op_failed(
            "work_dir",
            format!("failed to create file:{}: {err}", work_dir.display()),
            "choose a writable source-backed patch work root",
        )
    })?;
    run_git(
        &[],
        &["init", "--quiet", work_dir_str(work_dir)?],
        "initialize sparse source patch repository",
    )?;
    for rel in paths {
        if let Some(blob) = read_git_blob(repo_cache, base_commit, rel)? {
            write_readback_bytes(&work_dir.join(rel), &blob)?;
        }
    }
    run_git(
        &[work_dir],
        &["add", "-A"],
        "stage sparse base files for patch diff",
    )
}

fn capture_sparse_diff(work_dir: &Path) -> MutationResult<String> {
    run_git(
        &[work_dir],
        &["add", "--intent-to-add", "."],
        "mark sparse untracked files for patch diff",
    )?;
    run_git_capture(
        work_dir,
        &["diff", "--binary", "--no-ext-diff"],
        "capture mutated patch",
    )
}

fn work_dir_str(work_dir: &Path) -> MutationResult<&str> {
    work_dir.to_str().ok_or_else(|| {
        MutationError::invalid(
            "work_dir",
            "work directory path is not valid UTF-8",
            "use a UTF-8 work root for source-backed patch generation",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;

    fn run_git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git command must spawn");
        assert!(
            output.status.success(),
            "git {:?} failed stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git stdout must be UTF-8")
    }

    fn create_cached_repo() -> (TempDir, PathBuf, PathBuf, String, String) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        let cache_root = tmp.path().join("cache");
        let bare = cache_root.join("example__repo.git");
        fs::create_dir_all(repo.join("pkg")).expect("mkdir repo package");
        run_git(tmp.path(), &["init", "--quiet", repo.to_str().unwrap()]);
        run_git(&repo, &["config", "user.name", "ContextGraph Test"]);
        run_git(
            &repo,
            &["config", "user.email", "contextgraph@example.test"],
        );
        fs::write(
            repo.join("pkg/a.py"),
            "def check(value):\n    return value\n",
        )
        .expect("write a.py");
        fs::write(
            repo.join("pkg/b.py"),
            "def helper(value):\n    return value\n",
        )
        .expect("write b.py");
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "--quiet", "-m", "base"]);
        let base_commit = run_git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        fs::write(
            repo.join("pkg/a.py"),
            "def check(value):\n    if value == 1:\n        return value\n    return 0\n",
        )
        .expect("write official patch source");
        let official_patch = run_git(&repo, &["diff", "--binary", "--no-ext-diff"]);
        fs::create_dir_all(&cache_root).expect("mkdir cache");
        let repo_arg = repo.to_str().unwrap();
        let bare_arg = bare.to_str().unwrap();
        let output = Command::new("git")
            .args(["clone", "--quiet", "--bare", repo_arg, bare_arg])
            .output()
            .expect("git clone --bare must spawn");
        assert!(
            output.status.success(),
            "git clone --bare failed stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let work_root = tmp.path().join("work");
        (tmp, cache_root, work_root, base_commit, official_patch)
    }

    #[test]
    fn source_patch_builds_from_sparse_git_blobs() {
        let (_tmp, repo_cache_dir, work_root, base_commit, official_patch) = create_cached_repo();
        let outcome = mutate_task_source_patch(
            &SourcePatchTask {
                instance_id: "example__repo-1".to_string(),
                repo: "example/repo".to_string(),
                base_commit,
            },
            MutationCategory::SubtleFlip,
            &official_patch,
            17,
            &SourcePatchConfig {
                repo_cache_dir,
                work_root,
            },
        )
        .expect("source patch must build from sparse git blobs");
        assert!(outcome.note.contains("patch_builder=git_blob_sparse"));
        assert!(outcome
            .mutated_patch
            .contains("diff --git a/pkg/a.py b/pkg/a.py"));
        assert!(outcome.mutated_patch.contains("value != 1"));
        assert!(!outcome.mutated_patch.contains("pkg/b.py"));
        assert!(!outcome.mutated_patch.contains(".mejepa-official.patch"));
    }

    #[test]
    fn wrong_file_uses_alternate_blob_without_official_patch() {
        let (_tmp, repo_cache_dir, work_root, base_commit, official_patch) = create_cached_repo();
        let outcome = mutate_task_source_patch(
            &SourcePatchTask {
                instance_id: "example__repo-1".to_string(),
                repo: "example/repo".to_string(),
                base_commit,
            },
            MutationCategory::WrongFile,
            &official_patch,
            23,
            &SourcePatchConfig {
                repo_cache_dir,
                work_root,
            },
        )
        .expect("wrong-file patch must build from alternate sparse blob");
        assert!(outcome.note.contains("patch_builder=git_blob_sparse"));
        assert!(outcome
            .mutated_patch
            .contains("diff --git a/pkg/b.py b/pkg/b.py"));
        assert!(outcome.mutated_patch.contains("_unused_helper_"));
        assert!(!outcome.mutated_patch.contains("pkg/a.py"));
        assert!(!outcome.mutated_patch.contains(".mejepa-official.patch"));
    }
}
