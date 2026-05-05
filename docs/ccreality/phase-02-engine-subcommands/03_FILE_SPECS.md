# Phase 2 — File Specifications

## Subcommand structure (insert above the renamed `AttemptArgs`)

```rust
use clap::{Parser, Subcommand};

const DEFAULT_SMOKE_RUNTIME_ROOT: &str = "/tmp/cgreality-smoke";
const DEFAULT_REPLAY_OUTPUT_LABEL: &str = "replay";

#[derive(Parser, Debug, Clone)]
#[command(name = "reality-loop")]
#[command(version)]
#[command(about = "ContextGraph reality engine: runs Gemma SWE-bench attempts and synthetic smokes; writes durable artifacts.")]
#[command(long_about = "After Phase 1+ of the ccreality cutover, this binary is a pure reality engine. \
The outer optimizer lives in Claude Code, which talks to ContextGraph through the cgreality MCP server. \
See docs/ccreality/05_REALITY_ENGINE_RESPONSIBILITY.md.")]
struct TopArgs {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug, Clone)]
enum Cmd {
    /// Run one (or more) Gemma SWE-bench repair attempts. Writes attempt artifacts.
    Attempt(AttemptArgs),
    /// Run a synthetic smoke attempt. No Gemma, no Docker. For wiring verification.
    Smoke(SmokeArgs),
    /// Replay an existing attempt's reality-signal-packet builder against current code.
    Replay(ReplayArgs),
    /// Full-state-verify every artifact under <runtime>/<task>/attempt-N.
    Verify(VerifyArgs),
}

#[derive(Parser, Debug, Clone)]
struct SmokeArgs {
    #[arg(long, default_value = DEFAULT_SMOKE_RUNTIME_ROOT)]
    runtime_root: PathBuf,
    /// Label appended to the synthetic instance ID; lets you run multiple smokes side-by-side.
    #[arg(long, default_value = "smoke")]
    label: String,
    /// Force-fail the smoke (used by FSV to exercise the failure path).
    #[arg(long, default_value_t = false)]
    force_fail: bool,
}

#[derive(Parser, Debug, Clone)]
struct ReplayArgs {
    /// Path to an existing attempt-summary.json.
    #[arg(long)]
    attempt_summary_path: PathBuf,
    /// Where to write the replayed reality-signal-packet (defaults to alongside the summary).
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// Suffix on the output filename.
    #[arg(long, default_value = DEFAULT_REPLAY_OUTPUT_LABEL)]
    label: String,
}

#[derive(Parser, Debug, Clone)]
struct VerifyArgs {
    /// Runtime root containing attempt directories.
    #[arg(long)]
    runtime_root: PathBuf,
    /// Specific attempt to verify (default: latest).
    #[arg(long)]
    attempt: Option<u64>,
}
```

## Updated `main()` and dispatchers

```rust
#[tokio::main]
async fn main() {
    let top = TopArgs::parse();
    let exit_code = match top.cmd {
        Cmd::Attempt(args) => dispatch(run(args)).await,
        Cmd::Smoke(args)   => dispatch(run_smoke(args)).await,
        Cmd::Replay(args)  => dispatch(run_replay(args)).await,
        Cmd::Verify(args)  => dispatch(run_verify(args)).await,
    };
    std::process::exit(exit_code);
}

async fn dispatch<F>(future: F) -> i32
where
    F: std::future::Future<Output = Result<Value>>,
{
    match future.await {
        Ok(value) => {
            // Print structured success output to stdout for the caller (MCP server, operator).
            if let Ok(serialized) = serde_json::to_string_pretty(&value) {
                println!("{}", serialized);
            }
            0
        }
        Err(err) => {
            // Persist failure artifact (existing helper).
            let _ = write_failure_artifact_for_command(&err);
            // Emit error JSON on stderr.
            if let Ok(serialized) = serde_json::to_string_pretty(&err) {
                eprintln!("{}", serialized);
            }
            1
        }
    }
}
```

`write_failure_artifact_for_command` is a thin wrapper over the existing `write_failure_artifact` that uses a runtime root inferred from the error if possible; otherwise falls back to `~/.cache/cgreality/failures/`. Implementation:

