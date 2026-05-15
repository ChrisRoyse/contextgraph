# 19 — Operator Runbook

> **Purpose.** Day-to-day, week-to-week, and incident-response operations for ME-JEPA-as-truth-system. The single source of truth for "what does the operator do?"
>
> Owned by [`SPEC-RUNBOOK.md`](specs/functional/SPEC-RUNBOOK.md) + [`TECH-RUNBOOK.md`](specs/technical/TECH-RUNBOOK.md).

---

## 1. First-time setup (one-time)

```bash
# 1. Mount the D drive (WSL convention)
mkdir -p /mnt/d
# (Windows host should already mount D: at /mnt/d when WSL starts; verify with `mount | grep /mnt/d`)

# 2. Create the ContextGraph root
mkdir -p /mnt/d/contextgraph/{storage/{contextgraph-rocksdb,replay,cold,witness,train-certs,live-predictions,embedder-cache},runtime/{cgreality-shift-log,test-outcomes},logs,state/{locks,cgreality,gold-labels},fsv,exports/{active-learning,eval,pairwise-mi,load-tests,test-runs}}
mkdir -p /mnt/d/cabdru/contextgraph/models

# 3. Place SHA-pinned model artifacts under /mnt/d/cabdru/contextgraph/models/
#    (Operator-provided per the per-embedder digest list; not auto-fetched)

# 4. Build
cd /home/cabdru/contextgraph
cargo build --release --workspace

# 5. Install systemd unit
cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now mejepa-daemon

# 6. Verify
systemctl --user status mejepa-daemon          # should show "active (running)"
ls -la /mnt/d/contextgraph/state/locks/         # should see context-graph-daemon.pid
tail /mnt/d/contextgraph/logs/mejepa-daemon.stdout.log

# 7. MCP smoke test (from any directory)
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"mcp__cgreality__mejepa_quota_status","arguments":{}}}' \
    | nc -U /mnt/d/contextgraph/state/locks/mcp.sock
# expect: { "result": { "used_gb": ..., "hard_quota_gb": 115.0, ... } }
```

If any step fails, see `§7 Troubleshooting`.

---

## 1.5 Python SWE-bench data-explosion runbook

**Active scope.** This is the operator procedure for the Python ME-JEPA-Code
data foundation. It starts from real SWE-bench Lite Python code and ends in
durable D-drive stores that downstream DDA, TCT, reward tensors, and Q1-Q5
`RealityPrediction` training consume. Gridworld, counter-world, O*NET,
career-taxonomy, and other proof-domain harnesses are archived research
fixtures; they are not active prerequisites for the Python ship gate unless a
future GitHub issue proves they add durable Python code-reality signal.

### 1.5.1 Source-of-truth map

| Stage | Source of truth | Required readback |
|---|---|---|
| Raw Python corpus | `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` | instance count, mutation/oracle rows, provenance |
| AST/code chunks | chunk JSONL selected by the scheduler | chunk count and source byte limits |
| Forward cache | `/mnt/d/contextgraph/corpus/python-forward-cache-*` | manifest, schema, index JSONL, packed records, quarantine JSONL |
| FSV evidence | `/mnt/d/contextgraph/fsv/task-py-data-*-fsv/` | `passes=true`, happy path and edge-case readbacks |
| Runtime/checkpoints | `/mnt/d/contextgraph/runtime/` and `python_forward_cache_checkpoint.json` | completed shards, expected/accounted pairs, resource snapshots |
| Hot staging | `/home/cabdru/.cache/contextgraph/` or `/var/tmp/contextgraph/` | bounded `du -sh`; promoted D-drive target exists |

Bulk durable state stays on `/mnt/d/contextgraph/`. High-churn bounded working
sets may use WSL ext4 on the C-drive VHDX, preferably
`/home/cabdru/.cache/contextgraph/` or `/var/tmp/contextgraph/`. Do not use
`/mnt/c` as a hot path; it is the Windows NTFS mount.

### 1.5.2 Hardware-aware preflight

Run this before any new data-explosion run or after changing batch sizes:

```bash
cd /home/cabdru/contextgraph
sed -n '1,120p' docs2/systemspecs.md
nproc
free -b
df -B1 / /mnt/c /mnt/d
nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total,memory.used,memory.free,utilization.gpu,utilization.memory,temperature.gpu,power.draw --format=csv,noheader,nounits
nvcc --version
docker info
```

