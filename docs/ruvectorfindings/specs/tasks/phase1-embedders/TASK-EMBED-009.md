# TASK-EMBED-009 — Ensemble manager skeleton + forward-pass cache

```xml
<task_spec id="TASK-EMBED-009" version="1.0">
<metadata>
  <title>Ensemble manager owning EmbedderRegistrations + forward-pass cache; load_content_set / load_one / encode dispatch skeleton</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-EMBED-05">load_content_set + load_learner_state + unload</item>
    <item ref="REQ-EMBED-12">forward-cache for p50&lt;100ms panel materialization</item>
    <item ref="EC-EMBED-04">load mutex serializing concurrent inits</item>
    <item ref="TECH-EMBEDDERS::component_contracts::Ensemble">ensemble.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-003</task_ref>
    <task_ref>TASK-EMBED-007</task_ref>
    <task_ref>TASK-EMBED-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
The central resource manager. Owns `VramBudget`, `BTreeMap<EmbedderId, LoadedEmbedder>`, and the `(input_sha, embedder_id, weight_sha)` forward-cache. Provides the dispatch entry point `Ensemble::encode(embedder_id, input)` that downstream EmbedderInstrument wrappers call. This task ships the skeleton + cache + load_one dispatch; the per-embedder forward implementations land in TASK-EMBED-010..019.
</context>

<input_context_files>
  <file purpose="ensemble_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#component_contracts (Ensemble)</file>
  <file purpose="vram_budget">crates/context-graph-mejepa-embedders/src/vram_budget.rs</file>
  <file purpose="digest">crates/context-graph-mejepa-embedders/src/digest.rs</file>
  <file purpose="types">crates/context-graph-mejepa-embedders/src/types.rs</file>
  <file purpose="registration">crates/context-graph-mejepa-embedders/src/registration.rs</file>
  <file purpose="upstream_embedders">crates/context-graph-embeddings/src/types/model_id/repository.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-003, 007, 008 complete</check>
  <check>tokio workspace dep with `sync` feature</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct LoadedEmbedder { id: EmbedderId, registration: EmbedderRegistration, weight_sha: String, version: String, /* implementation handle filled by per-embedder tasks */ inner: Box<dyn EmbedderForward + Send + Sync> }`
    - `pub trait EmbedderForward: Send + Sync { fn forward(&self, input: &EmbedderInput) -> EmbedResult<EmbedderOutput>; }` — per-embedder forward impls (TASK-EMBED-010+) box into this
    - `pub struct Ensemble { config: ModelsConfig, device: candle_core::Device, loaded: tokio::sync::Mutex<BTreeMap<EmbedderId, LoadedEmbedder>>, vram_budget: tokio::sync::Mutex<VramBudget>, forward_cache: ForwardCache, code_version: String }`
    - `pub fn new(config_path: &Path, device: candle_core::Device) -> EmbedResult<Self>` — loads ModelsConfig, builds VramBudget, computes code_version SHA over Cargo.lock-pinned deps
    - `pub async fn load_content_set(&self) -> EmbedResult<VramBudgetReport>` — calls load_one for each E1..E14 in spec order via EmbedderId::CONTENT slice; on any failure the prior loads remain (idempotent)
    - `pub async fn load_one(&self, id: EmbedderId) -> EmbedResult<()>` — acquires load mutex, idempotent if already resident, verifies digest, reserves VRAM, calls per-embedder factory `register_embedder_factory(id)` to build the EmbedderForward impl, runs warmup encode
    - `pub async fn load_learner_state(&self, id: EmbedderId) -> EmbedResult<VramBudgetReport>` — validates id ∈ E15..E21; cuMemGetInfo + 512 MB safety margin pre-flight; cudaMalloc probe-then-free; load
    - `pub async fn unload(&self, id: EmbedderId)` — drops the LoadedEmbedder + releases VRAM budget
    - `pub fn encode(&self, id: EmbedderId, input: &EmbedderInput) -> EmbedResult<EmbedderOutput>` — synchronous dispatch (forward is GPU-bound, not tokio-await):
      1. compute input_sha = input.sha256()
      2. cache lookup (input_sha, id, weight_sha) → on hit, return clone
      3. lookup loaded[id]; if missing → MEJEPA_INFER_INSTRUMENT_MISSING
      4. forward via Box<dyn EmbedderForward>
      5. validate output (dim + finite)
      6. cache; return
    - `pub struct ForwardCache { inner: parking_lot::RwLock<lru::LruCache<CacheKey, EmbedderOutput>>, generation: AtomicU64 }`
    - `CacheKey = (sha: [u8;32], embedder: EmbedderId, weight_sha: [u8;32])`
    - `pub fn invalidate_for_embedder(&self, embedder_id: EmbedderId, new_weight_sha: &str)` — bumps generation; LoRA refresh calls this
    - `pub fn code_version() -> String` — SHA-256 over concat of pinned `Cargo.lock` versions of: candle-core, candle-nn, candle-transformers, cudarc, tokenizers, sha2, ed25519-dalek (per REQ-EMBED-10)
    - `EmbedderForwardFactory` trait: `fn build(device: &Device, registration: &EmbedderRegistration) -> EmbedResult<Box<dyn EmbedderForward + Send + Sync>>;` — used by per-embedder tasks to register themselves
    - `inventory::collect!(EmbedderForwardFactory)` or static registry pattern for plugin-style registration (per-embedder tasks emit `inventory::submit!`)
    - Tests:
      - new() reads a synthetic models_config.toml + tempfile weight stub and constructs Ensemble
      - load_one with stubbed factory + stubbed VRAM env succeeds, makes embedder resident
      - load_one second call idempotent (no double-VRAM-reserve)
      - load_content_set on partial config (only E1, E5) succeeds; E7 missing → DigestMismatch propagated
      - encode cache hit returns clone identically; cache miss invokes factory.forward exactly once; second call is hit
      - invalidate_for_embedder bumps generation; subsequent encode invokes forward
      - code_version is stable across two new() calls in same process
  </in_scope>
  <out_of_scope>
    - Per-embedder forward impls (TASK-EMBED-010..019)
    - LoraAdapterManager (TASK-EMBED-020)
    - MCP tool wiring (TASK-EMBED-023)
    - RocksDB CF wiring (deferred; in-memory cache for now)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/ensemble.rs">
      use std::collections::BTreeMap;
      use std::path::Path;
      use std::sync::Arc;
      use std::sync::atomic::{AtomicU64, Ordering};
      use candle_core::Device;
      use parking_lot::RwLock;
      use tokio::sync::Mutex as AsyncMutex;

      use crate::EmbedderId;
      use crate::cache::{ForwardCache, CacheKey};
      use crate::digest::{ModelsConfig, load_models_config, verify_embedder_digest};
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::{EmbedderRegistration, VramBudgetReport};
      use crate::types::{EmbedderInput, EmbedderOutput};
      use crate::vram_budget::VramBudget;

      pub trait EmbedderForward: Send + Sync {
          fn forward(&amp;self, input: &amp;EmbedderInput) -&gt; EmbedResult&lt;EmbedderOutput&gt;;
      }

      pub trait EmbedderForwardFactory: Send + Sync + 'static {
          fn embedder_id(&amp;self) -&gt; EmbedderId;
          fn build(&amp;self, device: &amp;Device, registration: &amp;EmbedderRegistration) -&gt; EmbedResult&lt;Box&lt;dyn EmbedderForward + Send + Sync&gt;&gt;;
      }

      pub fn register_factory(factory: Box&lt;dyn EmbedderForwardFactory&gt;);

      pub struct LoadedEmbedder {
          pub id: EmbedderId,
          pub registration: EmbedderRegistration,
          pub weight_sha: String,
          pub version: String,
          pub inner: Box&lt;dyn EmbedderForward + Send + Sync&gt;,
      }

      pub struct Ensemble {
          config: ModelsConfig,
          device: Device,
          loaded: AsyncMutex&lt;BTreeMap&lt;EmbedderId, LoadedEmbedder&gt;&gt;,
          vram_budget: AsyncMutex&lt;VramBudget&gt;,
          forward_cache: ForwardCache,
          code_version: String,
      }

      impl Ensemble {
          pub fn new(config_path: &amp;Path, device: Device) -&gt; EmbedResult&lt;Arc&lt;Self&gt;&gt;;
          pub async fn load_content_set(self: &amp;Arc&lt;Self&gt;) -&gt; EmbedResult&lt;VramBudgetReport&gt;;
          pub async fn load_one(self: &amp;Arc&lt;Self&gt;, id: EmbedderId) -&gt; EmbedResult&lt;()&gt;;
          pub async fn load_learner_state(self: &amp;Arc&lt;Self&gt;, id: EmbedderId) -&gt; EmbedResult&lt;VramBudgetReport&gt;;
          pub async fn unload(self: &amp;Arc&lt;Self&gt;, id: EmbedderId);
          pub fn encode(&amp;self, id: EmbedderId, input: &amp;EmbedderInput) -&gt; EmbedResult&lt;EmbedderOutput&gt;;
          pub fn vram_report(&amp;self) -&gt; VramBudgetReport;
          pub fn code_version(&amp;self) -&gt; &amp;str;
      }

      pub fn compute_code_version() -&gt; String;
    </signature>
    <signature file="crates/context-graph-mejepa-embedders/src/cache.rs">
      use lru::LruCache;
      use parking_lot::RwLock;
      use std::sync::atomic::{AtomicU64, Ordering};

      use crate::EmbedderId;
      use crate::types::EmbedderOutput;

      pub type CacheKey = (Vec&lt;u8&gt;, EmbedderId, String); // (input_sha32, embedder, weight_sha_hex)

      pub struct ForwardCache {
          inner: RwLock&lt;LruCache&lt;CacheKey, EmbedderOutput&gt;&gt;,
          generation: AtomicU64,
      }

      impl ForwardCache {
          pub fn new(capacity: usize) -&gt; Self;
          pub fn get(&amp;self, key: &amp;CacheKey) -&gt; Option&lt;EmbedderOutput&gt;;
          pub fn put(&amp;self, key: CacheKey, value: EmbedderOutput);
          pub fn invalidate_for_embedder(&amp;self, embedder_id: EmbedderId, new_weight_sha: &amp;str);
          pub fn generation(&amp;self) -&gt; u64;
      }
    </signature>
  </signatures>

  <constraints>
    - Both files &lt; 500 lines.
    - `Ensemble::new` returns `Arc<Self>` so async methods can move clones into spawned tasks (constitution `<file_organization>` separation).
    - `code_version` SHA-256 covers exactly the deps listed in TECH `<note category="determinism">`: candle-core, candle-nn, candle-transformers, cudarc, tokenizers, sha2, ed25519-dalek. Read versions from Cargo.lock at build via `build.rs` or include `option_env!` fallback for non-Cargo builds.
    - `load_one` MUST verify digest BEFORE reserving VRAM (early fail-closed; no orphan reservation on digest mismatch).
    - `encode` is synchronous; per-embedder forward impls block on the GPU stream — tokio-await isn't used inside the forward path.
    - Forward cache LRU capacity default 8192 entries; configurable via env `CG_MEJEPA_EMBED_CACHE_CAP`.
    - Cache invalidation on LoRA refresh: bump generation atomic; cache get checks generation match (otherwise treat as miss). Simplification: on invalidation, drop entries whose embedder == affected.
    - `register_factory` uses a `OnceLock<RwLock<Vec<Box<dyn EmbedderForwardFactory>>>>` global (or `inventory` if dev deps allow); per-embedder tasks call `register_factory(Box::new(MyFactory))` from a `#[ctor::ctor]` constructor or via explicit `register_all_factories()` invoked by Ensemble::new.
    - `MEJEPA_INFER_INSTRUMENT_MISSING` raised when `loaded[id]` absent in `encode()`.
    - NO panicking; NO `unwrap` outside test code.
    - tracing instrumentation on load / encode for observability.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders ensemble::tests` passes (using stub factories).
    - load_one with stub factory + tempfile fixtures succeeds; second call idempotent.
    - encode cache hit/miss covered.
    - code_version stable across two new() invocations within same process.
    - load_content_set partial-failure semantics: prior loads remain after E7 digest mismatch.
    - `cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
