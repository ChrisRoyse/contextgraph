# 05 — Embedder Signal Map for Code-Reality Prediction

> **Purpose.** For each of the 21 declared embedders in the workspace, name (a) what kind of signal it carries, (b) whether it adds value for code-reality prediction, (c) which prediction heads it feeds into, (d) which `EntityType × Language` combinations route to it, and (e) what failure / concern category it primarily surfaces. This is the operator's "figure out which embedders provide valuable signals … figure out which ones and which signals are important here" answered concretely for the futurebuild plan.
>
> Authoritative source for the embedder roster: `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` + `docs/ruvectorfindings/specs/_README.md` + the routing table in `crates/context-graph-mejepa-embedders/src/routing.rs`.

---

## 1. The 21 declared embedders

Per `crates/context-graph-mejepa-embedders/`. Slots E5 and E11 are **retired** post-2026-05-09 hardening (E5: causal asymmetric, replaced by inferred causal edges from E_DataFlow; E11: KEPLER entity, replaced by E10 multimodal-paraphrase + entity heads). All retired slots fail closed with `MEJEPA_EMBEDDER_RETIRED`.

| ID | Name | Dim | Family | Active for code? | Primary code-signal class |
|---|---|---|---|---|---|
| E1 | e5-large-v2 (general semantic) | 1024 | dense general-purpose | YES | semantic intent / docstring meaning |
| E2 | sinusoidal-time-PE | 512 | temporal positional | partial | recency of edit / git blame age |
| E3 | recurrence-time-PE | 512 | temporal cycle | partial | within-day / within-week edit pattern |
| E4 | session-position-PE | 512 | session ordering | partial | ordering of edits within agent session |
| E5 | (causal asymmetric) | 768 | retired | **NO** | — |
| E6 | sparse-keyword (BM25-ish) | 30k sparse | sparse lexical | YES (sparse) | rare identifier / typo robustness |
| E7 | Qodo-Embed (code-specific) | 1536 | dense code | YES | code semantics, function-level meaning, AST-token-aware |
| E8 | E11-style graph-attention (code graph) | 1024 | graph | YES | call graph / def-use / control flow |
| E9 | HDC (hyperdimensional) | 1024 | binary high-dim | YES | typo-robust, surface-form fingerprint |
| E10 | multimodal-paraphrase (doc/query asymmetric) | 1024 | dense paraphrase | YES | comment / docstring / commit message intent |
| E11 | (KEPLER entity) | 768 | retired | **NO** | — |
| E12 | ColBERT (token-level multi-vec) | 128/token | late-interaction | YES (selective) | fine-grained per-token matching for symbol resolution |
| E13 | SPLADE (sparse expansion) | 30k sparse | sparse expansion | YES (sparse) | term expansion for keyword-based queries |
| E14 | semantic-code (an additional code embedder; e.g., StarEmbed/CodeT5+) | 1536 | dense code | YES | code semantics with different inductive bias than E7 (ensemble diversity) |
| E15 | learner-state — affect (HRV-style) | 256 | learner-state | NO (not for code) | — |
| E16 | learner-state — focus (attention quality) | 256 | learner-state | NO (not for code) | — |
| E17 | learner-state — affect-text (E_Reasoning anchor) | 768 | learner-state | YES (E_Reasoning input) | tone of agent's prose; hedging / overclaiming |
| E18 | learner-state — context novelty | 256 | learner-state | partial | novelty of the chunk vs the agent's session history |
| E19 | learner-state — load (cognitive load proxy) | 256 | learner-state | NO (not for code) | — |
| E20 | learner-state — confidence | 256 | learner-state | partial | agent-side reported confidence calibration |
| E21 | learner-state — generative-style | 256 | learner-state | NO (not for code) | — |

**Active set for ME-JEPA-Code's content-side prediction (the "valuable signals"):** {E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14, E17}. The non-content learner-state slots (E15, E16, E19, E21) are reserved for non-code domains in the broader ME-JEPA-X family. **Per the operator directive ("use as many as would be valuable"), we keep all 13 in active routing for code-domain prediction.**