Expected local hardware profile: Ryzen 9 9950X3D, 16 cores / 32 threads on the
host, about 110 GiB RAM visible to WSL, 32 GiB swap visible to WSL, RTX 5090
with 32607 MiB VRAM, CUDA 13.2, and `/mnt/d` with enough free bytes for the
durable corpus. Use live Linux affinity for worker counts: `nproc` is the
source of truth for how many CPU workers this WSL process can actually use.

Fail closed if any required detector fails:

- Missing `/mnt/d/contextgraph/`: do not write to `/tmp`, `/mnt/c`, or the repo
  root; remount D: and rerun preflight.
- Missing Docker Desktop: do not run oracle/evaluator stages; file or update a
  `status:blocked` GitHub issue with the failed `docker info` output.
- Missing `nvidia-smi` or required CUDA mode: required-GPU forward runs must
  fail with a structured error such as `MEJEPA_SCHEDULER_CUDA_REQUIRED`; do not
  silently reroute required GPU outputs to CPU.
- VRAM guard exceeded: lower the relevant embedder batch size and rerun the
  smoke FSV before a full run.
- D-drive or hot-stage capacity insufficient: shrink the shard plan or free
  space; no partial write is acceptable without a failed-partial archive.

Worker policy for this host:

- D-drive/Docker I/O: keep bounded, normally `--io-workers 2`.
- CPU AST/static/token work: use the live WSL affinity budget, normally
  `--cpu-workers "$(nproc)"`. On this host that may be lower than the 32-thread
  hardware spec if WSL constrains the process affinity.
- GPU forwards: use CUDA where the embedder supports it; normally
  `--gpu-workers 1` plus per-embedder true-batch controls to avoid cumulative
  VRAM residency.
- Correctness beats speed. Prefer checkpointed, resumable, FSV-proven progress
  over aggressive tuning that risks hidden state loss.

### 1.5.3 Small-sample smoke path

Use a one-shard smoke run after changing scheduler, embedder, storage, or
hardware policy. Build the release runner first:

```bash
cargo build --release --example python_forward_cache_fsv
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
CPU_WORKERS="$(nproc)"
CACHE_ROOT="/mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-026-smoke-${RUN_ID}"
FSV_ROOT="/mnt/d/contextgraph/fsv/task-py-data-026-smoke-${RUN_ID}-fsv"
HOT_ROOT="/home/cabdru/.cache/contextgraph/task-py-data-026-smoke-${RUN_ID}"
python3 scripts/swebench_lite_ops/python_forward_shard_scheduler.py \
  --task-id TASK-PY-DATA-026-SMOKE \
  --cache-root "$CACHE_ROOT" \
  --fsv-root "$FSV_ROOT" \
  --hot-root "$HOT_ROOT" \
  --start-offset 0 \
  --shard-size 1 \
  --shard-count 1 \
  --max-source-bytes 32768 \
  --true-batch-size 8 \
  --embedder-batch-sizes e1=32,e6=32,e7=32,e8=32,e10=32,e12=32,e13=32,e14=8,e9=128 \
  --embedder-max-padded-source-bytes e1=1048576,e6=1048576,e7=1048576,e8=1048576,e10=1048576,e12=524288,e13=1048576,e14=262144,e9=4194304 \
  --gpu-vram-guard-bytes 34190917632 \
  --io-workers 2 \
  --cpu-workers "$CPU_WORKERS" \
  --gpu-workers 1 \
  --jsonl-sync-every-rows 512 \
  --runner-timeout-seconds 1800 \
  --cache-store-layout packed \
  --require-packed-store \
  --resume-completed-shards \
  --retry-failed-partial-shards \
  --checkpoint-every-shards 1 \
  --isolate-embedder-processes \
  --cargo-runner target/release/examples/python_forward_cache_fsv
```

Then read the bytes, not the return value:

```bash
jq '{passes, source_of_truth}' "$FSV_ROOT/python_forward_shard_scheduler_fsv.json"
jq '{progress, summary, resource}' "$CACHE_ROOT/python_forward_cache_checkpoint.json"
find "$CACHE_ROOT" -maxdepth 3 -type f | sort | head -40
du -sh "$HOT_ROOT" "$CACHE_ROOT" "$FSV_ROOT"

# After the D-drive cache/FSV readback succeeds, remove only this unique hot
# staging root. The D-drive cache and FSV roots are the durable SoT.
rm -rf -- "$HOT_ROOT"
test ! -e "$HOT_ROOT"
```

