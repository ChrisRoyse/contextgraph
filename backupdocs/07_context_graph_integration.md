# 07 — Context Graph Integration Architecture

**Purpose.** Concrete engineering specification for how UTL v2's measurement programme drops into Chris's Context Graph content registry (~370,000 lines of Rust, 59 RocksDB column families, 75 MCP tools, 68 integration-test files, 5,199 test attributes). The point: UTL is not a separate system; it is a small, well-bounded extension that reuses 90% of what Context Graph already does.

The result: 14 production content embedders plus 7 learner-state embedders (E15-E21), for 21 canonical embedder slots total. This adds +11 column families (59 -> 70), +8 MCP tools (75 -> 83), one new background task (`LearnerStateUpdater`), and one new dashboard route group (`/learner/{id}`). E14 is reserved for the production `BgeM3Dense` content embedder.

---

## 1. The architecture, top-down

```
                    Learner sensors (webcam, mic, EEG, wearable)
                                |
                                v
                    Per-modality preprocessing (face crop, audio resample, ECG denoise)
                                |
                                v
                    Embedder forward passes (E15-E21, in parallel)
                                |
                                v
                    Per-session embedding store (CF_FINGERPRINTS_LEARNER)
                                |
                                v
                    Aggregator (background task)
                                |
                                v   ┌─────────────────────────────┐
                    ΔS, ΔC, ΔE ───→ Update M(t) integral          │
                    diagnostics ─→ Classify learner state         │ persisted
                                ─→ Update constellation centroid  │ to RocksDB
                                ─→ Audit-log every action        │
                                                                  └─────────────────────────────┘
                                |
                                v
                    MCP tools expose: compute_L, get_M, get_state, schedule_review
                                |
                                v
                    Dashboard /learner/{id} renders trajectory
```

This is the same dataflow shape as Chris's existing memory-store path. The substitutions are:

- "memory" → "learner state observation"
- "memory store" → `CF_FINGERPRINTS_LEARNER`
- "constellation" (TCT) → `CF_LEARNER_CONSTELLATIONS` (per-learner regulated centroid)
- "search" → `compute_L`, `get_M`, `get_state`

---

## 2. New learner-state embedder slots E15-E21

Following Chris's existing slot convention, production content embedders occupy E1-E14. Learner-state embedders start at E15:

| Slot | Path under `models/` | Embedder | Output dim | VRAM |
|---|---|---|---|---|
| E15 | `models/affect-speech/` | Wav2Vec2-large-MSP-DIM (audeering) | 1024 + (A,D,V) head | 1.5 GB |
| E16 | `models/affect-face/` | OpenFace 3.0 + MTL emotion/AU/gaze head | 512 + (V,A) + AUs | 0.5 GB |
| E17 | `models/affect-text/` | MiniLM-L6-v2 + EmoBank regressor | 384 + (V,A) | 0.3 GB |
| E18 | `models/ppg/` | PaPaGei (ICLR 2025) | 512 | 0.5 GB |
| E19 | `models/eda/` | WESAD chest physiology stress head | 64 + stress-floor scalar | 0.2 GB |
| E20 | `models/eeg/` | LaBraM (ICLR 2024) | 768 | 1.2 GB |
| E21 | `models/eeg-robust/` | EEGPT (artifact-robust) | 768 | 1.2 GB |

**Total VRAM addition:** ~5.4 GB. Within Phoenix gpu_guard's 28.6 GB usable budget (existing E1-E14 content stack uses the production model budget; LaBraM/EEGPT can lazy-load on demand).

`crates/context-graph-embeddings/src/lib.rs` extension:

```rust
pub enum EmbedderSlot {
    // existing
    E1_Semantic,  E2_Recent,  E3_Periodic,  E4_Position,  E5_Causal,  E6_Sparse,
    E7_Code,      E8_Graph,   E9_HDC,       E10_Paraphrase, E11_KEPLER, E12_ColBERT,
    E13_SPLADE,   E14_BgeM3Dense,
    // learner-state
    E15_AffectSpeech,
    E16_AffectFace,
    E17_AffectText,
    E18_PPG,
    E19_EDA,
    E20_EEG,
    E21_EEGArtifactRobust,
}
```

