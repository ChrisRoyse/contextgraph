# 24 — Security + Adversarial Robustness

Source files covered: `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md`, `docs/ruvectorfindings/specs/constitution.md §security_requirements`, `CLAUDE.md §7-§8`.

---

## 1. Security rules (immutable)

| Rule | Source | Behavior |
|---|---|---|
| `SEC-MEJEPA-01` | constitution | Validate all MCP tool inputs at boundary (panel dim, finite floats, file path canonicalization) |
| `SEC-MEJEPA-02` | constitution | Embedder weights load from pinned SHA-256 digests; refuse on mismatch |
| `SEC-MEJEPA-03` | constitution | Witness-chain entries signed with ed25519 keypair in operator-controlled keyring; private key never leaves operator hardware |
| `SEC-MEJEPA-04` | constitution | Constellation centroids loaded with `corpus_sha + frozen_at`; refuse stale > 90 days without operator override |
| `SEC-MEJEPA-05` | constitution | Docker oracle invocations sanitize task_id + run_id (Docker-name-safe regex; reject `+`, spaces, control chars) |
| `SEC-MEJEPA-06` | constitution | No prompt injection from training corpus into MCP tool responses; CoT artifacts encoded as opaque embeddings via E17, never echoed verbatim |
| `SEC-MEJEPA-07` | constitution | FSV evidence files mode 0o600; never world-readable |

---

## 2. CLAUDE.md path discipline