The existing known-good observability smoke evidence is
`/mnt/d/contextgraph/fsv/task-py-data-024-observability-smoke-20260514-fsv/python_forward_shard_scheduler_fsv.json`
with `passes=true`. It proves packed-record readback, checkpointed progress,
capacity evidence, CUDA detection, RAM/VRAM/process/disk resource snapshots,
and fail-closed edge cases.

### 1.5.4 Full corpus run and checkpoints

Run the accepted post-#249 full Python corpus as a resumable shard schedule.
The active relaunch root is keyed to the accepted `python_ast_chunks.jsonl`
hash prefix `c8117f15`. The current full-run pattern uses 128-chunk shards,
packed storage, isolated embedder processes, one checkpoint per shard, and
failed-partial archiving. The old `20260514` root is historical evidence only
and must not be used for the post-#249 relaunch:

```bash
CPU_WORKERS="$(nproc)"
SESSION="cg_pydata016_full_c8117f15"
RUNTIME_ROOT="/mnt/d/contextgraph/runtime/task-py-data-016-full-c8117f15-20260515T0956Z"
mkdir -p "$RUNTIME_ROOT"
tmux new-session -d -s "$SESSION" "cd /home/cabdru/contextgraph && \
python3 scripts/swebench_lite_ops/python_forward_shard_scheduler.py \
  --task-id TASK-PY-DATA-016 \
  --cache-root /mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515 \
  --fsv-root /mnt/d/contextgraph/fsv/task-py-data-016-full-c8117f15-20260515-fsv \
  --hot-root /home/cabdru/.cache/contextgraph/task-py-data-016-full-c8117f15-20260515 \
  --start-offset 0 \
  --shard-size 128 \
  --shard-count 3124 \
  --max-source-bytes 32768 \
  --true-batch-size 8 \
  --embedder-batch-sizes e1=32,e6=32,e7=32,e8=32,e10=32,e12=32,e13=32,e14=8,e9=128 \
  --embedder-max-padded-source-bytes e1=1048576,e6=1048576,e7=1048576,e8=1048576,e10=1048576,e12=524288,e13=1048576,e14=262144,e9=4194304 \
  --gpu-vram-guard-bytes 34190917632 \
  --io-workers 2 \
  --cpu-workers \"$CPU_WORKERS\" \
  --gpu-workers 1 \
  --jsonl-sync-every-rows 512 \
  --runner-timeout-seconds 1800 \
  --cache-store-layout packed \
  --require-packed-store \
  --resume-completed-shards \
  --retry-failed-partial-shards \
  --checkpoint-every-shards 1 \
  --isolate-embedder-processes \
  --cargo-runner target/release/examples/python_forward_cache_fsv \
  > $RUNTIME_ROOT/scheduler.log 2>&1"
```

Monitor from source-of-truth files:

```bash
tmux list-sessions
ps -eo pid,ppid,stat,etime,%cpu,%mem,rss,cmd | rg 'python_forward_shard_scheduler|python_forward_cache_fsv|target/release/examples/python_forward_cache_fsv'
jq '{progress, summary, resource:{phase:.resource.phase, mem_available:.resource.memory.MemAvailable_bytes, gpu_free:.resource.gpu.gpus[0].memory_free_bytes, scheduler_pid:.resource.scheduler_process.pid}}' \
  /mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515/python_forward_cache_checkpoint.json
find /mnt/d/contextgraph/fsv/task-py-data-016-full-c8117f15-20260515-fsv/run_logs -maxdepth 1 -name 'shard-*.json' | wc -l
nvidia-smi --query-gpu=memory.total,memory.used,memory.free,utilization.gpu,utilization.memory,temperature.gpu,power.draw --format=csv,noheader,nounits
```

Resume/recovery rules:

- A completed shard is skippable only when its manifest, FSV, telemetry,
  index/quarantine, and packed-store readback pass.
- Partial or corrupt shard state is archived under
  `fsv_root/failed_partial_shards/` before retry.
- Checkpoint progress is authoritative. Return values and logs are evidence of
  attempt, not success.
- For every heartbeat, report `completed_shards`, `completed_chunk_capacity`,
  `expected_pairs`, `accounted_pairs`, `cache_record_count`,
  `quarantine_row_count`, `second_pass_hits`, RAM, VRAM, and active PIDs.

As of the 2026-05-15 refresh, #199 is still running and is not a training-ready
full-corpus result. The live checkpoint readback was:
`completed_shards=27/3124`, `completed_chunk_capacity=3456/399872`,
`expected_pairs=accounted_pairs=41472`, `cache_record_count=40098`, and
`quarantine_row_count=1374`.

