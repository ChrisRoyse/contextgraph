# aiwonder — ContextGraph-Specific Operations

> **Scope.** This doc covers **only what aiwonder hosts for ContextGraph / ME-JEPA-Code**. The full aiwonder system reference (Leapable marketplace, PostgreSQL, TEI services, secrets, networking) lives at `/home/cabdru/leapablememory/docs2/aiwonder-system.md`. Read that first if you've never touched the box.
>
> **Hard rules.** aiwonder is a **shared production server** for the Leapable platform. ContextGraph training **must coexist** with the resident leapable services without disrupting them. Every operation must follow the universal doctrine (`docs2/AICodingAgentSuperPrompt.md`) and the aiwonder rules (`leapablememory/docs2/aiwonder-system.md §8`).
>
> **Last verified against live host:** 2026-05-15. Verify before relying on any claim.

---

## 1. Why ContextGraph runs on aiwonder

ContextGraph training was originally on the operator's WSL2 box (`/mnt/d/contextgraph/...`). Three structural problems forced the move:

1. **D-drive I/O bottleneck.** `/mnt/d/` on WSL2 is NTFS-on-VHDX with cross-FS translation overhead. Per-shard wall in the local corpus run was ~323 s, **of which ~106 s was unaccounted subprocess spawn + I/O overhead**.
2. **HD space limit.** The operator's local disk can't hold the full corpus reward field (~73 GB projected) + the Q3 SWE-bench Lite Python Docker images (~1-2 TB future requirement).
3. **No CUDA MPS in WSL2.** Multi-process kernel overlap on the GPU is not supported on WSL2 → parallel-isolated mode in the scheduler can't actually deliver kernel-level concurrency. aiwonder runs `nvidia-cuda-mps-control.service` → MPS is active → parallel-isolated **does** get true kernel overlap on aiwonder.

The aiwonder migration is in flight as of 2026-05-15. The local WSL run on `cg_pydata016_full_shardsize-1024-20260515-173510Z` remains alive as a **calibration baseline** for comparing per-shard wall once aiwonder is producing real data.

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

**Isolation rule (operator directive 2026-05-15):** ContextGraph work lives entirely under `/zfs/hot/contextgraph/`. Nothing under `/zfs/hot/marketplace/`, `/zfs/hot/postgres/`, `/zfs/hot/models/`, `/opt/leapable/`, or any other leapable path is touched.

### 3.1 Filesystem layout

```text
/zfs/hot/contextgraph/                          (NVMe — fast random I/O)
├── repo/                                       Git clone of github.com/ChrisRoyse/contextgraph
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

/zfs/archive/contextgraph/                      (HDD mirror — bulk / cold)
├── corpus/                                     Generated cache, projected ~73 GB per run
│   └── python-forward-cache-v1-task-py-data-016-<TS>/
└── swe-bench/                                  Future: SWE-bench Lite Python Docker images (~1-2 TB)

/home/croyse/.cache/contextgraph/<run>/         Hot scratch on root NVMe (50 GB budget)
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
/tmp/ssh-aiwonder/run-remote.sh 'cd /zfs/hot/contextgraph/repo && cargo build --release -p context-graph-mejepa-embedders'
```

The Cisco VPN must be active on Windows; WSL2 inherits the routes. Allowed UFW source subnet on aiwonder is the Cisco-assigned one — if it changes on reconnect, escalate per `leapablememory/docs2/aiwonder-system.md §3`.

---

## 4. The corpus run on aiwonder (target architecture)

Goal: replicate the local-WSL corpus run with the speedups that aiwonder enables.

### 4.1 Speed expectations vs the local WSL baseline

| Factor | Local WSL | aiwonder | Speedup |
|---|---|---|---|
| Shard cold-start (subprocess spawn + model load) | ~106 s | ~50-80 s (no WSL FS tax + faster NVMe) | ~1.5× |
| Per-embedder GPU compute (e7 dominant) | sequential | **parallel via MPS** (4-6 wave) | ~3× |
| I/O (cache writes, FSV readback) | `/mnt/d/` cross-FS | `/zfs/hot/` direct NVMe | ~2× |
| **Estimated per-shard wall (1024-chunk shards)** | ~1100 s | **~250-400 s** | **2.5-4.5×** |
| **Estimated full 391-shard run** | ~4.9 days | **~1.5-2 days** | **~2.5-3×** |