E18 and E20 carry partial signal — context novelty (E18) helps with the active-learning queue prioritization, and confidence (E20) provides weak supervision for the Reasoning head. Both are in the routing table but at low weight.

---

## 2. What each ACTIVE embedder uniquely contributes

This is the "what would we lose if we dropped this embedder" section. Each row is the unique contribution; together they form the ensemble.

| Embedder | Unique contribution | Concrete prediction example |
|---|---|---|
| **E1** (general semantic) | Captures *what the code does* in natural-language terms; useful for comment ↔ code alignment, docstring drift detection | "this function's docstring says 'returns separability matrix' but the body returns a boolean — likely doc drift" |
| **E2** (sinusoidal time-PE) | Encodes *recency*; weights recent edits higher than stale code | "this chunk was just modified; weight its prediction more than stale code we haven't touched" |
| **E3** (recurrence time-PE) | Encodes *cyclic timing patterns*; useful for "this commit looks like a Friday-afternoon commit pattern (rushed)" weak supervision | "edits clustered in a 5-minute burst → likely WIP, conformal interval should widen" |
| **E4** (session position-PE) | Encodes *ordering within the agent's session*; useful for "is this an early edit or a late one in the agent's plan" | "this is the agent's 17th edit in the session; agents often regress around edit 15-20 (over-thinking) — caution flag" |
| **E6** (sparse keyword) | Captures *exact identifier matches*; resists typos and dense-encoder normalization | "the new identifier `_walk_compaund` (typo) doesn't match any callsite — likely typo bug" |
| **E7** (Qodo-Embed code) | Captures *code semantics* with code-specific tokenization; the workhorse for code prediction | "this implementation matches a known buggy implementation pattern at 0.91 cosine to corpus exemplar `psf__requests-2317`" |
| **E8** (graph-attention code graph) | Captures *call-graph / def-use / control-flow* structure; surfaces interface mismatches | "the new function calls `helper(a, b, c)` but `helper` expects `(b, c, a)` — signature shape mismatch" |
| **E9** (HDC) | Captures *surface-form fingerprint*; robust to typos but sensitive to copy-paste duplication | "this 30-line block is a 0.97 surface-form match to a block in another file — duplicated code" |
| **E10** (multimodal-paraphrase) | Captures *intent/documentation*; doc ↔ code alignment, commit-message ↔ diff alignment | "the commit message says 'fix off-by-one' but the diff shows a renaming — message-diff drift" |
| **E12** (ColBERT) | Captures *fine-grained per-token matching*; useful for symbol resolution and exact-substring queries | "the new code references `Model.coeff_` (with trailing underscore) but the class only has `coeff` — symbol resolution miss" |
| **E13** (SPLADE) | Captures *term expansion*; broadens keyword search | "the new function uses `sklearn.linear_model` patterns; expansion finds 12 related calls in the file that may need updating" |
| **E14** (semantic-code, ensemble diversity) | Captures *complementary code semantics* with different inductive bias than E7; the "second opinion" | "E7 says this is fine; E14 disagrees — pairwise cosine[E7][E14] = 0.62, blind-spot suspected" |
| **E17** (affect-text for E_Reasoning) | Captures *tone of agent's prose*; surfaces hedging / overclaiming | "the agent says 'I'm not sure if this is right' (hedging tone, E17 cosine to hedging-centroid = 0.81) — reasoning class = Hedging" |

**Pairwise diversity matters.** The DDA pairwise cosine matrix `cos[E_m1][E_m2]` is most informative where two embedders disagree — that disagreement is the "blind spot" signal. We deliberately keep semantically-overlapping embedders (E7, E14, E10) in the active set because their *disagreement patterns* are diagnostic.

---

## 3. Per-failure-mode embedder contribution