The smoke intelligence-matrix SoT is
`/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices/summary.json`
with FSV
`/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json`.
Use it to build smoke-dry-run ME-JEPA feature masks before full-corpus
training: e1/e8 are identical-alignment redundant, e7/e14/e9 carry high Q2-Q5
smoke contribution, e2/e3/e4 currently produce one unique vector hash each and
must be masked from semantic-credit claims unless downstream contribution
evidence proves utility, and missing/quarantined rows must remain explicit
masks rather than silent defaults.

### 1.5.4a Python scope guard

Generic CI and operator preflight must reject drift back to proof-domain
harnesses. Run this guard after editing workflows, futurebuild docs, or Python
data-explosion entrypoints:

```bash
python3 scripts/swebench_lite_ops.py python-proof-domain-isolation --clean-output
jq '{passes, summary, readback_counts, report_sha256}' \
  /mnt/d/contextgraph/fsv/task-py-g-037-proof-domain-isolation-fsv/python_proof_domain_isolation_fsv.json
find /mnt/d/contextgraph/corpus/python-proof-domain-isolation-v1 -maxdepth 1 -type f -print -exec sha256sum {} \;
```

Expected source-of-truth state: `passes=true`, zero forbidden repo/open-issue
rows, six edge-case rows, and a non-empty active-reference audit. Proof-domain
mentions are allowed only when they are archived, explicitly excluded from the
active Python gate, or part of the current guard implementation/issue.

### 1.5.4b Reward-field sample and training readiness

Use the sample/readiness commands to decide whether a training run is allowed.
Smoke-dry-run training may proceed from the 1,000-chunk smoke reward field;
production/full-corpus training must fail closed until #199 is final.

```bash
python3 scripts/swebench_lite_ops.py python-data-readiness-gate
jq '{passes, readback}' \
  /mnt/d/contextgraph/fsv/task-py-data-023-data-readiness-gate-fsv/python_data_readiness_gate_fsv.json

python3 scripts/swebench_lite_ops.py python-reward-field-sample
jq '{passes, happy_path, edge_cases, readback}' \
  /mnt/d/contextgraph/fsv/task-py-data-028-reward-field-sample-fsv/python_reward_field_sample_fsv.json
find /mnt/d/contextgraph/corpus/python-reward-field-sample-v1 -maxdepth 1 -type f -print -exec sha256sum {} \;

python3 scripts/swebench_lite_ops.py python-embedder-contribution
jq '{passes, readback:{per_head_embedder_contribution, feature_contribution_metrics, shared_contribution_pairs, stratified_contribution, edge_case_states, summary}}' \
  /mnt/d/contextgraph/fsv/task-py-data-055-embedder-contribution-fsv/python_embedder_contribution_fsv.json
find /mnt/d/contextgraph/corpus/python-embedder-intelligence-v1/contribution -maxdepth 1 -type f -print -exec sha256sum {} \;

python3 scripts/swebench_lite_ops.py python-tct-completeness
jq '{passes, readback:{mutation_category_completeness, cell_completeness, reward_dimension_completeness, missing_signal_completeness, edge_case_states, summary}}' \
  /mnt/d/contextgraph/fsv/task-py-g-039-tct-completeness-fsv/python_tct_completeness_fsv.json
find /mnt/d/contextgraph/corpus/python-tct-completeness-v1 -maxdepth 1 -type f -print -exec sha256sum {} \;
```

Expected current sample readback:

- output root: `/mnt/d/contextgraph/corpus/python-reward-field-sample-v1`
- representative failing sample has Q1-Q5 tensor, AST source slice, 12 DDA
  rows, TCT membership, 12 forward-cache index rows, and patch tensor linkage
- edge samples: no production-code change, syntax-error mutation, and real
  oracle timeout event
- FSV: `/mnt/d/contextgraph/fsv/task-py-data-028-reward-field-sample-fsv/python_reward_field_sample_fsv.json`
  with `passes=true`
- embedder contribution SoT:
  `/mnt/d/contextgraph/corpus/python-embedder-intelligence-v1/contribution/`
- contribution FSV:
  `/mnt/d/contextgraph/fsv/task-py-data-055-embedder-contribution-fsv/python_embedder_contribution_fsv.json`
  with `passes=true`
- contribution readback: 60 per-head/embedder rows, 840 feature metrics,
  34 shared-credit pairs, 96 stratified rows, and 5 edge-case records
