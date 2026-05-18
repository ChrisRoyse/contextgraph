# 16 — Storage Location Policy: aiwonder Production SoT

> **Current locked decision:** GitHub issue #363, filed 2026-05-18, supersedes the old D-drive production policy for post-migration ContextGraph work. The old WSL/D-drive policy (#212) remains historical for pre-migration local runs.
>
> **Mandatory read:** any task that adds corpus, runtime, project-ingest, cache, model-output, export, or FSV artifacts must apply this policy before it writes bytes.

---

## 1. The Current Rule

**Post-migration production artifacts live on aiwonder.** Durable state goes under `/zfs/archive/contextgraph/...`. Hot high-churn state goes under `/zfs/hot/contextgraph/...` or an explicitly budgeted aiwonder scratch path. Local `/mnt/d/contextgraph/...` is legacy/local evidence unless a GitHub issue explicitly scopes a local-only smoke.

This means a current production issue is not resolved by a local D-drive FSV path. If a task still cites `/mnt/d/contextgraph/...`, either:

1. classify it as historical/local evidence;
2. rerun it on aiwonder and cite the `/zfs/archive/contextgraph/...` replacement; or
3. explicitly state that the issue is a local-only smoke and not production proof.

#369 tracks the broad revalidation sweep for existing post-migration `/mnt/d` artifact references.

---

## 2. aiwonder Path Mapping

| Data class | Production path | Notes |
|---|---|---|
| Durable corpus/cache/fingerprint artifacts | `/zfs/archive/contextgraph/corpus/<corpus-id>/` | Includes forward-cache, fingerprint seed/calibration, adversarial/synthetic corpora when used as production evidence |
| Durable FSV evidence | `/zfs/archive/contextgraph/fsv/<task-slug>-fsv/` | Required for post-migration production closeout |
| Durable runtime evidence | `/zfs/archive/contextgraph/runtime/` | Shift logs, test outcomes, live-session traces, hook capture, durable runtime JSONL |
| Project-ingest outputs | `/zfs/archive/contextgraph/projects/<project_id>/` | Cache, panels, predictions, reports, project-local RocksDB stores |
| Durable exports | `/zfs/archive/contextgraph/exports/` | Weekly eval reports, operator exports, acceptance bundles |
| Durable state | `/zfs/archive/contextgraph/state/` | Production state that must survive reboot/restart |
| Hot repo/build root | `/zfs/hot/contextgraph/repo/` | aiwonder Git checkout and release binaries |
| Hot stable inputs/models | `/zfs/hot/contextgraph/{corpus,models}/` | Inputs that benefit from NVMe read locality |
| Hot runtime/logs | `/zfs/hot/contextgraph/runtime/` | Active scheduler logs and in-flight run state |
| Explicit scratch | `/home/croyse/.cache/contextgraph/<run>/` or issue-approved equivalent | Must have byte budget and cleanup/readback plan |

---

## 3. Legacy Local D-Drive Policy

The pre-migration rule was: durable bulk state under `/mnt/d/contextgraph/`, hot scratch under WSL ext4, and no C-drive hot path. That rule was correct for the local WSL era and is preserved by #212 as historical context.

After #296 completed the aiwonder migration and #363 locked the 2026-05-18 operator correction, D-drive paths mean one of these:

- historical evidence from pre-migration tasks;
- local calibration/smoke evidence;
- stale documentation that needs #364/#369 cleanup;
- an explicitly issue-scoped local smoke.

They do **not** mean current production source of truth by default.

---

## 4. Enforcement Expectations

Production writers and FSV harnesses should fail closed when a production durable root points at local `/mnt/d/contextgraph/...`. #365 owns the implementation guard. The intended structured error is `MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER` or the closest crate-local equivalent.

Required production checks:

1. Durable root starts with `/zfs/archive/contextgraph/`.
2. Hot root starts with `/zfs/hot/contextgraph/` or an explicitly budgeted aiwonder scratch path.
3. `df -B1`/capacity evidence is read from aiwonder mount points.
4. Physical readback proves the artifact exists at the aiwonder path.
5. RocksDB writes are reopened read-only and decoded from the aiwonder artifact path.
6. Local `/mnt/d` writes are rejected in production mode unless the issue explicitly names a local-only smoke.

---

## 5. aiwonder Coexistence Rules

ContextGraph runs on a shared production server. It must not disrupt resident Leapable services.

- Do not touch `/zfs/hot/marketplace/`, `/zfs/hot/postgres/`, `/zfs/hot/models/`, `/opt/leapable/`, `/etc/leapable/`, or `/run/leapable/`.
- Do not stop or restart `leapable-*` services or `leapable-tei-*` containers.
- Do not disable `nvidia-cuda-mps-control.service`.
- Keep ContextGraph hot work under `/zfs/hot/contextgraph/`.
- Keep ContextGraph durable work under `/zfs/archive/contextgraph/`.

Operational details live in `docs2/aiwonder.md`.

---

## 6. What Counts as Done

A post-migration production task is done only when the RESOLVED issue comment names:

- trigger, process, and outcome;
- exact aiwonder SoT path/CF/key;
- SoT before and after values;
- happy path plus at least three edge cases;
- direct file/CF/database/readback proof from aiwonder;
- sha256 for final JSON/manifest artifacts where applicable;
- residual local `/mnt/d` evidence classified as historical/local or replaced.

Passing tests, return values, local output, or local D-drive artifacts are not enough for production closeout.

---

## 7. Related Issues

- #363 — binding post-migration aiwonder storage decision.
- #296 — completed aiwonder migration EPIC.
- #212 — superseded WSL/D-drive policy for pre-migration local work.
- #364 — docs/context correction for this policy.
- #365 — fail-closed production-root guard implementation.
- #369 — classify/revalidate post-migration FSV artifacts that still cite `/mnt/d`.
