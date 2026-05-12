# SPEC-SECURITY — Functional Specification

> **Level 2.** Adversarial robustness, secret handling, multi-agent isolation, audit trails.
> Implementation reference: [`22_ADVERSARIAL_ROBUSTNESS.md`](../../22_ADVERSARIAL_ROBUSTNESS.md), [`23_MULTI_AGENT_ISOLATION.md`](../../23_MULTI_AGENT_ISOLATION.md).

**Status snapshot (2026-05-12):** REQ-SEC-03 is complete in Phase B. REQ-SEC-08 is aligned with the shipped unthrottled feedback path. Remaining security work is redaction, adversarial corpus, cross-language gating, markdown hardening, D-drive permission proof, and promotion controls.

## User stories

- **As an operator**, I want the system to resist agents that try to game predictions into Pass.
- **As an operator**, I want secrets in code redacted before being embedded.
- **As an operator**, I want audit trails for every model swap, label, and rollback.

## Requirements

### REQ-SEC-01 — Secret redaction pre-embed
Code chunks pass through a regex-based redactor (AWS keys, JWT, PEM, common API token patterns) before being passed to embedders or persisted.

### REQ-SEC-02 — Adversarial test corpus
30 adversarial test cases per `22 §7`; CI asserts ≥ 80% correctly classified non-Pass; 0 false-confident-Pass.

### REQ-SEC-03 — Multi-head contradiction detection
Verdict-assembly: when oracle.pass_confidence > 0.85 AND failure_mode.high_severity_count > 0, verdict = `Verdict::Abstain`.

### REQ-SEC-04 — Cross-language label gating
Failure-mode labels can transfer cross-language only when same RootCauseClass + patch-similarity > 0.85 + per-language correlation evidence. Cross-transferred labels weighted 0.5×.

### REQ-SEC-05 — Two-person-rule for catastrophic promotion (optional)
Operator config: `require_two_approvers_for_catastrophic_promotion = true|false` (default false). When set, `mejepa_promote_approval` requires two distinct operator IDs.

### REQ-SEC-06 — Markdown render hardening
Prediction-injection markdown render escapes `>>>`, `system:`, template-injection patterns; truncates at 4 KB.

### REQ-SEC-07 — D-drive permissions verified at startup
Daemon checks `/mnt/d/contextgraph/` is `chmod 700` for the operator's user; refuses to start if world-readable.

### REQ-SEC-08 — Unthrottled local feedback with fail-closed validation
`mejepa_record_agent_feedback` is not locally rate-limited. Valid feedback must be durably persisted; invalid payloads must return explicit errors without mutating feedback or queue state.

### REQ-SEC-09 — Witness-chain provenance audit
Every operator action (label, dismiss, approve, rollback, restore) writes a witness-chain entry. Tamper-evident.

### REQ-SEC-10 — Hook script lint
CI lint scans hook scripts for shell-injection patterns; blocks merge.

### REQ-SEC-11 — Per-cell promotion gate (anti-bias)
Per REQ-HEAL-FB-11: promotions require every cell to hold or improve. Prevents agent or operator bias from promoting models that regress unaffected cells.

### REQ-SEC-12 — Operator-override priority
Operator gold-labels weight 6× higher than agent labels. Floods don't outweigh operator labels at scale.

## Acceptance criteria

1. Synthetic patch with hardcoded AWS key → embedder receives `[REDACTED]` placeholder; no key in any CF.
2. Adversarial test corpus run: ≥ 80% non-Pass; 0 false-confident-Pass.
3. Synthetic adversarial agent submits 1000 valid feedback events in 1 hour → all 1000 are durably persisted or storage errors identify the exact failed write/readback point.
4. Two-person rule enabled: catastrophic promotion blocks until two distinct approvals.
5. World-readable D-drive → daemon refuses to start.
6. Witness-chain entries for last 10 operator actions exist + verify successfully.

## Non-functional

- Secret redaction adds < 5ms per chunk.
- Adversarial test corpus runs in < 60s.
- Feedback validation overhead < 1ms per call before storage.
