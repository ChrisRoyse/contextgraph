# 09 — End Goal: Code Review Replaced by Reality Verification

> **🔒 Operator-stated end goal, 2026-05-08:** "the ultimate goal of this system and Main focus is to develop a system that makes it so that coders don't need to review code anymore since they are reviewing that what the ai said actually exists in reality. this system can make that possible. that is the end goal. create a system that makes it so no code review is needed anymore."

This doc is the canonical answer to "why are we doing all this?" It connects every primitive in `02_CAPABILITIES_INVENTORY.md` to a specific verification gate that, when wired up, eliminates the need for human code review of AI-generated patches.

---

## The shift

### Before (today)

```
AI writes code → human reads code → human asks "is this correct?" → human approves/rejects
```

The bottleneck is human attention. The reviewer's mental model of correctness is informal, easily fooled, and doesn't scale.

### After (the end goal)

```
AI writes code → AI emits certificates ⟶ ⟶ ⟶ verifier runs ⟶ ⟶ ⟶
                  ├─ witness chain entry        ├─ chain integrity check
                  ├─ training-step proof        ├─ proof verification
                  ├─ recall-cert Merkle root    ├─ recall-cert verification
                  ├─ source-of-truth SHAs       ├─ SHA re-read against disk
                  ├─ conformal bound            ├─ bound covers oracle outcome
                  ├─ Granger attribution        ├─ causal attestation valid
                  └─ patch                      └─ Docker oracle passes
                                                ↓
                                          if ALL pass: APPROVE
                                          if ANY fail: surface the failed gate
```

The verifier runs in milliseconds, deterministically, without human eyeballs. The human's role shifts to:

1. **Defining the verification gates** (what counts as evidence?).
2. **Reviewing the verifier itself** (does the gate definition correctly capture what we care about?).
3. **Investigating only when verification fails** (the failure mode tells you which gate broke; you don't read all the code, you read the failed gate's input).

This is **review-by-mathematical-proof + cryptographic-replay + statistical-attestation + oracle-verdict** — not review-by-reading.

---

## Why this is achievable (vs naive "AI judges AI")

The naive proposal — "have GPT-5 review the code GPT-4 wrote" — fails because both are stochastic, both can hallucinate, neither produces evidence. We have ample empirical evidence (LLM-as-judge benchmarks) that this approach plateaus at moderate accuracy and gets fooled by superficial correctness markers.

The reality-loop proposal succeeds because **every verification gate grounds in a non-stochastic substrate**:

| Verification gate | Substrate | Why it can't lie |
|-------------------|-----------|------------------|
| Witness chain integrity | SHAKE-256 hash chain | Tamper requires breaking SHA-3, computationally infeasible |
| Training-step certificate | Per-step formal/statistical proof of invariant preservation | Either the proof type-checks or it doesn't |
| Recall-cert Merkle root | Cryptographic hash binding output to justifying evidence | Recompute and compare bytes |
| Source-of-truth SHAs | Bytes on disk | `sha256sum` is what it is |
| Conformal prediction set | Calibrated coverage guarantee on held-out data | Statistical, mathematically justified |
| Granger causality | Frequentist time-series test | Yes/no with a p-value |
| Docker oracle | Held-out tests in a sealed container | Outside the AI's reach |

**None of these gates involve "another AI judging."** Each gate produces a yes/no answer that a non-AI verifier can compute. The human reviewer's job becomes *trusting the gate definitions, not the per-PR output*.

---

## How the two-loop architecture serves this end goal

### Inner reality loop (Qwen3.6-27B + Rust harness on SWE-bench Lite)

The **producer** of verifiable artifacts.

- Inner model generates patches under instrumented harness control.
- Every generation is **observed by the harness**, which emits structured artifacts:

| Inner-loop event | Artifact emitted |
|------------------|------------------|
| File edit | Witness-chain entry: `(prev_hash, action_hash=SHAKE-256(file_sha + edit_kind), timestamp)` |
| Test run | Docker oracle verdict (pass/fail per test); recorded under `official-evidence/` |
| Past trajectories influencing this generation | Forward-Push PPR scores over the (failure_tag, recommendation, attempt) graph |
| Token-level confidence | Conformal-prediction prediction set with calibrated coverage |
| Training data shaping current weights | Lineage chain: `(parent_run_id, training_data_manifest_sha, hparams_sha)` |
| Skills cited from ReasoningBank | Granger causality test: did this skill *cause* the high-reward behavior? |

The harness can **refuse a patch whose certificates are incomplete** (fail-closed per `5090jepa/00_TASK.md` R2). This is the structural guarantee that the corpus is verifiable.