ensemble.rs:
  - LoadedEmbedder + EmbedderForward + EmbedderForwardFactory traits.
  - register_factory: OnceLock<RwLock<Vec<...>>>.
  - Ensemble::new: load_models_config + VramBudget::with_defaults + ForwardCache::new(8192) + compute_code_version.
  - load_one:
      let mut loaded = self.loaded.lock().await;
      if loaded.contains_key(&id) { return Ok(()); }
      let reg = self.config.embedders.get(&id).ok_or(InvalidInput)?;
      verify_embedder_digest(reg)?;
      { let mut budget = self.vram_budget.lock().await; budget.reserve(id, reg.vram_estimate_bytes)?; }
      let factory = lookup_factory(id).ok_or(InstrumentMissing)?;
      let inner = factory.build(&self.device, reg)?;
      let weight_sha = reg.weight_sha256.clone();
      loaded.insert(id, LoadedEmbedder { id, registration: reg.clone(), weight_sha, version: reg.version.clone(), inner });
  - encode (sync):
      let input_sha = input.sha256();
      let key = (input_sha.to_vec(), id, weight_sha_for(id));
      if let Some(out) = self.forward_cache.get(&key) { return Ok(out); }
      let loaded = self.loaded.blocking_lock(); // sync access via tokio::sync::Mutex::blocking_lock
      let le = loaded.get(&id).ok_or(InstrumentMissing)?;
      let out = le.inner.forward(input)?;
      out.validate_finite()?;
      out.validate_dim(id.projected_dimension())?;
      self.forward_cache.put(key, out.clone());
      Ok(out)
  - compute_code_version: env!("CARGO_PKG_VERSION") + Cargo.lock deps via build.rs-emitted env or hardcoded constants in build.rs.

