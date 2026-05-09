# 13 — Universal Agent Verification & Infinite Reward-Signal Supply

> **Operator framing 2026-05-09 (verbatim, the synthesis):** *"this would be able to
> convert anything into insane amounts of data that is fullyy labeled and should be able
> to give the me jepa everything it needs to predict reality if the system is working in
> reality or not and also verify if the agent has done what it says its done to verify
> and validate the agents outputs and verifyy they are working in reality or not. with
> all of this information we will be able to derive an insane amount of reward signals
> that can all be used to train and optimize the me jepa."*

> **What this doc captures.** Doc 10 specified the single-loop architecture. Doc 11
> specified the UTML v1.0 four-term learning formula. Doc 12 specified how the 14 frozen
> embedders × DDA produce 105 signals per input. **This doc captures the CAPABILITY
> SYNTHESIS those three docs unlock together** — a universal agent verifier that runs on
> 105-signal-per-input reward-signal supply, structurally outside the Shumailov 2024
> collapse regime, and that scales linearly with reality-loop activity.

---

## Section 0 — The capability claim (the headline)

**Given any AI agent's output `ŷ` (a code patch, a tool call, a plan, a generation),
ME-JEPA-Code can answer the three operator questions deterministically:**

```
Verdict = mejepa.verify(ŷ, context)
       = {
           exists:        bool,    // Q1: does what the AI claimed it did exist?
           works:         bool,    // Q2: does it work as intended?
           reality_impact: f32,    // Q3: how is it actually impacting reality?
           confidence:    [lo, hi],
           failed_modalities: Vec<EmbedderId>,
           constellation_violations: Vec<(centroid_id, cos_distance)>,
         }
```

The verdict is produced by:

1. **Computing the panel** — run all 14 frozen embedders on `ŷ` and its claimed source-of-
   truth chain. ~12.5 GB VRAM, all warm-loaded. Latency target <100 ms.
2. **Checking the constellation guard** `Gτ` — is the panel within cosine threshold of
   the (KnownGood, language) centroid in *every* embedder space? Deterministic, no learned
   model.
3. **Running the predictor** — does ME-JEPA's Pψ predict the panel will produce the
   claimed oracle outcome? Cross-checked against the constellation guard.
4. **Extracting the 105-signal vector** — 14 single-embedder + 91 cross-embedder pair
   signals — and inspecting which signals dropped below threshold. Each is a *named
   geometric reason* for the verdict.

**The agent doesn't need to be Claude Code. It doesn't need to be a Python coding agent.
It doesn't need to run in this repo.** It just needs to produce an output that the 14
embedders can encode. ME-JEPA is *output-source-agnostic*.

This is the universal agent verifier the operator described.

---

## Section 1 — Why this works mathematically

### 1.1 The DDA counting identity (Royse 2026c §3.1)

Every input `x` projected through `N` frozen embedders yields:

```
|signals(x)| = N + (N choose 2) per-input structured signals
```

| Stack | N | per-input signals | per /loop turn (50-200 inputs) | per day (96 turns) | per week | per quarter |
|---|---:|---:|---:|---:|---:|---:|
| Production content (E1-E14) | 14 | 105 | 5,250-21,000 | 504K-2M | 3.5M-14M | 45M-180M |
| Full stack incl. learner-state (E1-E21) | 21 | 231 | 11,550-46,200 | 1.1M-4.4M | 7.7M-31M | 100M-400M |

**These are labeled supervisory signals**, not raw bytes. Each carries:
- A specific embedder's geometric verdict ("from E7's view, this code is far from
  KnownGood"), or
