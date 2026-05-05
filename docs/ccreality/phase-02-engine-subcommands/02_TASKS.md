# Phase 2 — Tasks

Eight ordered tasks. Estimated 30–45 min total.

## Task 2.1 — Confirm Phase 1 state

```bash
cd /home/cabdru/contextgraph
[ "$(git branch --show-current)" = "ccreality-cutover" ] || { echo "wrong branch"; exit 1; }
! rg -nq "deepseek|openai|gpt-5|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs || { echo "Phase 1 incomplete"; exit 1; }
cargo build -p context-graph-cli --bin reality-loop --quiet
echo "Phase 1 state OK"
```

If any check fails, do not proceed.

## Task 2.2 — Rename existing `Args` to `AttemptArgs`

In `reality-loop.rs`:
- Rename the current `struct Args` to `struct AttemptArgs`.
- Update every `&Args` reference in function signatures to `&AttemptArgs` — these are throughout the file (run, build_failure_artifact, build_problem_reality_packet, run_openai_loop_optimizer (deleted), run_deepseek_optimizer_api_smoke_test (deleted), etc.).
- Update `Args::parse()` callsite in `main` (Task 2.4 will replace it).

```bash
cd /home/cabdru/contextgraph/crates/context-graph-cli/src/bin
# Sanity: count current references
rg -c "&Args|&mut Args| Args |Args::|args: Args" reality-loop.rs
```

After rename:
```bash
rg -c "&AttemptArgs|&mut AttemptArgs| AttemptArgs |AttemptArgs::|args: AttemptArgs" reality-loop.rs
```

The two counts should match.

Also update `test_args` in tests:
```rust
fn test_args(task_manifest: PathBuf, runtime_root: PathBuf) -> AttemptArgs {
    AttemptArgs { ... }
}
```

Build:
```bash
cargo build -p context-graph-cli --bin reality-loop --quiet
```
Expected: success.

## Task 2.3 — Add the four-subcommand `TopArgs` enum

Above `AttemptArgs`, insert (see [03_FILE_SPECS.md](03_FILE_SPECS.md) for full content):

```rust
#[derive(Parser, Debug, Clone)]
#[command(name = "reality-loop")]
#[command(about = "ContextGraph reality engine for SWE-bench attempts and synthetic smokes")]
struct TopArgs {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug, Clone)]
enum Cmd {
    /// Run one Gemma SWE-bench repair attempt
    Attempt(AttemptArgs),
    /// Run a synthetic smoke attempt (no Gemma, no Docker)
    Smoke(SmokeArgs),
    /// Re-emit a reality-signal-packet from an existing attempt-summary
    Replay(ReplayArgs),
    /// Full-state-verify every artifact under <runtime>/<task>/attempt-N
    Verify(VerifyArgs),
}
```

Add `use clap::Subcommand;` at the top if not already present.

Define the three new arg structs (`SmokeArgs`, `ReplayArgs`, `VerifyArgs`) per [03_FILE_SPECS.md](03_FILE_SPECS.md).

Build:
```bash
cargo build -p context-graph-cli --bin reality-loop --quiet
```

## Task 2.4 — Update `main()` to dispatch on subcommand

Replace:
```rust
async fn main() {
    let args = Args::parse();
    let exit_code = match run(args.clone()).await { ... };
    ...
}
```

With:
```rust
async fn main() {
    let top = TopArgs::parse();
    let exit_code = match top.cmd {
        Cmd::Attempt(args)  => dispatch_attempt(args).await,
        Cmd::Smoke(args)    => dispatch_smoke(args).await,
        Cmd::Replay(args)   => dispatch_replay(args).await,
        Cmd::Verify(args)   => dispatch_verify(args).await,
    };
    std::process::exit(exit_code);
}
```

Define `dispatch_attempt`, `dispatch_smoke`, `dispatch_replay`, `dispatch_verify` as small wrappers. Each wrapper must compute the command's failure-artifact runtime root from explicit command arguments before invoking the command logic. If a runtime root cannot be identified, return a structured error and exit 1; do not write failure artifacts to any alternate location. `dispatch_attempt` calls the existing `run()` (post-Phase-1) with the AttemptArgs. The other three call the new file-backed functions implemented in Task 2.5.

