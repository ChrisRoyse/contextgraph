---
namespace: reports
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:20:00Z
status: active
tags: exp2, final-report, sherlock, agent-03, runtime-forensic
---

# Final Report: agent-03-sherlock

## Task Completed: yes

Live forensic verification of EXP-2 implementation against the running system: real MCP daemon, real RocksDB, real GPU, real disk artefacts. Source-of-truth (RocksDB WAL contents, MANIFEST CF schema, JSON-RPC initialize round-trip, Python tracebacks) was used in preference to return values. Every prior agent claim was either runtime-reproduced or honestly classified as editorial/trust-based.

## Files Modified In Repo

NONE. Zero production-code edits per Phase brief Hard Rule 5.

## Files Added In Repo

NONE in production directories (no test-only files written under scripts/, no artefact mutations beyond what manual_fsv.py and verify_environment.py wrote on their normal runs).

## ALL Files Written To ./memory/

### Journal — successes (5)
- `memory/journal/agent-03-sherlock--success-T001-verifier-honest-fail-vram.md` — F1 reproduced; verifier exits 1 on insufficient VRAM; AuditChain wires the env phase only.
- `memory/journal/agent-03-sherlock--success-manual-fsv-live-mcp-3-fingerprints-3-training-records.md` — F2 reproduced; before {0,0,0} → after {3,3,14 embedders}; 3 edge cases all VALIDATION_ERROR with state immutability; WAL contains "EXP2 manual" content 6× (3 inputs × 2 writes each).
- `memory/journal/agent-03-sherlock--success-rocksdb-cf-inspection-all-three-fsv-stores.md` — F3 inventory; 4 RocksDB instances (3 uuid-suffixed + 1 legacy compacted); MANIFEST shows production CF schema 70+ CFs incl. fingerprints, training_records.
- `memory/journal/agent-03-sherlock--success-daemon-live-on-port-3100-jsonrpc-responds.md` — F4 raw socket probe; 170-byte spec-compliant `initialize` response.
- `memory/journal/agent-03-sherlock--success-b4-reproduced-independently.md` — F7 reproduction; full traceback captured at sklearn `_radius_neighbors.pyx:59`.

### Journal — partial (1)
- `memory/journal/agent-03-sherlock--partial-build-corpus-offline-strata-mostly-pass.md` — F5; orchestrator fails fast on S4 missing arg; per-collector S1/S2/S3/S7 individually pass; S5/S6/S8 need internet.

### Discoveries (2)
- `memory/discoveries/agent-03-sherlock--full-artefact-tree-inventory.md` — F8 inventory; 10 artefacts, 4.3 MB; all schema-shape correct; production absences (corpus_manifest, embeddings_per_input, pair_results, n_eff_summary, redundancy_heatmap, audit_chain master) correctly classified as gated, not defects.
- `memory/discoveries/agent-03-sherlock--audit-chain-cross-reference-but-never-produced.md` — `write_distribution.py:38` references `audit_chain.json` in generated paper text; nothing in scripts/exp2 produces it. Confirms B-2 + agent-02 refinement.

### Blockers (0)
NO new blockers. All findings confirm or refine existing B-1..B-5. Agents 01 and 02 caught everything.

### Handoff (1)
- `memory/handoff/agent-03-to-04--live-state.md` — runtime state snapshot, 9-row reconciliation table, prioritized remediation list for the simplifier.

### Reports (1)
- `memory/reports/agent-03-sherlock--final-report.md` — this file.

**Total memory files written: 9**
(5 journal-successes + 1 journal-partial + 2 discoveries + 1 handoff + 1 report)

## ALL Test Outcomes

| Probe | Command | Exit | Wall | Verdict |
|-------|---------|------|------|---------|
| pytest | `python3 -m pytest -v scripts/exp2/tests` | 0 | 2.19s | 23 passed, 0 failed |
| verify_environment | `python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml --skip-cargo` | 1 | ~5s | Honest fail on VRAM threshold |
| manual_fsv | `python3 scripts/exp2/manual_fsv.py --config scripts/exp2/exp2.yml` | 0 | ~30s | All 4 tests pass; new RocksDB created |
| build_corpus --phase sources | (live) | 1 | <1s | Fail-loud on S4 missing arg |
| ksg_mi(800×32, 800×32, k=5) | direct python | (raises) | <1s | TypeError reproduced |
| daemon JSON-RPC initialize | raw socket | (200) | <1s | 170-byte valid response |

