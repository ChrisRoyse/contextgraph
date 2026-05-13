# 22 — Adversarial Robustness

> **Purpose.** ME-JEPA is a verifier; verifiers attract adversarial pressure. Specifies the threats, the defenses, and the operational posture against agents that try to fool the predictor into `Pass`.
>
> Owned in part by [`SPEC-SECURITY.md`](specs/functional/SPEC-SECURITY.md).

---

## 1. Threat model

| Adversary | Goal | Capability |
|---|---|---|
| **Buggy agent** | not malicious; just confidently wrong | crafts patches that look plausible but break tests |
| **Misaligned agent** | optimize for "Pass verdict" (proxy reward) | generates patches that maximize predictor score, not actual correctness |
| **Compromised agent** | intentional sabotage | injects backdoors / data exfil that pass tests but harm reality |
| **Compromised operator** | unauthorized model swap / label injection | gold-label injection to bias the model |
| **External attacker** | DoS / state corruption | floods feedback events, corrupts shift log, steals secrets |

The buggy/misaligned cases are the dominant risk; compromised cases are lower probability but higher consequence.

---

## 2. Defenses against buggy / misaligned agents

### 2.1 Conformal predictor with hard widened intervals

Per `21 §4` cold start + `09_SELF_HEALING_AUTONOMY.md §6` recalibration. Even when the predictor is confident, the conformal interval reflects empirical uncertainty. An agent can't game past the conformal floor without genuinely improving accuracy on the holdout.

### 2.2 TCT Gτ guard (geometric verifier)

Per `04 §5` + `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §3.3`:

> The constellation centroid encodes what an *acceptable* prediction looks like at training-time — a specification the optimizer cannot Goodhart against, because optimizing toward the constellation centroid is not in the loss.

The Gτ guard sits structurally outside the predictor's loss. An agent gaming the predictor would need to also game the embedder ensemble's geometric structure simultaneously — that is much harder.

### 2.3 Negative-action ablation falsifier

Per `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §6.3 (d)` (shipped):

> Periodically run validation with action conditioning *removed*. If the predictor were degenerately predicting the next-state panel without using the action, the score would not drop.

If a predictor learns to ignore the action (a Goodhart sign), the falsifier catches it via per-week holdout drop.

### 2.4 Pairwise MI audit

Per `04 §8` (shipped). If the predictor collapses to a low-rank shortcut, MI between embedder slots spikes. Operator-visible weekly.

### 2.5 Multi-head consensus

When the oracle head says `Pass` but the failure-mode head also says `WrongAlgorithm` with high confidence, that contradiction is itself a signal. Heuristic in the verdict-assembly:

```
if oracle.pass_confidence > 0.85 AND failure_mode.high_severity_count > 0:
    verdict = Verdict::Abstain  # contradictory; can't commit
```

### 2.6 Active-learning queue prioritizes inconsistencies

Per `08 §5.5` (queue extension): when contradiction signal is detected, the prediction is auto-queued for operator label. Surprises are over-sampled in the next epoch.

---

## 3. Defenses against compromised agents

### 3.1 Feedback flood resistance without local rate limits

`mejepa_record_agent_feedback` is intentionally not rate-limited for local single-operator use. Flood resistance comes from fail-closed schema validation, witness-bound persistence, active-learning queue readback, and operator-label priority. A high-volume local run should either durably record valid feedback or return an explicit validation/storage error with enough context to fix the root cause.

### 3.2 Witness-chain provenance for every prediction + feedback

Every prediction has a witness_hash. Every feedback event binds to that hash. Chain-tampering is detectable; entries are append-only.

### 3.3 Operator-override priority

Operator-labeled gold-standard examples weight 6× higher than agent-labeled. An agent flooding feedback can't outweigh operator labels at scale.

### 3.4 Catastrophic-class promotion gate

Per `09_SELF_HEALING_AUTONOMY.md §3` + REQ-HEAL-FB-08: catastrophic A/B/C promotions BLOCK on operator approval. An agent can't autonomously trigger a model swap.

### 3.5 Per-cell promotion criterion

Per REQ-HEAL-FB-11 under the active Python gate: promotions require every Python mutation-category cell to hold or improve. An agent biasing one cell can't promote a model that regresses other cells.

---

## 4. Defenses against compromised operators

### 4.1 SMAP audit trail

Every operator action (label, dismiss, approve, rollback) writes to `CF_MEJEPA_HEAL_REPORTS` + the SMAP `memory/` tree. Tamper-evident via witness chain.

### 4.2 Two-person-rule for catastrophic promotions (optional, opt-in)