```rust
fn write_failure_artifact_for_command(err: &RealityError) -> std::result::Result<PathBuf, String> {
    // For Phase 2, simplest: write to ~/.cache/cgreality/failures/
    let cache = dirs::home_dir()
        .ok_or_else(|| "HOME unset".to_string())?
        .join(".cache/cgreality/failures");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let path = cache.join(format!("failure-{}-{}.json", unix_secs_for_logging(), safe_id(&err.error_code)));
    let body = json!({
        "record_kind": "rust_reality_loop_failure",
        "created_at_unix": unix_secs_for_logging(),
        "error": err,
        "source_of_truth": file_sot(&path)
    });
    let text = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}
```

## `run_smoke` body

```rust
async fn run_smoke(args: SmokeArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    create_dir_checked(&runtime_root)?;
    let run_id = format!("smoke-{}", unix_secs_for_logging());
    let task_id = format!("cgreality-smoke-{}", safe_id(&args.label));
    let attempt_dir = runtime_root.join(&run_id).join(&task_id).join("smoke-attempt-1");
    create_dir_checked(&attempt_dir)?;

    update_active_runtime_root(&runtime_root, Some(&task_id))?;

    if args.force_fail {
        return Err(RealityError::new(
            "RUST_REALITY_SMOKE_FORCED_FAIL",
            "smoke forced to fail via --force-fail",
            "smoke.force_fail", "remove --force-fail to verify happy path",
            json!({}), None));
    }

    // Synthetic problem-reality
    let problem = json!({
        "record_kind": "ccreality_smoke_problem_reality",
        "schema_version": 1,
        "task_id": task_id,
        "label": args.label,
        "synthetic": true,
        "source_of_truth": file_sot(&attempt_dir)
    });
    let problem_path = attempt_dir.join("problem-reality-packet.json");
    write_json_checked(&problem_path, &problem)?;

    // Synthetic reality-signal
    let signal = json!({
        "record_kind": "ccreality_smoke_reality_signal",
        "schema_version": 1,
        "task_id": task_id,
        "score_reality": { "reward": { "clipped": 0.5, "raw": 0.5 } },
        "patch_signals": { "patch_sha256": null, "repeated_patch": false },
        "outcome_reality": { "source_of_truth": { "official_evidence": null } },
        "reinjection_directive": "smoke synthetic; no real reinjection needed",
        "source_of_truth": file_sot(&attempt_dir)
    });
    let signal_path = attempt_dir.join("reality-signal-packet.json");
    write_json_checked(&signal_path, &signal)?;

    // Synthetic attempt-summary
    let summary = json!({
        "record_kind": "ccreality_smoke_attempt_summary",
        "schema_version": 1,
        "attempt": 1,
        "task_id": task_id,
        "synthetic": true,
        "validation": { "status": "passed", "synthetic": true },
        "official_evidence": null,
        "reality_signal_packet_path": path_str(&signal_path),
        "problem_reality_packet_path": path_str(&problem_path),
        "source_of_truth": file_sot(&attempt_dir)
    });
    let summary_path = attempt_dir.join("attempt-summary.json");
    write_json_checked(&summary_path, &summary)?;

    Ok(json!({
        "status": "ok",
        "kind": "smoke",
        "runtime_root": path_str(&runtime_root),
        "run_id": run_id,
        "task_id": task_id,
        "attempt_summary_path": file_sot(&summary_path),
        "source_of_truth": {
            "problem_reality": file_sot(&problem_path),
            "reality_signal": file_sot(&signal_path),
            "attempt_summary": file_sot(&summary_path)
        }
    }))
}
```

## `run_replay` body (minimal, full implementation in Phase 4 if needed)

