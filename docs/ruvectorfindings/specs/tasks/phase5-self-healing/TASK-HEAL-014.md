# TASK-HEAL-014 — LoraRefresher::refresh (rank=32 epochs=10; frozen-base byte-equal invariant)

```xml
<task_spec id="TASK-HEAL-014" version="1.0">

<metadata>
  <title>LoRA-adapter refresher in `crates/context-graph-mejepa/src/heal/lora_refresh.rs` (delegates to shipped context-graph-embeddings)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-HEAL-08">LoRA refresh on plasticity collapse: rank=32, epochs=10; reuses crates/context-graph-embeddings/src/training/lora.rs; frozen-base byte-equal invariant</item>
    <item ref="BR-HEAL-06">Mutate ONLY LoRA adapter weights; frozen-base embedder weights MUST be byte-equal before/after (FSV-asserted)</item>
    <item ref="EC-HEAL-09">Multiple embedders need refresh — serialize in priority order (highest ΔK contribution first); cap concurrent refreshes at 1 (VRAM)</item>
    <item ref="ERR-HEAL-07">MEJEPA_HEAL_LORA_REFRESH_FAIL (NaN, OOM, plasticity not restored, base mutated)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/lora_refresh.rs); 1 modified (src/heal/mod.rs); 1 modified (Cargo.toml)</estimated_files>
</metadata>

<context>
Logic 9 of 10. Implements `LoraRefresher::refresh()` — the LoRA-adapter refresher fired by REGULATE
priority when ΔΩ.effective_plasticity drops below 0.4. The body delegates the heavy lifting to the
SHIPPED `crates/context-graph-embeddings/src/training/lora.rs` `LoraTrainer::fine_tune` (rank=32,
alpha=16.0, dropout=0.1, epochs=10, lr=1e-4) and asserts the frozen-base byte-equality invariant
(BR-HEAL-06): the base embedder weights' SHA-256 MUST be byte-equal before and after refresh; only
the LoRA adapter weights mutate. On NaN/OOM/base-mutated/plasticity-not-restored: returns
LoraRefreshFail. Multiple-embedder refresh is serialized by ΔK contribution priority (EC-HEAL-09;
VRAM cap 1 concurrent refresh). This task is `ready` (not blocked) because the LoRA training
infrastructure is already shipped per `_index.md` Reuse note.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_HEAL_LORA_REFRESH_FAIL; `<utml_v1_invariants>` ΔΩ.effective_plasticity component)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-08; AC-HEAL-09-01..03; EC-HEAL-09; ERR-HEAL-07; BR-HEAL-06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts LoraRefresher::refresh 7-step body; data_models LoraRefresher)</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §6, §8 (LoRA self-healing wiring; refresh_lora_adapter contract)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError, LoraRefresher struct, LoraRefreshReport)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_HEAL_REPORTS for LoraRefreshReport persistence)</file>
  <file purpose="prior_task">TASK-HEAL-008 (SelfHealingPipeline::regulate_substrate dispatches to refresh on plasticity&lt;0.4)</file>
  <file purpose="shipped_lora">crates/context-graph-embeddings/src/training/lora.rs (LoraTrainer::fine_tune shipped; rank/alpha/dropout/epochs/lr signatures)</file>
  <file purpose="shipped_lora_config">crates/context-graph-embeddings/src/training/lora.rs (LoraConfig: rank, alpha, dropout, target_modules=["query","key","value"])</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-004..005 + TASK-HEAL-008 complete</check>
  <check>Shipped: `context_graph_embeddings::training::lora::LoraTrainer::fine_tune(base: &EmbedderHandle, lora_adapter: &mut LoraAdapter, corpus: &CorpusSlice, config: LoraConfig) -> Result<FineTuneReport, EmbeddingError>`</check>
  <check>Shipped: `context_graph_embeddings::training::lora::LoraConfig { rank: usize, alpha: f32, dropout: f32, epochs: u32, lr: f32, target_modules: Vec<String> }`</check>
  <check>Phase 3 train cert window exposes per-embedder dK contribution (so highest-ΔK embedder is identifiable)</check>
  <check>HealStatus + ComponentBreakdown (TASK-HEAL-008) expose `embedder_id_with_highest_dk_contribution: Option<u32>`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/lora_refresh.rs`:
      - `pub fn refresh(refresher: &mut LoraRefresher, embedder_id: EmbedderId, fresh_corpus_slice: &CorpusSlice, storage: Arc<RocksDbStore>, train_cert_reader: &dyn TrainCertReader) -> Result<LoraRefreshReport, HealError>` per component_contracts (7-step body):
        1. Capture `pre_base_sha = sha256(base_embedder.weights_canonical_bytes())` (the FROZEN base; MUST be unchanged after refresh)
        2. Capture `pre_lora_sha = sha256(lora_adapter.weights_canonical_bytes())` (the MUTABLE adapter; MUST differ after refresh)
        3. Capture `pre_plasticity = train_cert_reader.read_last_window_effective_plasticity(embedder_id)?` (ΔΩ.effective_plasticity from train cert window)
        4. Reuse `context_graph_embeddings::training::lora::LoraTrainer::fine_tune` with config:
           - `rank=refresher.rank` (default 32)
           - `alpha=refresher.alpha` (default 16.0)
           - `dropout=refresher.dropout` (default 0.1)
           - `epochs=refresher.epochs` (default 10)
           - `lr=1e-4`
           - `target_modules=["query", "key", "value"]` (Q/K/V of attention; per shipped LoRA config)
           - `corpus=fresh_corpus_slice`
        5. On NaN gradient mid-fine_tune → return Err(LoraRefreshFail { embedder_id, cause: "BatchNan during fine_tune".into() })
        6. On CUDA OOM mid-fine_tune → return Err(LoraRefreshFail { embedder_id, cause: "CUDA OOM (VRAM exhausted)".into() })
        7. Capture `post_base_sha = sha256(base_embedder.weights_canonical_bytes())`; assert `post_base_sha == pre_base_sha`; else return Err(LoraRefreshFail { embedder_id, cause: "base mutated (BR-HEAL-06 violation)".into() })
        8. Capture `post_lora_sha = sha256(lora_adapter.weights_canonical_bytes())`; assert `post_lora_sha != pre_lora_sha`; else return Err(LoraRefreshFail { embedder_id, cause: "LoRA weights unchanged after fine_tune".into() })
        9. Run mini-eval on fresh_corpus_slice to recompute `post_plasticity = compute_effective_plasticity(embedder_id, &fresh_corpus_slice)`; assert `post_plasticity > 0.4`; else return Err(LoraRefreshFail { embedder_id, cause: format!("plasticity not restored: {} (require > 0.4)", post_plasticity) })
        10. Build `LoraRefreshReport { embedder_id, pre_base_sha, post_base_sha, pre_lora_sha, post_lora_sha, pre_plasticity, post_plasticity, num_epochs_run, fine_tune_duration_ms, frozen_at }`
        11. Persist LoraRefreshReport to CF_MEJEPA_HEAL_REPORTS keyed by frozen_at (i64 big-endian)
      - `pub fn refresh_multiple_serial(refresher: &mut LoraRefresher, priority_queue: Vec<(EmbedderId, f32)>, fresh_corpus_slice: &CorpusSlice, storage: Arc<RocksDbStore>, train_cert_reader: &dyn TrainCertReader) -> Result<Vec<LoraRefreshReport>, HealError>` — serializes refresh by ΔK contribution priority per EC-HEAL-09 (cap concurrent at 1; VRAM constraint)
      - Helper: `pub fn weights_canonical_bytes(handle: &EmbedderHandle) -> Vec<u8>` — canonical f32 little-endian serialization (deterministic for sha256)
      - Helper: `pub fn compute_effective_plasticity(embedder_id: EmbedderId, corpus: &CorpusSlice) -> Result<f32, HealError>` — mini-eval against the train cert ΔΩ definition
      - Helper: `pub trait TrainCertReader { fn read_last_window_effective_plasticity(&self, embedder_id: EmbedderId) -> Result<f32, HealError>; }` — abstraction over Phase 3 train cert window
    - Tests:
      - `refresh_succeeds_when_plasticity_restored_above_zero_point_four` (synthetic embedder + LoRA adapter + corpus that triggers plasticity restoration)
      - `refresh_returns_lora_refresh_fail_on_batch_nan_mid_fine_tune` (inject NaN gradient via mock LoraTrainer)
      - `refresh_returns_lora_refresh_fail_on_cuda_oom` (inject OOM via mock LoraTrainer)
      - `refresh_returns_lora_refresh_fail_when_plasticity_not_restored` (post_plasticity = 0.35 → fail)
      - `refresh_asserts_frozen_base_byte_equal_invariant_BR_HEAL_06` (pre_base_sha == post_base_sha)
      - `refresh_returns_lora_refresh_fail_when_base_mutated` (force base weight modification → fail)
      - `refresh_asserts_lora_weights_changed` (pre_lora_sha != post_lora_sha)
      - `refresh_returns_lora_refresh_fail_when_lora_unchanged_after_fine_tune` (degenerate corpus → no gradient → fail)
      - `refresh_persists_lora_refresh_report_to_heal_reports` (CF_MEJEPA_HEAL_REPORTS keyed by frozen_at)
      - `refresh_uses_default_lora_config_rank_32_alpha_16_dropout_0_1_epochs_10`
      - `refresh_multiple_serial_orders_by_dk_contribution_descending`
      - `refresh_multiple_serial_caps_concurrent_at_1` (assert no parallelism via tracing span check)
      - `refresh_multiple_serial_continues_remaining_after_one_fails` (one embedder fails → Err returned for that one; remaining still attempted serially; collected reports include partial successes)
      - `lora_refresh_report_provenance_has_frozen_at_and_pre_post_shas`
      - End-to-end with synthetic embedder: full fine_tune cycle + plasticity recompute + persistence
    - FSV evidence emission for TASK-HEAL-019: `LoraRefreshEvidence { embedder_id, pre_base_sha, post_base_sha, pre_lora_sha, post_lora_sha, pre_plasticity, post_plasticity, num_epochs_run, fine_tune_duration_ms, frozen_base_byte_equal: bool }`
  </in_scope>
  <out_of_scope>
    - LoraTrainer::fine_tune internals (SHIPPED in crates/context-graph-embeddings/src/training/lora.rs)
    - Distillation (TASK-HEAL-015)
    - Plasticity computation formula (defined in UTML §VIII; this task only invokes train cert reader)
    - REGULATE dispatch logic (TASK-HEAL-008 regulate_substrate; this task is the implementation called)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/lora_refresh.rs">