Each new embedder follows the existing trait `Embedder` with `forward(&self, input: ModalityInput) -> Vec<f32>`.

**Schema compatibility rule.** Do not append E15-E21 directly to the existing `TeleologicalFingerprint` v1 bincode layout. Context Graph's core storage and training records use positional bincode with explicit version bytes, and the existing content fingerprint layout is an on-disk contract. Add learner-state data through a new versioned `LearnerFingerprint` / `LearnerStateVector` namespace, or bump a new learner-specific version. The content fingerprint remains E1-E14; the learner fingerprint carries E15-E21.

---

## 3. New RocksDB column families

Chris's existing 59 CFs use the pattern `CF_<NAME>` and are declared in `crates/context-graph-storage/src/column_families.rs`. Add 11 more:

| CF | Key layout | Value | Purpose |
|---|---|---|---|
| `CF_LEARNER_PROFILE` | `[learner_id:16]` | bincode `LearnerProfile` (handle, opt-ins, modalities enabled, calibration date) | learner registry |
| `CF_LEARNER_CONSTELLATIONS` | `[learner_id:16][selector_kind:u8]` | `Constellation` (per-modality centroids E15-E21) | personalised regulated-state centroid |
| `CF_FINGERPRINTS_LEARNER` | `[learner_id:16][session_ts:u64]` | bincode `LearnerFingerprint` (E15-E21 vectors + metadata) | per-session embedding |
| `CF_LEARNER_M_PER_TRACE` | `[learner_id:16][trace_id:16]` | `(M_value: f64, last_update_ts: u64, decay_rate: f32, num_retrievals: u32)` | consolidation per trace |
| `CF_LEARNER_STATE_HISTORY` | `[learner_id:16][session_ts:u64]` | `StateVector` (concatenated E15-E21, 4032-D) | session-by-session state |
| `CF_LEARNER_GOAL_STATES` | `[learner_id:16][skill_id:16]` | `StateVector` (deployment centroid) | T-term reference |
| `CF_LEARNER_RETRIEVAL_LOG` | `[learner_id:16][trace_id:16][ts:u64]` | `(correct: bool, score: f32, state_at_retrieval: StateVector)` | retrieval-practice events |
| `CF_LEARNER_K_SLEEP` | `[learner_id:16][session_ts:u64]` | `(k: f32, slow_wave_minutes: u16)` | sleep-derived consolidation multiplier |
| `CF_GOAL_CENTROIDS` | `[skill_id:16][modality:u8]` | per-skill expert-answer centroid | ΔC reference, all learners |
| `CF_LEARNER_DELTA_LOG` | `[learner_id:16][session_ts:u64]` | `(L: f32, ΔS: f32, ΔC: f32, ΔE: f32, state: u8)` | per-session diagnostic |
| `CF_LEARNER_AUDIT` | `[learner_id:16][ts:u64]` | bincode `AuditEntry` (action, modality, params, result_hash) | provenance chain for learner data |

**Total:** 11 new CFs. 59 → 70.

Pattern matches the existing `CF_TELEOLOGICAL_*` family (28 CFs) — these new CFs are in a new namespace `CF_LEARNER_*`.

---

## 4. New MCP tools

Add 8 to `crates/context-graph-mcp/src/tools/`:

| Tool | Category | Inputs | Outputs |
|---|---|---|---|
| `register_learner` | learner-registry | handle, opt-ins | learner_id |
| `record_session_observation` | learner-state | learner_id, session_ts, raw_signals (audio/video/EEG/PPG/EDA paths) | per-modality embeddings + state vector |
| `compute_delta_s` | UTL | predicted_text, actual_text, simulated_text?, exploration_rate? | ΔS scalar |
| `compute_delta_c` | UTL | learner_id, answer_text, goal_skill_id, recent_score_window | ΔC scalar + breakdown |
| `compute_delta_e` | UTL | learner_id, session_ts | ΔE scalar + subcomponents |
| `compute_L` | UTL | learner_id, session_ts | L = ΔS · ΔC · ΔE + state classification |
| `get_learner_M` | UTL | learner_id, trace_id | M(t) value, decay rate, next-review prediction |
| `next_review_for_trace` | scheduler | learner_id, trace_id | datetime |

