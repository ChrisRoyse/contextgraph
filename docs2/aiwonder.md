# aiwonder — ContextGraph-Specific Operations

Privileged credentials are managed outside this repo (Infisical/operator env). Never write them to disk, paste them into docs, or echo them in logs.

> **Scope.** This doc covers **only what aiwonder hosts for ContextGraph / ME-JEPA-Code**. The full aiwonder system reference (Leapable marketplace, PostgreSQL, TEI services, secrets, networking) lives at `/home/cabdru/leapablememory/docs2/aiwonder-system.md`. Read that first if you've never touched the box.
>
> **Hard rules.** aiwonder is a **shared production server** for the Leapable platform. ContextGraph training **must coexist** with the resident leapable services without disrupting them. Every operation must follow the universal doctrine (`docs2/AICodingAgentSuperPrompt.md`) and the aiwonder rules (`leapablememory/docs2/aiwonder-system.md §8`).
>
> **Last verified against live host:** 2026-05-22 for ContextGraph storage topology and env/tooling paths. #372 is resolved: the canonical ContextGraph roots now mount from dedicated aiwonder ZFS datasets (`archive/contextgraph` and `hotpool/contextgraph`). Verify before relying on any claim.
>
> **EVERYTHING FOR THIS SYSTEM IS RUNNING AND SHOULD ALL BE RUNNING ON AIWONDER DATACENTER PC. everything gets stored there, all fsv ran there, whole system runs and works over there. ensure that is where all the work is happening, is on the aiwonder datacenter pc.
---

## 1. Why ContextGraph runs on aiwonder

ContextGraph training was originally on the operator's WSL2 box. Three structural problems forced the move:

1. **Local I/O bottleneck.** Local WSL2 storage is NTFS-on-VHDX with cross-FS translation overhead. Per-shard wall in the local corpus run was ~323 s, **of which ~106 s was unaccounted subprocess spawn + I/O overhead**.
2. **HD space limit.** The operator's local disk can't hold the full corpus reward field (~73 GB projected) + the Q3 SWE-bench Lite Python Docker images (~1-2 TB future requirement).
3. **No CUDA MPS in WSL2.** Multi-process kernel overlap on the GPU is not supported on WSL2 → parallel-isolated mode in the scheduler can't actually deliver kernel-level concurrency. aiwonder runs `nvidia-cuda-mps-control.service` → MPS is active → parallel-isolated **does** get true kernel overlap on aiwonder.