- contribution interpretation: Q1 is explicitly invalid in the smoke field
  because all existence labels are true; Q2-Q5 have usable smoke signal;
  missing/quarantined rows are masks, not zero-imputed vectors; redundant
  embedder pairs are marked `share_credit_not_double_count`
- TCT completeness SoT:
  `/mnt/d/contextgraph/corpus/python-tct-completeness-v1/`
- TCT completeness FSV:
  `/mnt/d/contextgraph/fsv/task-py-g-039-tct-completeness-fsv/python_tct_completeness_fsv.json`
  with `passes=true`
- TCT completeness readback: 8 mutation-category rows, 93 cell rows, 8
  reward-dimension rows, 123 missing-signal rows, and 5 edge-case rows
- TCT completeness interpretation: all 1,000 smoke reward rows have TCT
  memberships and all 12,000 expected embedder pairs are accounted as present
  or missing/quarantined; production/full-corpus training remains blocked
  because sparse TCT cells and fail-closed cell memberships are still real
  source-of-truth states

### 1.5.5 Edge cases operators must exercise

Before promoting a new scheduler or runbook change, capture happy path plus at
least these edge cases in `/mnt/d/contextgraph/fsv/`:

| Edge | Trigger | Expected source-of-truth result |
|---|---|---|
| Missing Docker | `docker info` fails before oracle work | no corpus/oracle write; GitHub issue marked `status:blocked` |
| Missing corpus | source corpus path absent | scheduler exits before cache write; checkpoint absent or unchanged |
| Missing CUDA detector | invalid `--nvidia-smi-command` | structured CUDA-required error; no silent CPU fallback |
| Failed embedder output | corrupt/missing manifest or output row | quarantine or failed-partial archive; `accounted_pairs == expected_pairs` remains required |
| Duplicate/resume | rerun with `--resume-completed-shards` | completed shard skipped only after full manifest/FSV/readback proof |

When any edge fails unexpectedly, stop the run, inspect the SoT, fix the root
cause, write or update a GitHub issue, then rerun the adjacent happy path and
edges. Do not delete failed-partial archives; they are evidence.

---

## 2. Daily operations (2-5 minutes)

```bash
# Once per day, e.g., as part of morning routine

# 2.1 Daemon health
mcp_call mejepa_daemon_status
# Look for:
# - subscriber.lag_ms < 5000
# - heal.last_drift_class != Catastrophic
# - quota.used_gb < 110
# - vram.peak_gb < budget_gb

# 2.2 Glance at any operator alerts
ls /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/ 2>/dev/null
# If a weekly report exists, glance at the top "Catastrophic events" section

# 2.3 Active-learning queue check (do once per session, not per day)
context-graph-cli mejepa active-learning list --top-n 5
# If anything is gold-standard-ready, label it
```

If anything is degraded, see §6 Incident Response. Otherwise, you're done.

---

## 3. Weekly operations (15-30 minutes)

```bash
# Mondays at start of week

# 3.1 Read the weekly evaluation report
less /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/weekly.md

# Look for:
# - Operator Cell Triage top-10 table; take the suggested action per row
# - Convergence ETA Per Cell rows; non-converging or unavailable ETAs need targeted data/action
# - Oracle flakiness quarantine changes in config/corpus_quarantine.toml before train/holdout reruns
# - Per-Failure-Mode-Class Metrics rows below precision 0.90 or recall 0.70; inspect the weakness column
# - Python mutation-category ship-gate status (≥0.95 met / not met per cell)
# - Drift events of the week
# - Promotions / rollbacks
# - Agent-feedback summary (top surprises)
# - Storage trend

# 3.1a Produce repeat-oracle observations and refresh flakiness quarantine
mejepa-corpus verify \
  --corpus /mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511 \
  --sample-fraction 1.0 \
  --oracle-repeat-runs 3 \
  --oracle-repeat-output /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/oracle-repeat-observations.json
context-graph-cli mejepa oracle-flakiness-audit \
  --input /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/oracle-repeat-observations.json \
  --output config/corpus_quarantine.toml

# 3.2 Address active-learning queue
context-graph-cli mejepa active-learning list --top-n 20
# For each entry, decide:
# - Label as gold-standard (operator-override): context-graph-cli mejepa active-learning label <id> <label> "<reason>"
# - Dismiss (not actionable): context-graph-cli mejepa active-learning dismiss <id> "<reason>"

# 3.3 Pairwise MI heatmap inspection (every other week)
xdg-open /mnt/d/contextgraph/exports/pairwise-mi/$(ls /mnt/d/contextgraph/exports/pairwise-mi/ | tail -1)
# If any embedder pair has MI > 0.85 sustained over 2 audits, decide:
# - Retire one slot (mejepa_routing_lookup --retire <embedder_id>)
# - Recalibrate (mejepa_audit_pairwise_mi --force-recalibrate)
# - Accept (no action)

# 3.4 Disk usage check
du -sh /mnt/d/contextgraph/
mcp_call mejepa_quota_status   # expect breakdown by slice
```