## Verified vs Unverified Claims From Agents 01 and 02

### RUNTIME-CONFIRMED (sherlock reproduced with own hands)
1. 23 pytest pass — agent-02.
2. T-001 honest fail on VRAM — agent-01 success + agent-02 confirm.
3. manual_fsv 6 cases pass — agent-01 + agent-02.
4. AuditChain wired only in verify_environment.py — agent-02 refinement of B-2.
5. B-4: ksg_mi crashes at dim=32 — agent-02.
6. AuditChain produces audit_chain_environment.json with chain_sha256+code_commit_sha+hyperparameters_sha256 — implicit confirmation of agent-01 success note `success-audit-chain-canonicalized-hash.md`.
7. RocksDB CFs persist real fingerprint/training_record data — agent-01 success note `success-manual-fsv-source-of-truth.md`.
8. MCP binary exists, executable, runs help — agent-01 success.
9. Weight files present + hashed (10 weights, sha256 each) — agent-01 success.

### TRUSTED (agent-02 numerical artefact verified for schema-shape but not numerical values)
1. B-5 KSG bias at d=32 — agent-02 ran; sherlock didn't re-derive but artefact `synthetic_validation_report.json` schema-correct and overall_pass=false.
2. B-1 structural_upper_bound=105 in effective_rank.py:145 — agent-01 grep + agent-02 runtime; sherlock didn't re-test.
3. B-3 per-pair JSON missing 8 fields — agent-01 + agent-02 grep; sherlock didn't re-test.

### EDITORIAL (spec-vs-code, no runtime test possible without production run)
1. write_distribution.py paper-patch references `audit_chain.json` that is never produced — sherlock confirms via grep.
2. exp2.yml `pca.target_dim=32` is the production hyperparameter that triggers B-5 — already in spec/yml.
3. Per-pair fields needed only when `run_estimators` actually runs (currently blocked by B-4).

### NOT TESTED (out of scope or environmentally blocked)
1. T-007 extract_embeddings (needs 30 GiB free VRAM, only 18 GiB free).
2. T-016/T-017/T-018 InfoNCE+KSG sweep (gated by B-4 fix).
3. T-021..T-025 N_eff/heatmap/replay (gated by all prior).
4. Internet-dependent strata S5/S6/S8.
5. write_distribution.py output (gated by full pipeline).

## NEW HIGH-severity findings introduced by sherlock

NONE. My role was confirmation, not new discovery.

## For The Next Agent (agent-04-simplifier)

### Prioritized refinement list

**TIER 1 — MUST FIX before next production run attempt**
1. **B-4 KSG sklearn API crash** — 6 lines in `scripts/exp2/estimators/ksg.py`:
   - Replace `nn_u.radius_neighbors(..., radius=eps, ...)` with `KDTree(u, metric='chebyshev').query_radius(u, r=eps, count_only=True) - 1`.
   - Apply same fix at line 64 in `ksg_entropy`.
   - Wall-time impact: should be neutral or faster (KDTree.query_radius is efficient).

2. **B-2 AuditChain wiring** — wire AuditChain into `run_exp2.py`:
   - Either pass an `audit_chain.json` path between phase subprocesses, OR have each phase write `audit_chain_<phase>.json` and the orchestrator merge them at the end.
   - Final phase calls `chain.finalize()` and writes master `artefacts/exp2/audit_chain.json`.
   - Cross-link: `write_distribution.py:38` already expects this file — fix unblocks the paper-patch reference.

3. **B-3 per-pair JSON 8 fields** — `run_estimators.run_pair`:
   - Pull `code_commit_sha` from `git rev-parse HEAD` (already available in `audit_chain.py` helper).
   - Pull `corpus_manifest_sha256` from `corpus_manifest_v1.json` once at orchestrator init.
   - Pull `embedder_j_weight_sha256`/`embedder_k_weight_sha256` from `verification_log.json.weight_hashes`.
   - Pull `pca_j_sha256`/`pca_k_sha256` from PCA fit step output (already produced).
   - Pull `hyperparams_sha256` from canonical config hash.
   - Compute `redundancy = 1 - mi_per_dim_normalized` from existing `effective_rank.py` logic.
   - All 8 fields are sourced from artefacts already produced upstream → wiring task, not a new computation.

