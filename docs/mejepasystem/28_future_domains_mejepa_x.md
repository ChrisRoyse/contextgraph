# 28 — Future Domains (ME-JEPA-X Template)

Source files covered: `docs/futurebuild/25_FUTURE_DOMAINS_MEJEPA_X.md`, `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`, `crates/context-graph-dynamicjepa/`.

> Template for adapting the ME-JEPA architecture to non-code modalities. The architecture is **language-agnostic and domain-agnostic** at the framework level; modality-specific work is in the instrument set + label sources.

---

## 1. ME-JEPA-Code is the code-domain instance of TCT

Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §6`. The TCT (Teleological Constellation Training) framework is the abstract pattern:

```
Domain X requires:
  - Frozen externally-defined instruments (E_X1, E_X2, ...) that map typed events to fixed-dim vectors
  - A deterministic reality oracle (the test runner / ground truth source for domain X)
  - A constellation of per-(category × entity × language × embedder) centroids
  - A Gτ guard test
  - A predictor mapping (panel_t-1, panel_t) → predicted_panel
  - 4 UTML factors ΔP / ΔK / ΔΩ / ΔΞ with target_collapse ≡ 0
  - A continuous self-optimization loop
  - An agent-feedback channel
```

Code domain (ME-JEPA-Code) is the first concrete instantiation. Voice / Image / etc. follow the same template.

---

## 2. Modality-specific work per domain

| Domain | Frozen instruments | Reality oracle | Chunker | Embedders |
|---|---|---|---|---|
| **ME-JEPA-Code** (shipped) | E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff, E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning, Scalars | Docker test runner (SWE-bench) | tree-sitter 11-language | E1, E7, E8, E10, E12, E13, E14, E17 |
| **ME-JEPA-Voice** (future) | E_Phoneme, E_Pitch, E_Cadence, E_Spectrum, E_VAD, E_TTS_Reference, E_Speaker_ID, ... | TTS faithfulness oracle (waveform MSE + perceptual loss) | Word-segmented + voiced-region chunker | Wav2Vec2, HuBERT, Whisper-encoder |
| **ME-JEPA-Image** (future) | E_Object, E_Saliency, E_OCR, E_Color, E_Composition, E_Style, E_Edge, ... | Vision-task oracle (classification accuracy / segmentation IoU / detection mAP) | Object-segment / region-based chunker | CLIP, DINO, SAM, Stable-Diffusion-VAE-encoder |
| **ME-JEPA-Robotics** (future) | E_Pose, E_Force, E_TorqueTrace, E_VisionFlow, E_SafetyEnvelope, ... | Real-world / sim oracle (task completion + constraint violation) | Action-bracketed time-window chunker | RT-1 / RT-2 features, PhysCLIP |
| **ME-JEPA-Math** (future) | E_AST_FormalProof, E_TypeContext, E_UnifierTrace, E_TacticPath, ... | Coq / Lean / Isabelle proof-checker | Tactic-bounded chunker | math-domain embedders |

---

## 3. Reusable across all domains

The following crates are **modality-agnostic** and can be reused unchanged:

| Crate | Purpose |
|---|---|
| `context-graph-witness` | Witness chain (SHAKE-256 + ed25519 + Merkle) |
| `context-graph-tier-compression` | 8/7/5/3-bit groupwise quantization |
| `context-graph-mincut` | Stoer-Wagner global mincut for splits |
| `context-graph-paths` | D-drive durable path resolver |
| `context-graph-solver` | MMR + conjugate gradient + conformal + PageRank + Granger F-test |
| `context-graph-mejepa-cf` | Canonical CF name registry (extended per domain) |
| `context-graph-mejepa-train` (trunk + UTML carriers) | Training loop, AdamW, cosine LR, cert chain, BatchSampler, ΔP/ΔK/ΔΩ/ΔΞ |
| `context-graph-mejepa-tct` | Constellation + Gτ guard (parameterized by domain's centroid layout) |
| `context-graph-mejepa-hygiene` | Tier transitions + quota + GC + Merkle compression |
| `context-graph-mejepa-shift-subscriber` | Tokio JSONL tail + watermark + cache |
| `context-graph-mejepa::compiler` | `compile / verify / observe` API |
| `context-graph-mejepa::eval` | Holdout eval + active-learning + patch-similarity graph |
| `context-graph-mejepa::heal` | Drift / EWC++ / A/B/C / continual backprop |
| Daemon supervisor + scheduler | per `context-graph-mcp::daemon` |
| Hook layer (`.claude/`) | UserPromptSubmit + PreToolUse + PostToolUse + Stop + PreCompact |
| SMAP (`./memory/`) | Cross-turn team narrative |

---

## 4. Per-domain work

For a new domain D:

| Step | Work |
|---|---|
| 1 | Define D's typed event taxonomy (analog of `Chunk` for code) |
| 2 | Implement D's chunker (analog of `memory::ast::chunk_with_options`) |
| 3 | Select D's frozen embedders (the 13 active for code → some N for D) |
| 4 | Define D's instrument slots (the 15 for code → some M for D, summing to e.g. 5,120-d or different) |
| 5 | Implement D's reality oracle bridge (Docker → D's deterministic verifier) |
| 6 | Define D's mutation operators (8 for code → some K for D) |
| 7 | Define D's `EntityType` + `MutationCategory` enums |
| 8 | Generate D's corpus (analog of 300×8 SWE-bench) |
| 9 | Build D's constellation (per-(category × entity_kind × subdomain × embedder)) |
| 10 | Calibrate D's Gτ thresholds |
| 11 | Define D's prediction taxonomy heads (Q3/Q4 for D) |
| 12 | Train D's predictor (reusing trunk architecture) |
| 13 | Wire D's MCP tools (`mcp__cgreality__mejepa_<D>_*`) |
| 14 | Establish D's ship gate (≥0.95 per-cell correlation)) |

Most of these are **structural copies** of the code-domain analogs in different crates: `context-graph-mejepa-voice-corpus`, `context-graph-mejepa-voice-instruments`, etc.

---

## 5. Plugin architecture for prediction heads

Per `D8`. Today's heads are hard-coded enum variants. For modality plug-in:

```rust
pub trait PredictionHead: Send + Sync {
    fn head_id(&self) -> HeadId;
    fn output_dim(&self) -> usize;
    fn forward(&self, trunk_output: &Tensor, panel: &Panel) -> Result<HeadOutput, HeadError>;
    fn label_source(&self) -> Box<dyn LabelSource>;
    fn loss_weight_lambda(&self) -> f32;
}