This is the "which embedders surface which failure" cross-reference. Reading direction: pick a failure mode → see which embedders are most likely to surface it.

| Failure / concern category | Embedders that lead the signal | Why |
|---|---|---|
| **WrongAlgorithm / OffByOne / NullDeref** (Q3) | E7, E8, E14 (code semantics + graph + 2nd opinion) | algorithmic bugs show up as deviation in code-specific embedders' centroid distance |
| **TypeError / SignatureMismatch** (Q3) | E8 (graph-attention) + E12 (ColBERT for symbol resolution) | def-use & per-symbol matching is exactly what these capture |
| **ImportMissing** (Q3) | E6 (sparse) + E12 (ColBERT) | sparse exact-match for missing imports |
| **SyntaxError** (Q3) | E_AST instrument (deterministic, parse-fail) — embedder-independent | parse failure in chunker is structurally caught before embedding |
| **ResourceLeak / RaceCondition / DeadlockPotential** (Q3) | E8 (call graph) + E_StaticAnalysis instrument | analyzer signal dominates; E8 helps with cross-chunk leak |
| **InfiniteLoop / StackOverflow** (Q3) | E8 (loop pattern) + E_CFG instrument | CFG instrument catches the loop topology |
| **AssertionViolation / ContractViolation** (Q3) | E1 (intent) + E10 (doc/code paraphrase alignment) | contract is in the docstring; E1/E10 catch when impl drifts from contract |
| **EmptyInput / SingleElement / BoundaryValue** (Q4 edge case) | E_DataFlow instrument + symbolic-execution proxy + E8 | branch coverage diff is dominant; E8 helps locate the missing branch |
| **UnicodeEdge / Locale / TimezoneTransition** (Q4 edge) | E1 + E10 (doc-level signal that the code handles or claims to handle this) + E_StaticAnalysis | weak per-language pattern catalogs feed scalar features |
| **DeepNesting / ConcurrentAccess** (Q4 edge) | E_CFG + E8 | structural; CFG dominates |
| **ShadowedVariable / InconsistentNullCheck** (Q4 latent bug) | E_StaticAnalysis (analyzer warnings) + E_DataFlow | analyzer-driven |
| **SwitchFallthrough / LooseEquality** (Q4 latent bug) | E_AST + E_StaticAnalysis | language-specific syntax pattern |
| **ForgottenAwait / ForgottenYield / ForgottenReturn** (Q4 latent bug) | E8 (call graph for await chains) + E_StaticAnalysis (linter rules) | structural signal + linter rule |
| **HighCyclomaticComplexity / LongFunction / LongFile** (Q4 tech debt) | scalar features (per-chunk metric), E_AST | scalar dominant; embedders tertiary |
| **DuplicatedCode / DuplicatedTest** (Q4 tech debt) | E7 + E14 + E9 (surface-form fingerprint) | similarity across chunks; E9's HDC is fast for surface-form |
| **DataClumps / FeatureEnvy / GodObject** (Q4 tech debt) | E8 (graph; cross-chunk usage patterns) | graph-attention surfaces these |
| **PrematureAbstraction / DeadFlagBranch** (Q4 tech debt) | E_DataFlow (single-call interface detection) | def-use says "this trait has one impl" |
| **TodoWithoutOwner / CommentedOutCode** (Q4 tech debt) | E_AST (comment node walk) + scalar regex | structural extraction |
| **MagicNumber / StringLiteralForEnum** (Q4 tech debt) | E_AST + scalar | structural |
| **UnusedFunction / UnusedClass / UnusedImport** (Q4 dead code) | E8 (call graph reachability) + scalar (analyzer output) | analyzer-driven; E8 confirms cross-file |
| **UnreachableAfterReturn / AlwaysFalseBranch** (Q4 dead code) | E_CFG | CFG instrument is structural |
| **DuplicatedFunction / DuplicatedTest / OverlappingErrorMessage** (Q4 redundancy) | E7 + E14 + E10 | semantic-code cosine across the working tree |
| **ReinventedWheel** (Q4 redundancy) | E1 + E10 + per-language stdlib registry | "this looks like a hand-rolled retry-with-backoff; tenacity / backoff already provides this" |
| **CpuMs / WallclockMs / RssKb regressions** (Q4 perf) | scalar (benchmark deltas) + E8 (loop/recursion patterns) | scalar dominant; E8 catches new hot loops |
| **HeapAllocs / HeapBytes regressions** (Q4 perf) | E_AST (allocation-pattern walk) + scalar | language-specific patterns |
| **BinarySize regressions** (Q4 perf) | scalar (build-script output) | scalar only |
| **SqlInjection / CommandInjection / PathTraversal / Xss / Csrf / Ssrf** (Q4 security) | E_StaticAnalysis (semgrep / bandit / cargo-audit) + E12 (sink/source per-token) | analyzer-driven; E12 helps locate |
| **HardcodedSecret / LoggingSecret** (Q4 security) | E_StaticAnalysis (regex/entropy) + scalar | analyzer-driven |
| **InsecureCryptoAlgo / InsufficientCryptoKeyLength** (Q4 security) | E1 (semantic — "MD5" / "SHA-1" tokens) + E_StaticAnalysis | analyzer + semantic |
| **MissingAuth / BrokenAccessControl** (Q4 security) | E8 (cross-endpoint flow) + E_StaticAnalysis | structural |
| **Accuracy degradations** (Q4) | scalar (test-output metrics) + E_Test instrument | scalar dominant |
| **Cost regressions** (Q4) | scalar (CI minutes / token counter / dep counter) | scalar only — no embedder needed |
| **Reasoning class** (Q4) | E17 (affect-text), pairwise cosine[E_Reasoning][E_Diff], pairwise cosine[E_Reasoning][E_Oracle] | tone + agreement of agent's prose with the change and the outcome |