cache.rs:
  - ForwardCache wraps lru::LruCache (workspace dep; add to Cargo.toml).
  - get returns clone; put inserts.
  - invalidate_for_embedder iterates LRU (clear matching keys; LRU lacks selective remove → drain + filter + reinsert).
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/cache.rs">ForwardCache LRU + tests</file>
  <file path="crates/context-graph-mejepa-embedders/build.rs">Emit pinned Cargo.lock versions as env! at compile time for code_version</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/ensemble.rs">Ensemble + EmbedderForward + EmbedderForwardFactory + tests</file>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod ensemble;` + `pub mod cache;` + re-exports</file>
  <file path="crates/context-graph-mejepa-embedders/Cargo.toml">Add `lru`, `parking_lot`, `tracing` workspace deps; build = "build.rs"</file>
</files_to_modify>

<validation_criteria>
  <criterion>load_one digest verify happens BEFORE VRAM reserve</criterion>
  <criterion>load_one idempotent on already-resident</criterion>
  <criterion>encode cache hit returns identical clone; miss invokes factory.forward exactly once</criterion>
  <criterion>code_version stable across new() calls within same process</criterion>
  <criterion>load_content_set partial-failure: prior loads remain</criterion>
  <criterion>register_factory + lookup wired via OnceLock</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders ensemble cache</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