Following Chris's existing tool-definitions pattern (`crates/context-graph-mcp/src/tools/definitions/`):

```rust
// definitions/utl.rs
pub fn utl_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "compute_L".to_string(),
            description: "Compute UTL learning efficiency L = ΔS · ΔC · ΔE for a learner-session.",
            input_schema: json!({ ... }),
            handler: dispatch_utl_compute_l,
        },
        // ... 7 more
    ]
}
```

Tool count: 75 → 83.

---

## 5. Background task: `LearnerStateUpdater`

Following Chris's existing pattern (e.g., `BackgroundGraphBuilder` for K-NN edges), add a new background task in `crates/context-graph-mcp/src/server/`:

```rust
pub struct LearnerStateUpdater {
    storage: Arc<RocksDbTeleologicalStore>,
    embedders: Arc<EmbedderRegistry>,
    cycle_seconds: u64,                  // default 60s
}

impl LearnerStateUpdater {
    pub async fn run(&self) -> anyhow::Result<()> {
        loop {
            // 1. Find learners with new fingerprints since last cycle
            // 2. Aggregate into ΔS, ΔC, ΔE
            // 3. Update M(t) for any traces touched
            // 4. Update CF_LEARNER_DELTA_LOG
            // 5. If ΔE drops below threshold, emit alert via MCP notification
            tokio::time::sleep(Duration::from_secs(self.cycle_seconds)).await;
        }
    }
}
```

Cycle: 60 s (matches `BackgroundGraphBuilder`). Lock pattern: per-learner, so multiple learners process concurrently.

---

## 6. Dashboard extension

Chris's existing dashboard runs FastAPI on port 3200 in ClipCannon (Python). For Context Graph, the analogous extension would be a new route group:

```
/learner/{id}/dashboard         current state, ΔE breakdown
/learner/{id}/trajectory        L, M, T over time (plot)
/learner/{id}/traces            per-trace M values, next-review schedule
/learner/{id}/sessions          session log with state classification
/learner/{id}/calibration       baseline + Phase-1 status
/learner/{id}/audit             learner-specific audit chain
```

Six new routes. UI pattern: same as existing `/credits/`, `/projects/`, `/provenance/`.

---

## 7. Integration with the existing teleological subsystem

Chris's `crates/context-graph-storage/src/teleological/` already implements:

- `SynergyMatrix` (13×13 cross-embedder synergy)
- `TeleologicalProfile` (per-task weight profile, 14 built-in)
- `TeleologicalFingerprint` (~63 KB full: 13D topic profile + 78 cross-correlations + 6D group alignments)
- `MultiSpaceRetriever`, `FusionEngine`, `MeaningPipeline`

UTL extends rather than replaces:

- The existing fingerprint (E1-E14 production content embeddings) becomes the **content fingerprint** of any learning material.
- The new `LearnerFingerprint` (E15-E21 embeddings + state metadata) is the **learner fingerprint** of the consumer of that material.
- ΔS is the cosine distance between the content fingerprint and the learner's predicted-content fingerprint.
- ΔC is the panel agreement of the content fingerprint with the learner's prior-knowledge centroid.
- Context Graph's contrastive pair mining can generate hard negatives for ΔC: semantic-but-not-causal, keyword-but-not-paraphrase, code-shape-but-different-intent, entity-shared-but-different-structure, HDC-robust-but-semantically-different. These are exactly the disagreement cases UTL needs to diagnose "looks right but isn't coherent."
- Context Graph's training export already bundles production content embeddings, topic profiles, cross-correlations, group alignments, typed edges, KNN neighbors, causal labels, and temporal labels into versioned training records. UTL should mirror that pattern for learner traces instead of creating an unrelated export format.

The 14 weight profiles can be extended to UTL-aware profiles:

```toml
[teleological_profiles.utl_default]
e1_semantic = 0.20      # ΔC content matching
e5_causal = 0.15        # contradiction detection
e7_code = 0.05
e12_colbert = 0.10      # token-level coherence
# ... existing E1-E14 content weights
e15_affect_speech = 0.10  # ΔE
e16_affect_face = 0.10    # ΔE
e18_ppg = 0.10            # ΔE / HRV
e20_eeg = 0.20            # ΔE / plasticity
```