**Architectural reading.** The Q3 (failure) and Q4 (concern) heads do not each need a *new* embedder; they need the existing 12-active embedders' DDA signals consumed differently. The ensemble is sufficient; what's missing today is the multi-head consumption layer in the predictor.

---

## 4. Routing table (extension of the 143-row shipped table)

Today: 143 routes shipped per `crates/context-graph-mejepa-embedders` covering all 11 languages × 13 entity types. Two extensions for the futurebuild plan:

### 4.1 Per-EntityType routing for the new heads

The shipped routing handles which embedders run on which (Language × EntityType). The futurebuild plan adds a second-tier routing: **for each prediction head, which embedders' DDA signals does it consume?** This is the per-head feature-projection table. Specified in the multi-head technical spec (`SPEC-PREDICT.md`).

### 4.2 Per-Action routing for the agent-feedback channel

When the agent calls `mejepa_record_agent_feedback` with a `SurpriseEvent`, the system should bump the sampling weight of the chunks involved AND mark them as "agent-flagged" for the next constellation refresh. This requires a routing layer that maps `(SurpriseEvent.kind, chunk.kind, chunk.language)` → which embedders to recompute / which centroids to refresh.

---

## 5. Embedder sourcing (where the model files live)

Per `docs/ruvectorfindings/specs/_README.md` Phase 1b: SHA-256 digest verification, VRAM budget gate, E17 calibration cert, retired-slot policy. Files live under `models/` (per-machine, not committed). Per the operator directive *"upstream reference-only — clean room only"* (`memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`), models are loaded from local disk; no HTTP fetches at runtime.

