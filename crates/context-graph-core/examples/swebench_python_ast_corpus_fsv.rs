use context_graph_core::memory::ast::{
    chunk_with_options, AstChunk, AstChunkOptions, EntityType, Language,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

type FsvResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const CORPUS_ROOT: &str = "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const REPO_CACHE: &str = "/mnt/d/contextgraph/cache/mejepa-corpus/repos";
const TASKS_DIR: &str = "tasks/swebench-lite";
const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv";

#[derive(Debug)]
struct TaskCase {
    task_id: String,
    repo: String,
    base_commit: String,
    patch_path: PathBuf,
    python_paths: Vec<String>,
    image: String,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from(FSV_ROOT);
    reset_secure_dir(&root)?;
    let sources_dir = root.join("sources");
    fs::create_dir_all(&sources_dir)?;

    let cases = load_cases()?;
    require("SWE-bench selected task count", cases.len() == 300)?;
    let before = json!({
        "corpus_root": CORPUS_ROOT,
        "repo_cache": REPO_CACHE,
        "tasks_dir": TASKS_DIR,
        "selected_task_count": cases.len(),
        "source_files_before": count_files(&sources_dir)?,
    });
    println!(
        "before swebench_corpus: {}",
        serde_json::to_string(&before)?
    );

    let concurrency = configured_concurrency();
    let rows = run_cases_parallel(&cases, &sources_dir, concurrency)?;

    let after = aggregate_state(&sources_dir, &rows)?;
    println!("after swebench_corpus: {}", serde_json::to_string(&after)?);
    let edge_cases = run_edge_cases(&cases[0])?;
    let evidence_path = root.join("swebench-python-ast-corpus-fsv.json");
    let evidence = json!({
        "source_of_truth": {
            "corpus_index": format!("{CORPUS_ROOT}/index.json"),
            "corpus_verify_evidence": format!("{CORPUS_ROOT}/verify-evidence.json"),
            "docker_desktop_data": "/mnt/d/contextgraph/docker-desktop-data",
            "repo_cache": REPO_CACHE,
            "host_source_readbacks": sources_dir.display().to_string(),
            "evidence_json": evidence_path.display().to_string(),
        },
        "before_state": before,
        "execution": {
            "docker_concurrency": concurrency,
            "host_parallelism": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        },
        "after_state": after,
        "edge_cases": edge_cases,
        "rows": rows,
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require("evidence readback", readback == evidence)?;
    require(
        "aggregate passes",
        readback["after_state"]["passes"] == Value::Bool(true),
    )?;
    require(
        "edge cases pass",
        readback["edge_cases"]
            .as_object()
            .unwrap()
            .values()
            .all(|edge| edge["passes"] == Value::Bool(true)),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fsv": "swebench_python_ast_corpus",
            "evidence_json": evidence_path,
            "after_state": readback["after_state"],
            "edge_cases": readback["edge_cases"],
        }))?
    );
    Ok(())
}

fn run_cases_parallel(
    cases: &[TaskCase],
    sources_dir: &Path,
    concurrency: usize,
) -> FsvResult<Vec<Value>> {
    let next = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, FsvResult<Value>)>> =
        Mutex::new(Vec::with_capacity(cases.len()));
    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                if index >= cases.len() {
                    break;
                }
                let result = run_task_case(&cases[index], sources_dir);
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                if done == 1 || done.is_multiple_of(25) || done == cases.len() {
                    println!(
                        "swebench python AST FSV progress: task {} / {}",
                        done,
                        cases.len()
                    );
                }
                results.lock().unwrap().push((index, result));
            });
        }
    });

    let mut indexed = results.into_inner().unwrap();
    indexed.sort_by_key(|(index, _)| *index);
    let mut rows = Vec::with_capacity(indexed.len());
    for (index, result) in indexed {
        rows.push(result.map_err(|err| format!("task index {index} failed: {err}"))?);
    }
    require("all cases produced rows", rows.len() == cases.len())?;
    Ok(rows)
}

fn configured_concurrency() -> usize {
    if let Ok(value) = std::env::var("CONTEXTGRAPH_AST_FSV_DOCKER_CONCURRENCY") {
        if let Ok(parsed) = value.parse::<usize>() {
            return parsed.clamp(1, 16);
        }
    }
    let host = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (host / 4).clamp(4, 8)
}

