# 8. Verification Protocol

The framework's claims are only as trustworthy as the discipline that produces its numbers. This section states the verification contract every figure in §7 satisfies. The principle is simple and strict:

> *A return value is a claim. The source of truth is the verdict. Read the verdict.*

## 8.1 The four questions

Before any result is called done, four questions must have answers:

1. **What state changed?** (e.g., an MI estimate was computed; an oracle verdict was produced.)
2. **Where does that state live?** A concrete source of truth: a file path, a database key, a metric record, an external system's record.
3. **How is it read back independently?** The artifact must be re-openable and re-readable without re-running the code that produced it.
4. **What is the exact expected value?** Stated in advance, so a discrepancy is detectable.

If any of the four lacks an answer, the feature is not understood well enough to be called complete. This is why every figure in §7 carries an FSV path or issue number: the number is not the claim, the readback is.

## 8.2 No-mock, fail-closed, falsifiable

Three rules keep the verification honest:

- **No mocks in gating tests.** A test that gates a completion claim runs against the real oracle, the real corpus, the real panel. Synthetic data is permitted only as an estimator self-test (a known-signal control), never as evidence that the system works on the real task. In §7.3 the synthetic self-test (strong-signal → `0.977` bits; noise → `0.0`) validates the *estimator*; the *result* is measured on the real 2,400-row corpus.
- **Fail closed on unknowns.** Unhandled conditions raise structured errors, never silent defaults. In §7 the MI runner failed closed on degenerate all-Pass labels (`DEGENERATE_ORACLE_LABELS`) rather than fabricating a value, and a wrong-slice dataset was retained as a deliberate fail-closed readback rather than silently used.
- **Tests must fail when the feature is broken.** A test that passes on a broken system is worse than no test. Substrate sufficiency is the archetype: it is *designed* to return `false` when the panel is inadequate, and in §7 it did.

## 8.3 Source-of-truth readback, independently re-opened

Each result in §7 is backed by a physical artifact written to durable storage and re-read independently:

- **Oracle self-consistency (§7.2):** 300 `report.json` files + a per-instance agreement matrix at the FSV path; readback confirmed 300/300 reports, all exit codes 0, agreement = 1.0.
- **Substrate MI (§7.3):** a dataset manifest, the panel/oracle dataset, per-estimator logs (`mine_estimates.jsonl`, `classifier_lower_bounds.jsonl`, `kraskov_crosschecks.jsonl`), and an MI manifest, each with a recorded content hash; readback confirmed the dataset shape, slot-identity flags, and the three estimates.
- **Per-cell / per-sensor (§7.4):** the per-cell MI matrix and drop-one ablation deltas persisted alongside the MI run; readback confirmed the cell ranking and the ablation ordering.

Content hashing of each artifact makes the readback tamper-evident: the number reported is the number in the hashed file, not a value re-derived in memory.

## 8.4 Assume the task is wrong

The protocol's default stance is adversarial toward its own claims. A result is treated as *guilty until proven innocent*: paths, symbols, and field references are audited against the live tree before they are believed; a "pass" is examined for the ways it could be a false pass (selection bias, leakage, a metric that moves without the underlying quality moving). This is the discipline that flags the `0.7942` rank-lift (§7.5) as a Goodhart artifact rather than a gate pass, and that keeps oracle *validity* (§7.6) listed as **open** despite oracle *flakiness* being measured at `1.0` — because the protocol refuses to let a clean self-consistency number stand in for a validity claim it does not establish.

## 8.5 Why this matters for the super-intelligence claim

The whole point of the framework is to license the removal of human review. That license is only safe if the evidence that the predicate holds is itself verifiable without trusting the system that produced it. The verification protocol is therefore not an engineering nicety bolted on at the end; it is the mechanism that makes the central claim — *human review becomes statistically unjustified* — a checkable statement rather than an act of faith. A predicate (§3) asserted without source-of-truth readback is exactly the kind of unanchored claim (§4) the framework exists to forbid.