---

## 4. Monthly operations (30-60 minutes)

```bash
# 4.1 Backup verification (per 20_DISASTER_RECOVERY_AND_BACKUP.md)
ls -la /mnt/d/contextgraph-backups/   # ensure most recent < 7 days old

# 4.2 Storage cold-tier audit
context-graph-cli mejepa hygiene cold-audit
# Confirm cold tier compression ratios are healthy

# 4.3 Witness chain integrity check (forced)
mcp_call optimizer_witness_chain_verify --depth full
# Should return "valid" with chain_length and last_verified_ts

# 4.4 Soak-test trigger (quarterly per 18_INTEGRATION_AND_E2E_TESTING.md §8)
# only quarterly; skip if not due

# 4.5 Read latest doc updates
git log --since="1 month ago" --oneline -- docs/futurebuild/ docs/ruvectorfindings/
```

---

## 5. Incident response — alert classes

### 5.1 `MEJEPA_SCHEDULER_FLAPPING` — supervised task restarting > 5 times in 5 min

**Cause:** persistent error in a tokio task (panic, OOM, dependency crash).

**Investigate:**
```bash
journalctl --user -u mejepa-daemon -n 200
tail -100 /mnt/d/contextgraph/logs/mejepa-daemon.stderr.log
```

**Resolution:**
```bash
systemctl --user stop mejepa-daemon
systemctl --user restart mejepa-daemon
```

1. Identify which supervised task is flapping (log lines name it).
2. Check if it's a known issue (CF corruption → see 5.2; OOM → see 5.5; etc.).
3. If unknown: `systemctl --user stop mejepa-daemon`; investigate; create or update a `status:blocked` GitHub issue with the log evidence; restart only after the blocker condition is understood.

### 5.2 `MEJEPA_INFER_WITNESS_CHAIN_BROKEN`

**Cause:** hash chain integrity check failed.

**Investigate:**
```bash
mcp_call optimizer_witness_chain_diff --since-last-verified
journalctl --user -u mejepa-daemon -n 200
```

**Resolution:**
```bash
mcp_call optimizer_witness_chain_repair_legacy --force
context-graph-cli mejepa storage-verify --db-path /mnt/d/contextgraph/state/cgreality/rocksdb
```

1. Auto-repair has already attempted. Check: `mcp_call optimizer_witness_chain_diff --since-last-verified`
2. If auto-repair succeeded: log + carry on.
3. If auto-repair failed: `mcp_call optimizer_witness_chain_repair_legacy --force` and check.
4. Last resort: restore the last known-good RocksDB checkpoint per `20_DISASTER_RECOVERY_AND_BACKUP.md §4`.

### 5.3 `MEJEPA_OBSERVE_DRIFT_DETECTED`

**Cause:** rolling accuracy < 80%.

**Investigate:**
```bash
mcp_call mejepa_heal_status
context-graph-cli mejepa report-weekly --date "$(date -u +%F)"
```

**Resolution:**
```bash
mcp_call mejepa_promote_approval --promotion-id <id> --approve true
mcp_call mejepa_promote_approval --promotion-id <id> --approve false --reason "rejecting suspicious drift candidate"
```

1. Check `mejepa_heal_status` for the queued full retrain candidate.
2. Read `weekly.md` for context (data shift? Operator-introduced label change?).
3. If retrain looks reasonable: `mcp_call mejepa_promote_approval --promotion-id <id> --approve true`
4. If suspicious: `--approve false --reason "..."`; investigate.

### 5.4 `MEJEPA_HYGIENE_QUOTA_EXCEEDED`

**Cause:** disk usage > 115 GB hard cap.

**Investigate:**
```bash
mcp_call mejepa_quota_status
du -sh /mnt/d/contextgraph/runtime /mnt/d/contextgraph/state /mnt/d/contextgraph/fsv
```

**Resolution:**
```bash
mcp_call mejepa_gc_run --force-eviction true
context-graph-cli mejepa hygiene aggressive-cold-tier-down
```