fn load_cases() -> FsvResult<Vec<TaskCase>> {
    let index_path = Path::new(CORPUS_ROOT).join("index.json");
    let index: Value = serde_json::from_str(&fs::read_to_string(&index_path)?)?;
    let entries = index["entries"]
        .as_array()
        .ok_or("corpus index entries must be an array")?;
    let mut known_good = BTreeMap::new();
    for entry in entries {
        if entry["category"] == Value::String("known_good".to_string()) {
            let task_id = entry["task_id"]
                .as_str()
                .ok_or("entry task_id must be a string")?
                .to_string();
            known_good.insert(task_id, entry.clone());
        }
    }
    let task_ids = index["selected_task_ids"]
        .as_array()
        .ok_or("selected_task_ids must be an array")?;
    let mut cases = Vec::with_capacity(task_ids.len());
    for task_id_value in task_ids {
        let task_id = task_id_value
            .as_str()
            .ok_or("selected task id must be a string")?;
        let manifest_path = Path::new(TASKS_DIR).join(format!("{task_id}.json"));
        let manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        let repo = manifest["repo"]
            .as_str()
            .ok_or("manifest repo must be a string")?
            .to_string();
        let base_commit = manifest["base_commit"]
            .as_str()
            .ok_or("manifest base_commit must be a string")?
            .to_string();
        let entry = known_good
            .get(task_id)
            .ok_or_else(|| format!("known_good entry missing for {task_id}"))?;
        let patch_rel = entry["patch_path"]
            .as_str()
            .ok_or("known_good patch_path must be a string")?;
        let patch_path = Path::new(CORPUS_ROOT).join(patch_rel);
        let python_paths = parse_python_paths_from_patch(&patch_path)?;
        require(
            &format!("{task_id} must touch exactly one Python source file"),
            python_paths.len() == 1,
        )?;
        cases.push(TaskCase {
            task_id: task_id.to_string(),
            repo: repo.clone(),
            base_commit,
            patch_path,
            python_paths,
            image: image_name(&repo, task_id)?,
        });
    }
    Ok(cases)
}

fn run_task_case(case: &TaskCase, sources_dir: &Path) -> FsvResult<Value> {
    let image = docker_image_state(&case.image)?;
    let source_path = &case.python_paths[0];
    let docker = docker_python_compile_and_hash(&case.image, source_path)?;
    let git_source = git_show_source(case, source_path)?;
    let git_sha = sha256(&git_source);
    require(
        &format!("{} docker/git source sha must match", case.task_id),
        docker["sha256"] == Value::String(git_sha.clone()),
    )?;

    let host_path = sources_dir
        .join(&case.task_id)
        .join(source_path.replace('/', "__"));
    write_checked(&host_path, &git_source)?;
    let host_readback = fs::read(&host_path)?;
    let chunks = chunk_with_options(
        &host_readback,
        Language::Python,
        &AstChunkOptions::for_path(source_path),
    )?;
    require(
        &format!("{} must produce chunks", case.task_id),
        !chunks.is_empty(),
    )?;
    let atomic = python_atomic_checks(&chunks);
    require(
        &format!(
            "{} class/module chunks must be non-overlapping",
            case.task_id
        ),
        atomic["passes"] == Value::Bool(true),
    )?;
    let chunk_counts = chunk_counts(&chunks);
    Ok(json!({
        "task_id": case.task_id,
        "repo": case.repo,
        "base_commit": case.base_commit,
        "patch_path": case.patch_path.display().to_string(),
        "python_path": source_path,
        "docker_image": image,
        "docker_source": docker,
        "host_source": {
            "path": host_path.display().to_string(),
            "bytes": host_readback.len(),
            "sha256": sha256(&host_readback),
        },
        "chunk_state": {
            "chunk_count": chunks.len(),
            "counts": chunk_counts,
            "atomic": atomic,
            "sample_chunks": sample_chunks(&chunks),
        },
        "passes": true,
    }))
}