The exact numbers calibrate after the first 3-5 shards complete on aiwonder.

### 4.2 Scheduler invocation on aiwonder (target)

```bash
export CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=6   # MPS enables true kernel overlap
python3 /zfs/hot/contextgraph/repo/scripts/swebench_lite_ops/python_forward_shard_scheduler.py \
  --task-id TASK-PY-DATA-016 \
  --chunks-jsonl /zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl \
  --cache-root /zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-<TS> \
  --fsv-root /zfs/archive/contextgraph/fsv/python-forward-cache-v1-aiwonder-<TS>-fsv \
  --hot-root /home/croyse/.cache/contextgraph/<TS> \
  --shard-size 1024 \
  --shard-count 391 \
  --max-source-bytes 32768 \
  --true-batch-size 16 \
  --embedder-batch-sizes e1=64,e6=32,e7=64,e8=32,e9=128,e10=64,e12=32,e13=32,e14=16 \
  --gpu-vram-guard-bytes 30064771072 \
  --io-workers 4 \
  --cpu-workers 28 \
  --gpu-workers 1 \
  --cache-store-layout packed \
  --require-packed-store \
  --stage-packed-shards-on-hot \
  --active-embedder-count 12 \
  --hot-stage-budget-bytes 51539607552 \
  --resume-completed-shards \
  --retry-failed-partial-shards \
  --isolate-embedder-processes \
  --checkpoint-every-shards 1 \
  --cargo-runner /zfs/hot/contextgraph/repo/target/release/examples/python_forward_cache_fsv
```

**Note:** `promote_tree_verified` byte-count race in parallel-isolated mode (committed in `4cd003e1`) must be fixed before `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS > 1` is used. Without the fix, MPS does us no good. **Filing this as a blocker GitHub issue.**

### 4.3 Workflow for an AI agent picking this up

1. **Read this doc** + `leapablememory/docs2/aiwonder-system.md` + `docs/futurebuild/00_INDEX.md`.
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
| 1 — Isolated workspace + repo clone | TBD on filing | pending |
| 2 — Rust toolchain + Python deps | TBD | pending |
| 3 — Embedder model transfer | TBD | pending |
| 4 — AST chunks transfer (or regenerate) | TBD | pending |
| 5 — Fix `promote_tree_verified` race for parallel-isolated | TBD | **BLOCKER** |
| 6 — Build release binaries on aiwonder | TBD | pending |
| 7 — Launch corpus run on aiwonder | TBD | pending |
| 8 — Calibration: WSL baseline vs aiwonder | TBD | pending |
| 9 — Cutover decision after calibration | TBD | pending |
| Future — SWE-bench Lite Python Docker images | TBD | not yet needed for embedding pipeline |

---

## 6. Hard rules (ContextGraph on aiwonder)

1. **Never touch leapable paths.** `/zfs/hot/marketplace/`, `/zfs/hot/postgres/`, `/zfs/hot/redis/`, `/zfs/hot/models/`, `/opt/leapable/`, `/etc/leapable/`, `/run/leapable/` are off-limits unless explicitly redirected.
2. **Never stop/restart leapable-* services or leapable-tei-* containers.** Marketplace traffic is live.
3. **Never disable MPS.** `nvidia-cuda-mps-control.service` is load-bearing for the leapable-tei warm contexts and our own parallel-isolated mode benefits from it.
4. **Never write secrets into this repo.** Use Infisical CLI or env-var names; see `leapablememory/docs2/infisical-secrets-guide.md §0`.
5. **`/zfs/hot/` has no redundancy** (single NVMe). The corpus run's checkpoint + cache should go to `/zfs/archive/` (HDD mirror) for durability; only the in-flight hot-stage scratch should live on `/zfs/hot/contextgraph/` or `/home/croyse/.cache/`.
6. **Verify by SoT readback.** Per doctrine §4. Remote `systemctl is-active`, `du -sh`, `df -B1`, file hashes, RocksDB read-only reopen — not just SSH return codes.
7. **Comment on the relevant GitHub Issue at every milestone.** Per doctrine §3.5.

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
- **Phase G roadmap**: `/home/cabdru/contextgraph/docs/futurebuild/27_PHASE_G_ROADMAP.md`
- **Scheduler source**: `/home/cabdru/contextgraph/scripts/swebench_lite_ops/python_forward_shard_scheduler.py`
- **Parallel-isolated patch + race bug**: commit `4cd003e1`