Build:
```bash
cargo build -p context-graph-cli --bin reality-loop --quiet
```

## Task 2.5 — Implement `run_smoke`, `run_replay`, `run_verify`

Add three new top-level async functions (full bodies in [03_FILE_SPECS.md](03_FILE_SPECS.md)):

- `run_smoke(args: SmokeArgs) -> Result<Value>` — writes a deterministic synthetic `attempt-summary.json`, `reality-signal-packet.json`, `problem-reality-packet.json` under `<runtime>/<run-id>/smoke-attempt-1/`. Does NOT call Ollama, does NOT call Docker.
- `run_replay(args: ReplayArgs) -> Result<Value>` — reads the named `attempt-summary.json`, calls `build_reality_signal_packet` (existing function) with synthetic args sufficient to re-emit the packet, writes back to disk. Used to test reality-signal builder changes against historical attempts.
- `run_verify(args: VerifyArgs) -> Result<Value>` — walks every JSON file under `<runtime>/<task>/attempt-N/`, recomputes SHA-256 of each, compares against any sha recorded in adjacent `*.sha256` files (or just reports current SHAs).

For Phase 2, `run_replay` and `run_verify` are intentionally narrow but must be real file-backed implementations: read their source-of-truth files, write or enumerate real artifacts, and error with structured diagnostics when required data is missing. The smoke implementation needs to be functional because Phase 9 depends on it.

## Task 2.6 — Add active-root writer

Define a helper `update_active_runtime_root(path: &Path, target_instance: Option<&str>)`:

```rust
fn update_active_runtime_root(runtime_root: &Path, target_instance: Option<&str>) -> Result<()> {
    let cache_dir = dirs::home_dir()
        .ok_or_else(|| RealityError::new(
            "RUST_REALITY_HOME_DIR_MISSING", "...", "...", "...", json!({}), None))?
        .join(".cache/cgreality");
    fs::create_dir_all(&cache_dir).map_err(|e| fs_error("...", &cache_dir, e))?;
    let abs = runtime_root.canonicalize()
        .map_err(|e| fs_error("...", runtime_root, e))?;
    fs::write(cache_dir.join("active_runtime_root"), abs.to_string_lossy().as_bytes())?;
    if let Some(ti) = target_instance {
        fs::write(cache_dir.join("active_target_instance"), ti.as_bytes())?;
    }
    Ok(())
}
```

Call this:
- At the start of `run()` (the Attempt path) — pass the runtime root + task.instance_id.
- At the start of `run_smoke()` — pass the runtime root + a smoke-specific instance ID like `cgreality-smoke-{label}`.

`dirs` may not be a dependency yet; add `dirs = "5"` to `Cargo.toml` `[dependencies]` if missing.

## Task 2.7 — Update any in-repo callers of the old binary shape

```bash
rg -n "context-graph-cli reality-loop|/target/(debug|release)/reality-loop\b" --hidden -g '!target/' -g '!docs/ccreality/'
```

Each match (excluding the new docs we just wrote) is a script invoking the old shape. Update to insert `attempt` subcommand.

Common locations:
- `scripts/*.sh`
- `tasks/*.md` example commands
- Any CI YAML

## Task 2.8 — Build, test, smoke

```bash
cd /home/cabdru/contextgraph
cargo fmt -p context-graph-cli
cargo build -p context-graph-cli --bin reality-loop --quiet
cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored

# Smoke: dispatch each subcommand
./target/debug/reality-loop --help | head -30
./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-phase02-smoke --label phase02-test
ls /tmp/cgreality-phase02-smoke/
ls -la ~/.cache/cgreality/active_runtime_root
```

Expected:
- `--help` shows the subcommands.
- `smoke` writes artifacts.
- `active_runtime_root` is updated.

Then [04_VERIFICATION.md](04_VERIFICATION.md).