Per-task weight profiles: `utl_focus` (high weights on plasticity/beta), `utl_creativity` (high weights on alpha/theta), `utl_recall` (high weights on E1/E12/M).

---

## 8. The Constellation analogy made literal

Chris's Constellation subsystem (`CF_CONSTELLATIONS`, `MIN_CONSTELLATION_MEMBERS=3`, `RESERVOIR_SAMPLE_SIZE=1024`) was designed for content constellations — a compiled summary of a memory set. UTL re-uses this **literally**:

- A learner's regulated-state centroid IS a constellation (selector kind: `LEARNER_BASELINE`).
- The reservoir-sample percentiles serve as the natural variance estimate for state thresholding.
- The compile-once-frozen-thereafter discipline matches TCT Phase-1 exactly.
- The existing `compile_constellation` MCP tool already does what UTL Phase 1 needs; we just add a new selector kind and document the use case.

This is a particularly elegant integration: **UTL Phase-1 calibration uses no new code in Context Graph**. It just calls `compile_constellation` with the learner-baseline selector.

---

## 9. ClipCannon integration (Phoenix-side)

For learners who are also being recorded for video (e.g., the Santa pipeline), the same FLAME, expression-skills, constellation-controller, and constellation-guard infrastructure applies:

- The `expression_skills.py` 4-level hierarchy (AU → Group → Skill → Constellation) IS a learner's expression vocabulary.
- The existing `ConstellationGuard.validate_frame()` is the per-frame learner-state check.
- The 8 named behavioral constellations (warm_conversational, emotional_recall, …) match UTL's regulated-state-vs-dysregulated-state classifier directly.

Chris already has this. The UTL extension is to add the *learning-context* layer on top: when the user is a learner consuming content, log the constellation-state per-moment and integrate into M(t).

---

## 10. Test coverage extension

Chris's test discipline (`crates/context-graph-test-utils/`, 5,199 test attributes in the current verification report) suggests the following test additions:

| Test category | New tests |
|---|---|
| Embedder forward pass (E15-E21) | 7 (one per slot) |
| Constellation compile for learner-baseline | 3 |
| ΔS/ΔC/ΔE computation | 6 |
| M(t) update on retrieval | 4 |
| State-classification correctness | 6 (one per state) |
| Schedule next-review | 3 |
| Audit-chain consistency for learner data | 3 |
| Privacy: data-minimisation rules enforced | 5 |
| Cross-embedder agreement on synthetic data | 4 |
| FSV (full-state-verification) integration | 3 |

**~44 new tests.** Tracking the existing pattern of "every public function has a test" plus FSV scripts.

---

## 11. Provenance extension

Chris's provenance chain (SHA-256 hash chain in `provenance` table for ClipCannon, audit log in Context Graph) extends naturally:

```
DOCUMENT → OCR → CHUNK → EMBEDDING (existing chain)
                                   |
                                   v
                       LEARNER_OBSERVATION (new) → ΔS/ΔC/ΔE → M_UPDATE
```

Each `LEARNER_OBSERVATION` record carries:

