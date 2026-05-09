# TASK-EMBED-020 — LoraAdapterManager (register / refresh / merge_oldest_delta)

```xml
<task_spec id="TASK-EMBED-020" version="1.0">
<metadata>
  <title>LoraAdapterManager — register/refresh/merge_oldest_delta with frozen-base SHA pre/post invariant; ≤4 delta-LoRAs per base + SVD compression</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>20</sequence>
  <implements>
    <item ref="REQ-EMBED-06">LoRA fine-tuning surface</item>
    <item ref="BR-EMBED-04">Frozen-base SHA invariance per fit step</item>
    <item ref="BR-EMBED-07">SVD-merge oldest delta on cap (5 = 1 base + 4 delta)</item>
    <item ref="EC-EMBED-05">interrupted fit rolls back; partial cert marked aborted</item>
    <item ref="TECH-EMBEDDERS::component_contracts::LoraAdapterManager">lora_manager.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-011</task_ref>
    <task_ref>TASK-EMBED-015</task_ref>
    <task_ref>TASK-EMBED-016</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
The LoRA adapter manager. Wraps the shipped `crates/context-graph-embeddings/src/training/{lora,optimizer,loss}.rs` infrastructure (clean-room reuse via path-dep; no upstream code copied). Provides `register / refresh / merge_oldest_delta` per REQ-EMBED-06. The frozen-base SHA pre/post invariant is constitutional (`<utml_v1_invariants>` ΔΞ.target_collapse = 0). Per-step SHA check across all 10 epochs catches accidental gradient leakage immediately.
</context>

<input_context_files>
  <file purpose="lora_pattern">crates/context-graph-embeddings/src/training/lora.rs</file>
  <file purpose="optimizer_pattern">crates/context-graph-embeddings/src/training/optimizer.rs</file>
  <file purpose="loss_pattern">crates/context-graph-embeddings/src/training/loss.rs</file>
  <file purpose="lora_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#component_contracts (LoraAdapterManager)</file>
  <file purpose="registration">crates/context-graph-mejepa-embedders/src/registration.rs</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="e5_e11_e14_wrappers">crates/context-graph-mejepa-embedders/src/embedders/{e5_causal.rs,e11_kepler.rs,e14_bge_m3.rs}</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-011 / 015 / 016 complete (E5/E11/E14 wrappers expose forward_with_lora and LoraAdapterHandle placeholder)</check>
  <check>Upstream LoraAdapter + AdamW + InfoNCE accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct LoraAdapterHandle { adapter_id: Uuid, embedder_id: EmbedderId, a: Tensor, b: Tensor, rank: usize, alpha: f32, dropout: f32, kind: AdapterKind, parent_adapter_id: Option<Uuid> }` (concrete, replacing the placeholder defined in TASK-EMBED-011)
    - `pub struct CausalTrainingPair { anchor_text: String, positive_text: String, negative_texts: Vec<String> }` (or import from upstream)
    - `pub struct LoraFitReport { adapter_id: Uuid, embedder_id: EmbedderId, epochs: u32, loss_curve: Vec<f32>, final_loss: f32, walltime_ms: u64, aborted: bool }`
    - `pub struct LoraConfig { rank: usize, alpha: f32, dropout: f32 }` with sensible defaults (32, 32.0, 0.05)
    - `pub struct LoraAdapterManager { ensemble: Arc<Ensemble>, registry: tokio::sync::Mutex<BTreeMap<EmbedderId, Vec<LoraAdapterRegistration>>>, active_handles: tokio::sync::Mutex<BTreeMap<Uuid, LoraAdapterHandle>> }`
    - `pub fn register(&mut self, embedder_id: EmbedderId, kind: AdapterKind, cfg: LoraConfig) -> EmbedResult<Uuid>` per spec behavior (SHA-capture base pre, allocate A/B Vars, persist to RocksDB CF_MEJEPA_LORA_ADAPTERS)
    - `pub async fn refresh(&self, adapter_id: Uuid, corpus_slice: &[CausalTrainingPair], epochs: u32) -> EmbedResult<LoraFitReport>` per spec:
      1. Load adapter; capture frozen_base_sha_pre on the embedder's base buffer
      2. Wrap forward pass so gradient flows ONLY into A, B Vars
      3. Run AdamW + InfoNCE for `epochs` epochs
      4. **Per-epoch**: capture frozen_base_sha_post; assert equals pre (BR-EMBED-04 strict). On mismatch → `EmbedError::FrozenBaseViolation` and roll back A,B to last checkpoint; mark report `aborted = true`
      5. Persist updated A,B to RocksDB
      6. Bump `Ensemble.forward_cache.invalidate_for_embedder(embedder_id, new_weight_sha)`
      7. Return LoraFitReport
    - `pub fn merge_oldest_delta(&mut self, embedder_id: EmbedderId) -> EmbedResult<Uuid>` per spec (SVD-merge oldest delta into base; drop oldest delta from registry; return new base adapter_id)
    - `pub fn list_adapters(&self, embedder_id: EmbedderId) -> Vec<LoraAdapterRegistration>` (read-only)
    - Cap: if Delta count >= 4 at register call → invoke merge_oldest_delta first, then register (BR-EMBED-07)
    - Tests:
      - register Base on E5 with synthetic base buffer → returns Uuid; SHA captured
      - register a 2nd Base on same embedder rejected (one base per embedder)
      - register 4 Deltas on E11 → succeeds; 5th triggers merge_oldest_delta then registers
      - refresh on E5 with 1k synthetic CausalTrainingPair, 10 epochs → final_loss < initial_loss; SHA pre==post; aborted=false
      - refresh with injected gradient leak (test stub that mutates base buffer) → FrozenBaseViolation; aborted=true; rollback observed
      - merge_oldest_delta with 4 deltas → SVD-truncated base, oldest delta dropped, count=4 (1 new base + 3 remaining deltas)
      - forward_cache invalidated after refresh (different weight_sha → cache miss on subsequent encode)
  </in_scope>
  <out_of_scope>
    - LoRA training loss formulation beyond InfoNCE (UTML domain)
    - Multi-GPU LoRA training (single-GPU only)
    - Adapter weight quantization (Phase 6 hygiene domain)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/lora_manager.rs">
      use std::collections::BTreeMap;
      use std::sync::Arc;
      use std::time::Instant;
      use candle_core::{Device, Tensor};
      use chrono::Utc;
      use tokio::sync::Mutex as AsyncMutex;
      use uuid::Uuid;

      use crate::EmbedderId;
      use crate::ensemble::Ensemble;
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::{LoraAdapterRegistration, AdapterKind};

      #[derive(Debug, Clone, Copy, PartialEq)]
      pub struct LoraConfig {
          pub rank: usize,
          pub alpha: f32,
          pub dropout: f32,
      }
      impl Default for LoraConfig { fn default() -&gt; Self { Self { rank: 32, alpha: 32.0, dropout: 0.05 } } }

      pub struct LoraAdapterHandle {
          pub adapter_id: Uuid,
          pub embedder_id: EmbedderId,
          pub a: Tensor,
          pub b: Tensor,
          pub rank: usize,
          pub alpha: f32,
          pub dropout: f32,
          pub kind: AdapterKind,
          pub parent_adapter_id: Option&lt;Uuid&gt;,
      }

      #[derive(Debug, Clone)]
      pub struct CausalTrainingPair {
          pub anchor_text: String,
          pub positive_text: String,
          pub negative_texts: Vec&lt;String&gt;,
      }

      #[derive(Debug, Clone)]
      pub struct LoraFitReport {
          pub adapter_id: Uuid,
          pub embedder_id: EmbedderId,
          pub epochs: u32,
          pub loss_curve: Vec&lt;f32&gt;,
          pub final_loss: f32,
          pub walltime_ms: u64,
          pub aborted: bool,
      }

      pub const MAX_DELTA_PER_EMBEDDER: usize = 4;

      pub struct LoraAdapterManager {
          ensemble: Arc&lt;Ensemble&gt;,
          registry: AsyncMutex&lt;BTreeMap&lt;EmbedderId, Vec&lt;LoraAdapterRegistration&gt;&gt;&gt;,
          active_handles: AsyncMutex&lt;BTreeMap&lt;Uuid, LoraAdapterHandle&gt;&gt;,
      }

      impl LoraAdapterManager {
          pub fn new(ensemble: Arc&lt;Ensemble&gt;) -&gt; Self;
          pub async fn register(&amp;self, embedder_id: EmbedderId, kind: AdapterKind, cfg: LoraConfig) -&gt; EmbedResult&lt;Uuid&gt;;
          pub async fn refresh(&amp;self, adapter_id: Uuid, corpus_slice: &amp;[CausalTrainingPair], epochs: u32) -&gt; EmbedResult&lt;LoraFitReport&gt;;
          pub async fn merge_oldest_delta(&amp;self, embedder_id: EmbedderId) -&gt; EmbedResult&lt;Uuid&gt;;
          pub async fn list_adapters(&amp;self, embedder_id: EmbedderId) -&gt; Vec&lt;LoraAdapterRegistration&gt;;

          /// Compute SHA-256 over the embedder's base weight buffer (cudarc → CPU pipe in 4 MB chunks).
          async fn capture_base_sha(&amp;self, embedder_id: EmbedderId) -&gt; EmbedResult&lt;String&gt;;

          /// Roll back active LoRA tensors to last checkpoint snapshot.
          async fn rollback(&amp;self, adapter_id: Uuid) -&gt; EmbedResult&lt;()&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - rank ∈ [1, 256]; default 32.
    - alpha ∈ (0.0, 1024.0]; default 32.0.
    - dropout ∈ [0.0, 0.5]; default 0.05.
    - One Base adapter per embedder; up to 4 Delta. 5th Delta triggers merge_oldest_delta.
    - **BR-EMBED-04**: capture_base_sha BEFORE training; capture_base_sha AFTER each epoch; on mismatch → FrozenBaseViolation, rollback, abort.
    - SVD-merge per spec: U = α_d/r_d · (A_d @ B_d) + α_b/r_b · (A_b @ B_b); SVD-truncate to rank r_b → A_b', B_b'.
    - Rollback strategy: snapshot A,B before each epoch; on abort, restore from snapshot, write LoraFitReport with `aborted = true` and `loss_curve` truncated.
    - On successful refresh, bump `forward_cache.invalidate_for_embedder(embedder_id, new_weight_sha)` so subsequent encodes go through the adapter.
    - Persist registrations to RocksDB CF_MEJEPA_LORA_ADAPTERS via `context-graph-storage` (TODO: storage CF wiring is a separate concern; this task uses an in-memory KV trait + a stub file-backed impl for tests; production wiring is a Phase 6 hygiene task).
    - All training calls go through upstream `context-graph-embeddings::training` — clean-room reuse via path-dep, no upstream modification.
    - tracing instrumentation per epoch.
  </constraints>

  <verification>
    - register Base on E5 succeeds; second Base rejected
    - 4 Deltas allowed; 5th triggers merge
    - refresh happy path: aborted=false, SHA pre==post
    - refresh with stub gradient leak: FrozenBaseViolation; aborted=true; rollback observed
    - merge_oldest_delta SVD math correct (verified against known reference matrix)
    - forward_cache invalidated post-refresh (different weight_sha)
    - cargo test lora_manager passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
lora_manager.rs:
  - Structs as above.
  - register():
      let mut registry = self.registry.lock().await;
      let adapters = registry.entry(embedder_id).or_default();
      let bases = adapters.iter().filter(|a| a.kind == AdapterKind::Base).count();
      if kind == AdapterKind::Base && bases > 0 { return InvalidInput; }
      let deltas = adapters.iter().filter(|a| a.kind == AdapterKind::Delta).count();
      if kind == AdapterKind::Delta && deltas >= MAX_DELTA_PER_EMBEDDER {
          drop(registry); self.merge_oldest_delta(embedder_id).await?;
          let mut registry = self.registry.lock().await;
          adapters = registry.entry(embedder_id).or_default();
      }
      let base_sha = self.capture_base_sha(embedder_id).await?;
      let reg = LoraAdapterRegistration::new_base(embedder_id, base_sha, ...) or new_delta(...);
      adapters.push(reg.clone());
      // Allocate A,B vars; insert handle into active_handles.
      Ok(reg.adapter_id)
  - refresh():
      let pre_sha = self.capture_base_sha(embedder_id).await?;
      let mut snapshot = clone(A, B);
      let mut loss_curve = Vec::new();
      for epoch in 0..epochs {
          run_one_epoch_via_upstream_trainer(corpus_slice)?;
          let post_sha = self.capture_base_sha(embedder_id).await?;
          if post_sha != pre_sha {
              self.rollback(adapter_id).await?;
              return Err(FrozenBaseViolation { ... });
          }
          loss_curve.push(epoch_loss);
      }
      ensemble.forward_cache.invalidate_for_embedder(embedder_id, new_weight_sha);
      Ok(LoraFitReport { aborted: false, loss_curve, ... })
  - merge_oldest_delta():
      let mut registry = self.registry.lock().await;
      let adapters = registry.get_mut(&embedder_id).ok_or(InvalidInput)?;
      let mut deltas = adapters.iter().filter(|a| a.kind == AdapterKind::Delta).collect::<Vec<_>>();
      deltas.sort_by_key(|d| d.created_at);
      let oldest = deltas.first().ok_or(InvalidInput)?;
      let base = adapters.iter().find(|a| a.kind == AdapterKind::Base).ok_or(InvalidInput)?;
      // SVD-merge math via candle SVD; replace base A,B in active_handles.
      // Drop oldest from registry.
      Ok(new_base_uuid)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/lora_manager.rs">LoraAdapterManager + handles + reports + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e5_causal.rs">Replace `pub struct LoraAdapterHandle;` placeholder with re-export from lora_manager</file>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub use lora_manager::{LoraAdapterManager, LoraConfig, LoraAdapterHandle, LoraFitReport, CausalTrainingPair, MAX_DELTA_PER_EMBEDDER};`</file>
  <file path="crates/context-graph-mejepa-embedders/Cargo.toml">Re-confirm path-dep on `context-graph-embeddings` (already in TASK-EMBED-001)</file>
</files_to_modify>

<validation_criteria>
  <criterion>register Base / Delta cap enforced; second Base on same embedder rejected</criterion>
  <criterion>5th Delta triggers merge_oldest_delta then registers</criterion>
  <criterion>refresh happy path: SHA pre==post across 10 epochs</criterion>
  <criterion>Injected gradient leak triggers FrozenBaseViolation with rollback</criterion>
  <criterion>SVD-merge math verified against reference matrix</criterion>
  <criterion>forward_cache invalidated post-refresh</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders lora_manager</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