1. Check `mcp_call mejepa_quota_status` for slice breakdown.
2. Run emergency eviction: `mcp_call mejepa_gc_run --force-eviction true`.
3. If eviction insufficient: `context-graph-cli mejepa hygiene aggressive-cold-tier-down`.
4. If still over: investigate why a specific slice is growing (e.g., shift-log retention misconfigured).

### 5.5 OOM / RSS climbing

**Cause:** memory leak in supervised task.

**Investigate:**
```bash
journalctl --user -u mejepa-daemon -n 200
ps -o pid,rss,comm -C context-graph-mcp
```

**Resolution:**
```bash
systemctl --user restart mejepa-daemon
context-graph-cli mejepa daemon-status
```

1. `systemctl --user restart mejepa-daemon`
2. Check `valgrind`-style profiling (collect data during repro)
3. Create or update a `status:blocked` GitHub issue with severity, reproducible scenario, and the direct source-of-truth readback.

### 5.6 `MEJEPA_D_DRIVE_UNAVAILABLE`

**Cause:** D-drive unmounted or corrupted.

**Investigate:**
```bash
mount | grep /mnt/d
ls -ld /mnt/d/contextgraph
```

**Resolution:**
```bash
systemctl --user restart mejepa-daemon
context-graph-cli mejepa storage-verify --db-path /mnt/d/contextgraph/state/cgreality/rocksdb
```

1. Check mount: `mount | grep /mnt/d`
2. If unmounted: remount; restart daemon.
3. If corrupted: see `20_DISASTER_RECOVERY_AND_BACKUP.md §3`.

### 5.7 `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT`

**Cause:** the shift subscriber has sustained pending shift-log lines above its configured threshold, so predictions may be stale.

**Investigate:**
```bash
mcp_call mejepa_subscriber_status
find /mnt/d/contextgraph/runtime/cgreality-shift-log -maxdepth 1 -type f -name "*.jsonl" -printf "%p %s bytes\n"
```

**Resolution:**
```bash
systemctl --user restart mejepa-daemon
mcp_call mejepa_subscriber_status
```

1. Check `mcp_call mejepa_subscriber_status` and confirm `lag_alert_active=true`, pending shift count, and latest watermark.
2. Inspect `/mnt/d/contextgraph/runtime/cgreality-shift-log/` for oversized or malformed JSONL files.
3. If the daemon is alive but not consuming, restart it once: `systemctl --user restart mejepa-daemon`.
4. If lag returns, file a high-severity blocker with the offending session id, shift-log path, and subscriber status payload.

### 5.8 Predictions silently wrong (no alert, but operator notices)

**Cause:** drift below detection threshold; OR data distribution shift the detector hasn't caught; OR head's λ ramped too high prematurely.

**Investigate:**
```bash
mcp_call mejepa_ship_gate_status
context-graph-cli mejepa report-weekly --date "$(date -u +%F)"
```

**Resolution:**
```bash
mcp_call mejepa_constellation_inspect --force-refit --category <category> --language <language>
context-graph-cli mejepa active-learning list
```

1. Pull `mejepa_ship_gate_status` → check per-cell status.
2. If a specific (category, language) cell is regressing, force constellation refresh: `mcp_call mejepa_constellation_inspect --force-refit --category <c> --language <l>`.
3. If pervasive: file blocker; consider triggering full retrain.

### 5.9 `MEJEPA_TRAINING_HOLDOUT_DRIFT`

**Cause:** the training-batch `(mutation_category, language)` distribution has diverged from the holdout distribution with `KL > 0.5` for at least 100 batches.

**Investigate:**
```bash
mcp_call mejepa_heal_status
context-graph-cli mejepa report-weekly --date "$(date -u +%F)"
```

**Resolution:**
```bash
context-graph-cli mejepa cell-status --threshold 0.95
context-graph-cli mejepa active-learning list
```

1. Read the latest `telemetry_feedback` action and confirm `metric_name=training_holdout_kl_divergence`.
2. Compare weak cells in `cell-status` against recent active-learning and operator-override entries.
3. Reduce the skew by draining high-priority labels or broadening corpus sampling for underrepresented cells before promoting a new model.

---

## 6. Configuration tuning

| Knob | Default | Where set | When to change |
|---|---|---|---|
| Drift soft/hard/catastrophic thresholds | 88/85/80% | `daemon config` | when production data calibrates |
| Subscriber tick period | 250 ms | `daemon config` | rarely; tighten if lag > 5s p50 |
| Promote tick period | 3600 s | `daemon config` | rarely |
| Storage hard quota | 115 GB | `daemon config` | when hardware changes |
| GC schedule | nightly 03:00 local | systemd timer | per operator preference |
| Active-learning surface threshold | top-N=10 / period=600s | `daemon config` | per operator capacity |
| Per-head loss coefficients λ_h | per `10 §8` defaults | trained-in | only via supersession |