use std::sync::Arc;
use crate::heal::{errors::HealError, data_models::{LoraRefresher, LoraRefreshReport}};
use context_graph_embeddings::training::lora::{LoraTrainer, LoraConfig, LoraAdapter, EmbedderHandle};
use context_graph_mejepa_corpus::CorpusSlice;

pub trait TrainCertReader: Send + Sync {
    fn read_last_window_effective_plasticity(&amp;self, embedder_id: EmbedderId) -&gt; Result&lt;f32, HealError&gt;;
}

pub fn refresh(
    refresher: &amp;mut LoraRefresher,
    embedder_id: EmbedderId,
    fresh_corpus_slice: &amp;CorpusSlice,
    storage: Arc&lt;RocksDbStore&gt;,
    train_cert_reader: &amp;dyn TrainCertReader,
) -&gt; Result&lt;LoraRefreshReport, HealError&gt;;

pub fn refresh_multiple_serial(
    refresher: &amp;mut LoraRefresher,
    priority_queue: Vec&lt;(EmbedderId, f32)&gt;,
    fresh_corpus_slice: &amp;CorpusSlice,
    storage: Arc&lt;RocksDbStore&gt;,
    train_cert_reader: &amp;dyn TrainCertReader,
) -&gt; Result&lt;Vec&lt;LoraRefreshReport&gt;, HealError&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoraRefreshEvidence {
    pub embedder_id: EmbedderId,
    pub pre_base_sha: [u8; 32],
    pub post_base_sha: [u8; 32],
    pub pre_lora_sha: [u8; 32],
    pub post_lora_sha: [u8; 32],
    pub pre_plasticity: f32,
    pub post_plasticity: f32,
    pub num_epochs_run: u32,
    pub fine_tune_duration_ms: u64,
    pub frozen_base_byte_equal: bool,
}
    </signature>
  </signatures>
  <constraints>
    - DELEGATE to shipped `context_graph_embeddings::training::lora::LoraTrainer::fine_tune` (NEVER reinvent — already shipped per `_index.md` Reuse note + `12_EMBEDDER_INTEGRATION.md` §6 verbatim)
    - BR-HEAL-06 frozen-base byte-equality invariant is HARD: pre_base_sha == post_base_sha; violation → Err(LoraRefreshFail), NEVER a soft warning
    - Default LoRA config: rank=32, alpha=16.0, dropout=0.1, epochs=10, lr=1e-4, target_modules=["query","key","value"] (per technical spec data_models)
    - VRAM cap: refresh_multiple_serial caps concurrent at 1 (no tokio::spawn parallelism); priority queue is a sorted Vec by ΔK contribution descending
    - Post-plasticity threshold &gt; 0.4 (NOT &gt;= 0.4); the threshold is the plasticity-collapse trigger, refresh must DEMONSTRABLY restore above
    - LoraRefreshReport persisted to CF_MEJEPA_HEAL_REPORTS with full provenance (pre/post SHAs + duration + epochs)
    - File &lt; 500 lines
    - SCREAMING_SNAKE error code: MEJEPA_HEAL_LORA_REFRESH_FAIL
    - tracing::error! before every Err return; tracing::info! on success with embedder_id + plasticity delta
    - No mock data in tests; real LoraTrainer (small synthetic embedder) + real synthetic corpus
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::lora_refresh` shows ≥ 15 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Frozen-base byte-equality verified via pre_base_sha == post_base_sha assertion
    - LoRA weights changed verified via pre_lora_sha != post_lora_sha assertion
    - post_plasticity > 0.4 enforced (post_plasticity = 0.4 → fail; post_plasticity = 0.41 → pass)
    - Multiple-embedder refresh serializes (no parallelism) per EC-HEAL-09
    - LoraRefreshReport persisted with all 9 fields populated
    - NaN/OOM/base-mutated/lora-unchanged/plasticity-not-restored all return LoraRefreshFail with distinct cause strings
  </verification>
</definition_of_done>

<pseudo_code>
pub fn refresh(refresher, embedder_id, fresh_corpus_slice, storage, train_cert_reader) -> Result<LoraRefreshReport, HealError> {
    let started_at = Instant::now();
    let base_handle = refresher.lora_handle.embedder_handle(embedder_id)?;
    let pre_base_sha = sha256(&weights_canonical_bytes(&base_handle));
    let mut lora_adapter = refresher.lora_handle.adapter_for(embedder_id)?;
    let pre_lora_sha = sha256(&weights_canonical_bytes_lora(&lora_adapter));
    let pre_plasticity = train_cert_reader.read_last_window_effective_plasticity(embedder_id)?;
    let config = LoraConfig {
        rank: refresher.rank,           // 32
        alpha: refresher.alpha,         // 16.0
        dropout: refresher.dropout,     // 0.1
        epochs: refresher.epochs,       // 10
        lr: 1e-4,
        target_modules: vec!["query".into(), "key".into(), "value".into()],
    };
    let fine_tune_report = LoraTrainer::fine_tune(&base_handle, &mut lora_adapter, fresh_corpus_slice, config)
        .map_err(|e| match e.kind() {
            EmbeddingErrorKind::BatchNan => HealError::LoraRefreshFail { embedder_id, cause: "BatchNan during fine_tune".into() },
            EmbeddingErrorKind::CudaOom => HealError::LoraRefreshFail { embedder_id, cause: "CUDA OOM (VRAM exhausted)".into() },
            other => HealError::LoraRefreshFail { embedder_id, cause: format!("fine_tune error: {:?}", other) },
        })?;
    let post_base_sha = sha256(&weights_canonical_bytes(&base_handle));
    if post_base_sha != pre_base_sha {
        tracing::error!(error_code = "MEJEPA_HEAL_LORA_REFRESH_FAIL", "BR-HEAL-06 violation: base mutated");
        return Err(HealError::LoraRefreshFail { embedder_id, cause: "base mutated (BR-HEAL-06 violation)".into() });
    }
    let post_lora_sha = sha256(&weights_canonical_bytes_lora(&lora_adapter));
    if post_lora_sha == pre_lora_sha {
        return Err(HealError::LoraRefreshFail { embedder_id, cause: "LoRA weights unchanged after fine_tune".into() });
    }
    let post_plasticity = compute_effective_plasticity(embedder_id, fresh_corpus_slice)?;
    if post_plasticity <= 0.4 {
        return Err(HealError::LoraRefreshFail { embedder_id, cause: format!("plasticity not restored: {} (require > 0.4)", post_plasticity) });
    }
    let report = LoraRefreshReport {
        embedder_id, pre_base_sha, post_base_sha, pre_lora_sha, post_lora_sha,
        pre_plasticity, post_plasticity,
        num_epochs_run: fine_tune_report.epochs_run,
        fine_tune_duration_ms: started_at.elapsed().as_millis() as u64,
        frozen_at: now_secs(),
    };
    storage.put(CF_MEJEPA_HEAL_REPORTS, &report.frozen_at.to_be_bytes(), &bincode::serialize(&report)?)?;
    tracing::info!(embedder_id = ?embedder_id, pre_plasticity, post_plasticity, "LoRA refresh succeeded");
    Ok(report)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/lora_refresh.rs">refresh + refresh_multiple_serial + TrainCertReader trait + helpers + LoraRefreshEvidence + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod lora_refresh;`; re-export refresh + LoraRefreshReport</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `context-graph-embeddings = { workspace = true, features = ["lora"] }` if not already present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::lora_refresh shows ≥ 15 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Frozen-base byte-equality (BR-HEAL-06) enforced: pre_base_sha == post_base_sha</criterion>
  <criterion>LoRA weights changed: pre_lora_sha != post_lora_sha</criterion>
  <criterion>post_plasticity > 0.4 required (= 0.4 → fail; > 0.4 → pass)</criterion>
  <criterion>Default config rank=32 alpha=16 dropout=0.1 epochs=10 lr=1e-4 target_modules=Q/K/V</criterion>
  <criterion>refresh_multiple_serial serializes by ΔK descending; cap concurrent at 1</criterion>
  <criterion>NaN/OOM/base-mutated/lora-unchanged/plasticity-not-restored → LoraRefreshFail with distinct cause</criterion>
  <criterion>LoraRefreshReport persisted with full provenance (pre/post SHAs + duration + epochs)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::lora_refresh</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