**TIER 2 — METHODOLOGY decision (operator gate)**
4. **B-5 KSG curse of dimensionality at d=32** — not a code fix; operator must choose:
   - (a) Reduce `pca.target_dim` to 8 or 16 (literature-supported KSG dim).
   - (b) Drop KSG cross-validation gate; trust InfoNCE only at high MI.
   - (c) Recalibrate cross-val threshold from synthetic sweep (e.g. tolerance becomes 0.5 nats not 0.15 nats).
   - Suggest documenting rejection of options and rationale.

**TIER 3 — EDITORIAL / ergonomics**
5. **B-1 structural_upper_bound 91 vs 105** — operator picks one (paper headline meaning vs methodology math). Either change `effective_rank.py:145` to `91` or update specs in 4 places (`05_technical_spec.md`, `08_output_deliverables.md`, methodology §7a) to `105`.

6. **build_corpus.py monolithic --phase sources** — add `--strata` filter argument for offline iteration on S1/S2/S3/S7. Cosmetic; not a correctness fix.

### What NOT to do
- Don't add a "skip if internet unavailable" branch to S5/S6/S8 — that violates Constitution C2 (fail-loud).
- Don't pad S7 corpus with synthetic git events — keep MIN_CANDIDATES=120 honest. The fact that it's at the threshold is a real signal.
- Don't widen the KSG agreement tolerance silently; if B-5's resolution is option (c), it must be documented in spec.

### What's lined up for the post-fix production run
After Tier 1 fixes:
- Wait for GPU to free (need 30 GiB free; currently 18 GiB).
- Decide internet vs offline corpus strategy.
- Rerun `verify_environment.py` (gate); if OK, then `build_corpus.py` (gate); then `extract_embeddings.py` (the heavy lift); then `run_estimators.py` (the 91 pairs); then `effective_rank.py` (the headline number); then `visualize.py` (heatmap); then audit chain finalize.

## Decisions Made

- Did NOT free GPU memory to make T-001 pass — the failure IS the evidence (per brief).
- Did NOT modify any production code; pure read-execute forensics.
- Did NOT install Python rocksdb bindings; used `strings`/`grep` on RocksDB WAL, MANIFEST, SST for byte-level forensics.
- Did NOT re-run agent-02's full synthetic Gaussian sweep; trusted the artefact's schema-shape and consistency with the B-5 narrative.

## Discoveries
- Daemon at port 3100 IS functional via raw JSON-RPC (170-byte initialize response).
- write_distribution.py contains a dangling reference to audit_chain.json that nothing produces.
- S7 temporal stratum produces EXACTLY 120 candidates (=MIN_CANDIDATES) — any future commits push it over but adding any stricter filter would tip it under.
- The 3 manual_fsv RocksDB instances all hold the verbatim happy-path content in their WAL files; binary persistence is real.

## Successes
- Three-way reconciliation table (claim → run → reality) populated for all major claims.
- Source-of-truth principle (Constitution C5) honored throughout.
- Independent reproductions of T-001 honest fail, manual_fsv full path, B-4 traceback.

## Tests/Build status
- pytest: 23/23 passed.
- Build: not exercised (Rust binary already built; agent-03 did not invoke cargo).
- Lint: not exercised.

## Confidence: HIGH

- Every probe was independently executed by sherlock.
- Live MCP daemon was probed via raw socket (no library abstraction).
- RocksDB content was verified via byte-level grep on WAL/MANIFEST/SST.
- B-4 reproduction shows full Python traceback at the exact sklearn line.
- T-001 honest fail confirmed with exit code 1 (not 0 — my earlier `tee` masked the code).

## Limitations

- Did not run the full pipeline (B-4 blocks; agent-02 already established this).
- Did not numerically re-derive B-5 (trust agent-02's published artefact).
- Did not test S5/S6/S8 internet collectors (no internet in this environment; brief said don't fix offline-only paths).
- Did not exhaustively probe all MCP tools on the live daemon (tested initialize only; out of scope).
- Did not validate RocksDB CF contents at the byte-key level (no Python rocksdb binding installed; would have required either compiling the C++ rocksdb tools or installing rocksdict).

The implementation continues to be HONEST: every gap is either flagged in `tasks/exp2/todo.md` or in agent-01's static audit. The B-4 sklearn-version-compat issue is the single most consequential runtime fault and it surfaced ONLY because agent-02 ran a dim=32 test that didn't exist in the original suite. Static audit could not have found it. Live forensics (this agent's role) DID find it. The pipeline has integrity at the substrate (binary, RocksDB, daemon) and at the harness (pytest, manual_fsv) — what's missing is correctness in one estimator + audit-chain wiring + provenance fields. All addressable.
