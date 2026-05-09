use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::patch_mutation::PatchMutationOutcome;
use crate::source_patch_support::{
    candidate_python_paths, ensure_repo_cache, list_python_files, parse_changed_paths, run_git,
    run_git_capture, validate_patch_text, validate_task, write_readback, WorkDirGuard,
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

/// Build a SWE-bench `model_patch` by mutating real checked-out source.
///
/// The previous diff-only path could mutate only lines added by the official
/// patch. Full SWE-bench Lite coverage needs a source-backed path because many
/// valid fixes do not add a boolean operator, numeric literal, assertion, or
/// swappable local name. This function checks out the base commit, applies the
/// official patch when appropriate, runs the parser-backed source operator on
/// real repository files, and captures the result with `git diff --binary`.
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
    let work_dir = WorkDirGuard::create(&config.work_root, task, category, seed)?;
    run_git(
        &[],
        &[
            "clone",
            "--quiet",
            "--shared",
            cache.to_str().ok_or_else(|| {
                MutationError::invalid(
                    "repo_cache_dir",
                    "repository cache path is not valid UTF-8",
                    "use a UTF-8 filesystem path for the repository cache",
                )
            })?,
            work_dir.path_str()?,
        ],
        "clone cached repository",
    )?;
    run_git(
        &[work_dir.path()],
        &["checkout", "--quiet", "--detach", &task.base_commit],
        "checkout task base commit",
    )?;

    let changed_paths = parse_changed_paths(official_patch)?;
    if category == MutationCategory::WrongFile {
        mutate_wrong_file(task, seed, work_dir.path(), &changed_paths)?;
    } else {
        apply_official_patch(official_patch, work_dir.path())?;
        mutate_patched_source(task, category, seed, work_dir.path(), &changed_paths)?;
    }

    let mutated_patch = run_git_capture(
        work_dir.path(),
        &["diff", "--binary", "--no-ext-diff"],
        "capture mutated patch",
    )?;
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
            "source-backed mutation category={} repo={} base_commit={}",
            category.slug(),
            task.repo,
            task.base_commit
        ),
    })
}

fn apply_official_patch(official_patch: &str, work_dir: &Path) -> MutationResult<()> {
    let patch_path = work_dir.join(".mejepa-official.patch");
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
    seed: u64,
    work_dir: &Path,
    changed_paths: &[String],
) -> MutationResult<()> {
    let candidates = candidate_python_paths(work_dir, changed_paths)?;
    let mut last_error = None;
    for rel in candidates {
        let path = work_dir.join(&rel);
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
        write_readback(&path, &outcome.mutated_source)?;
        return Ok(());
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
    work_dir: &Path,
    changed_paths: &[String],
) -> MutationResult<()> {
    let changed = changed_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut candidates = list_python_files(work_dir)?
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
        let path = work_dir.join(rel);
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
        write_readback(&path, &outcome.mutated_source)?;
        return Ok(());
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