| Rule | Source |
|---|---|
| Never commit secrets / credentials / `.env` files | `CLAUDE.md §7` |
| Validate input at system boundaries | `CLAUDE.md §7` |
| Sanitize file paths to prevent directory traversal | `CLAUDE.md §7` |
| Path-policy gate REMOVED 2026-05-07 (operator directive) | `CLAUDE.md §8` — path discipline is now BEHAVIORAL |
| Never edit: `.env*`, `swebench/**`, `tests/oracle_**/**`, `crates/context-graph-benchmark/datasets/swe_bench_*/**`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`, `*.pem`, `*.key`, `**/credentials.json` | `CLAUDE.md §8` |

---

## 3. Adversarial threat model

Per `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md`:

| Threat | Detection | Mitigation |
|---|---|---|
| **Adversarial patch** crafted to fool predictor into `Pass` | Gτ guard (TCT) + OOD score + multi-head agreement check | `Verdict::GuardRejected` or `OutOfDistribution`; predictor cannot Goodhart against the constellation (no gradient flow through centroids) |
| **Hardcoded secret in patch** | E_StaticAnalysis + secret-detection regex (`SEC-MEJEPA-pred-secret`) | `predicted_security_concerns.HardcodedSecret`; redact in injection (per `D3`) |
| **Prompt injection via training corpus** | E17 encodes prose as opaque embedding; never echoes verbatim | `SEC-MEJEPA-06` |
| **Patch with malicious dependency** | E_StaticAnalysis + cargo-audit/cargo-deny + CVE-db | `predicted_security_concerns.*` |
| **Coverage-game** (claim test passes by deleting assertions) | `DeleteTestCall` mutation category in corpus trains predictor to flag | `FailureModeClass::WrongTestUpdated` |
| **Cross-language label injection** | label gating per language (`TASK-PREDICT-LABEL-011`) | reject cross-lang labels at trainer boundary |
| **Two-person rule bypass attempt** (single-operator catastrophic promote) | `mejepa_promote_approval` requires operator action | `SEC-005` two-person rule for Catastrophic promotions |
| **Witness chain tamper** | hash chain + ed25519 + Merkle | `optimizer_witness_chain_verify` periodic (10k attempts) |

---

## 4. Secret redaction pre-embed

Per `TASK-PREDICT-010-secret-redaction-pre-embed`. Before chunks reach embedders:

1. Detect potential secrets via regex/entropy heuristics:
   - High-entropy strings (≥ 4.5 bits/char) of length ≥ 16
   - Known patterns: `AWS_*=*`, `AKIA[0-9A-Z]{16}`, `ghp_[A-Za-z0-9]{36}`, `xoxb-*`, JWT, private key headers (`-----BEGIN`)
2. Replace with `<REDACTED_SECRET_TYPE>` token before embedding.
3. Log redaction event to `CF_AUDIT_LOG` keyed by `(memory_uuid, ts)`.
4. **Never emit the secret in `RealityPrediction.predicted_security_concerns.explanation`** — only "hardcoded secret detected at chunk X line Y".

---

## 5. Cross-language label gating

Per `D10`, `TASK-PREDICT-LABEL-011-cross-language-label-gating`. Issue: a Python failure mode (e.g., `DefaultValueLeakage` — mutable default arg) applied via patch-similarity graph to a Rust patch is **noise**.

Gate at trainer boundary: when a label is propagated from corpus exemplar X to current patch Y via patch-similarity, require `language(X) == language(Y)`. Reject cross-language propagation with `MEJEPA_TRAIN_CROSS_LANG_LABEL_REJECTED`.

---

## 6. Concurrent agent edits (race conditions)

Per `D11`, `23_multi_agent_isolation.md §3`. Shift log is per-session JSONL; external mtime conflicts across sessions detected via `reality_shift_compare_to_my_view`:

```rust
mcp_call reality_shift_compare_to_my_view {
    session_id: "my-session",
    files: ["src/foo.py"],
}
// returns: { external_drift_detected: bool, divergences: [...] }
```

If `external_drift_detected=true`, re-perceive before next edit. Continued editing on stale state is a violation of the reality-shift principle.

---

## 7. Audit trail integrity

| Surface | Protection |
|---|---|
| Witness chain (`storage/witness/witness-chain.bin`) | Append-only; SHAKE-256 hash chain; ed25519 segment signing; Merkle compression > 90d |
| `CF_AUDIT_LOG` | Append-only; bound by `tool_use_id` via `CF_TOOL_CALL_INDEX` |
| `CF_MERGE_HISTORY` | Permanent (never expires); operator-pinned |
| `CF_IMPORTANCE_HISTORY` | Per-(memory, ts) delta log |
| `CF_ENTITY_PROVENANCE` | Per-entity extraction provenance |
| Training certs (`CF_MEJEPA_TRAIN_CERTS`) | Hash-chained via `parent_witness_hash → self_hash` |
| Agent feedback (`CF_MEJEPA_AGENT_FEEDBACK`) | Bound by `witness_hash` to original prediction; both sides immutable |
| Active-pointer changes (`CF_MEJEPA_ACTIVE_POINTERS`) | Atomic + witness-chain entry on every promotion |

---

## 8. Operator authorization model

| Action | Authorization |
|---|---|
| Non-Catastrophic A/B/C promotion | automatic via promotion gate |
| Catastrophic A/B/C promotion | requires `mejepa_promote_approval` from operator |
| Force constellation refit | operator-only (`mejepa_constellation_inspect --force-refit`) |
| Force allow stale constellation | operator-only (`--force-allow-stale`) |
| Force rollback to prior model | operator-only (`mejepa_rollback_to`) |
| Retire/recalibrate embedder slot | operator (after MI audit) |
| Disable retired E5/E11 slots | hardcoded; bypass requires explicit operator override flag |
| Force GC eviction | operator (`mejepa_gc_run --force-eviction true`) |
| Pause predictions | operator (`mejepa_pause_predictions --duration-mins`) |
| Mark `OperatorOverride` on active-learning entry | operator (×6 sampling weight + Fisher-protected) |

---

## 9. Privacy considerations

| Concern | Treatment |
|---|---|
| Sensitive code in agent's patches | E17 affect-text encodes prose as opaque embedding; secrets redacted pre-embed (§4) |
| Operator labels in `state/gold-labels/` | Daily backup; permanent; mode 0o600 |
| Daemon logs | Rotated 30d; live under `/mnt/d/contextgraph/logs/` (operator-private) |
| Witness chain | Cryptographic; signed by operator keyring; private key never in code |
| Models | Loaded from operator-controlled D-drive; not auto-fetched from HF |

---

## 10. CI security lints

| Lint | Purpose |
|---|---|
| `TASK-OBS-005` C-drive path lint | Forbid `/home/`, `~/.cache/`, `/tmp/contextgraph-` in code |
| `TASK-SEC-006` markdown hardening | Sanitize markdown in prediction-injection rendering (no script tags) |
| `TASK-SEC-007` D-drive permissions | Enforce `0700` on `state/gold-labels/`, `0600` on FSV evidence files |
| `TASK-SEC-008` hook script lint | Validate `scripts/cc/hook_*.sh` follow set -euo pipefail + JSON-stdin pattern |
| `cargo audit` (CI) | Per-PR check for dep CVEs |
| `cargo deny` (CI) | License + dep-allowlist enforcement |

---

## 11. Reality-boundary enforcement

`CLAUDE.md §8` — though the deny gate is REMOVED from code, the rule stands behaviorally:

| Path | Reason |
|---|---|
| `.env`, `.env.*` | secrets |
| `swebench/**` | upstream evaluator (DO NOT MODIFY) |
| `tests/oracle_**/**` | benchmark fixtures |
| `crates/context-graph-benchmark/datasets/swe_bench_*/**` | benchmark data |
| `crates/*/src/reward_specs/**` | reward specs |
| `crates/*/src/verifiers/**` | verifiers |
| `*.pem`, `*.key`, `**/credentials.json` | secrets |

Editing one of these for legit operator-directed task: ask operator to confirm first.

---

## 12. Cross-references

[`15_witness_chain.md`](15_witness_chain.md), [`25_multi_agent_isolation.md`](25_multi_agent_isolation.md), [`22_operator_runbook.md`](22_operator_runbook.md) (incident response).
