# ME-JEPA — Embedders, Instruments, and Prediction Heads (source-verified)

> **Purpose.** A complete operator-facing reference for every embedder slot, every instrument-panel slot, and every prediction head in ME-JEPA-Code. For each: what it does, why it was chosen, what data its prediction head is fed, the desired outcome, how it works mechanically, and **why what it does works** at the level of the underlying mathematical or epistemic principle.
>
> **Authority.** This doc cross-references **direct source-of-truth files** so the operator can verify each claim:
>
> | What | Source file |
> |---|---|
> | 21-embedder enum, dims, frozen status, model repos | `crates/context-graph-mejepa-embedders/src/embedder_id.rs` |
> | 15-slot panel + PANEL_DIM=5120 + slot dims + offsets | `crates/context-graph-mejepa-instruments/src/lib.rs` (lines 179–270) |
> | Primary Q2 oracle head (Pass/Fail logits over Docker tests) | `crates/context-graph-mejepa/src/oracle_head.rs` |
> | 8 auxiliary heads with SlotAwareHead late-fusion architecture | `crates/context-graph-mejepa/src/predictor/heads.rs` |
> | Q1 claim graph reconciliation | `crates/context-graph-mejepa/src/claim_graph.rs` |
> | Q3 closest-exemplar HNSW + failure fingerprint catalog | `crates/context-graph-mejepa/src/patch_similarity.rs`, `failure_fingerprint.rs` |
> | Q5 reality-impact shift-log replay | `crates/context-graph-mejepa/src/reality_impact.rs` |
> | Doctrinal constraints (frozen, slot identity, ambiguity boundary) | `CLAUDE.md`, `docs/futurebuild/01_overview.md`, `docs/futurebuild/05_predictor_pipeline.md` |
>
> **One paragraph orientation.** ME-JEPA-Code is structured as three layers stacked vertically: (1) **21 embedder slots** that emit dense or sparse vectors from raw code/text/runtime artifacts — 12 are content embedders shipped for the code substrate, 7 are learner-state physiological embedders (post-code domain extension), 2 (E5/E11) are retired empty slots reserved for in-slot causal-embedder replacement. (2) **15 instrument-panel slots** that consume those embedder outputs (plus deterministic graph and oracle signals) and produce the canonical 5,120-byte panel — *always read as an array of 15 per-embedder vectors, never as a flat vector*. (3) **One primary Q2 head** (Pass/Fail oracle prediction) plus **8 auxiliary slot-aware heads** (only `failure_mode` and `edge_case`/`tech_debt` are training-active; Q4-class heads `perf, security, accuracy, cost, reasoning` exist but are frozen at zero loss per #408), plus the explanation surfaces (Q1 claim reconciliation, Q3 closest-exemplar HNSW, Q5 shift-log replay). The product surface is binary `Verdict::Pass | Fail`.

---

## §1. The locked architectural rules (read these first)

Before any embedder or head details, the rules below are non-negotiable; everything below preserves them.

1. **Slot identity is sacred** (`01 §1.6`). The 5,120-byte panel is *byte layout only*. The semantic unit is the array of 15 per-embedder vectors, each in its own embedder's space. Same-embedder cosines are valid (`gtau.rs:14` iterates per embedder); cross-embedder dim compare is forbidden and errors `MEJEPA_INFER_DIM_MISMATCH` (`gtau.rs:90`).
2. **No flat-vector projection** over the packed 5,120 buffer (`Q4_DOCTRINE_FREEZE_ACTIVE = true`; `flat_vector_concat_used = false` Tier-4 invariant). All prediction heads operate via **per-slot encoders → late fusion → per-slot decoders**.
3. **Frozen by design**: E1, E6, E7, E8, E10, E12, E13, E14 receive no gradient updates. Frozen-target adapter has empty `parameters()` (`ΔΞ.target_collapse = 0` invariant).
4. **Binary product contract**. Q1 (claim landed in bytes), Q2 (oracle Pass/Fail), Q5 (shift event occurred) are the three binary predictions. Q3 is **explanation only** (named failure mode + closest exemplar, attached to Q2). Q4 (perf, accuracy, cost, reasoning) is **frozen out of the prediction contract** per #408 — schema lives, producers don't.
5. **No inner LLM.** `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`. Every embedder below is a *frozen encoder* — none generates text/code.
6. **Anti-compensation**. A predictor cannot trade a strong slot for a weak slot via aggregate fusion. Corrupted/missing/stale/dimension-shifted slots remain ship-blocking even when an aggregate score would pass.

---

## §2. The 21 embedder slots

Verified against `crates/context-graph-mejepa-embedders/src/embedder_id.rs` (lines 16–246). Status as of 2026-05-27.

| Slot | Display name | Dim | Kind | Frozen | Status | Model source (`default_repo`) |
|---|---|---|---|---|---|---|
| **E1** | Semantic | 1024 | ContentPretrained | ✅ | active | `intfloat/e5-large-v2` |
| **E2** | TemporalRecent | 512 | ContentDeterministic | tunable | weight-gated; retired for static AST (`#704/#714`) | algorithmic (recency PE) |
| **E3** | TemporalPeriodic | 512 | ContentDeterministic | tunable | weight-gated; retired for static AST | algorithmic (periodic pattern) |
| **E4** | TemporalPositional | 512 | ContentDeterministic | tunable | weight-gated; retired for static AST | algorithmic (session/sequence position) |
| **E5** | Causal | 768 | — | — | **retired empty slot** — #745 replacement candidate (C_TemporalCommit) | (was `nomic-ai/nomic-embed-text-v1.5`) |
| **E6** | Sparse | 1536 | ContentPretrained | ✅ | active | `naver/splade-cocondenser-ensembledistil` |
| **E7** | Code | 1536 | ContentPretrained | ✅ | active | `Qodo/Qodo-Embed-1-1.5B` |
| **E8** | Graph | 1024 | ContentPretrained | ✅ | active (post-`ff4d4c03` distinctness fix) | `intfloat/e5-large-v2` (structural-variant; see §3.E8) |
| **E9** | HDC | 1024 | ContentDeterministic | tunable | active | algorithmic (10,000-bit hyperdimensional projected to 1024-d) |
| **E10** | Contextual | 768 | ContentPretrained | ✅ | active (dual vectors: doc/query) | `intfloat/e5-base-v2` |
| **E11** | Kepler | 768 | — | — | **disabled empty slot** — #745 replacement candidate (C_CommonsenseCausal) | (was `THU-KEG/KEPLER-Wiki5M-KE`) |
| **E12** | LateInteraction | 128 per token | ContentPretrained | ✅ | active | `colbert-ir/colbertv2.0` |
| **E13** | SpladeV3 | 1536 | ContentPretrained | ✅ | active (sparse vocab expansion) | `prithivida/Splade_PP_en_v1` |
| **E14** | BgeM3Dense | 1024 | ContentPretrained | ✅ | active; #745 replacement candidate (C_BugFix) pending #746 audit | `BAAI/bge-m3` |
| **E15** | AffectSpeech | 1024 | LearnerState | trained | learner-state domain extension | `msp-dim/wav2vec2-large` |
| **E16** | AffectFace | 512 | LearnerState | trained | learner-state domain extension | `OpenFace-3.0+MTL-local` |
| **E17** | AffectText | 384 | LearnerState | trained | learner-state domain extension | `MiniLM-L6-v2+EmoBank-local` |
| **E18** | PPG | 512 | LearnerState | trained | learner-state (photoplethysmography → heart rate) | `PaPaGei/ppg-foundation` |
| **E19** | EDA | 64 | LearnerState | trained | learner-state (electrodermal activity → stress) | `WESAD/eda-stress-head` |
| **E20** | EEG | 768 | LearnerState | trained | learner-state (electroencephalogram foundation) | `LaBraM/eeg-foundation` |
| **E21** | EegArtifactRobust | 768 | LearnerState | trained | learner-state (artifact-robust EEG) | `EEGPT/eeg-artifact-robust` |

**Selection rules from source:**
- `EmbedderId::all().len() == 21`
- `required_registrations().len() == 19` (everything except E5/E11)
- `content().len() == 12` (E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14)
- `learner_state().len() == 7` (E15–E21)
- `is_retired() == true` for E5 and E11 only

The current ME-JEPA-Code ship gate (Python on SWE-bench Lite) reads **only the 12 content embedders**. The 7 learner-state embedders (E15–E21) are domain-extension scaffolding for a separate application surface and do not feed the code prediction pipeline.

### §2.1 Why each content embedder is in the panel — purpose, signal, choice rationale, mechanism, and why it works

---

#### **E1 — Semantic (e5-large-v2, 1024-d, frozen) ✅**

**What it does.** Dense semantic embedding of natural-language and free-form text — issue descriptions, problem statements, docstrings, commit messages, test descriptions, error messages.

**Signal it produces.** A 1024-d L2-normalizable vector where same-meaning text clusters together regardless of surface form (paraphrase, word order, vocabulary substitution).

**Why this model was chosen.** `intfloat/e5-large-v2` is the canonical generalist text embedder of the 2024 retrieval era — Pareto-optimal on MTEB across STS, retrieval, clustering, and classification. The asymmetric "query: / passage:" prefix discipline lets it serve as both retriever and labeller without re-training. It is freely available, BSD-licensed, and reproducible from a published Hugging Face checkpoint.

**What its head is fed.** E1 appears in three instrument slots (E_Test, E_Problem, E_CommitMsg — see §3) and contributes to the base PanelB cross-panel Goodhart guard. The auxiliary heads downstream consume the per-slot tensor `predicted.tensor.narrow(1, offset, dim)` — so the failure_mode head, for example, sees the slice of the predicted panel corresponding to E_Problem(1024-d) projected through its own slot-specific Linear layer.

**Desired outcome.** Semantically-near patches end up with semantically-near E1 vectors → closest-exemplar HNSW retrieval (Q3) returns historically similar failures.

**How it works.** Transformer encoder (~335M params); CLS-token pooled, L2-normalized output. Frozen forward pass: `text → tokenize → forward → 1024-d vector`. No fine-tuning at the embedder level.

**Why it works.** E1 inherits the *contrastive-learning prior* that "passages and their relevant queries should be close in vector space." For code-debugging tasks, the same prior gives "issue description and the bug fix should be close" with no domain-specific training. The slot's role in ME-JEPA is to give the predictor an axis along which "is this patch addressing the issue described?" becomes a geometric question rather than a parsing question.

---

#### **E2 — TemporalRecent (algorithmic, 512-d) — weight-gated, retired for static AST**

**What it does.** Encodes a memory's age via *sinusoidal positional encoding* — converts wall-clock `created_at` into a 512-d vector where recency manifests as a smooth, multi-scale rotation in phase space.

**Signal it produces.** "How recently was this observed?" — distinct from "what is this?" Two semantically identical memories created an hour apart have nearly-identical E2 vectors; two created six months apart are distant.

**Why this design.** Multi-scale sinusoidal PE is the same primitive transformers use for token positions — proven to give downstream models a *continuously interpolable* notion of distance. Alternative designs (exponential decay, bucket encoding, time-since-epoch scalar) compress all the information into one or a few dimensions and lose the multi-scale structure.

**What its head is fed.** Only via downstream MultiSpace fusion when weight > 0.0. Currently retired from the static-AST forward cache per `#704/#714` (the AST of a function does not have a "creation time" in a way that's meaningfully temporal). E2 remains live for runtime-trace-style data where temporal evidence is real.

**Desired outcome.** "Recent" failures rank above "ancient" ones at equal semantic distance — so the predictor can model that a regression introduced *last week* is more relevant than the same shape introduced *two years ago*.

**How it works.** Deterministic, no model. `timestamp_to_position(t) → sin/cos(t * ω_k)` for `k = 0..256` distinct frequencies `ω_k = 1 / 10000^(k/d_model)`.

**Why it works.** Sinusoidal PE solves the "no natural ordering" problem of timestamps in a way that downstream linear layers can decode losslessly (Fourier transform inverse). Equivalent to a continuous compositional encoding of time.

---

#### **E3 — TemporalPeriodic (algorithmic, 512-d) — weight-gated, retired for static AST**

**What it does.** Encodes *cyclical* temporal patterns — hour-of-day, day-of-week, day-of-month, day-of-year. Multi-frequency Fourier-style projection over each cycle.

**Signal it produces.** "What time of day / day of week did this happen?" — orthogonal to recency. A bug that always appears at 3am cron-time has a stable E3 vector; a bug that's uniformly distributed in time has a low-norm E3 vector.

**Why this design.** Periodic behavior is real in production systems (cron jobs, time-of-day load patterns, batch windows). A single linear-time embedding (E2) cannot represent "every Monday at 9am." Decomposing into harmonics is the smallest faithful representation.

**What its head is fed.** Same MultiSpace fusion as E2; weight-gated; retired for static AST.

**Desired outcome.** Failures that pattern-match a periodic regime are recognized even when surface evidence (test outputs, traceback) differs across instances.

**How it works.** For each cycle (hour, day, week, etc.), emit `[sin(2π·t/T_cycle), cos(2π·t/T_cycle), sin(4π·t/T_cycle), cos(4π·t/T_cycle), …]` at multiple harmonics. Concatenate across cycles to 512-d.

**Why it works.** Any continuous periodic function on a circle is uniquely identified by enough of its Fourier coefficients. By using multiple harmonics per cycle and multiple cycles, the embedding can geometrically discriminate any reasonable periodic pattern.

---

#### **E4 — TemporalPositional (algorithmic, 512-d) — weight-gated, retired for static AST**

**What it does.** Encodes a memory's *sequence position* within a session, episode, or conversation turn — independent of wall-clock time.

**Signal it produces.** "Which step of the session was this?" The 3rd edit in a session has a stable E4 vector regardless of how many wall-clock seconds elapsed between steps 2 and 3.

**Why this design.** Conversation/session order is causally relevant in a way wall-clock time is not — "the agent claimed X, then edited Y, then saw error Z" is a sequence, not a timestamped continuous trajectory. E4 captures the sequence; E2 captures the time.

**What its head is fed.** MultiSpace fusion, weight-gated, retired for static AST.

**Desired outcome.** "Step-3 of a session" failures cluster with other step-3 failures across sessions.

**How it works.** Same multi-scale sinusoidal PE as E2 but over *position index* rather than timestamp.

**Why it works.** Same Fourier argument as E2. The orthogonality of position-from-time is what justifies having both E2 and E4.

---

#### **E5 — Causal (768-d) — RETIRED empty slot, #745 replacement candidate**

**Status.** `is_retired() == true`. Slot reserved for in-slot causal-embedder replacement per #745 TASK-CR-007 — proposed replacement is **C_TemporalCommit** trained on CommitPack 4TB on aiwonder. Empty until #744 + #746 + #715 prerequisites discharge.

**Why retired.** The original `nomic-ai/nomic-embed-text-v1.5` causal-asymmetric design did not produce structurally distinct vectors from E1 on the code substrate — the panel had two near-aliased semantic slots, which trips the inter-embedder distinctness gate (#709). Better to keep the slot frozen-empty until a true cause→effect directional embedder replaces it.

---

#### **E6 — Sparse (SPLADE-distil, 1536-d, frozen) ✅**

**What it does.** Sparse lexical-expansion embedding — projects text into a vocabulary-sized space (30K terms collapsed to a 1536-d dense surface) where each dimension corresponds to a learned vocabulary term and the value is the term's *learned importance* given context.

**Signal it produces.** "What keywords are essential to this text, including synonyms the surface text doesn't contain?" E6 will activate `assertion_error` for a stack trace mentioning `AssertionError` even though those are different tokens.

**Why this model was chosen.** SPLADE bridges classical lexical retrieval (BM25, term-frequency) with neural relevance. It catches *keyword-level* matches that dense embedders blur (variable names, exception classes, error codes). The distillation variant (`cocondenser-ensembledistil`) is fast and trained on MS-MARCO.

**What its head is fed.** Same as E1 — appears in multiple instrument slots, slot-projected linearly into per-head logits.

**Desired outcome.** Bug-fix patches whose tokens overlap key error-message tokens (`KeyError`, `IndexError`, `recursion`) rank above patches that are merely semantically close.

**How it works.** BERT-style encoder + MLM head + log-saturation: `score(term) = log(1 + ReLU(MLM_logits))`. Sparse vector, max-pooled across input tokens, top-K kept.

**Why it works.** Dense embedders lose information in the *direction* of frequent keywords (the gradient is small near the bulk of the manifold). SPLADE explicitly recovers it as a multi-hot lexical expansion. The complementarity with E1 is the central design rationale.

---

#### **E7 — Code (Qodo-Embed-1-1.5B, 1536-d, frozen) ✅**

**What it does.** Code-native dense embedding — produces vectors where syntactically distinct but semantically equivalent code (refactors, renamed variables, equivalent control flows) clusters together.

**Signal it produces.** "Is this code doing the same thing as that code?" — at the function-level granularity.

**Why this model was chosen.** Qodo-Embed-1 is, as of 2026, the top open code embedder on CoIR (Code Information Retrieval), beating CodeT5, CodeBERT, and UniXcoder on every code-search benchmark. 1.5B parameters with explicit code-pretraining via Qodo's internal pipeline. 1536-d output gives enough capacity for fine-grained function-level disambiguation.

**What its head is fed.** E7 is part of **E_AST(384)** via projection — the original 1536-d vector is reduced through the instrument-side encoder before joining the panel. Also feeds into Q3 closest-exemplar HNSW (`patch_similarity.rs:25` — structural signature combines E_AST + E_CFG + E_DataFlow + E_TypeGraph at 384+256+256+256 = 1152-d).

**Desired outcome.** Two patches that *fix the same bug differently* (e.g., one adds a guard, one rewrites the loop) end up similarly placed in E7 space because the *effect* is similar.

**How it works.** Transformer encoder pretrained on contrastive code-pair tasks (positive: same function in different versions; negative: random function from different repo). L2-normalized 1536-d.

**Why it works.** E1 (general-purpose text) doesn't know that `range(n)` and `enumerate(list)` are nearly equivalent. E7 was trained on hundreds of millions of code pairs that taught it equivalence. The frozen prior is a code-specific equivalence relation the predictor can geometrically query.

---

#### **E8 — Graph (e5-large-v2 structural variant, 1024-d, frozen) ✅**

**What it does.** Graph-aware text embedding — same backbone as E1 but trained/used on *graph-serialized* representations of code (function bodies serialized as edge lists, control-flow paths as token sequences).

**Signal it produces.** "What is the structural shape of this code?" — independent of variable names or comments.

**Why this design.** A single embedder over raw text cannot distinguish `O(n)` from `O(n²)` if the surface tokens are similar. Serializing the AST/CFG into a structured string and re-embedding gives the predictor a structural axis.

**What its head is fed.** Same as E1/E7 — slot-projected into the panel via E_CFG / E_DataFlow / E_TypeGraph instrument slots.

**Desired outcome.** Patches with similar control-flow shape (loop-inside-conditional vs flat-sequential) cluster together regardless of variable naming.

**How it works.** AST → canonical edge-list serialization (`enter_function -> if_then -> loop -> return`) → e5-large-v2 forward → 1024-d.

**Why it works.** The structural serialization is *the same string for two refactors of the same function with different names*, which means E8 is invariant to the naming change but covariant with the structural change — the exact orthogonality E1 alone could not provide. The post-`ff4d4c03` fix and v2 9-slot static AST cache (#715) restore inter-embedder distinctness with E1 (which had previously aliased per #709).

---

#### **E9 — HDC (hyperdimensional computing, 1024-d, algorithmic) — tunable, active**

**What it does.** Hyperdimensional binary encoding projected to 1024-d — typo-robust, surface-form-robust text representation. Each character/n-gram contributes a high-dimensional random-projection vector; the bundle is the text's HDC fingerprint.

**Signal it produces.** "What is the rough shape of these tokens, robust to misspellings, character-level noise, and minor variations?" — bridges between exact lexical match (E6/E13) and dense semantic match (E1).

**Why this design.** HDC has the property that bundling many high-dimensional random vectors preserves *similarity-via-Jaccard-of-tokens* with very high redundancy. Single-character typos perturb the HDC vector by ~1/n_tokens — much less than they perturb tokenized lexical embeddings.

**What its head is fed.** Slot-projected like other embedders; primarily contributes to blind-spot recovery — the `cg-blind-spot` MCP surface specifically routes through E9.

**Desired outcome.** Stack traces, error messages, and bug reports with typos, OCR artifacts, or paraphrasing still match — the predictor doesn't fail closed on cosmetic noise.

**How it works.** 10,000-bit Plate-style HDC: each token → random binary hypervector; bind / bundle via XOR / majority-vote; project to 1024-d via fixed random matrix.

**Why it works.** Random high-dim projections preserve relative distances in expectation (Johnson-Lindenstrauss lemma). HDC adds the bind/bundle algebra so token order and bag-of-tokens can both be represented. The result is a low-information-loss, high-robustness representation that occupies a real axis no other embedder covers. (Distinctness defect #716 is patched but pending v2 cache rebuild.)

---

#### **E10 — Contextual / Paraphrase-asymmetric (e5-base-v2, 768-d, frozen, dual vectors) ✅**

**What it does.** Asymmetric paraphrase-aware embedding — stores **two vectors** (doc and query) per memory, optimized so paraphrases of the same content map onto each other's space.

**Signal it produces.** "Are these two pieces of text saying the same thing, even if rephrased?" — distinct from E1's "are these about the same topic?"

**Why this model was chosen.** e5-base-v2 is the smaller, asymmetric-trained cousin of E1. The asymmetric variant explicitly trains separate encoders for doc/query, which produces a stronger paraphrase prior than the symmetric large variant.

**What its head is fed.** Same as E1; slot-projected. The dual-vector property lets the predictor distinguish *what is being claimed* (query side) from *what was actually done* (doc side) when reconciling agent claims against bytes.

**Desired outcome.** The agent's natural-language claim "I added bounds checking" matches the actual diff (which says `if i >= len(arr):`) even though the surface tokens are entirely different.

**How it works.** Two-tower e5: `query_encoder` and `doc_encoder` with shared backbone but different prefix tokens. L2-normalized 768-d output, indexed twice per memory.

**Why it works.** Symmetric encoders (like E1) drift toward "topical similarity" because their training pairs are symmetric. Asymmetric training (query vs document, with hard-negative mining) gives the encoder an explicit signal for "rephrase of," which is exactly what bridging agent claims to code edits requires.

---

#### **E11 — Kepler (768-d) — DISABLED empty slot, #745 replacement candidate**

**Status.** Reserved-disabled. Slot proposed for **C_CommonsenseCausal** replacement per #745 — would train on CauseNet 11M + ConceptNet + ATOMIC corpora on aiwonder.

**Why disabled.** The original KEPLER (knowledge-graph entity embedder, TransE-style) is structurally a *graph embedding* over Wikipedia entities — not aligned with code-substrate signal. Audit-pending utilization showed near-zero downstream lift; better to repurpose the slot once a causal-commonsense embedder is trained.

---

#### **E12 — LateInteraction (ColBERTv2, 128 per token, frozen) ✅**

**What it does.** Per-token late-interaction embedding — each input token gets its own 128-d vector. Retrieval similarity is `MaxSim`: for each query token, find the max cosine over all document tokens, sum.

**Signal it produces.** "Token-level fine-grained relevance" — catches cases where one rare keyword in a long document drives relevance, which CLS-pooled embedders (E1) wash out.

**Why this model was chosen.** ColBERT-v2 is the canonical late-interaction retriever. Token-level granularity at low per-token dimensionality (128-d) is the right trade-off — you pay a per-token storage cost but gain fine-grained lexical-semantic discrimination.

**What its head is fed.** Slot-projected; primary value to Q3 exemplar retrieval where the predictor checks for any token-level match between candidate patch and known failure modes.

**Desired outcome.** A patch that contains the exact token `recursion_limit` is recognized as relevant to a `RecursionError` failure mode even if the surrounding semantic context is different.

**How it works.** BERT encoder; final layer per-token outputs (not pooled); each token L2-normalized to 128-d. At retrieval time: `score(q, d) = sum over q_tok of max over d_tok of cosine(q_tok, d_tok)`.

**Why it works.** Pooled embeddings (E1, E7, E10) average over tokens, losing the strength of any single highly-relevant token. Token-level MaxSim recovers it exactly. The price is more storage and more compute at retrieval; the gain is precision on long documents and rare-keyword queries.

---

#### **E13 — SpladeV3 (1536-d, frozen) ✅**

**What it does.** Successor to E6 — a more recent SPLADE checkpoint with improved sparse expansion. Same architecture as E6 (vocabulary-projected, ReLU+log saturation) but a different training mix.

**Signal it produces.** Same family as E6 — lexical sparse expansion — but with a slightly different vocabulary activation pattern. Used in pipeline retrieval as the initial sparse-recall stage (`E13 → E1 → MultiSpace rerank → E12 MaxSim`).

**Why both E6 and E13.** Two SPLADE checkpoints with different training mixes provide a small but real ensemble signal on lexical relevance. The pipeline architecture explicitly uses E13 first because of its slightly better recall-at-low-K.

**What its head is fed.** Same as E6 — slot-projected.

**Why it works.** Same mechanism as E6 — but the second checkpoint covers slightly different rare-vocabulary terms because of the training mix, so the union is genuinely informative.

---

#### **E14 — BgeM3Dense (1024-d, frozen) ✅**

**What it does.** BAAI's BGE-M3 dense embedding — multilingual, multi-granularity (sentence + paragraph + document), multi-functional (dense + sparse + multi-vector heads). ME-JEPA uses the dense head.

**Signal it produces.** A second dense semantic axis distinct from E1 — different pretraining mix and different model family, so the geometry is genuinely independent.

**Why this model was chosen.** Cross-panel Goodhart guard (#405) needs a non-overlapping Panel B encoder. BGE-M3 is BGE-family (trained at BAAI on a non-MTEB-leaderboard mix) and `bge-large-en-v1.5` is the canonical Panel B candidate for the E1 slot. E14 staying live in Panel A gives the predictor a second-opinion dense signal at training time.

**What its head is fed.** Slot-projected like other embedders.

**Desired outcome.** When E1 and E14 disagree about semantic similarity, the disagreement is itself an informative signal — it activates the constellation-disagreement-pressure head (#390).

**How it works.** Standard transformer encoder, dense head only. L2-normalized 1024-d.

**Why it works.** Two independently-pretrained dense encoders, asked the same question, give correlated-but-not-identical answers. The mutual-information audit (RO-8) is what would measure how much of that disagreement is signal vs noise; the working hypothesis is enough.

---

### §2.2 The 7 learner-state embedders (E15–E21) — domain extension

These do **not** feed the Python ship gate. They exist for a learner-state / wearable / affective-computing domain extension where ME-JEPA is instantiated per `SuperIntelligence.md` §3 against a different `D`. Brief reference:

| Slot | Source | What it captures |
|---|---|---|
| **E15** AffectSpeech | wav2vec2-large + MSP-DIM emotion fine-tune | Prosody / emotion from spoken audio |
| **E16** AffectFace | OpenFace 3.0 + multi-task local head | Facial action units → emotion |
| **E17** AffectText | MiniLM-L6-v2 + EmoBank fine-tune | Affect/valence/arousal in written text |
| **E18** PPG | PaPaGei foundation model | Photoplethysmography → heart-rate variability |
| **E19** EDA | WESAD stress-detection head | Electrodermal activity → arousal/stress |
| **E20** EEG | LaBraM EEG foundation model | Brain activity baseline encoder |
| **E21** EegArtifactRobust | EEGPT artifact-robust head | EEG with muscle/eye-artifact suppression |

These slots demonstrate the universal-predicate (`SuperIntelligence.md` §0): the same architecture (frozen-encoder-array panel + binary predictor + cross-panel guard) extends to any domain where (a) a deterministic oracle exists, (b) the question reduces to free binary form, (c) signals are slot-decomposable. For learner-state, the oracle would be a deterministic test-outcome / engagement-event function; the binary question is `did_learning_event_occur(observation_window) → {Yes, No}`.

---

## §3. The 15 instrument-panel slots

Verified against `crates/context-graph-mejepa-instruments/src/lib.rs:187–243`. **`PANEL_DIM = 5120` bytes contiguous layout; semantic unit is array of 15 per-embedder vectors.**

| # | Slot | Dim | Offset | What it encodes | Backing embedder(s) | Why this slot exists |
|---|---|---|---|---|---|---|
| 1 | `EAst` | 384 | 0 | Tree-sitter AST structure of the candidate patch | E7 (Qodo-Embed) projected to 384-d | Syntactic shape — captures "what did this patch do at the parse-tree level" |
| 2 | `ECfg` | 256 | 384 | Control-flow graph attention encoding | E8 (Graph) projected to 256-d | Control-flow shape — what branches, loops, returns changed |
| 3 | `EDataFlow` | 256 | 640 | Def-use graph attention encoder | E8 + deterministic def-use graph | Data-dependency shape — what variables flow into what |
| 4 | `ETypeGraph` | 256 | 896 | Type signature + inferred-type graph | E8 + tree-sitter types | Type-level shape — what type signatures changed |
| 5 | `ETest` | 384 | 1152 | Test descriptions (natural language) | E1 (e5-large-v2) projected to 384-d | What the tests *claim* to verify |
| 6 | `ETrace` | 512 | 1536 | Execution-trace sequence | Custom transformer over trace events | What the runtime *actually did* — stack frames, calls, exceptions |
| 7 | `EDiff` | 256 | 2048 | AST-diff edge encoder | Custom over (before, after) AST | The *delta* — what changed structurally |
| 8 | `EWitness` | 256 | 2304 | Witness-chain entry sequence | Custom over signed witness events | Provenance — who did what when, ed25519-signed |
| 9 | `EOracle` | 128 | 2560 | One-hot test outcomes + exception-class one-hot | Deterministic encoding (no model) | The Q2 ground truth — what the Docker oracle said |
| 10 | `EProblem` | 1024 | 2688 | SWE-bench problem text (issue description) | E1 (e5-large-v2) | What the user wanted fixed |
| 11 | `ECommitMsg` | 384 | 3712 | Candidate fix commit message | E1 projected to 384-d | What the agent *claimed* to do |
| 12 | `EStaticAnalysis` | 256 | 4096 | Linter / typecheck / complexity aggregates | Deterministic aggregator | What linters and type-checkers say independent of runtime |
| 13 | `ERuntime` | 256 | 4352 | Wall-clock / RSS / coverage / network signal aggregates | Deterministic aggregator | What runtime *cost / coverage* of the patch was |
| 14 | `EReasoning` | 384 | 4608 | Reasoning / reflexion-trace encoder | E1 projected to 384-d | The agent's stated reasoning |
| 15 | `Scalars` | 128 | 4992 | BFS depth, blame age, churn, coverage Δ, repo-health | Deterministic packed scalars | Cheap repo-state features |

**Total: 5,120 contiguous f32 bytes; 15 array slots; one per slot, never fused.**

### §3.1 Why these 15 specific slots — the design logic

The slot set was chosen to satisfy three constraints simultaneously:

1. **Cover the consequence-prediction contract (#397).** The predictor must observe explicit task text (E_Test, E_Problem), agent claims (E_CommitMsg, E_Reasoning), edits (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff), AST deltas (E_Diff), oracle outcomes (E_Oracle), and verification feedback (E_Trace, E_Witness, E_StaticAnalysis, E_Runtime, Scalars). No hidden human intent (forbidden by #397).
2. **Preserve slot identity / anti-compensation.** Each slot lives in its own embedder's space — no two slots share a coordinate system. This is why E_Test (E1@384) and E_Problem (E1@1024) are *separate slots even though they share an embedder* — they get separate Linear projections, separate calibration, separate OOD detection, and a strong-slot cannot compensate for a weak-slot through aggregate fusion.
3. **Match leakage policy.** Target-side labels (oracle/Docker/test outcomes) are tagged `leakage_policy: target_side_only`. E_Oracle is allowed only as a *supervision/reward target*, never as a live predictor input. The predictor sees E_Oracle only during training; at inference, it must predict E_Oracle from the other 14 slots.

### §3.2 Why the panel is an *array*, not a *fused vector*

This is the single most-doctrinally-loaded architectural choice in the project. Stated as a theorem:

> **If the panel is fused into a flat 5,120-d vector before the head, any strong slot can numerically compensate for a corrupted/missing/stale weak slot in the predictor's logits. The predictor passes when it should fail — a Goodhart failure mode.**

The fix: per-slot encoders → late fusion → per-slot decoders. Each slot is projected to head logits *independently*, then logits are concatenated and passed through a *late projection* (`SlotAwareHead::late_projection` at `heads.rs:149-153`). The slot identity is preserved through the head, so a single corrupted slot is observable in the per-slot logits trace.

The structural collapse-immunity invariant `ΔΞ.target_collapse = 0` (`PRED_FROZEN_TARGET_GRAD` / `MEJEPA_INSTRUMENT_GRADIENT_LEAK`) extends this: the *target* panel has zero trainable parameters, so the predictor cannot learn to collapse its target to match its prediction.

---

## §4. The prediction heads

ME-JEPA has **one primary binary head** (Q2 oracle), **eight slot-aware auxiliary heads** (FailureMode/EdgeCase/TechDebt + Q4 frozen: Perf/Security/Accuracy/Cost/Reasoning), and **three explanation surfaces** (Q1 claim reconciliation, Q3 closest-exemplar HNSW, Q5 shift-log replay).

### §4.1 Primary Q2 — `OracleHead` (Pass/Fail oracle prediction)

**Source:** `crates/context-graph-mejepa/src/oracle_head.rs`.

**What it does.** For a predicted panel of shape `(B, 5120)`, projects to `(B, num_tests + 1)` oracle logits — one logit per Docker test in the oracle, plus one for the "overall verdict" class. Softmax over this gives oracle probabilities.

**What is fed to it.** The output of `MeJepaPredictor.forward()` — the predicted panel `(B, 5120)`. This is the slot-preserving late-fused output, **not** the raw input panel. The head does *not* see ground truth at inference; it predicts.

**Desired outcome.** `argmax(softmax(logits)) == oracle_outcome` on the holdout set, with raw Pearson `corr(predicted_pass_probability, oracle_pass) ≥ 0.95` for the ship gate (Tier-1 #1 in `TARGET-METRICS.md`).

**How it works.** Single `Linear(PANEL_DIM, num_tests + 1)` (~5,121 × num_tests params). At inference: `logits = predicted_panel @ W^T + b`, then softmax. `predict_probabilities` validates dims, errors `MEJEPA_INFER_DIM_MISMATCH` on shape mismatch, and `ensure_finite` errors on NaN/Inf.

**Why it works.** The predictor's job is to *learn the mapping from observed evidence (the panel) to oracle outcome*. Because all 15 slots are present and identity-preserved through the predictor trunk, the head sees a representation where Q2-relevant axes are linearly separable in expectation (this is the geometric promise of contrastive/JEPA-style pretraining). The OracleHead is deliberately tiny — most of the work is done by the per-slot encoders and the transformer trunk; the head itself is the readout.

**Note on `num_tests + 1`.** The `+1` is the catch-all "the oracle did not even run / could not parse" class — important so the predictor can model `error` outcomes distinct from `fail`. Per `01 §1`: `OracleHead` outputs `OracleLogits over Pass/Fail/Skip/Error`.

### §4.2 Auxiliary heads — `AuxiliaryHeads` with `SlotAwareHead` design

**Source:** `crates/context-graph-mejepa/src/predictor/heads.rs`.

**Eight heads, eight class counts:**

| Head | Classes | Status | Source class count |
|---|---|---|---|
| `failure_mode` | 36 | **active** — Q3 explanation surface | `FAILURE_MODE_CLASSES = 36` |
| `edge_case` | 23 | active | `EDGE_CASE_CLASSES = 23` |
| `tech_debt` | 21 | active | `TECH_DEBT_CLASSES = 21` |
| `perf` | 17 | **Q4 frozen** (#408) | `PERF_CLASSES = 17` |
| `security` | 20 | **Q4 frozen** (#408) | `SECURITY_CLASSES = 20` |
| `accuracy` | 13 | **Q4 frozen** (#408) | `ACCURACY_CLASSES = 13` |
| `cost` | 11 | **Q4 frozen** (#408) | `COST_CLASSES = 11` |
| `reasoning` | 8 | **Q4 frozen** (#408) | `REASONING_CLASSES = 8` |

**The `SlotAwareHead` architecture (`heads.rs:136-176`).** Each head is *not* a `Linear(5120, classes)` flat projection. That would violate the anti-compensation invariant. Instead:

1. **15 per-slot projections.** One `Linear(slot.dim(), class_count)` per slot — so `failure_mode` has 15 separate linear layers, e.g. `Linear(384, 36)` for E_AST, `Linear(256, 36)` for E_CFG, …
2. **Late projection.** A single `Linear(class_count * 15, class_count)` combines per-slot logits into final logits.
3. **Slot-logit contributions preserved.** Per-slot logits are kept as `SlotLogitContribution { slot_id, logits, class_count }` so per-slot attribution is observable post-hoc.

**Pseudocode for the forward pass (`heads.rs:162-176`):**
```rust
for each slot in 15 slots:
    slot_tensor = predicted.tensor.narrow(1, slot.offset(), slot.dim())   // slice the panel
    slot_logits = slot.projection.forward(slot_tensor)                    // Linear(slot.dim, classes)
    record SlotLogitContribution
concatenated = concat(slot_logits across 15 slots)                         // (B, 15 * classes)
final_logits = late_projection.forward(concatenated)                      // Linear(15*classes, classes)
final_probs = softmax(final_logits)
```

**Why this design (not flat Linear(5120, classes)).** Two reasons stated above; restated:
- **Anti-compensation.** A strong E_Oracle slot cannot mask a corrupted E_AST slot when the per-slot logits are computed independently. The per-slot Linear forces each slot to *individually contribute its share of the classification signal*.
- **Per-slot attribution.** The `SlotLogitContribution` trace lets the operator answer "*which slot drove this prediction*?" — a question a flat projection structurally erases.

#### §4.2.1 `failure_mode` head (36 classes) — active, Q3 explanation surface

**What is fed.** The predicted panel (post-trunk, slot-preserving), narrowed per-slot.

**Desired outcome.** When `OracleHead → Fail`, the failure_mode head's argmax names the *kind* of failure: `subtle_flip_invariant_violation`, `off_by_one_boundary_miss`, `wrong_file`, `mock_only_change`, `solution_leak`, `incorrect_fix_weak_test`, etc. The 36 classes are the leaf level of the 4-level failure-mode hierarchy (`#404`: 8 Level-1 groups → 157 Level-2 skills → 379 Level-3 constellations → 536 named predictions; this head reads to Level-3 / Level-4 from per-slot evidence).

**How it works.** Per-slot Linear over the 15 panel slots → concat 15 × 36 = 540 logits → late projection to 36 → softmax.

**Why it works.** Failure modes manifest *differently in different slots*. `subtle_flip` shows up most strongly in E_AST and E_Diff (one boolean operator flipped). `wrong_file` shows up in E_AST (huge structural change) + E_Diff (large) + E_Reasoning (incoherent rationale). `solution_leak` shows up in E_CommitMsg/E_Problem cosine. Letting each slot contribute its own evidence to each class, then aggregating, gives a fail-mode classifier that doesn't paper over the slot signal.

#### §4.2.2 `edge_case` head (23 classes) — active

**Desired outcome.** Classify the patch into a known edge-case taxonomy: `empty_input`, `null_handling`, `zero_division_boundary`, `unicode_boundary`, `concurrency_race`, `negative_input`, `large_input_overflow`, etc. (23 total).

**Why it works.** Edge-case classes are stable enough across repos that a per-slot read finds them. E_TypeGraph + E_StaticAnalysis are the strongest signal slots for edge-case classification.

#### §4.2.3 `tech_debt` head (21 classes) — active

**Desired outcome.** Identify the technical-debt signature of the patch: `god_function_grew`, `cyclomatic_jumped`, `comment_lying`, `dead_branch_added`, `magic_constant_introduced`, etc.

**Why it works.** Static-analysis-friendly classes — E_StaticAnalysis and E_CFG are the carrying slots; the per-slot SlotAwareHead pulls them out.

#### §4.2.4 Q4 heads (perf 17, security 20, accuracy 13, cost 11, reasoning 8) — **frozen per #408**

These heads exist in the trained network *but contribute zero loss* during training. Per `CLAUDE.md §15` and `TARGET-METRICS.md §4`:

- `Q4_LOSS_COEFFICIENT_MAX = 0.0` (exactly)
- `q4_inputs_to_ship_gate() == 0` (exactly)
- `q4_row_count_delta` weekly = 0 (exactly)

**Why they exist if they contribute nothing.** Architectural rollback path. If a future decision unfreezes Q4 (a doctrine change), the head infrastructure exists and the persisted CFs (`CF_MEJEPA_Q4_*`) hold historical rows. The freeze is enforced at the loss-coefficient and trust-gate levels (`q4_trust_gate.rs::Q4_DOCTRINE_FREEZE_ACTIVE = true`), not by removing the heads.

**Why they're frozen.** Per `05 §0`: ambiguous heads (perf, accuracy, cost, reasoning class) have **no free binary form against reality**. Continuous quantities (perf wall-clock) only become binary via arbitrary thresholds; thresholds are heuristic, not reality; mistake-loop convergence requires real disagreement with reality, not threshold artifacts. Q4 prediction would be uncalibratable against ground truth that doesn't exist.

### §4.3 Q1 — `claim_graph.rs` (claim reconciliation, not a neural head)

**Source:** `crates/context-graph-mejepa/src/claim_graph.rs`.

**What it does.** Deterministic parser of agent natural-language claims ("I added a guard clause at line 47") → reconciliation against bytes/AST on disk.

**Input.** Agent's response text + repo state at `repo_after_sha`.

**Output.** `ClaimReconciliationRecord` — per-claim Y/N: did this claim land in bytes?

**How it works.** Regex + AST-based claim extraction → `AgentClaimGraph` of structured (file, line, action) triples → check each triple against actual diff between `repo_before_sha` and `repo_after_sha`. Default tolerance: `CLAIM_GRAPH_DEFAULT_LINE_WINDOW = 3` lines.

**Why it works.** Q1 is binary against an oracle that already exists — the disk. There is no learning needed; the question "does the file at path X contain the line at Y after my edit?" is a `read + ast_diff` away. The claim parser is deliberately conservative — if a claim is ambiguous, it errors rather than guessing.

**Why no neural head.** A neural head over claim text could hallucinate matches. Determinism is the safety property.

### §4.4 Q3 — `patch_similarity.rs` (closest-exemplar HNSW) + `failure_fingerprint.rs` (named failure mode)

**Source:** `crates/context-graph-mejepa/src/patch_similarity.rs`, `failure_fingerprint.rs`.

**What it does.** When `OracleHead → Fail`, retrieve the K-nearest historical patches by structural signature, and tag the prediction with a `FailureShapeFingerprint`.

**Input to the HNSW.** `PATCH_SIMILARITY_STRUCTURAL_DIM = 384 + 256 + 256 + 256 = 1152-d` — the per-slot concatenation of `E_AST + E_CFG + E_DataFlow + E_TypeGraph` from the candidate patch's panel.

**Output.** `PatchSimilaritySearchOutcome` — top-K exemplars with metadata (`PatchExemplarRecord` — patch hash, repo, instance_id, oracle outcome, named failure mode if known).

**How it works.**
1. Build index: every training-corpus patch's panel produces a 1152-d signature; persist to `CF_MEJEPA_PATCH_SIMILARITY_META` + `patch_similarity_hnsw.usearch`.
2. At inference: candidate patch → panel → 1152-d signature → HNSW K-NN search → top-K `PatchExemplarRecord`.
3. Cross-check against `FailureShapeFingerprint` catalog — if K-nearest neighbours all share a fingerprint, attach it.

**Default `K = 5`** (`PATCH_SIMILARITY_DEFAULT_K`).

**Why it works.** Failure modes recur. A patch that fails the same way as `django__django-12286` historically tends to also fail; showing the operator the historical exemplar gives ground truth on what went wrong without claiming a new prediction. The structural signature (`E_AST + E_CFG + E_DataFlow + E_TypeGraph`) is *structural-only* — does not depend on E_Test/E_Problem text — so retrieval is robust to surface-text drift.

**Why this is "explanation only," not a prediction.** Q3 attaches a *label* to Q2's `Fail` verdict; it does not produce a verdict subject to oracle correlation on its own. There is no "Q3 oracle." The structural similarity is the evidence; the named fingerprint is the explanation.

**Top-1 same-failure-mode accuracy: 1.0** (per `05 §15`) on the current holdout.

### §4.5 Q5 — `reality_impact.rs` (shift-log replay → side-effect prediction verification)

**Source:** `crates/context-graph-mejepa/src/reality_impact.rs`.

**What it does.** For each predicted side effect (a "shift event" — e.g. `next_session_admin_save_failed`, `downstream_consumer_regression`), check whether it actually occurred against the durable shift log at `/zfs/archive/contextgraph/runtime/shift-log.jsonl`.

**Input.** `RealityPrediction` rows (containing predicted shift items) + session shift-log JSONL.

**Output.** `RealityImpactRecord` — per predicted item, classified as:
- `Confirmed` — predicted and observed
- `Missed` — predicted but did not occur
- `Surprise` — observed but not predicted (this is gold for the mistake loop)
- `NotYetObserved` — bounded replay window still open

**How it works.**
1. Predictor emits `RealityPrediction.predicted_shift_outcomes` with per-leaf-event predicted probabilities and conformal intervals.
2. Shift subscriber tails the JSONL log and persists `ShiftEntry` rows.
3. `reality_impact.rs` replays the log over the bounded replay window, matching predicted items to observed items by `(file_sha_before, file_sha_after, event_kind)`.
4. Fail-closed on missing predictions, out-of-order timestamps, explicit SHA-chain breaks.

**Why it works.** Q5 is the *forward verification* of the predictor — Q2 says "this patch passes," Q5 says "if you apply this patch, here's what else will happen next." The shift log is the ground truth for the "what else" — file modifications, test re-runs, session events. Surprise items (`observed but not predicted`) are the most valuable mistake-loop material because they reveal what the predictor isn't modeling.

**Calibration.** `CF_MEJEPA_Q5_CALIBRATIONS` keyed by `(calibration_id, language, mutation_category, side_effect_kind)` records TP/FP/FN, precision/recall/F1, surprise rate.

### §4.6 Verdict assembly — how the heads compose into the final `Verdict::{Pass, Fail}`

**Source:** `crates/context-graph-mejepa/src/verdict_assembly.rs` (`05 §4.2`).

```
1. Materialize Panel for candidate change                            (instruments)
2. MeJepaPredictor.forward(panel)                                    (predictor trunk + 8 heads)
3. Per-head DDA features                                             (head_projection.rs)
4. Split-conformal interval                                          (conformal.rs)
5. Per-slot OOD reasons (#389)                                       (per_slot_ood_reasons)
6. Aggregate OOD gate (strict-conjunction)                           (ood_harvest)
7. TCT Gτ guard                                                      (mejepa-tct)
8. Cold-cell metric: n_supporting < 50 → Abstain                     (cold_cell_metrics)
9. Multi-head contradiction (#263)                                   (contradiction.rs)
10. Constellation intelligence (#390)                                (constellation_intelligence.rs)
11. Match fingerprint catalog                                        (failure_fingerprint.rs)
12. Pull closest exemplars                                           (patch_similarity.rs)
13. Reconcile claim graph                                            (claim_graph.rs)
14. Emit RealityPrediction → CF_MEJEPA_LIVE_PREDICTIONS               (compiler.rs::compile)
```

The product surface ships only `Verdict::Pass | Fail`. Internal `OutOfDistribution`/`Abstain`/`GuardRejected` exist as failure modes and **must be projected to Pass | Fail at the boundary** using closest-exemplar majority outcome. The projected verdict is then subject to ground-truth verification and mistake-loop update.

**Why this composition order matters.** Each step is a *fail-closed gate* on the previous — OOD before TCT before cold-cell before contradiction. The order is deliberately defensive: if any step finds insufficient confidence, the verdict assembly *short-circuits* to an abstain, which then projects to Pass/Fail via the safest exemplar majority.

---

## §5. Why this whole stack works — the underlying principle

The architecture rests on three converging mathematical/epistemic facts.

1. **Frozen contrastively-pretrained embedders inherit a useful prior.** Each Eₘ was trained on hundreds of millions to billions of pairs. The prior it encodes — "similar things are close in vector space, dissimilar things are not" — is exactly the prior a small downstream predictor needs to learn a binary mapping efficiently. ME-JEPA does not have to learn that "issue text and bug fix should be related"; E1 already learned that. ME-JEPA only learns the *thin shell* — the patch-level oracle-correlation mapping.

2. **Slot-preserving array geometry is anti-compensation-safe.** A flat fused vector lets the predictor average across slots. The slot-preserving array forces each slot to contribute independently, with per-slot calibration and per-slot OOD detection. A predictor that scores high *requires* every slot to actually support the verdict, not just enough slots to win on average. This is the engineering version of the Hermetic correspondence principle (`docs/MEJEPA_RESEARCH_SYNTHESIS.md §4.1`): planes (per-embedder spaces) are *structurally related* but never *dim-collapsed into each other*.

3. **Binary form against deterministic reality is calibratable.** Q2's oracle is the Docker test suite — deterministic, reproducible, near-100% self-consistency (modulo flaky-test noise audited by #723). Conformal calibration on a binary target with a deterministic oracle gives provable coverage. The predictor's confidence interval is not a heuristic; it has a measured coverage rate against ground truth.

The combination — frozen embedders + slot-identity panel + binary deterministic oracle — is what lets ME-JEPA-Code aim for a `0.95` correlation predicate as a falsifiable engineering target rather than as an aspiration.

---

## §6. Cross-reference table — slot, embedder, head, CF

| Question | Head | Input slots | Output | Storage CF | Source file |
|---|---|---|---|---|---|
| **Q1** "did the claim land?" | `claim_graph.rs` (deterministic) | none (operates on disk) | `ClaimReconciliationRecord` | `CF_MEJEPA_CLAIM_RECONCILIATION` | `claim_graph.rs` |
| **Q2** "does it pass the oracle?" | `OracleHead` | all 15 slots (slot-preserving) | `OracleLogits` → `Verdict::Pass\|Fail` | `CF_MEJEPA_LIVE_PREDICTIONS` | `oracle_head.rs` |
| **Q3** "if Fail, why?" | `failure_mode` head (36 classes) + `patch_similarity` HNSW | all 15 slots + structural 1152-d signature | `NamedFailureMode` + closest exemplars | `CF_MEJEPA_PATCH_SIMILARITY_META`, `CF_MEJEPA_FAILURE_MODE_*` | `failure_fingerprint.rs`, `patch_similarity.rs` |
| **Q4** ambiguous (perf, sec, acc, cost, reasoning) | `perf`/`security`/`accuracy`/`cost`/`reasoning` heads | all 15 slots | **frozen — zero loss** | `CF_MEJEPA_Q4_*` (historical only) | `heads.rs`, `q4_trust_gate.rs` |
| **Q5** "what side effects occur?" | `reality_impact.rs` (deterministic matcher) | predicted shift items + shift log | `RealityImpactRecord` | `CF_MEJEPA_REALITY_IMPACT`, `CF_MEJEPA_Q5_CALIBRATIONS` | `reality_impact.rs` |
| Edge case taxonomy | `edge_case` head (23 classes) | all 15 slots | `HeadPrediction.probabilities` | (head output, not a CF) | `heads.rs` |
| Tech debt taxonomy | `tech_debt` head (21 classes) | all 15 slots | `HeadPrediction.probabilities` | (head output, not a CF) | `heads.rs` |

---

## §7. Quick reference — embedder selection heuristic for new domains

If you're instantiating the `SuperIntelligence.md §3` universal predicate on a new domain `D`, the embedder selection rule is:

1. **Pick one strong generalist text embedder** (E1 analog). For natural language, e5-large-v2 / bge-large; for domain-specific text, a domain-pretrained variant.
2. **Pick one domain-native dense embedder** (E7 analog). For code: Qodo. For molecules: ChemBERT. For medical images: a domain CNN/ViT. For audio: WavLM.
3. **Pick one sparse lexical recoverer** (E6/E13 analog) — SPLADE family is the canonical choice for any text-bearing domain.
4. **Pick one robustness embedder** (E9 HDC analog) for typo/noise resistance, or a deterministic feature aggregator for non-textual robustness.
5. **Pick one second-opinion dense** (E14 analog) — a non-overlapping pretrained encoder for the cross-panel Goodhart guard.
6. **Add deterministic instrument slots** for oracle outputs, runtime signals, static analysis, and scalar features — these slots have no neural embedder; they are deterministic aggregators.
7. **Reserve 2–3 empty slots** for future replacement-candidate embedders (E5/E11 analog) — gives in-slot replacement workspace without schema change.

The minimum viable panel for any domain is ~5–7 slots. The current code-domain panel has 15. Adding more slots is constrained by `inter_sensor_distinctness` (Tier 5; `#709`) — each slot must add information not already covered by existing slots, measured by maximum-pair correlation < 0.7.

---

## §8. What this doc does NOT cover

- **Per-slot calibration and OOD** internals (see `05 §12` and `crates/context-graph-mejepa/src/calibration.rs`, `ood.rs`).
- **TCT constellation Gτ guard** (see `crates/context-graph-mejepa-tct/src/`).
- **Heal scheduler / promotion / rollback** (see `crates/context-graph-mejepa/src/heal/`).
- **UTML training loss `L_step = ΔP × ΔK × ΔΩ × ΔΞ`** (see `crates/context-graph-mejepa-train/src/learning_signal.rs`).
- **Mistake-loop key-local online-head state** (see `online_head_state.rs` and `mistake_log.rs`).
- **Dynamic embedder registration / freeze / VRAM budget** (see `dynamic_registry.rs` and `dynamic_embedder*.rs`).

For those, follow `05_predictor_pipeline.md` sections 4, 5, 9, 9.6, and 10.

---

**End of document.**