fn run_edge_cases(first: &TaskCase) -> FsvResult<Value> {
    let missing_image_before = json!({
        "image": "swebench/contextgraph-missing-image:latest",
        "expected_code": "DOCKER_IMAGE_MISSING",
    });
    println!(
        "before missing_image: {}",
        serde_json::to_string(&missing_image_before)?
    );
    let missing_image = Command::new("docker")
        .args([
            "image",
            "inspect",
            "swebench/contextgraph-missing-image:latest",
        ])
        .output()?;
    let missing_image_after = json!({
        "accepted": false,
        "actual_code": if missing_image.status.success() { "UNEXPECTED_SUCCESS" } else { "DOCKER_IMAGE_MISSING" },
        "exit_code": missing_image.status.code().unwrap_or(-1),
        "stderr": String::from_utf8_lossy(&missing_image.stderr),
    });
    println!(
        "after missing_image: {}",
        serde_json::to_string(&missing_image_after)?
    );

    let missing_source_before = json!({
        "image": first.image,
        "path": "/testbed/contextgraph_missing_source.py",
        "expected_code": "DOCKER_SOURCE_MISSING",
    });
    println!(
        "before missing_source: {}",
        serde_json::to_string(&missing_source_before)?
    );
    let missing_source = docker_python_probe_raw(&first.image, "contextgraph_missing_source.py")?;
    let missing_source_after = json!({
        "accepted": false,
        "actual_code": if missing_source.status.success() { "UNEXPECTED_SUCCESS" } else { "DOCKER_SOURCE_MISSING" },
        "exit_code": missing_source.status.code().unwrap_or(-1),
        "stderr": String::from_utf8_lossy(&missing_source.stderr),
    });
    println!(
        "after missing_source: {}",
        serde_json::to_string(&missing_source_after)?
    );

    let invalid_before = json!({
        "source": "def broken(:\\n    return 1\\n",
        "expected_code": "MEJEPA_CHUNKER_PARSE_FAILED",
    });
    println!(
        "before invalid_python: {}",
        serde_json::to_string(&invalid_before)?
    );
    let invalid_err = chunk_with_options(
        b"def broken(:\n    return 1\n",
        Language::Python,
        &AstChunkOptions::for_path("broken.py"),
    )
    .unwrap_err();
    let invalid_after = json!({
        "accepted": false,
        "actual_code": invalid_err.code(),
        "error": invalid_err.to_string(),
    });
    println!(
        "after invalid_python: {}",
        serde_json::to_string(&invalid_after)?
    );

    Ok(json!({
        "missing_image": {
            "before_state": missing_image_before,
            "after_state": missing_image_after,
            "passes": missing_image_after["actual_code"] == Value::String("DOCKER_IMAGE_MISSING".to_string()),
        },
        "missing_source_in_image": {
            "before_state": missing_source_before,
            "after_state": missing_source_after,
            "passes": missing_source_after["actual_code"] == Value::String("DOCKER_SOURCE_MISSING".to_string()),
        },
        "invalid_python_source": {
            "before_state": invalid_before,
            "after_state": invalid_after,
            "passes": invalid_after["actual_code"] == Value::String("MEJEPA_CHUNKER_PARSE_FAILED".to_string()),
        },
    }))
}

fn aggregate_state(sources_dir: &Path, rows: &[Value]) -> FsvResult<Value> {
    let mut total_chunks = 0_u64;
    let mut total_classes = 0_u64;
    let mut total_methods = 0_u64;
    let mut total_functions = 0_u64;
    let mut total_modules = 0_u64;
    let mut repos = BTreeSet::new();
    for row in rows {
        repos.insert(row["repo"].as_str().unwrap_or("").to_string());
        total_chunks += row["chunk_state"]["chunk_count"].as_u64().unwrap_or(0);
        total_classes += row["chunk_state"]["counts"]["class"].as_u64().unwrap_or(0);
        total_methods += row["chunk_state"]["counts"]["method"].as_u64().unwrap_or(0);
        total_functions += row["chunk_state"]["counts"]["function"]
            .as_u64()
            .unwrap_or(0);
        total_modules += row["chunk_state"]["counts"]["module"].as_u64().unwrap_or(0);
    }
    let passes = rows.len() == 300
        && rows.iter().all(|row| row["passes"] == Value::Bool(true))
        && total_chunks >= 300;
    Ok(json!({
        "passes": passes,
        "task_count": rows.len(),
        "repo_count": repos.len(),
        "repos": repos,
        "source_files_after": count_files(sources_dir)?,
        "docker_image_count": rows.len(),
        "docker_py_compile_pass_count": rows.iter().filter(|row| row["docker_source"]["py_compile_success"] == Value::Bool(true)).count(),
        "docker_git_sha_match_count": rows.iter().filter(|row| row["docker_source"]["sha256"] == row["host_source"]["sha256"]).count(),
        "total_chunks": total_chunks,
        "total_classes": total_classes,
        "total_methods": total_methods,
        "total_functions": total_functions,
        "total_modules": total_modules,
    }))
}