Operator config: `require_two_approvers_for_catastrophic_promotion = true`. When set, `mejepa_promote_approval` requires two distinct operator IDs. Default off (single-operator deploys); on for multi-operator deploys.

### 4.3 Witness-chain integrity check rotates operator-callable

`optimizer_witness_chain_verify` is operator-callable but the verification itself is reproducible — any third party can re-run on the same data and get the same result.

---

## 5. Defenses against external attackers

### 5.1 No HTTP listener (default)

Per `2.4 No-HTTP rule`. Daemon's MCP listener is on a Unix socket (or stdio). No remote attack surface unless the operator opts into TCP exposure.

### 5.2 D-drive permissions

`/mnt/d/contextgraph/` should be `chmod 700` for the operator's user. Anyone else on the host can't read or write.

### 5.3 Hook script sandboxing

Hook scripts run with the operator's privileges. Don't put untrusted scripts in `.claude/`. The CI lint checks for shell-injection patterns in hook scripts.

### 5.4 Secret redaction before embedding (D3)

Code being embedded may contain secrets:

```rust
fn redact_before_embed(content: &str) -> String {
    let mut out = content.to_string();
    // Patterns: AWS keys, API tokens, JWT, SSH keys, PEM headers, common env-var conventions
    for pattern in SECRET_PATTERNS {
        out = pattern.replace_all(&out, "[REDACTED]").into_owned();
    }
    out
}
```

`TASK-PREDICT-010-secret-redaction-pre-embed.md` covers this. The redaction is applied before chunks are passed to embedders + before being stored in `CF_MEJEPA_PANELS`.

### 5.5 Prompt-injection defense in markdown render

The prediction-injection markdown block is rendered into the agent's context. Adversarial agents could try to influence ME-JEPA via the prediction content itself. Mitigation:
- Render is escaped: any `>>>` / `system:` / template-injection patterns are escaped by the renderer.
- Truncation at 4 KB prevents floods.

---

## 6. Cross-language label noise (D10)

Risk: a Python `WrongAlgorithm` failure label is applied to a Rust patch via patch-similarity. The label may not transfer.

Mitigation:
- `TASK-PREDICT-LABEL-011-cross-language-label-gating.md`: failure-mode labels can only transfer across languages when:
  - Same `RootCauseClass` (e.g., `LogicError`)
  - Patch-similarity > 0.85 cosine in E_AST + E_CFG (structural, not surface)
  - Per-language correlation evidence in the patch-similarity graph
- Cross-language transferred labels are weighted 0.5× vs same-language labels in the loss.
- Cross-language false-positive rate is monitored per language pair in the weekly report.

---

## 7. Adversarial test corpus

Per `18 §7`: 30 adversarial test cases:

- 10 patches that pass tests but introduce subtle bugs (e.g., off-by-one that's caught by tests but missed by E_TypeGraph)
- 10 patches that look like KnownGood but aren't (high cosine similarity to KnownGood but actually buggy)
- 10 patches that game specific embedders (e.g., crafted to maximize E7 cosine while ignoring E_DataFlow)

Stored under `tests/fixtures/adversarial/`. The integration test `TASK-TEST-010` runs against these and asserts:
- ≥ 80% are correctly classified as `Fail` or `Abstain` (not `Pass`)
- 0 are classified as `Pass` with confidence > 0.95 (high-confidence false positives are catastrophic)

---

## 8. Reporting

Adversarial-event signature in the weekly report:
- Count of `Verdict::Abstain` due to contradictory heads
- Count of `Verdict::GuardRejected` per embedder (concentrated rejection on one embedder = blind spot)
- Top-N high-severity Surprise events from operators (vs agents) — operator surprises are stronger signal of adversarial / overfitting issues
- Per-cell ship-gate regression as a signal of distribution-shift attacks

---

## 9. What this commits us to

Tasks (in `tasks/security/`):

- `TASK-SEC-001` — secret redaction module + integration into chunk pipeline (D3)
- `TASK-SEC-002` — adversarial test corpus (10 + 10 + 10 cases) + integration into CI (D14, `TASK-TEST-010`)
- `TASK-SEC-003` — multi-head contradiction detection in verdict assembly (§2.5) — complete 2026-05-12
- `TASK-SEC-004` — cross-language label gating (D10, `TASK-PREDICT-LABEL-011`)
- `TASK-SEC-005` — two-person-rule config option for catastrophic promotion
- `TASK-SEC-006` — markdown render hardening (escape adversarial patterns)
- `TASK-SEC-007` — D-drive permissions documented + verified at startup