pub trait LabelSource: Send + Sync {
    fn extract_labels(&self, observation: &Observation) -> Result<Vec<Label>, LabelError>;
}
```

Per-domain plug-in registers heads via `HeadRegistry::register_head(domain: Domain, head: Box<dyn PredictionHead>)`. Today's code-domain heads (Panel, Oracle, FailureMode, EdgeCase, TechDebt, Perf, Security, Accuracy, Cost, Reasoning) become the canonical implementations for `Domain::Code`.

---

## 6. Cross-domain transfer

Per the `dynamicjepa_cross_domain_transfer` MCP tool (already shipped at code level): patches/states/observations in one domain can predict in another via **shared trunk** + per-domain projections.

The shared trunk learns modality-agnostic dynamics; per-domain heads + per-domain instruments provide modality-specific signal. Transfer experiments quantify how much code-domain training helps math-domain or voice-domain prediction.

Operationally: `dynamicjepa cross-domain-transfer --source counter_world --target gridworld_5x5` exists today as a pilot. Future-domain ME-JEPA-X work will use a similar mechanism scaled up.

---

## 7. Status

| Domain | Status |
|---|---|
| ME-JEPA-Code | **active build** (Phase A–H; futurebuild plan) |
| ME-JEPA-Voice | template documented; concrete build deferred per operator |
| ME-JEPA-Image | template documented; concrete build deferred per operator |
| ME-JEPA-Robotics | template documented; concrete build deferred per operator |
| ME-JEPA-Math | template documented; concrete build deferred per operator |

Per `docs/futurebuild/17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md §O4`: concrete domain builds wait for operator priority. Template + plugin architecture (`D8`) make subsequent domains primarily configuration + instrument-set work, not architecture work.

---

## 8. Cross-references

[`07_instruments_panel.md`](07_instruments_panel.md) (the instrument trait — generic interface), [`09_predictor_multi_head.md`](09_predictor_multi_head.md) (per-domain heads), [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) (constellation framework), [`17_prediction_taxonomy_reference.md`](17_prediction_taxonomy_reference.md) (taxonomy template).