The aiwonder migration completed on 2026-05-17 (#296). The 2026-05-18 storage correction (#363), tightened by #412 on 2026-05-19, makes aiwonder the production durable source of truth for Phase G and later work.

---

## 2. Hardware on aiwonder (relevant subset)

| | |
|---|---|
| Host | `aiwonder` / `aiwonder.mst.com` (VPN-only, `68.171.3.249`) |
| OS / kernel | Ubuntu 26.04 LTS, kernel `7.0.0-15-generic` |
| CPU | Ryzen 9 9950X (16c/32t, 32 logical CPUs visible) |
| RAM | **121 GB** total, ~88 GB free at recon |
| GPU | **RTX 5090 Blackwell, 32607 MiB VRAM** (currently ~3.2 GB used by leapable-tei warm contexts; ~29.4 GB free) |
| GPU driver | `595.71.05` (slightly newer than local 596.21) |
| CUDA toolkit | **13.2.0** (verified via `docker run nvidia/cuda:13.2.0-base-ubuntu24.04 nvidia-smi`) |
| **MPS** | **`nvidia-cuda-mps-control.service` active** — concurrent CUDA processes overlap on the SMs (critical advantage over WSL) |
| Docker | 29.4.3, **default runtime `nvidia`** (no need for `--gpus all` flag) |
| Storage — `/zfs/hot` (NVMe, no redundancy) | 1.81 TB total, **1.76 TB free** at recon |
| Storage — `/zfs/archive` (HDD mirror) | 9.09 TB total, **8.96 TB free** at recon |
| Storage — `/` (root NVMe) | 1.83 TB total, ~1.69 TB free |

**Network:** VPN-only via Cisco Secure Client (operator). DNS + HTTPS egress to github.com, huggingface.co, ghcr.io all reachable (verified 2026-05-15).

**Resident services consuming GPU/CPU/RAM (must not disrupt):**

- `leapable-marketplace.service` — Bun API at `127.0.0.1:4000`
- `leapable-outbox-relay.service` — PostgreSQL → Redis BullMQ
- `leapable-postgres-exporter.service`, `leapable-pgbouncer-exporter.service` — Prometheus exporters
- `leapable-tei-general.service` — Docker `leapable-tei-general` (gte-multilingual-base, ~1 GB VRAM)
- `leapable-tei-legal.service` — Docker `leapable-tei-legal` (FreeLawProject ModernBERT, ~1 GB VRAM)
- `leapable-tei-reranker.service` — Docker `leapable-tei-reranker` (~1 GB VRAM)
- `nvidia-cuda-mps-control.service` — **MPS daemon** (do not disable)
- `dcgm-exporter` Docker — Prometheus GPU metrics

---

## 3. ContextGraph layout on aiwonder

**Isolation rule (operator directive 2026-05-15, reinforced by #363):** ContextGraph hot work lives under `/zfs/hot/contextgraph/`, and durable production state lives under `/zfs/archive/contextgraph/`. Nothing under `/zfs/hot/marketplace/`, `/zfs/hot/postgres/`, `/zfs/hot/models/`, `/opt/leapable/`, or any other leapable path is touched.

### 3.1 Filesystem layout

```text
/zfs/hot/contextgraph/                          (hot ZFS dataset: hotpool/contextgraph)
├── repo-current -> repo-current-<TS>-<agent>/   Active aiwonder mirror used by contextgraph-shell
├── repo/                                       Historical/alternate clone; do not assume active
│   ├── crates/                                 Rust workspace
│   ├── scripts/                                Python schedulers
│   └── target/release/                         Compiled Rust binaries (after cargo build --release)
├── models/                                     Embedder model artifacts (~6.5 GB total)
│   ├── e1-e5-large-v2/
│   ├── e7-qodo-embed-1.5b/
│   ├── e10-multilingual-e5-large/
│   ├── e12-colbert-v2/
│   ├── e13-splade-v3/
│   ├── e14-codet5p/
│   └── e17-roberta-affect/
├── corpus/                                     Inputs that don't change often
│   └── python-ast-chunks-v1/                   821 MB JSONL (transferred or regenerated)
├── runtime/                                    Active scheduler outputs
│   └── task-py-data-016-<TS>/scheduler.log
├── fsv/                                        FSV evidence
│   └── task-py-data-016-<TS>-fsv/
└── state/cgreality/                            Active runtime root pointer + shift logs

/zfs/archive/contextgraph/                      (durable ZFS dataset: archive/contextgraph)
├── corpus/                                     Generated cache, projected ~73 GB per run
│   └── python-forward-cache-v1-task-py-data-016-<TS>/
└── swe-bench/                                  Future: SWE-bench Lite Python Docker images (~1-2 TB)

/home/croyse/.cache/contextgraph/<run>/         Hot scratch on root NVMe (50 GB budget)
```

As of the 2026-05-22 #372 closeout, `/archive` and `/hotpool` are real ZFS roots and the ContextGraph logical roots resolve to dedicated ZFS datasets:

```bash
findmnt -T /zfs/archive/contextgraph -o TARGET,SOURCE,FSTYPE --noheadings
# /zfs/archive/contextgraph archive/contextgraph zfs

findmnt -T /zfs/hot/contextgraph -o TARGET,SOURCE,FSTYPE --noheadings
# /zfs/hot/contextgraph hotpool/contextgraph zfs
```

The accepted policy is not a path rewrite. Keep the canonical paths and mount dedicated datasets at those paths:

```text
archive/contextgraph  -> /zfs/archive/contextgraph
hotpool/contextgraph  -> /zfs/hot/contextgraph
```

The #372 migration retained ext4 backups and verified both roots with checksum dry-runs plus key artifact hash comparison:

```text
/zfs/archive/contextgraph.ext4-pre-zfs-20260522T114531Z-migrate
/zfs/hot/contextgraph.ext4-pre-zfs-20260522T114531Z-migrate

/tmp/contextgraph-zfs-root-migration/migrate-20260522T114531Z-migrate/
  archive-rsync-checksum-verify.txt       # 0 bytes
  hot-rsync-checksum-verify.txt           # 0 bytes
  key-artifacts.before.sha256
  key-artifacts.after.manual.sha256
  key-artifacts.manual.diff               # 0 bytes

/tmp/contextgraph-zfs-root-migration/audit-20260522T122116Z-post-manual/
  audit.txt                               # VERDICT: ContextGraph roots are on intended ZFS datasets.
```

Use the fail-loud runbook script for future audit/recovery. Do not rerun `migrate` on the current host unless deliberately rebuilding these roots:

```bash
# Read-only audit; exits 0 when both ContextGraph roots resolve to the intended ZFS datasets.
scripts/setup/aiwonder_contextgraph_zfs_roots.sh audit

# Root-only migration for a fresh/unresolved host only. Retains the ext4 source
# directories as backups and verifies the post-copy tree with rsync --checksum
# plus key artifact hashes.
I_UNDERSTAND_CONTEXTGRAPH_DOWNTIME=1 sudo -E \
  scripts/setup/aiwonder_contextgraph_zfs_roots.sh migrate
```

The hot-stage scratch (`/home/croyse/.cache/contextgraph/...`) replaces the WSL `~/.cache/contextgraph/` pattern. It must NOT exceed the documented hot-stage budget (`--hot-stage-budget-bytes 51539607552 ≈ 48 GB`).

### 3.2 GPU coexistence policy

**Total VRAM 32 607 MiB.**

| Workload | Peak VRAM |
|---|---|
| `leapable-tei-general` (gte-multilingual) | ~1 GB |
| `leapable-tei-legal` (ModernBERT legal) | ~1 GB |
| `leapable-tei-reranker` (gte-reranker) | ~1 GB |
| MPS context + driver overhead | ~0.2 GB |
| **Resident leapable VRAM ceiling** | **~3.5 GB** |
| ContextGraph all 12 active embedders loaded (E1/E6/E7/E8/E9/E10/E12/E13/E14 + E2/E3/E4 algorithmic + GPU buffers) | **~6.5 GB** if everything in-process |
| Per-isolated-subprocess peak (single embedder + CUDA ctx + activations) | ~3.5 GB |

**Budget:** with leapable holding ~3.5 GB and ContextGraph running parallel-isolated at 6 concurrent embedder subprocesses (`CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=6` × 3.5 GB = ~21 GB), total ~24.5 GB of 32 GB used. **~8 GB headroom** for buffers and any transient spikes. Safe.

**Rules:**
1. **Never stop, restart, or `docker rm` any `leapable-tei-*` container or `leapable-*` systemd unit.** They are warm on purpose. Disrupting them affects marketplace API customers.
2. **Set `--gpu-vram-guard-bytes` to ~28 GB** (`30064771072`) in the scheduler to leave headroom for leapable-tei in case any of them transiently spikes.
3. **MPS is enabled (`nvidia-cuda-mps-control.service`)** — ContextGraph processes automatically participate. Do not disable MPS; leapable-tei may rely on the kernel-overlap.
4. **If running adversarially-priced workloads** (e.g., a `cargo bench` or `cargo run --release` that pegs the GPU at 100% util), do it during marketplace off-peak hours. Default assumption: aiwonder is always handling marketplace API traffic.

### 3.3 SSH access from the operator's WSL

Per `~/.config/aiwonder.env`:

```bash
# Operator-side helper at /tmp/ssh-aiwonder/run-remote.sh runs a remote command
# non-interactively using the password from the env file (never logged).
# Use contextgraph-shell so the aiwonder env contract and active repo mirror are applied.
/tmp/ssh-aiwonder/run-remote.sh 'contextgraph-shell "cargo build --release -p context-graph-mejepa-embedders"'
```

The Cisco VPN must be active on Windows; WSL2 inherits the routes. Allowed UFW source subnet on aiwonder is the Cisco-assigned one — if it changes on reconnect, escalate per `leapablememory/docs2/aiwonder-system.md §3`.

### 3.4 Cargo build environment

The repo-level `.cargo/config.toml` is part of the aiwonder build contract. It pins native C++ build scripts to one job by default, pins CUDA 13.2 paths, and sets a non-forced RocksDB/GCC 15 shim:

```toml
CXXFLAGS = { value = "-include cstdint", force = false }
CXXFLAGS_x86_64_unknown_linux_gnu = { value = "-include cstdint", force = false }
```

Why: aiwonder currently exposes GCC 15.2.0 as `c++`. Fresh target builds of the vendored `librocksdb-sys` trace replay sources can fail before ContextGraph code compiles because RocksDB's trace headers use `uint32_t` / `uint64_t` before `<cstdint>` is visible to that translation unit. The shim is harmless for C++ builds, is not forced over an operator-provided `CXXFLAGS`, and should be removed once RocksDB or the dependency wrapper includes `<cstdint>` directly. Tracked by #525.

---

## 4. The corpus run on aiwonder (target architecture)

Goal: replicate the local-WSL corpus run with the speedups that aiwonder enables.

### 4.1 Speed expectations vs the local WSL baseline

| Factor | Local WSL | aiwonder | Speedup |
|---|---|---|---|
| Shard cold-start (subprocess spawn + model load) | ~106 s | ~50-80 s (no WSL FS tax + faster NVMe) | ~1.5× |
| Per-embedder GPU compute (e7 dominant) | sequential | **parallel via MPS** (4-6 wave) | ~3× |
| I/O (cache writes, FSV readback) | local WSL cross-FS | `/zfs/hot/` direct NVMe | ~2× |
| **Estimated per-shard wall (1024-chunk shards)** | ~1100 s | **~250-400 s** | **2.5-4.5×** |
| **Estimated full 391-shard run** | ~4.9 days | **~1.5-2 days** | **~2.5-3×** |

The exact numbers calibrate after the first 3-5 shards complete on aiwonder.

### 4.2 Scheduler invocation on aiwonder (verified 2026-05-15)

**Required environment contract (current):** source `/zfs/hot/contextgraph/env/contextgraph-aiwonder.env`, or run through `/zfs/hot/contextgraph/bin/contextgraph-shell`. Do not hand-type path roots unless an issue explicitly overrides a specific artifact path. The env contract sets canonical aliases for active data, labels, tensors, models, FSV output, CUDA, Cargo, `rg`, and `gh`.

```bash
contextgraph-shell 'printf "repo=%s\nchunks=%s\nlabels=%s\nfsv=%s\n" \
  "$CG_REPO" "$CG_AST_CHUNKS" "$CG_LABEL_ROOT" "$CG_FSV"'
```

High-value aliases currently expected inside `contextgraph-shell`:

| Alias | Meaning |
|---|---|
| `CG_REPO` | Active aiwonder source mirror, default `/zfs/hot/contextgraph/repo-current` |
| `CG_AST_CHUNKS` / `CG_AST_CHUNKS_DURABLE` | 399,777-row Python AST chunk JSONL under `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/`; active `absolute_path` / `dataschema` rows were canonicalized by #613 |
| `CG_FORWARD_CACHE` | Completed 391-shard forward cache under `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z` |
| `CG_FORWARD_CACHE_COMPLETENESS_ROOT` | Optional override for #614 slot-completeness masks; default `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1` |
| `CG_LABEL_ROOT` | Current live-split deterministic label/constellation root |
| `CG_TRAINING_ARRAY_ROOT` | #396 constellation training array artifacts |
| `CG_SPLIT_STABLE_TENSOR_ROOT` | Split-stable tensor root for slot/pairwise distributions |
| `CG_SHIP_GATE_FSV_ROOT` | Current ship-gate FSV root |
| `CG_MODELS` / `CONTEXTGRAPH_MEJEPA_MODELS_CONFIG` | aiwonder model root/config for real embedder execution |

As of #613, the canonical 300x8 AST source tree is:

```text
/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/
```

Direct readback at #613 closeout: `4224` source files, `148346301` bytes. The active `python_ast_chunks.jsonl` has `399777` rows with `0` `/mnt/d` or `/mnt/c` hits in `absolute_path` and `dataschema`; mounted-drive strings survive only in explicit `legacy_provenance_path` / `legacy_dataschema_path` fields.

As of #614, forward-cache slot completeness is explicit rather than assumed:

```text
/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl
```

Direct readback at #614 closeout: `399777` mask rows, `378444` complete 12-slot rows, `20706` partial rows, `627` zero-slot rows, `131552` quarantined missing chunk/embedder pairs, and `0` unaccounted missing pairs. Downstream pair/constellation mining must join this mask before claiming complete all-pair coverage.

`gh` and `rg` are installed in `/zfs/hot/contextgraph/bin` and visible through `contextgraph-shell`. `gh` authentication remains blocked by #562 until an operator-approved token path exists; do not move or print secrets to solve that.

**Scheduler invocation:**

```bash
contextgraph-shell '/zfs/hot/contextgraph/venv/bin/python3 \
  "$CG_REPO/scripts/swebench_lite_ops/python_forward_shard_scheduler.py" \
  --task-id TASK-PY-DATA-016-AIWONDER \
  --chunks-jsonl "$CG_AST_CHUNKS" \
  --cache-root "$CG_CORPUS/python-forward-cache-v1-aiwonder-<TS>" \
  --fsv-root "$CG_FSV/python-forward-cache-v1-aiwonder-<TS>-fsv" \
  --hot-root /home/croyse/.cache/contextgraph/aiwonder-<TS> \
  --shard-size 1024 \
  --shard-count 391 \
  --max-source-bytes 32768 \
  --true-batch-size 8 \
  --embedder-batch-sizes e1=32,e6=32,e7=32,e8=32,e9=128,e10=32,e12=32,e13=32,e14=8 \
  --embedder-max-padded-source-bytes e1=1048576,e6=1048576,e7=1048576,e8=1048576,e9=4194304,e10=1048576,e12=524288,e13=1048576,e14=262144 \
  --gpu-vram-guard-bytes 30064771072 \
  --io-workers 2 --cpu-workers 28 --gpu-workers 1 \
  --cache-store-layout packed --require-packed-store --stage-packed-shards-on-hot \
  --active-embedder-count 12 --estimated-record-bytes 16384 \
  --hot-stage-budget-bytes 51539607552 --fsv-edge-copy-budget-bytes 2147483648 \
  --resume-completed-shards --retry-failed-partial-shards \
  --isolate-embedder-processes --checkpoint-every-shards 1 \
  --cargo-runner "$CG_REPO/target/release/examples/python_forward_cache_fsv"'
```

**Use a non-`TASK-PY-DATA-016` task ID** to bypass the smoke-gate preflight (which expects WSL2-only artifacts). E.g., `TASK-PY-DATA-016-AIWONDER-SMOKE`. The smoke gate only fires when `task_id == "TASK-PY-DATA-016"`.

### 4.3 First-shard speedup data (2026-05-15)

The first aiwonder e1 subrun on shard 0 completed at **37 s for 1024 chunks** (smoke-test SoT FSV passing). The WSL baseline was **~14.6 s for 128 chunks** for the same e1 work. Per-chunk: **3.2× faster on aiwonder**. The `#308` one-shard smokes below are the current full-shard calibration data.

Issue `#308` fixed the parallel-isolated packed-record consolidation path and verified it on aiwonder with the release Rust runner:

| Mode | FSV root | Wall | Result |
|---|---|---:|---|
| Sequential regression | `/zfs/archive/contextgraph/fsv/aiwonder-308-fix-seq-regression-20260515T203800Z-fsv/` | 875.002 s | Pass; 11,880 cache rows, 408 quarantine rows, 12,288 accounted pairs; payload files byte-equal and packed indexes byte-equal after durable-root normalization vs pre-refactor sequential smoke |
| Parallel=4 | `/zfs/archive/contextgraph/fsv/aiwonder-308-fix-par4-20260515T204800Z-fsv/` | 253.622 s | Pass; 11,880 cache rows, 408 quarantine rows, 12 subrun logs, merged packed manifest from `isolated_subrun_snapshots`, 54.871% faster than the 561.994 s aiwonder sequential smoke |
| Parallel=6 | `/zfs/archive/contextgraph/fsv/aiwonder-308-fix-par6-20260515T205500Z-fsv/` | 198.924 s | Pass; 11,880 cache rows, 408 quarantine rows, 12 subrun logs, merged packed manifest from `isolated_subrun_snapshots`, 64.604% faster than the 561.994 s aiwonder sequential smoke |

`CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS > 1` now requires packed hot-stage isolation and the scheduler auto-sets `CG_MEJEPA_PY_FORWARD_SKIP_EXISTING_PACKED_READ=1` for each isolated embedder subprocess. That prevents parallel subruns from forward-copying stale durable `packed_records/`; the scheduler snapshots each subrun after lock-serialized promotion and then reconstructs the final packed manifest from those per-embedder snapshots.

Do **not** relaunch the production corpus run into parallel mode after a single smoke. The `#308` handoff requires parallel=6 to pass end-to-end at least three times on different shards before prod cutover.

### 4.4 Workflow for an AI agent picking this up

1. **Read this doc** + `leapablememory/docs2/aiwonder-system.md` + `docs/futurebuild/README.md` and `docs/futurebuild/08_roadmap_and_open_work.md`.
2. **VPN + SSH preflight** per `leapablememory/docs2/aiwonder-system.md §3`.
3. **Check the open migration issues** in this repo with labels `area:aiwonder` + `phase:migration`. Pick the lowest-numbered unclaimed one.
4. **Claim it per doctrine §3.4.** Do not touch leapable paths.
5. **Execute under FSV discipline.** Every state change → SoT readback. Every operation → remote command logged in the issue comment.
6. **Comment milestones** on the issue. Pause/Done/Blocked per doctrine §3.

---

## 5. Migration status snapshot

| Phase | Issue | Status |
|---|---|---|
| 0 — Preflight + recon | — | **complete 2026-05-15**: VPN ok, SSH ok, MPS confirmed, disk available, Docker + NVIDIA runtime verified |
| 1 — Isolated workspace + repo clone | #297 | complete |
| 2 — Rust toolchain + Python deps | #298, #299 | complete |
| 3 — Embedder model transfer | #300 | complete |
| 4 — AST chunks transfer (or regenerate) | #301 | complete |
| 5 — Fix parallel-isolated packed-record consolidation | #308 | **complete 2026-05-15**: sequential regression, parallel=4, and parallel=6 FSV smokes passed on aiwonder |
| 6 — Build release binaries on aiwonder | #303 | complete |
| 7 — Launch corpus run on aiwonder | #304 | complete |
| 8 — Calibration: WSL baseline vs aiwonder | #305 | complete |
| 9 — Cutover decision after calibration | #296, #363 | complete: production durable SoT is aiwonder `/zfs/archive/contextgraph/...` |
| Future — SWE-bench Lite Python Docker images | #306 | not yet needed for embedding pipeline |

Current corrective work after the #363 storage decision:

- #365 adds fail-closed production-root guards.
- #366 reruns TASK-PY-G-056 on aiwonder.
- #367 audits project-ingest/runtime locality.
- #368 audits aiwonder capability utilization.
- #372 is resolved: live readback shows `/zfs/archive/contextgraph` and `/zfs/hot/contextgraph` resolve to the intended `archive/contextgraph` and `hotpool/contextgraph` ZFS datasets. Use `scripts/setup/aiwonder_contextgraph_zfs_roots.sh audit` for future verification.
- #562 owns aiwonder-native GitHub/search tooling. `rg` and `gh` are installed in the aiwonder env; `gh auth` is blocked until an operator-approved credential path exists.

---

## 6. Hard rules (ContextGraph on aiwonder)

1. **Never touch leapable paths.** `/zfs/hot/marketplace/`, `/zfs/hot/postgres/`, `/zfs/hot/redis/`, `/zfs/hot/models/`, `/opt/leapable/`, `/etc/leapable/`, `/run/leapable/` are off-limits unless explicitly redirected.
2. **Never stop/restart leapable-* services or leapable-tei-* containers.** Marketplace traffic is live.
3. **Never disable MPS.** `nvidia-cuda-mps-control.service` is load-bearing for the leapable-tei warm contexts and our own parallel-isolated mode benefits from it.
4. **Never write secrets into this repo.** Use Infisical CLI or env-var names; see `leapablememory/docs2/infisical-secrets-guide.md §0`.
5. **`/zfs/hot/` has no redundancy** (single NVMe). The corpus run's checkpoint + cache should go to `/zfs/archive/` (HDD mirror) for durability; only the in-flight hot-stage scratch should live on `/zfs/hot/contextgraph/` or `/home/croyse/.cache/`.
6. **Production SoT is aiwonder.** Durable production corpus, runtime, project, cache, export, and FSV outputs go under `/zfs/archive/contextgraph/...`; hot staging goes under `/zfs/hot/contextgraph/...` or explicitly budgeted aiwonder scratch. Local paths and mounted drives are control-plane or legacy provenance only.
7. **Use `contextgraph-shell` for aiwonder commands.** It sources `/zfs/hot/contextgraph/env/contextgraph-aiwonder.env`, enters the active `repo-current` mirror, and prepends the aiwonder `rg`/`gh`/Cargo/CUDA paths.
8. **Verify by SoT readback.** Per doctrine §4. Remote `systemctl is-active`, `du -sh`, `df -B1`, file hashes, RocksDB read-only reopen — not just SSH return codes.
9. **Comment on the relevant GitHub Issue at every milestone.** Per doctrine §3.5.

---

## 7. Hand-off and resumption

If a session ends or the agent crashes mid-migration:

1. Next agent reads this file.
2. Then reads the highest-numbered comment on the active GitHub Issue with label `area:aiwonder + status:in-progress`.
3. Then does the SSH preflight from `leapablememory/docs2/aiwonder-system.md §3`.
4. Then resumes at the "Resume at" pointer in the PAUSE comment.

If the local WSL corpus run is still alive, do not kill it — the calibration baseline is more valuable than the few hours of saved compute.

---

## 8. References

- **Master aiwonder doc** (host architecture, services, secrets, gotchas): `/home/cabdru/leapablememory/docs2/aiwonder-system.md`
- **Infisical secrets guide**: `/home/cabdru/leapablememory/docs2/infisical-secrets-guide.md`
- **ContextGraph project doctrine**: `/home/cabdru/contextgraph/CLAUDE.md`
- **Universal coding doctrine**: `/home/cabdru/contextgraph/docs2/AICodingAgentSuperPrompt.md`
- **Phase G roadmap / open work**: `/home/cabdru/contextgraph/docs/futurebuild/08_roadmap_and_open_work.md`
- **Scheduler source**: `/home/cabdru/contextgraph/scripts/swebench_lite_ops/python_forward_shard_scheduler.py`
- **Parallel-isolated patch + race bug**: commit `4cd003e1`