```rust
async fn run_replay(args: ReplayArgs) -> Result<Value> {
    let summary_path = resolve_path(&args.attempt_summary_path)?;
    require_file(&summary_path, "attempt_summary_path", "RUST_REALITY_REPLAY_SUMMARY_MISSING")?;
    let summary: Value = read_json(&summary_path)?;
    let out_dir = args.output_dir.as_deref().unwrap_or_else(|| summary_path.parent().expect("attempt-summary has parent"));
    let out_path = out_dir.join(format!("reality-signal-packet.{}.json", args.label));

    // Phase 2 stub: just round-trip the existing reality-signal-packet to demonstrate the path.
    let signal_path_str = summary.pointer("/reality_signal_packet_path")
        .and_then(Value::as_str)
        .ok_or_else(|| RealityError::new(
            "RUST_REALITY_REPLAY_NO_SIGNAL_REF", "attempt-summary missing reality_signal_packet_path",
            "attempt.reality_signal_packet_path", "use a properly-formed attempt-summary",
            json!({}), Some(file_sot(&summary_path))))?;
    let signal: Value = read_json(Path::new(signal_path_str))?;
    let replayed = json!({
        "record_kind": "ccreality_replay",
        "schema_version": 1,
        "source_attempt_summary": file_sot(&summary_path),
        "label": args.label,
        "replayed_at_unix": unix_secs_for_logging(),
        "signal": signal,
        "source_of_truth": file_sot(&out_path)
    });
    write_json_checked(&out_path, &replayed)?;
    Ok(json!({ "status": "ok", "kind": "replay", "output_path": file_sot(&out_path) }))
}
```

## `run_verify` body (minimal)

```rust
async fn run_verify(args: VerifyArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    if !runtime_root.is_dir() {
        return Err(RealityError::new(
            "RUST_REALITY_VERIFY_RUNTIME_ROOT_MISSING", "runtime root not a directory",
            "verify.runtime_root", "pass an existing runtime root", json!({}), None));
    }
    let mut json_files = Vec::new();
    collect_json_files(&runtime_root, &mut json_files)?;
    let mut entries = Vec::new();
    for path in &json_files {
        let sha = sha256_file(path)?;
        let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        entries.push(json!({
            "path": path_str(path),
            "sha256": sha,
            "size_bytes": size
        }));
    }
    Ok(json!({
        "status": "ok",
        "kind": "verify",
        "runtime_root": path_str(&runtime_root),
        "json_artifact_count": entries.len(),
        "artifacts": entries
    }))
}
```

## `update_active_runtime_root` helper

Insert near the other helpers (e.g., between `unix_secs` and `parse_u64`):

```rust
fn update_active_runtime_root(runtime_root: &Path, target_instance: Option<&str>) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| RealityError::new(
        "RUST_REALITY_HOME_DIR_MISSING", "could not resolve $HOME for cgreality cache",
        "process.env.HOME", "set HOME to a writable directory",
        json!({}), None))?;
    let cache = home.join(".cache/cgreality");
    fs::create_dir_all(&cache).map_err(|e| fs_error("RUST_REALITY_CGREALITY_CACHE_DIR_FAILED", &cache, e))?;
    let canonical = runtime_root.canonicalize()
        .map_err(|e| fs_error("RUST_REALITY_RUNTIME_ROOT_CANONICALIZE_FAILED", runtime_root, e))?;
    fs::write(cache.join("active_runtime_root"), canonical.to_string_lossy().as_bytes())
        .map_err(|e| fs_error("RUST_REALITY_ACTIVE_ROOT_WRITE_FAILED", &cache.join("active_runtime_root"), e))?;
    if let Some(ti) = target_instance {
        fs::write(cache.join("active_target_instance"), ti.as_bytes())
            .map_err(|e| fs_error("RUST_REALITY_ACTIVE_INSTANCE_WRITE_FAILED", &cache.join("active_target_instance"), e))?;
    }
    Ok(())
}
```

Call this at:
- Top of `run()` (the Attempt dispatcher), after task manifest is parsed (so we have the instance_id).
- Top of `run_smoke()`, after the synthetic task_id is computed.

## `Cargo.toml` change (if needed)

If `dirs` is not already a workspace dependency:

```toml
[dependencies]
dirs = "5"
```

(or whatever the current major version is on crates.io as of May 2026). Run `cargo build` to confirm.
