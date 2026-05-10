use std::path::{Path, PathBuf};

use crate::error::MejepaInferError;
use crate::gates::{sha256_bytes, valid_witness_segment};
use crate::types::{
    AstDiff, DiffHunk, Language, PatchBundle, TaskContext, TaskEnvironment, TaskId, TestId,
};

pub fn fixture_patch_context(
    repo_root: &Path,
    scenario: &str,
) -> Result<(PatchBundle, TaskContext), MejepaInferError> {
    std::fs::create_dir_all(repo_root)
        .map_err(|source| MejepaInferError::io("create_dir_all", repo_root, source))?;
    let rel_path = PathBuf::from(format!("{scenario}.py"));
    let path = repo_root.join(&rel_path);
    let bytes = format!("def answer():\n    return '{}'\n", scenario).into_bytes();
    std::fs::write(&path, &bytes).map_err(|source| MejepaInferError::io("write", &path, source))?;
    let post_sha = sha256_bytes(&bytes);
    let patch_sha = sha256_bytes(format!("patch:{scenario}").as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: rel_path,
                pre_sha: post_sha,
                post_sha,
                before: String::new(),
                after: String::from_utf8(bytes).expect("fixture is UTF-8"),
            }],
        },
        valid_witness_segment(),
        format!("ME-JEPA inference fixture {scenario}"),
        patch_sha,
    )?;
    let context = TaskContext {
        task_id: TaskId(format!("task-{scenario}")),
        session_id: [9; 16],
        language: Language::Python,
        problem_statement: format!("scenario: {scenario}"),
        tests: vec![TestId("test_fixture".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    Ok((patch, context))
}