fn parse_python_paths_from_patch(path: &Path) -> FsvResult<Vec<String>> {
    let text = fs::read_to_string(path)?;
    let mut paths = BTreeSet::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            if rest.ends_with(".py") {
                paths.insert(rest.to_string());
            }
        }
    }
    Ok(paths.into_iter().collect())
}

fn image_name(repo: &str, task_id: &str) -> FsvResult<String> {
    let owner = repo
        .split('/')
        .next()
        .ok_or_else(|| format!("invalid repo {repo}"))?;
    let (_, repo_issue) = task_id
        .split_once("__")
        .ok_or_else(|| format!("invalid task id {task_id}"))?;
    Ok(format!(
        "swebench/sweb.eval.x86_64.{owner}_1776_{repo_issue}:latest"
    ))
}

fn docker_image_state(image: &str) -> FsvResult<Value> {
    let output = Command::new("docker")
        .args([
            "image",
            "inspect",
            "--format",
            "{{.Id}}\t{{.Size}}\t{{.Created}}",
            image,
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Docker image inspect failed for {image}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut parts = stdout.trim().split('\t');
    Ok(json!({
        "name": image,
        "id": parts.next().unwrap_or(""),
        "size_bytes": parts.next().unwrap_or("0").parse::<u64>().unwrap_or(0),
        "created": parts.next().unwrap_or(""),
    }))
}

fn docker_python_compile_and_hash(image: &str, source_path: &str) -> FsvResult<Value> {
    let output = docker_python_probe_raw(image, source_path)?;
    if !output.status.success() {
        return Err(format!(
            "Docker py_compile/hash failed for image={image} path={source_path}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let parsed: Value = serde_json::from_slice(&output.stdout)?;
    Ok(parsed)
}

fn docker_python_probe_raw(image: &str, source_path: &str) -> FsvResult<std::process::Output> {
    let testbed_path = format!("/testbed/{source_path}");
    let script = r#"import hashlib,json,pathlib,py_compile,sys
p = pathlib.Path(sys.argv[1])
b = p.read_bytes()
pyc = py_compile.compile(str(p), doraise=True)
pyc_b = pathlib.Path(pyc).read_bytes()
print(json.dumps({"path": str(p), "bytes": len(b), "sha256": "sha256:" + hashlib.sha256(b).hexdigest(), "py_compile_success": True, "pyc_path": pyc, "pyc_bytes": len(pyc_b), "pyc_sha256": "sha256:" + hashlib.sha256(pyc_b).hexdigest()}, sort_keys=True))
"#;
    Ok(Command::new("docker")
        .args([
            "run",
            "--rm",
            "--entrypoint",
            "python",
            image,
            "-c",
            script,
            &testbed_path,
        ])
        .output()?)
}

fn git_show_source(case: &TaskCase, source_path: &str) -> FsvResult<Vec<u8>> {
    let git_dir = Path::new(REPO_CACHE).join(format!("{}.git", case.repo.replace('/', "__")));
    let object = format!("{}:{source_path}", case.base_commit);
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(git_dir)
        .arg("show")
        .arg(object)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git show failed for {} {}: {}",
            case.task_id,
            source_path,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}

fn python_atomic_checks(chunks: &[AstChunk]) -> Value {
    let mut class_overlap = Vec::new();
    let mut module_overlap = Vec::new();
    let mut shell_parse_errors = Vec::new();
    let mut method_parent_missing = Vec::new();
    let mut routing_missing = Vec::new();
    for chunk in chunks {
        let source = chunk_source(chunk);
        match chunk.entity_type {
            EntityType::Class => {
                match python_shell_has_nested_definitions(&source, EntityType::Class, &chunk.sha256)
                {
                    Ok(true) => {
                        class_overlap.push(chunk.sha256.clone());
                    }
                    Ok(false) => {}
                    Err(err) => shell_parse_errors.push(err),
                }
            }
            EntityType::Module => match python_shell_has_nested_definitions(
                &source,
                EntityType::Module,
                &chunk.sha256,
            ) {
                Ok(true) => {
                    module_overlap.push(chunk.sha256.clone());
                }
                Ok(false) => {}
                Err(err) => shell_parse_errors.push(err),
            },
            EntityType::Method if chunk.parent_chain.is_empty() => {
                method_parent_missing.push(chunk.sha256.clone());
            }
            _ => {}
        }
        if chunk.embedder_routing_set.is_empty() {
            routing_missing.push(chunk.sha256.clone());
        }
    }
    let passes = class_overlap.is_empty()
        && module_overlap.is_empty()
        && shell_parse_errors.is_empty()
        && method_parent_missing.is_empty()
        && routing_missing.is_empty();
    json!({
        "passes": passes,
        "class_overlap": class_overlap,
        "module_overlap": module_overlap,
        "shell_parse_errors": shell_parse_errors,
        "method_parent_missing": method_parent_missing,
        "routing_missing": routing_missing,
    })
}

fn python_shell_has_nested_definitions(
    source: &str,
    shell_type: EntityType,
    sha256: &str,
) -> Result<bool, String> {
    let source = dedent_python_shell_source(source);
    let chunks = chunk_with_options(
        source.as_bytes(),
        Language::Python,
        &AstChunkOptions::for_path("<quality-shell>.py"),
    )
    .map_err(|err| format!("{sha256}: shell parse failed: {}", err.code()))?;
    let nested = match shell_type {
        EntityType::Class => {
            let class_count = chunks
                .iter()
                .filter(|chunk| chunk.entity_type == EntityType::Class)
                .count();
            class_count > 1
                || chunks.iter().any(|chunk| {
                    matches!(
                        chunk.entity_type,
                        EntityType::Function | EntityType::Method | EntityType::TestFunction
                    )
                })
        }
        EntityType::Module => chunks.iter().any(|chunk| {
            matches!(
                chunk.entity_type,
                EntityType::Class
                    | EntityType::Function
                    | EntityType::Method
                    | EntityType::TestFunction
            )
        }),
        _ => false,
    };
    Ok(nested)
}

fn dedent_python_shell_source(source: &str) -> String {
    let indent = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.as_bytes()
                .iter()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count()
        })
        .min()
        .unwrap_or(0);
    if indent == 0 {
        return source.to_string();
    }
    source
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                &line[indent.min(line.len())..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn chunk_counts(chunks: &[AstChunk]) -> Value {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for chunk in chunks {
        *counts
            .entry(format!("{:?}", chunk.entity_type).to_ascii_lowercase())
            .or_default() += 1;
    }
    let mut out = Map::new();
    for key in [
        "module",
        "import",
        "class",
        "method",
        "function",
        "testfunction",
        "docstring",
        "commentblock",
    ] {
        out.insert(key.to_string(), Value::from(*counts.get(key).unwrap_or(&0)));
    }
    Value::Object(out)
}

fn sample_chunks(chunks: &[AstChunk]) -> Vec<Value> {
    chunks
        .iter()
        .take(5)
        .map(|chunk| {
            json!({
                "entity_type": chunk.entity_type,
                "parent_chain": chunk.parent_chain,
                "line_start": chunk.line_start,
                "line_end": chunk.line_end,
                "sha256": chunk.sha256,
                "source_first_160": chunk_source(chunk).chars().take(160).collect::<String>(),
            })
        })
        .collect()
}

fn chunk_source(chunk: &AstChunk) -> String {
    chunk
        .content
        .split_once("--- source ---\n")
        .map(|(_, source)| source.to_string())
        .unwrap_or_default()
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn count_files(path: &Path) -> FsvResult<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            count += 1;
        } else if file_type.is_dir() {
            count += count_files(&entry.path())?;
        }
    }
    Ok(count)
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let readback = fs::read(path)?;
    require(
        &format!("host source readback mismatch {}", path.display()),
        readback == bytes,
    )?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    let text = serde_json::to_string_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    require(
        &format!("secure JSON readback mismatch {}", path.display()),
        readback == *value,
    )?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("SWE-bench Python AST corpus FSV invariant failed: {label}").into())
    }
}
