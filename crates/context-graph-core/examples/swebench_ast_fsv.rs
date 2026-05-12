use context_graph_core::memory::ast::chunk_path;
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let corpus = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-phase0-smoke-one-task");
    let manifest_path = PathBuf::from("tasks/swebench-lite/sympy__sympy-20590.json");
    let patch_path = corpus.join("mutations/sympy__sympy-20590/known_good.patch");
    let repo_cache =
        PathBuf::from("/home/cabdru/.cache/contextgraph-mejepa-corpus/repos/sympy__sympy.git");
    require("phase0 smoke patch exists", patch_path.is_file())?;
    require("repo cache exists", repo_cache.is_dir())?;

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    let base_commit = manifest["base_commit"]
        .as_str()
        .ok_or("manifest missing base_commit")?;
    let patch = fs::read_to_string(&patch_path)?;
    let target_rel = first_python_changed_path(&patch)?;

    let root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-swebench-ast-fsv");
    let work = root.join("work");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    run(
        Command::new("git")
            .arg("clone")
            .arg("--quiet")
            .arg("--shared")
            .arg(&repo_cache)
            .arg(&work),
        "clone sympy repo cache",
    )?;
    run(
        Command::new("git")
            .arg("-C")
            .arg(&work)
            .arg("checkout")
            .arg("--quiet")
            .arg("--detach")
            .arg(base_commit),
        "checkout base commit",
    )?;
    let local_patch = root.join("known_good.patch");
    write_checked(&local_patch, patch.as_bytes())?;
    run(
        Command::new("git")
            .arg("-C")
            .arg(&work)
            .arg("apply")
            .arg("--whitespace=nowarn")
            .arg(&local_patch),
        "apply known_good patch",
    )?;

    let source_path = work.join(&target_rel);
    let source = fs::read(&source_path)?;
    run(
        Command::new("python3")
            .arg("-m")
            .arg("py_compile")
            .arg(&source_path),
        "py_compile patched source",
    )?;
    let chunks = chunk_path(&source, &target_rel)?;
    require("swebench source chunks non-empty", !chunks.is_empty())?;
    require(
        "swebench changed file has class chunk",
        chunks
            .iter()
            .any(|chunk| format!("{:?}", chunk.entity_type) == "Class"),
    )?;
    require(
        "swebench changed file has method chunk",
        chunks
            .iter()
            .any(|chunk| format!("{:?}", chunk.entity_type) == "Method"),
    )?;

    let evidence_path = root.join("swebench-ast-evidence.json");
    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_path.display().to_string(),
            "manifest": manifest_path.display().to_string(),
            "phase0_patch": patch_path.display().to_string(),
            "repo_cache": repo_cache.display().to_string(),
            "worktree_source": source_path.display().to_string(),
        },
        "before_state": {
            "instance_id": manifest["instance_id"],
            "repo": manifest["repo"],
            "base_commit": base_commit,
            "patch_bytes": patch.len(),
            "target_rel": target_rel,
            "source_bytes": source.len(),
        },
        "after_state": {
            "chunk_count": chunks.len(),
            "chunks": chunks.iter().map(|chunk| json!({
                "entity_type": chunk.entity_type,
                "line_start": chunk.line_start,
                "line_end": chunk.line_end,
                "sha256": chunk.sha256,
                "routing": chunk.embedder_routing_set,
            })).collect::<Vec<_>>(),
        },
        "passes": true,
    });
    write_checked(
        &evidence_path,
        serde_json::to_string_pretty(&evidence)?.as_bytes(),
    )?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "evidence readback passes",
        readback["passes"] == Value::Bool(true),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn first_python_changed_path(patch: &str) -> FsvResult<String> {
    for line in patch.lines() {
        if !line.starts_with("diff --git ") {
            continue;
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        let Some(path) = parts.get(3).and_then(|part| part.strip_prefix("b/")) else {
            continue;
        };
        if path.ends_with(".py") {
            return Ok(path.to_string());
        }
    }
    Err("patch does not contain a Python changed path".into())
}

fn run(command: &mut Command, label: &str) -> FsvResult<()> {
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{label} failed: status={:?} stdout={} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("readback bytes", fs::read(path)? == bytes)?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("SWE-bench AST FSV invariant failed: {label}").into())
    }
}