- A specific cross-embedder agreement verdict ("E1 says the description matches but E7
  says the code structure doesn't — disagreement = AI lying about what it did").

This is the **infinite reward-signal supply** the operator described — *quantified*. The
limits are not data; they are inference compute and storage.

### 1.2 Why this is collapse-safe (Shumailov scope claim)

Royse 2026c §3.3 makes the formal scope argument:

- The Shumailov 2024 / Dohmatob 2024 / Alemohammad 2023 collapse theorems require a
  recursion: `D_{t+1} ∋ x̃ ∼ p_θ_t` — the next training set contains samples drawn
  from the prior generator.
- DDA does not instantiate this. Each signal `φ_m(x)` depends only on a real `x` and
  frozen embedder parameters. **Centroid computation depends only on real inputs and
  frozen embedders.** Training adjusts only ME-JEPA's predictor `Pψ`, never the
  embedders or the centroids.
- Therefore the collapse precondition is not satisfied. *This is scope, not refutation*
  — the collapse theorems still hold for recursive self-training; DDA is structurally
  outside their domain.

This is what makes the 105 signals/input *durable* training material. It does not
collapse under recursion because there is no recursion.

### 1.3 Why this is per-output verifiable (the structural advantage over RLHF)

Scalar-reward alignment (InstructGPT, DPO, Constitutional AI) optimizes
`E[r_ψ(y|x)]` — but at inference time, `r_ψ` is trained, not auditable, and a single
scalar offers no diagnostic signal when it fails.

The TCT guard `Gτ(ŷ; CP) = 𝟙[∀m: cos(φ_m(ŷ), c_m) ≥ τ_m]` is:
- **Deterministic** — no learned model, no RNG.
- **Geometric** — produces a typed reason for refusal (which modalities failed).
- **Re-runnable** — given the same `ŷ` and `CP`, anyone can recompute and verify.
- **Computationally bounded** — `O(M · d_max)` per output.

Royse 2026c §1 / §7 makes this contrast explicit: this is a structural property scalar-
reward methods *cannot* provide in the same technical sense.

---

## Section 2 — What this means for any AI agent (universal verifier)

### 2.1 Verifying Claude Code (the immediate use case)

When Claude Code claims:

> "I fixed the bug in `astropy/coordinates/earth.py` line 387 by changing `>` to `>=`."

ME-JEPA verifies by:

1. **Q1 (exists?)**: Read the post-edit SHA. Run E11 (Kepler) + E1 (semantic) on
   `(claim, post_edit_diff)`. If `cos(claim_embed, diff_embed) < τ_exists`, the claim
   does NOT match the actual edit — refuse with `MEJEPA_CLAIM_DOESNT_EXIST`.
2. **Q2 (works?)**: Run all 14 embedders on the post-edit code. Check
   `Gτ(panel; KnownGood_python)`. If the panel is far from KnownGood Python centroid
   in (E5, E7, E11), refuse with `MEJEPA_INFER_CONSTELLATION_VIOLATION` listing the
   failed modalities.
3. **Q3 (impact?)**: Run the predictor's oracle-pass head. Compare against the actual
   Docker oracle (when available). The 91 pair signals give a 91-dim diagnostic vector
   for *how* reality diverged from the prediction.

### 2.2 Verifying any other AI agent

The exact same machinery applies to:

- **External LLM agents** (GPT-5, Gemini, Cursor, Aider, etc.) — they emit a patch; ME-JEPA
  verifies it the same way.
- **Human PRs** — verifies humans aren't hallucinating about what their patches do.
- **Automated bots** — Dependabot, Renovate, etc. — verify upgrade patches don't break
  the constellation.
- **ME-JEPA itself** when it eventually proposes patches via gradient ascent on its
  predicted reality (Royse 2026c §1) — ME-JEPA verifies its own outputs by the same
  guard, no privileged path.

The agent's identity is irrelevant. **What matters is that the output can be encoded by
the 14 frozen embedders and compared to constellations computed from real working code.**

### 2.3 Cross-language universality

Per the operator's cross-language mandate and doc 12 §2 / §7:

- E14 BGE-M3 multilingual head anchors panel layout dimension-wise across Python, Rust,
  C, C++, Java, C#, JS, TS, Ruby.
- Per-language constellation centroids capture "working code" per language.
- T(train, deploy) — the state-transfer factor — is bounded by the divergence between
  the training-corpus language distribution and the deployment-language distribution.

For a Rust patch: load the (KnownGood, Rust) constellation; run `Gτ` against it. The
machinery is unchanged.

### 2.4 Cross-domain universality (beyond code)

Royse 2026c instantiates the same machinery on:
- Voice cloning (Case 3): WavLM + DNSMOS + UTMOS centroids → 0.961 mean SECS.
- Stylistic LoRA (Case 1): 9-embedder text-style panel → robust Shakespearean generation
  with emergent zero-shot Spanish Golden Age period transfer.
- Talking-head clone (Case 2): visual + speaker + AU + text panel → projected.

**The pattern transfers.** ME-JEPA-Code is the code-domain instance of TCT. Future ME-JEPA-
Voice, ME-JEPA-Image, ME-JEPA-X domain packs follow the same structure: pick N frozen
embedders covering the modality, build constellations from real working examples, train a
predictor with constellation-attraction loss, gate inference with `Gτ`.

---

## Section 3 — How the reward-signal supply gets used

### 3.1 Three load-bearing uses

The 105-signal-per-input vector feeds three downstream consumers:

**a. Training ME-JEPA itself.** Every panel triple `(panel[t=0], panel[t=1],
panel[t=2])` × 105 signals × `L_step` weighting → gradient on the predictor `Pψ`. This
is the standard Phase 3 training loop.

**b. Training other agents (reward signal for RL / RLHF / fine-tuning).** Any agent that
wants to improve its code-generation ability can use ME-JEPA's verdict as a reward
signal. The verdict is geometrically grounded (not a scalar from a learned reward model
that can be Goodharted), so it is **safer than RLHF rewards** — Goodharting the
constellation requires simultaneously fooling 14 independent embedders, which is empirically
hard.

**c. Operator dashboards / debugging signal.** When an agent's output fails the guard,
the failed-modalities vector tells the operator *which embedders disagreed* — a typed
debugging signal. "E5 + E7 failed" means "causal direction wrong AND code structure
wrong." This is a categorical fault label, not a vague low-confidence flag.

### 3.2 Signal supply scales linearly with reality-loop activity

The training-data growth rate is **linear in /loop-turn rate × signals-per-input**:

```
signal_rate = turns_per_day × inputs_per_turn × signals_per_input
            = 96 × 100 × 105
            = ~1M signals per day per Claude Code instance
```

Multiple Claude Code instances scale this. External agents using ME-JEPA's verifier
*also* generate signals (their inputs go through the panel + guard, producing 105 signals
each).

**Within one quarter of /loop running, the per-Claude-Code-instance signal supply is
~90M signals — far more than ME-JEPA's predictor capacity needs.** The bottleneck is
not data; it is inference compute (running the 14 embedders) and storage (panels in
RocksDB).

### 3.3 Linking back to the audit substrate (UTML v1.0 §XII)

Every panel, every constellation distance, every guard verdict is persisted to the
existing 25 RocksDB CFs (`dj_*`) + BlackBoxFix SQLite ledger at
`LEDGER_SCHEMA_VERSION = 14`. This is the **physical M(t)** UTML v1.0 §XII names.

Each verdict is byte-equal-readback-verified before influencing the next decision (Royse
2026a §7). The training corpus, the constellation centroids, the 105-signal vectors, the
guard decisions — all queryable, all replay-able, all auditable.

ME-JEPA is not "a model with predictions" — it is **a model + an audit substrate where
every prediction is geometrically grounded and re-verifiable**.

---

## Section 4 — Concrete capabilities unlocked

### 4.1 "Did the AI do what it said?" — answered structurally

Today: the operator has to read the diff and compare against the agent's claim manually.
Or run tests. Or trust the agent.

Post-integration: `mejepa.verify(claim, diff)` returns `exists: bool` with a typed reason
when it fails. *Per-output* verifiability — no auxiliary learned model, no scalar
proxy. This is the Q1 question answered structurally for the first time at this scale.

### 4.2 "Will the patch work in reality?" — predicted before deployment

Today: run the test suite. Slow, sometimes flaky, often incomplete coverage.

Post-integration: `mejepa.verify(...)` returns a calibrated confidence interval on
oracle outcome before any test runs. When confidence ≥ 0.95 *and* OOD score < 0.5 *and*
constellation guard passes, the patch is *already verified* — the test run becomes
optional confirmation, not the primary signal.

### 4.3 "How is the patch impacting reality?" — diagnosed via 91-dim vector

Today: the operator reads logs, runs profilers, checks memory, asks "did this break
something else?".

Post-integration: the 91 cross-embedder pair signals form a **diagnostic vector**.
Specific entries name specific failure modes: "(E5, E7) low" means causal direction
contradicts code structure; "(E1, E14) low" means the change works in English-only and
breaks for multilingual users; etc. The operator reads a 91-dim verdict, not raw logs.

### 4.4 Infinite training data without leaving the reality-grounded regime

Per §1.2: every signal is a deterministic function of real inputs and frozen embedders.
**The training corpus is structurally outside the Shumailov collapse regime**. The corpus
can grow without bound — every Claude Code action, every external agent's verified
output, every PR review, every failed Docker oracle run — and contribute durable
training signal.

This is **the third path around the data wall** (Royse 2026c §1): not acquire (license
proprietary corpora), not synthesise (recursive self-training), but **decompose** a
fixed corpus through frozen embedders.

### 4.5 Self-improving reward signal for any future agent

Once ME-JEPA's correlation crosses ~0.95 (the Phase 8 ship gate), it becomes a
**reusable verifier**:
- Future Claude Code versions train RLHF on ME-JEPA's verdicts.
- Future external agents calibrate their generation against ME-JEPA's `Gτ`.
- Future ME-JEPA-X domain packs inherit the architecture.

The reward signal is **shareable infrastructure**, not Claude-Code-specific.

---

## Section 5 — Verification claims (these are FALSIFIABLE)

To remain honest, this doc commits to specific empirical claims that future evidence can
falsify:

1. **At N=14 production embedders, the effective signal count exceeds 50.** Pairwise MI
   audit (`mejepa_audit_pairwise_mi`) might reduce 91 pair signals to fewer effective
   ones; the headline 105 may need to be revised down. **Falsifies if MI audit shows
   effective signal count < 50.**
2. **Constellation guard `Gτ` rejection rate correlates with Docker oracle failure
   rate** at correlation ≥ 0.7 on a held-out 50-task set. **Falsifies if correlation
   < 0.5.**
3. **ME-JEPA-vs-oracle prediction agreement reaches ≥ 0.95** within 6 months of /loop
   running with the integration shipped. **Falsifies if agreement plateaus < 0.85.**
4. **Cross-language T(train, deploy) gap is bounded:** Python-trained ME-JEPA evaluating
   Rust patches loses < 30 percentage points on (predict-correct ∩ guard-passes) accuracy
   relative to Rust-only evaluation. **Falsifies if gap > 50 pp.**
5. **The agent-output-verification verdict reproducibility is ≥ 99.9%:** running
   `mejepa.verify(ŷ, context)` twice on the same input returns identical verdicts.
   **Falsifies if non-determinism > 0.1%.**

If any of these falsifications hold, the relevant section gets `status: superseded` and
a corrected version describes what to do instead.

---

## Section 6 — What ships when

Per doc 12 §8 implementation roadmap:

| Quarter | Deliverable | Capability unlocked |
|---|---|---|
| Q1 (current) | Memory decisions + docs 10-13 + specs updated | Operating contract clarified; UTML v1.0 + TCT/DDA + 105-signal capability locked in |
| Q2 | Pairwise-MI audit + `mejepa_compute_panel`/`mejepa_compute_signal_set` MCP tools + cross-language Phase 0 corpus | DDA tightness measured; per-input panel + 105-signal vector callable from agents |
| Q3 | Phase 1-3 instruments + predictor + constellation-aware training | ME-JEPA produces calibrated predictions on the held-out mutation corpus |
| Q4 | Phase 4 `mejepa_verify` MCP tool + `Gτ` guard | Universal agent verifier callable; "verify what the AI did" capability live |
| Q5+ | Phase 5 self-healing + Phase 7 reality-loop integration | ME-JEPA continuously improves from /loop turns; correlation climbs toward 0.95 |
| When correlation ≥ 0.95 | Phase 8 evaluation gate met | "No code review needed" gate achieved on the Q1 ship target |

---

## Section 7 — One-paragraph summary

The 14 production embedders + UTML v1.0 four-term L_step + Royse 2026c TCT/DDA framework
**together** turn every Claude Code action — every edit, every test, every tool call,
every recommendation — into **up to 105 fully-labelled supervisory signals per input**,
structurally outside the Shumailov 2024 collapse regime. ME-JEPA-Code uses these signals
to (a) train its own predictor under multiplicative gating that drops noise and protects
prior knowledge, (b) verify any AI agent's output against frozen "what working code looks
like" constellations using a deterministic geometric guard `Gτ` that requires no
auxiliary learned model, and (c) emit reward signals for downstream RLHF / fine-tuning of
any other agent. Per-output verifiability is the structural property scalar-reward
alignment cannot provide. Cross-language and cross-modality coverage are constellation-
set-selection problems, not retraining problems. The data factory is **infinite in supply
and finite in collapse risk** — a third path around the data wall. **This is the
mechanism by which "software engineers no longer review code" becomes technically
defensible:** they review ME-JEPA's geometric verdict, which is itself re-verifiable
against bytes-on-disk reality.

---

## See also

- `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan.
- `10_MEJEPA_PIVOT_2026_05_09.md` — single-loop architecture.
- `11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 four-term formula.
- `12_EMBEDDER_INTEGRATION.md` — embedder inventory + TCT/DDA mapping.
- `docs2/UTL_v2_embodied.md` — UTML v1.0 — JEPA Edition (the source theory).
- `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md` — locked
  decision.
- Royse, C. (2026a). *ContextGraph Reality Loop*. ResearchGate DOI
  10.13140/RG.2.2.16013.91366.
- Royse, C. (2026b). *Dynamic / ME-JEPA*. ResearchGate DOI 10.13140/RG.2.2.28485.41446;
  Zenodo DOI 10.5281/zenodo.19977981.
- Royse, C. (2026c). *Teleological Constellation Training: Multi-Modal Embedding
  Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path
  Around the Data Wall*. ResearchGate DOI 10.13140/RG.2.2.24370.57288.