Configuration lives in `~/.config/mejepa/daemon.toml` (or daemon args). Edits require daemon restart.

---

## 7. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Daemon won't start | D-drive missing | mount D-drive; verify with `ls /mnt/d/contextgraph/` |
| Daemon won't start | model file missing | check digest report; place model under `/mnt/d/cabdru/contextgraph/models/` |
| Daemon won't start | pid lock held | another instance running; `pkill -f context-graph-mcp.*--daemon` |
| Predictions never appear | subscriber lagged | `mcp_call mejepa_subscriber_status`; check shift-log dir; check tokio runtime |
| Predictions appear but agent doesn't see them | injection hook misconfigured | check `.claude/settings.json hooks.UserPromptSubmit`; test the script manually |
| `mejepa_record_agent_feedback` fails | invalid payload, missing prediction, or storage validation error | inspect the returned `MEJEPA_FEEDBACK_*` code and source-of-truth readback details; fix the payload or storage root |
| Sampling weight not bumping after surprise | scheduler not consuming queue | check active-learning tick is firing; check `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` |
| Disk filling fast | shift-log retention broken | check GC logs; manually run `mcp_call mejepa_gc_run` |
| Wrong predictions on Python tasks but Rust looks fine | per-language drift | `mejepa_ship_gate_status --by-language python`; consider per-language LoRA refresh |
| `nvidia-smi` reports N/A | WSL passthrough issue | per `TECH-OBSERVABILITY §5`: flagged not silently swapped |

---

## 8. Reference: every operator MCP tool

| Tool | When | Notes |
|---|---|---|
| `mejepa_daemon_status` | health check (daily) | shipped in Phase F |
| `mejepa_subscriber_status` | check subscriber | shipped |
| `mejepa_heal_status` | check drift / promotion state | shipped |
| `mejepa_quota_status` | disk usage | shipped + Phase A extension |
| `mejepa_vram_budget_report` | GPU usage | shipped |
| `mejepa_eval_run` | force a holdout-eval run | shipped |
| `mejepa_ship_gate_status` | per-(cat × lang) ship-gate | shipped |
| `mejepa_active_learning_queue` | queue contents | shipped |
| `mejepa_eval_build_graph` | rebuild patch-similarity graph | shipped |
| `mejepa_audit_pairwise_mi` | weekly MI audit | shipped |
| `mejepa_constellation_inspect` | TCT cell inspection / forced refit | shipped |
| `mejepa_gc_run` | force GC / eviction | shipped + Phase F extension |
| `mejepa_witness_compress` | force Merkle compression | shipped |
| `mejepa_promote_approval` | approve catastrophic promotions | NEW Phase E |
| `mejepa_rollback_to` | force rollback to prior model | shipped |
| `optimizer_witness_chain_verify` | integrity check | shipped |
| `optimizer_witness_chain_diff` | drift in past recommendations | shipped |
| `optimizer_witness_chain_repair_legacy` | force repair | shipped |
| `optimizer_recall_recommendations` | recent recommendations | shipped |

---

## 9. Reference: every operator CLI command

```bash
context-graph-cli mejepa active-learning list [--top-n N]
context-graph-cli mejepa active-learning label <id> <label> <reason>
context-graph-cli mejepa active-learning dismiss <id> <reason>
context-graph-cli mejepa hygiene cold-audit
context-graph-cli mejepa hygiene aggressive-cold-tier-down
context-graph-cli mejepa rollback --version <version>
context-graph-cli mejepa eval run --force
context-graph-cli mejepa eval ship-gate
context-graph-cli mejepa daemon status
context-graph-cli mejepa daemon restart
context-graph-cli mejepa daemon stop
```

These wrap the MCP tools for easier shell-side use.

---

## 10. Escalation path

If anything in §5 doesn't resolve within 1 day:

1. Open or update a high-severity GitHub issue with `status:blocked`, severity, reproducible scenario, and what's been tried.
2. If user-facing degradation: pause prediction emission via `mcp_call mejepa_pause_predictions --duration-mins 60`; the agent receives `Verdict::Abstain` until resumed.
3. Investigate root cause; fix; resume.

For systemic issues: file an issue at the project repo with all artifacts (daemon logs, training certs, the failing prediction's witness hash).