- `learner_id`
- `session_ts`
- `modality` (E14/E15/E17/etc.)
- `input_sha256` (raw signal hash, retained 24h)
- `embedding_sha256` (persistent fingerprint hash)
- `parent_record_id` (chains to learner registry)
- `chain_hash` (SHA-256 of the above + parent's chain_hash)

This gives every learner-data access a provenance trail. The audit MCP tool (`audit_trail`, already exists) extends to cover learner-data queries.

The implementation should use two linked ledgers:

| Ledger | Stores | Retention |
|---|---|---|
| Evidence ledger | raw-input hash, embedder versions, threshold versions, output hashes, parent IDs | indefinite hashes; raw payloads per consent |
| Decision ledger | ΔS/ΔC/ΔE, M updates, scheduler decisions, human overrides, report exports | indefinite unless learner deletion request applies |

For any generated voice/video/avatar outputs, the export step should attach C2PA / Content Credentials or an equivalent cryptographic manifest. The learner-data audit proves how the system computed internal state; the media provenance proves how a public artifact was generated.

---

## 12. The minimum viable extension

For a Phase-0 (file 09) integration in 1–2 weeks of work:

1. Add E14, E15, E16 (the three affect embedders, no specialised hardware needed).
2. Add `CF_LEARNER_PROFILE`, `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_DELTA_LOG`.
3. Add `compute_delta_s`, `compute_delta_c`, `compute_delta_e`, `compute_L`.
4. Test on Chris himself using the webcam + microphone in the Context Graph dev workstation.
5. Show plots of L, ΔS, ΔC, ΔE over a 30-min coding session.

Hardware required: existing webcam + microphone. Total new code: ~2000 lines Rust + ~500 Python (preprocessing). Tests: ~20.

This is the proof of concept. Phase 1 onwards adds the wearable, EEG, full M(t) integral, and the dashboard UI (file 09).

---

## 13. Compatibility with the v3 Queen Coordinator (PAPER.md)

Chris's v3 plan (per the agent listing in the system) involves a v3-queen-coordinator with 15-agent concurrent swarm. The UTL extension fits cleanly:

- Each learner gets a dedicated `LearnerStateAgent` (a v3-style worker).
- The Queen coordinator can spawn UTL diagnostic agents for any active learner.
- Cross-learner queries (cohort comparisons, anonymised aggregations) go through the Queen.
- Learner data isolation is enforced by per-learner agent boundaries.

This makes the UTL extension naturally horizontally scalable and aligns with Chris's existing v3 roadmap.

---

## 14. Summary of touch points

| Layer | Existing | Added | Total |
|---|---|---|---|
| Embedder slots | 14 | 7 (E15-E21) | 21 |
| RocksDB column families | 59 | 11 | 70 |
| MCP tools | 75 | 8 | 83 |
| Background tasks | several | 1 (`LearnerStateUpdater`) | +1 |
| Dashboard route groups | several | 1 (`/learner`) | +1 |
| Test cases / attributes (estimate) | 5,199 | ~44 | ~5,243 |
| Lines of code (estimate) | 370,000 Rust + scripts | ~5,000 Rust + ~2,000 Python | ~377,000 |

This is a bounded extension rather than a fork: the existing content-memory substrate stays intact, and the learner-measurement layer is added beside it with versioned storage, explicit provenance, and new MCP tools.

---

## Sources

- [Context Graph 01 System Overview](../../../contextgraph/docs2/contextgraphspecs/01_system_overview.md)
- [Context Graph 04 Database Schema](../../../contextgraph/docs2/contextgraphspecs/04_database_schema.md)
- [Context Graph 05 Embedding Subsystem](../../../contextgraph/docs2/contextgraphspecs/05_embedding_subsystem.md)
- [Context Graph 08 MCP Tools Reference](../../../contextgraph/docs2/contextgraphspecs/08_mcp_tools_reference.md)
- [Context Graph 13 Teleological Subsystem](../../../contextgraph/docs2/contextgraphspecs/13_teleological_subsystem.md)
- [Context Graph 16 Training Subsystem](../../../contextgraph/docs2/contextgraphspecs/16_training_subsystem.md)
- [Context Graph 17 Constellation Subsystem](../../../contextgraph/docs2/contextgraphspecs/17_constellation_subsystem.md)
- [Context Graph 18 Contrastive and Typed Edges](../../../contextgraph/docs2/contextgraphspecs/18_contrastive_and_typed_edges.md)
- [Context Graph 19 CAMD-DPO Subsystem](../../../contextgraph/docs2/contextgraphspecs/19_camd_dpo_subsystem.md)
- [ClipCannon Provenance System](../../../clipcannon/docs2/clipcannondocs/05_provenance_system.md)
- [ClipCannon Phoenix Engine docs (16_phoenix_engine.md)](../../../clipcannon/docs2/clipcannondocs/16_phoenix_engine.md)
- [ClipCannon VoiceAgent docs (17_voiceagent.md)](../../../clipcannon/docs2/clipcannondocs/17_voiceagent.md)
- [C2PA Technical Specification 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
- [PAPER.md TCT framework](../PAPER.md)
- [UTL v2 Embodied Edition (this plan's foundation)](../UTL_v2_embodied.md)
