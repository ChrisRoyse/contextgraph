# 16 — Storage Location Policy: D-Drive for All Bulk State

> **Locked decision** per `memory/decisions/2026-05-10--d-backed-durable-data-root-and-mcp-daemon.md`. This document is the futurebuild-side restatement, with the new flywheel CFs and prediction CFs accounted for. Reading this is mandatory before any task that adds an on-disk artifact.

---

## 1. The rule, in one sentence

**Anything that grows over time and matters across reboots lives under `/mnt/d/contextgraph/` (D:\ drive). Code, docs, configs, and reproducible build artifacts live under the source tree (C:\ root via `/home/cabdru/contextgraph/`). Ephemeral test scratch lives in `/tmp/`. Nothing growing lives on C:\.**

---

## 2. Path mapping

| Data class | Path | Owner |
|---|---|---|
| RocksDB workspace store (101 CFs total, 28+ ME-JEPA CFs) | `/mnt/d/contextgraph/storage/contextgraph-rocksdb/` | `crates/context-graph-storage` |
| Shift-log JSONL files (per session, append-only) | `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` | shift writer + subscriber |
| Daemon log files (rotated daily) | `/mnt/d/contextgraph/logs/<task>.log` | daemon supervisor |
| Daemon pid + lock files | `/mnt/d/contextgraph/state/locks/` | wrapper script |
| Active runtime root pointer | `/mnt/d/contextgraph/state/cgreality/active_runtime_root` | already at this path per existing hook context |
| FSV evidence files (formerly `/tmp/contextgraph-*-fsv/`) | `/mnt/d/contextgraph/fsv/<crate>-fsv/` | per-crate FSV examples |
| Replay buffer materializations | `/mnt/d/contextgraph/storage/replay/` | flywheel scheduler |
| Active-learning queue exports | `/mnt/d/contextgraph/exports/active-learning/<date>.json` | weekly eval cron |
| Eval reports + weekly snapshots | `/mnt/d/contextgraph/exports/eval/<date>/` | weekly eval cron |
| Witness-chain segments + Merkle bundles | `/mnt/d/contextgraph/storage/witness/` | witness writer |
| Tier-compressed cold panel slabs | `/mnt/d/contextgraph/storage/cold/` | hygiene tier_demote |
| Model artifacts (GGUF, safetensors, ONNX, tokenizers) | `/mnt/d/cabdru/contextgraph/models/` | embedder loaders |
| Pairwise MI heatmap CSVs | `/mnt/d/contextgraph/exports/pairwise-mi/<date>.csv` | weekly MI auditor |
| Per-step training certs (also in CF, mirrored to disk for sanity) | `/mnt/d/contextgraph/storage/train-certs/` | trainer |
| Per-prediction live cache (mirrors CF_MEJEPA_LIVE_PREDICTIONS) | `/mnt/d/contextgraph/storage/live-predictions/` | predictor |
| Operator-approved gold labels | `/mnt/d/contextgraph/state/gold-labels/` | active-learning queue + operator |

| Stays on C:\ | Path | Why |
|---|---|---|
| Source tree | `/home/cabdru/contextgraph/` | code; reproducible from git |
| docs/ | `/home/cabdru/contextgraph/docs/` | text; bounded |
| target/release/ build artifacts | `/home/cabdru/contextgraph/target/release/` | reproducible from `cargo build`; can be wiped + rebuilt |
| memory/ SMAP tree | `/home/cabdru/contextgraph/memory/` | bounded; tiny per file; under git |
| .claude/ config + hooks + agents | `/home/cabdru/contextgraph/.claude/` | bounded; under git |
| /tmp/ ephemeral test scratch | `/tmp/` | survives test run only; do not depend on cross-reboot |

---

## 3. Why this rule exists

1. **C:\ is small and shared with the OS.** Most operator C:\ partitions are 256-1000 GB, shared with Windows + WSL + the user's other tools. Filling C:\ degrades the host system.
2. **D:\ is large and dedicated.** Operator's D:\ has 2-4 TB and is for ContextGraph durable state.
3. **Cleanup must be predictable.** With everything under `/mnt/d/contextgraph/`, `du -sh /mnt/d/contextgraph/` gives the full footprint; quota policy applies once.
4. **MCP reconnects must be cheap.** Heavy state living on a stable path means MCP restarts don't reload model weights or replay buffers; the daemon stays warm via the wrapper.
5. **Per `memory/decisions/2026-05-10--...md`**: this is **not a fallback path**. The configured D-backed paths must exist and work. Missing paths fail closed with logs, not silent migration to C:\.

---

## 4. Enforcement

### 4.1 Daemon startup validator

`TASK-FLYWHEEL-001-mcp-daemon-mode.md` adds a startup check:

```rust
fn validate_d_drive_root() -> Result<()> {
    let root = Path::new("/mnt/d/contextgraph");
    if !root.exists() {
        return Err(MejepaError::DDriveUnavailable {
            path: root.to_path_buf(),
            hint: "D-drive root /mnt/d/contextgraph must exist and be writable. \
                   See docs/futurebuild/16_STORAGE_LOCATION_POLICY.md.",
        });
    }
    let test = root.join(".write_test");
    std::fs::write(&test, b"ok").map_err(|e| MejepaError::DDriveNotWritable { source: e })?;
    std::fs::remove_file(&test).ok();
    
    for required_subdir in &[
        "storage/contextgraph-rocksdb",
        "runtime/cgreality-shift-log",
        "logs",
        "state/locks",
        "fsv",
        "storage/replay",
        "exports/eval",
        "exports/active-learning",
        "exports/pairwise-mi",
        "storage/witness",
        "storage/cold",
        "storage/train-certs",
        "storage/live-predictions",
        "state/gold-labels",
    ] {
        let p = root.join(required_subdir);
        if !p.exists() {
            std::fs::create_dir_all(&p).map_err(|e| MejepaError::DDriveSubdirCreateFailed {
                path: p,
                source: e,
            })?;
        }
    }
    Ok(())
}
```