| Embedder | Model artifact | Source family | Approx VRAM at fp16 | Comments |
|---|---|---|---|---|
| E1 | `intfloat/e5-large-v2.gguf` | E5 | 0.6 GB | general dense |
| E2-E4 | deterministic (no model) | — | 0 | computed from timestamp / position |
| E6 | sparse codebook | — | 0 | sparse, no neural |
| E7 | `Qodo/Qodo-Embed-1-1.5B.gguf` | Qodo | 1.5 GB | code-specific |
| E8 | code-graph adapter on top of E7 | learned head | <1 GB | shares E7 backbone |
| E9 | HDC codebook | — | <0.1 GB | binary HDC |
| E10 | `intfloat/multilingual-e5-large.gguf` | E5 family | 0.7 GB | multi-modal paraphrase |
| E12 | `colbert-ir/colbertv2.0` | ColBERT | 0.5 GB | per-token late-interaction |
| E13 | `naver/splade-v3` | SPLADE | 0.4 GB | sparse expansion |
| E14 | `Salesforce/codet5p-110m-embedding.gguf` | CodeT5+ | 0.4 GB | code, ensemble diversity vs E7 |
| E17 | `cardiffnlp/twitter-roberta-base-sentiment-latest` | RoBERTa | 0.5 GB | affect / tone |
| E18, E20 | small adapters | — | <0.2 GB | learner-state |

**Total VRAM at fp16 with all 13 active loaded simultaneously: ~6.5 GB.** The RTX 5090 (32 GB) has ample headroom; the existing VRAM budget gate per `mejepa_vram_budget_report` MCP tool tracks this. The retired-slot policy (E5, E11) prevents accidentally loading them.

**Lazy-loading is enabled** for E15, E16, E19, E21 (non-code learner-state) — they're only loaded when a non-code domain prediction is requested.

---

## 6. The "blind-spot" detection across embedders

This is the operator's "we have a code embedder that is really good and lots of other embedders" — we exploit *disagreement* among them as a signal class.

```rust
fn cross_embedder_anomaly(chunk, panel) -> Vec<BlindSpot> {
    let mut spots = vec![];
    for (m1, m2) in unique_pairs(active_embedders) {
        let c = cosine(panel.per_chunk_emb[chunk][m1],
                       panel.per_chunk_emb[chunk][m2]);
        let mu = expected_pairwise[m1][m2];          // baseline cosine for this embedder pair on KnownGood corpus
        let z = (c - mu) / sigma_pairwise[m1][m2];
        if z.abs() > BLIND_SPOT_Z_THRESHOLD {
            spots.push(BlindSpot {
                chunk, m1, m2, observed_cosine: c, expected_cosine: mu, z_score: z,
                interpretation: if z < 0 { "embedders disagree more than usual on this chunk" }
                                 else { "embedders agree more than usual on this chunk" },
            });
        }
    }
    spots
}
```

This runs as part of the DDA signal computation; high-magnitude `BlindSpot`s are surfaced via `mejepa_cross_embedder_anomalies` MCP tool (already shipped as `search_cross_embedder_anomalies`). For the futurebuild plan, this becomes a feature in the multi-head predictor — `BlindSpot.z_score` is a 144-d feature vector (the upper-triangle of the embedder-pair grid) consumed by the failure-mode head as auxiliary input.

---

## 7. What this signal map commits us to

1. **Keep all 13 active embedders in code-domain routing.** Drop none; pairwise diversity is the source of the blind-spot signal. (No work; it's the existing routing.)
2. **Build neural forward wrappers for all 13.** `EmbedderForward::forward(chunk) -> EmbeddingVec` per embedder; today these are deterministic placeholders. (Task: `TASK-ENRICH-001`.)
3. **Wire the per-head feature projection** so each prediction head consumes its required DDA signal subset. (Task: `TASK-PREDICT-005-per-head-features.md`.)
4. **Cross-embedder anomaly z-score features** as an auxiliary input to the failure-mode head. (Task: `TASK-PREDICT-006-blind-spot-features.md`.)
5. **VRAM budget enforcement** at startup; refuse to start with an over-budget configuration. (Mostly shipped; small task to add a startup check to the daemon mode.)
6. **Per-language stdlib registry** for `ReinventedWheel` detection, populated from a curated YAML per language. (Task: `TASK-ENRICH-007-stdlib-registry.md`.)