### Outer reality loop (Claude Code as harness optimizer)

The **harness sculptor** that ensures the inner loop emits complete, verifiable artifacts.

- Every turn the outer loop **diagnoses** what's missing from inner-loop artifacts (no critique field? incomplete witness chain? uncalibrated confidence?).
- Every turn the outer loop **edits the harness** so the next attempt produces *more verifiable* artifacts.
- Outer-loop decisions go through **their own** witness chain (per `03_REALITY_LOOP_APPLICATIONS.md §2`) — meta-provenance.
- Every harness change is itself a recorded artifact, replayable.

The outer loop is what turns a working SWE-bench-passing harness into a **verification factory** — not just a patch generator.

### Training material accumulates from running the loop

Across 300 SWE-bench Lite tasks × N attempts × M optimizer turns, the corpus grows:

```
(problem_statement, harness_state, generated_patch, oracle_verdict,
 witness_chain, training_certs, recall_cert, conformal_bound,
 granger_attribution, lineage_record)
```

This corpus becomes:

- **Calibration data** for conformal prediction (next inner-model knows what confidence to assign because past confidence intervals' coverage was empirically measured).
- **Skill candidates** for the ReasoningBank (only promoted when Granger-attested causal uplift exists across enough samples).
- **Failure-mode training set** for the outer optimizer's bandit / SONA learner.
- **Bit-exact reproducibility set** for `verify_ai_output(patch, ...)` regression suites.

The reality loop is **not a one-off SWE-bench-passing exercise**. It's the **training data factory** for verifiable AI code generation. Each attempt adds a row to the corpus whether the inner model passes or fails — both passing patches with valid certs and failing patches with valid "I-tried-and-failed" certs are training signal.

---

## How each Tier 1 + Tier 2 primitive (from `05`) serves the end goal

These six clean-room reimplementations close the six core verification gates. Until they're built, the verifier can't run; the end goal is unreachable.

### Tier 1.1 — `crates/context-graph-witness/` (witness chain + ed25519 + lineage + attestation)

**Closes the gate:** Witness chain integrity + source-of-truth attestation.

**End-goal role:** When the inner model claims "I edited file X at SHA Y, then ran tests Z," the witness chain encodes that claim as a 73-byte hash-linked entry. A reviewer running `verify_witness_chain(chain)` can independently confirm:

- The sequence of actions was not reordered post-hoc.
- Each action's content hash matches the bytes that exist now.
- The chain is signed by the agent that produced it.

Without this gate, every "the AI said X happened" claim requires manual re-reading. With this gate, claims become cryptographically replayable. **This is the foundation gate** — every other gate references the witness chain by offset.

### Tier 1.2 — `crates/context-graph-solver/` (Forward Push + BMSSP + CG)

**Closes the gate:** Influence attribution.

**End-goal role:** When the inner model says "I drew on past attempts A, B, C," the verifier runs Forward-Push PPR over the (failure_tag, recommendation, attempt) graph and checks: is the model's claim about which past attempts influenced this generation **statistically supported by the influence graph**? If the model claims A influenced its decision but PPR shows A's mass at this query is near zero, the claim fails.

The verifier doesn't read the model's reasoning. It runs PPR and compares. Influence claims become falsifiable.

### Tier 1.3 — 8/7/5/3-bit tier compression module

**Closes the gate:** Verifiable cold storage of training data.

**End-goal role:** Training-data manifests cited in the lineage chain must be **bit-exact reproducible**. If the inner model was trained on dataset `D` at SHA `S`, the verifier needs to fetch `D`, decompress its cold-tier panels, and confirm the SHA still matches. The 8/7/5/3-bit tiering must be lossless-on-decompress (within the tier's specified RMSE bound) for the SHA assertion to hold under tier-aware semantics.

Without this gate, training-data provenance claims can't be re-verified — anyone can claim they trained on dataset `D` and you can't check.

### Tier 1.4 — MMR diversity rerank

**Closes the gate:** Evidence-citation honesty.

**End-goal role:** The recall certificate (Tier 2.3) cites the chunks of evidence that justified the AI's output. A naive citation strategy might list the top-5 cosine matches, which often are 5 redundant variants of the same evidence. MMR ensures the cited evidence **spans the relevant evidence space** — the AI can't hide behind "I cited 5 things" when the 5 things are essentially one.

Without this gate, citation completeness can't be measured. With it, the verifier can compute "did the AI's citations cover the diverse facets of the relevant evidence?" — and reject outputs whose citation diversity falls below threshold.

### Tier 1.5 — `crates/context-graph-retrieval-diffusion/` (training-free retrieval LM + masked diffuser)

**Closes the gate:** Auditable generation (no learned-weights opacity).

**End-goal role:** A trained transformer is *partly opaque* — its weights encode patterns we can describe statistically but can't enumerate. A training-free retrieval LM is *fully auditable*: every output token comes from associative-memory lookup over a corpus, deterministic given (corpus, seed, schedule). For sub-domains where this is feasible (gridworld_5x5.v1, counter_world.v1), the planner produces outputs that are **mechanically traceable to corpus entries**. The verifier can answer: "which corpus entries did this output retrieve from?" — and the answer is exact.

This is a *gradient-of-opacity* approach: where training-free retrieval is feasible, use it (less to verify). Where a trained model is necessary, fall back to verified-training certificates (Tier 2.2). Both paths converge on auditability.

### Tier 1.6 — Conformal prediction module

**Closes the gate:** Calibrated uncertainty.

**End-goal role:** Every AI output ships with a prediction set covering the true outcome with stated probability (default 90%). When the oracle returns a verdict, the verifier checks: was the actual outcome inside the predicted set? Across many attempts, the empirical coverage rate must match the stated rate (90% within ±2σ). If it doesn't, the model's confidence is mis-calibrated and the verifier flags ALL outputs from that model state for human review until recalibration.

This is the gate that detects **model self-overconfidence** — the most dangerous failure mode in code generation, where the model is confidently wrong.

### Tier 2.1 — `crates/context-graph-sona/` (SONA-style adaptive learner)

**Closes the gate:** Auditable behavior provenance + EWC++ task-boundary hygiene.

**End-goal role:** The model's behavior at each attempt is determined by its current weight state, which evolves with each REINFORCE update. Without provenance, we can't ask "what training experience produced this behavior?" SONA records every trajectory + reward + learning signal. EWC++ Fisher information records "this weight matters because it encoded experience X." Together they allow the verifier to answer: "which past experiences shaped this output?"

This is the bridge between the training-data lineage chain (offline) and the per-output behavior (online). Without it, the lineage chain stops at "model state was X at time T" — with it, we can attribute *specific outputs* to *specific experiences*.

### Tier 2.2 — `crates/context-graph-verified-training/` (per-step training certificates)

**Closes the gate:** Training-step certificate.

**End-goal role:** Every gradient update during training is a potential point where invariants get violated (loss diverges, weights blow up, Lipschitz bound broken, permutation equivariance lost). The verified-training crate generates a **per-step `TrainingCertificate`** with formal/statistical proofs of:

- Loss stability bound preserved.
- Weight norm bound preserved.
- Lipschitz bound preserved (statistical).
- Permutation equivariance preserved (statistical).
- Energy gate not crossed.

Each cert is appended to a witness chain. Delta-apply: gradients go to a scratch buffer; the cert verifies the *proposed* state; only then does the gradient commit. **Fail-closed.**

The verifier checks: every training step in the lineage chain has a valid cert. If any cert fails, the resulting model is rejected. Training-time invariant violations become impossible to hide.

### Tier 2.3 — Stage 0 + Stage F certificates (built atop witness crate)

**Closes the gate:** Recall-cert Merkle root.

**End-goal role:** Every model output ships with a Merkle root over `(evidence_chunk_sha, contribution_score)` pairs. The verifier:

- Recomputes the Merkle root from the cited chunks.
- Compares it to the cert's stated root.
- For ME-JEPA outputs, also verifies the policy-version + access-level fields against the operating policy.

This binds **AI claim ↔ justifying evidence** cryptographically. The reviewer doesn't ask "did the AI's reasoning make sense?"; the reviewer asks "does the Merkle root match what was cited?" — and that's a deterministic computation.

### Tier 2.4 — `crates/context-graph-mincut/` (Stoer-Wagner mincut)

**Closes the gate:** Statistically clean train/test boundaries.

**End-goal role:** The conformal-prediction calibration (Tier 1.6) requires a calibration set held out from training. Naive holdout splits leak (shared ancestors, shared features). Mincut over the appropriate similarity graph produces a split with provably minimal cross-set edge weight. The verifier checks: was the training/calibration split mincut-clean? If not, the calibration coverage guarantee is suspect, and ALL conformal bounds are flagged.

This gate prevents **calibration cheating** — the model can't claim 90% coverage if its calibration data was contaminated with training data.

### Tier 2.5 — Matryoshka funnel + Graph RAG (Leiden communities)

**Closes the gate:** Retrieval scalability under verification.

**End-goal role:** Recall certs (Tier 2.3) require the AI to cite specific evidence chunks. As the corpus grows, naive retrieval becomes too slow. Matryoshka funnel + Graph RAG (Leiden community summaries) keep retrieval verifiable at billion-chunk scale: coarse-to-fine search bounds the verification time, and community-level summaries provide aggregate citations that scale.

Without this gate, the verifier degrades from "verify in 100 ms" to "verify in 100 seconds" as the corpus grows — slow enough that humans bypass the verifier and fall back to reading code.

### Tier 2.6 — Granger causality test

**Closes the gate:** Causal attribution.

**End-goal role:** "Skill X caused outcome Y" is a *causal* claim. Correlation passes the bandit's reward signal even when X just happens to coincide with easy contexts. Granger causality is a frequentist time-series test that distinguishes "X temporally predicts Y given history" from "X correlates with Y." When the AI promotes a skill, the verifier runs the Granger test on the trajectory-history. If the test fails, the skill is demoted regardless of its raw reward.

This gate prevents **lucky-pattern hoarding** — skills that look good in the bandit but don't actually cause uplift get filtered before they reach production.

---

## The verifier function in concrete pseudo-code

```rust
fn verify_ai_output(
    patch: &Patch,
    problem: &Problem,
    harness_state: &HarnessState,
    operator_policy: &Policy,
) -> Result<VerifyVerdict, VerifyFailure> {
    // Gate 1: Witness chain integrity
    verify_witness_chain(&harness_state.witness_chain)
        .map_err(|e| VerifyFailure::WitnessChainBroken(e))?;

    // Gate 2: Training-step certificates exist for every gradient
    for step in &harness_state.model_lineage.training_steps {
        verify_training_cert(step.cert)
            .map_err(|e| VerifyFailure::TrainingCertInvalid(step.id, e))?;
    }

    // Gate 3: Recall-cert Merkle root matches cited evidence
    let recomputed_root = compute_merkle_root(&patch.recall_cert.cited_chunks);
    if recomputed_root != patch.recall_cert.merkle_root {
        return Err(VerifyFailure::RecallCertMismatch);
    }

    // Gate 4: Source-of-truth SHAs match disk
    for cited_file in &patch.cited_files {
        let actual_sha = sha256_file(&cited_file.path)?;
        if actual_sha != cited_file.sha {
            return Err(VerifyFailure::SourceShaDrift(cited_file.path.clone()));
        }
    }

    // Gate 5: Conformal coverage holds
    if !patch.conformal.set.contains(&problem.oracle_verdict) {
        return Err(VerifyFailure::ConformalMissed {
            stated_coverage: patch.conformal.alpha,
            // The miss itself is OK on a single attempt; the verifier
            // reports it so coverage statistics can be tracked across attempts.
        });
    }

    // Gate 6: Granger causality attests cited skills
    for cited_skill in &patch.cited_skills {
        let granger_p = granger_test(&cited_skill.behavior_log, &harness_state.outcome_log);
        if granger_p > GRANGER_REJECT_THRESHOLD {
            return Err(VerifyFailure::GrangerRejection {
                skill: cited_skill.id.clone(),
                p_value: granger_p,
            });
        }
    }

    // Gate 7: Influence attribution honesty (PPR)
    for cited_attempt in &patch.cited_past_attempts {
        let ppr_mass = harness_state.influence_graph.ppr_mass_at(cited_attempt);
        if ppr_mass < PPR_MIN_INFLUENCE {
            return Err(VerifyFailure::WeakInfluenceClaim(cited_attempt.clone()));
        }
    }

    // Gate 8: Docker oracle (already in place — the only gate not from the findings docs)
    let oracle_result = run_docker_oracle(patch, problem)?;
    if !oracle_result.passes {
        return Err(VerifyFailure::OracleFailed(oracle_result));
    }

    Ok(VerifyVerdict::Approved {
        gates_passed: 8,
        verifier_version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp_ns: now_ns(),
    })
}
```

When this function returns `Ok`, no human review is needed. When it returns `Err`, the variant tells the human exactly which gate to investigate.

---

## How the inner loop's training corpus closes the loop

The inner loop's per-attempt artifact bundle is the **calibration set** for `verify_ai_output`. After enough attempts:

- **Conformal coverage** is empirically measured: across N attempts with stated 90% coverage, did 90% of oracle verdicts fall in the prediction set? If not, the conformal calibration is wrong; recalibrate.
- **Granger thresholds** are tuned: at what `p_value` does the test reliably distinguish causal skills from coincidental ones? Sweep on the corpus.
- **PPR influence threshold** is tuned: at what minimum mass does an "X influenced this output" claim become meaningful?
- **Witness-chain entry validity rate** is tracked: are there any tampering events? (Should be 0.)

**The loop is self-calibrating.** As the corpus grows, the verifier gets sharper. After 300 tasks × ~5-10 attempts × ~10 turns each, the verifier has ~15,000-30,000 calibration rows. That's enough to tune thresholds with statistical confidence.

The certificate ↔ oracle correlation becomes our primary metric of progress:

```
correlation = corr(verify_ai_output_predicts_pass, oracle_actually_passes)
                 over the running corpus
```

When this correlation crosses ~0.95, **the verifier is more reliable than human reviewers** at predicting oracle outcomes. At that point, human review of AI-generated patches that pass `verify_ai_output` becomes statistical waste.

---

## What this means for prioritization

Re-rank the Tier 1 + Tier 2 list from `05_QUICK_WINS_VS_DEEP_INTEGRATION.md` by **which verification gate it closes**, not by general capability:

| Priority | Reimplementation | Verification gate closed | Critical-path? |
|----------|------------------|--------------------------|----------------|
| 1 | Tier 1.1 — Witness chain | Foundation; every other gate references it | YES |
| 2 | Tier 2.3 — Stage 0/F certs | Recall-cert Merkle root | YES |
| 3 | Tier 2.2 — Verified-training certs | Training-step cert | YES |
| 4 | Tier 1.6 — Conformal prediction | Calibrated uncertainty | YES |
| 5 | Tier 2.6 — Granger causality | Causal attribution | YES |
| 6 | Tier 1.2 — Forward-Push PPR | Influence attribution | YES |
| 7 | Tier 2.4 — Mincut splits | Calibration cleanliness (precondition for #4) | HIGH |
| 8 | Tier 1.3 — 8/7/5/3-bit tier | Lineage cold-storage verifiability | HIGH |
| 9 | Tier 1.4 — MMR | Citation-completeness measurement | MEDIUM |
| 10 | Tier 2.5 — Matryoshka + Graph RAG | Retrieval scalability under verification | MEDIUM |
| 11 | Tier 2.1 — SONA learner | Behavior provenance | MEDIUM |
| 12 | Tier 1.5 — Retrieval diffusion | Auditable generation in narrow domains | LOW (subset wins) |

After #1-6 ship, the **verifier is operational** and `verify_ai_output` runs end-to-end. After #7-8 ship, the verifier is **defensible** (calibration is provably clean, lineage is bit-exact). After #9-12 ship, the verifier is **complete** (handles full-scale corpora, full behavior provenance, narrow-domain auditable generation).

This re-prioritization differs from the capability-only ordering in `05`. **The end goal forces prioritization-by-gate-closure**, not prioritization-by-individual-feature-impact.

---

## Why the locked clean-room rule is not optional for this end goal

Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`, every primitive is a clean-room reimplementation. This isn't a stylistic preference — it's structurally required by the end goal:

**The verifier itself must be ours, fully owned, fully auditable.** If our verifier path-deps on a foreign crate, then "did the AI's claim verify?" depends on "does the foreign crate's verifier behave correctly?" — which is unaudited. Worse: a malicious upstream commit could silently weaken the verifier (e.g., make `verify_witness_chain` always return `true`), and our system would happily "verify" tampered AI claims.

A reality-grounded verifier must be:

- **Authored by us** (we know every line).
- **Tested by us** (we know the test coverage).
- **Pinned by us** (no upstream surprise can mutate it).
- **Auditable end-to-end** (a third party can review every gate's implementation).

This is why `06_RISKS_AND_NON_TRANSFERABLE.md` insists on no vendor / no path-dep / no npm. The verifier IS the deliverable; foreign crates can't be load-bearing in it.

---

## Cross-references

- Memory: `memory/context/end-goal-verifiable-ai-code.md` — operator-stated goal + design rationale.
- Auto-memory: `~/.claude/projects/-home-cabdru-contextgraph/memory/project_end_goal_verifiable_ai_code.md`.
- Locked rule: `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.
- Sister doc (agentdb): `docs/agentdbfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — short mirror pointing here.
- Findings: `docs/ruvectorfindings/{02,03,04,05}.md` — the primitives, their applications, the triage.
- ccreality architecture: `docs/ccreality/01_ARCHITECTURE.md` — the two-loop diagram already in place.
- 5090jepa rules: `backupdocs/5090jepa/00_TASK.md` — R4 (FSV mandatory), R5 (trigger-outcome verification mandatory) are the precursor discipline that this end goal generalizes.