Daemon refuses to start if validation fails. Error code `MEJEPA_D_DRIVE_UNAVAILABLE` (in the project error namespace).

### 4.2 CI lint

`TASK-OBS-005-c-drive-lint.md` adds a CI lint that grep-asserts no new code introduces a hardcoded path matching `/home/`, `~/.cache/`, `/tmp/contextgraph-`, or any C:\-rooted artifact path. Allowed exceptions:
- The source tree itself (paths inside `/home/cabdru/contextgraph/` for code/doc/config files).
- `/tmp/` references in test scratch directories (must be wrapped in `tempfile::TempDir`).
- The wrapper script's own startup (it bootstraps the D-drive paths).

### 4.3 FSV migration

Today many FSV examples write to `/tmp/contextgraph-*-fsv/`. Per the locked decision, these should be redirected to `/mnt/d/contextgraph/fsv/<crate>-fsv/`. `TASK-OBS-006-fsv-path-migration.md` updates each FSV example's output path and adjusts the corresponding evidence-loading test code. This is a mechanical sweep across `crates/*/examples/*_fsv.rs`.

### 4.4 Hook output paths

Shift log files currently land at `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/`. The hook scripts must be updated to write under `/mnt/d/contextgraph/runtime/cgreality-shift-log/`. `TASK-LOOP-007-hook-output-paths.md` covers this — small change to `scripts/cc/hook_post_tool_use_*.sh`.

---

## 5. Quota policy and observability

The 115 GB hard quota from `06 HYGIENE` (REQ-HYG-05, shipped) targets `/mnt/d/contextgraph/` as a whole. Allocation:

| Slice | Size budget | Tier policy |
|---|---|---|
| `storage/contextgraph-rocksdb/` (all CFs) | 70 GB hot + warm; cool tier compresses to 25 GB | hot 8-bit, warm 7-bit, cool 5-bit, cold 3-bit |
| `runtime/cgreality-shift-log/` | 5 GB rolling window | per-session GC after 30 days |
| `storage/witness/` | 2 GB hot; >90d Merkle-compressed | per Phase 6 REQ-HYG-03 |
| `storage/cold/` | up to 25 GB cold blobs | 3-bit / 1-bit frozen |
| `storage/replay/` | 5 GB | bounded by replay-buffer policy |
| `logs/` | 2 GB rolling (daily rotation) | 30-day retention |
| `fsv/` | 1 GB cap | bounded by per-crate FSV size |
| `exports/` | 2 GB rolling (90-day retention) | weekly snapshots |
| `state/` + `state/gold-labels/` | 100 MB | tiny but pinned (operator labels are permanent) |
| Models | not in 115 GB cap (separate budget at `/mnt/d/cabdru/contextgraph/models/`) | per `Phase 1b` digest verification |
| **Total cap** | **115 GB** | hard ceiling per REQ-HYG-05 |

The `mejepa_quota_status` MCP tool reports the rollup. When usage > 110 GB, emergency eviction kicks in (per `09_SELF_HEALING_AUTONOMY.md §11`).

---

## 6. Operator quickstart

For an operator setting up a fresh machine:

```bash
# 1. Mount the D drive (WSL convention)
mkdir -p /mnt/d
# (Windows host should already mount D: at /mnt/d when WSL starts)

# 2. Create the ContextGraph root
mkdir -p /mnt/d/contextgraph/{storage/{contextgraph-rocksdb,replay,cold,witness,train-certs,live-predictions},runtime/cgreality-shift-log,logs,state/locks,state/cgreality,state/gold-labels,fsv,exports/{active-learning,eval,pairwise-mi}}
mkdir -p /mnt/d/cabdru/contextgraph/models

# 3. Place SHA-pinned model artifacts under /mnt/d/cabdru/contextgraph/models/
#    Run the digest verification check; it'll fail closed if any artifact is missing.

# 4. Start the daemon
scripts/ensure_daemon.sh   # bootstraps the warm daemon + stdio proxy

# 5. Verify
ls -la /mnt/d/contextgraph/state/locks/
# should see context-graph-daemon.pid

cat /mnt/d/contextgraph/logs/context-graph-daemon.log | head -20
# should see "validated D-drive root, starting supervised tasks"

mejepa_quota_status   # via MCP — should show < 1 GB used initially
mejepa_subscriber_status   # should show 0 ms lag
```

If anything fails, the error code (`MEJEPA_D_DRIVE_UNAVAILABLE` / `MEJEPA_EMBEDDER_DIGEST_MISMATCH` / `MEJEPA_DAEMON_LOCK_HELD`) tells the operator what to fix.

---

## 7. What this storage policy commits us to

1. **Daemon startup validator** that fail-closes if D-drive root absent. (`TASK-FLYWHEEL-001`, shipped; keep enforced)
2. **CI lint** that prevents regression. (`TASK-OBS-005`)
3. **FSV path migration** sweep across `crates/*/examples/*_fsv.rs`. (`TASK-OBS-006`)
4. **Hook output path update** for shift log files. (`TASK-LOOP-007`)
5. **Quota observability extension** to break down by slice (RocksDB / shift-log / cold / replay / etc.) for operator visibility. (`TASK-OBS-007`)
6. **Operator quickstart documentation** in CLAUDE.md or a top-level INSTALL.md (small task; not separately listed because the content above suffices).

All listed in [`13_TASK_INDEX.md`](13_TASK_INDEX.md).
