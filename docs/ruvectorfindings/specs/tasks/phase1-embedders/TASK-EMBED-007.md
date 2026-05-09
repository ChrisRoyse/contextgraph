# TASK-EMBED-007 — VramBudget + cudarc cuMemGetInfo enforcer

```xml
<task_spec id="TASK-EMBED-007" version="1.0">
<metadata>
  <title>VramBudget reserve/release with cudarc cuMemGetInfo + cudaMalloc probe-then-free fragmentation pre-check</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-EMBED-05">≤29 GB hard budget enforcement</item>
    <item ref="BR-EMBED-03">refuse load above safety threshold</item>
    <item ref="EC-EMBED-10">cudaMalloc probe-then-free fragmentation pre-check</item>
    <item ref="TECH-EMBEDDERS::component_contracts::VramBudget">vram_budget.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Hard fail-closed VRAM enforcer. cudarc `cuMemGetInfo` is the source of truth (Rust-side bookkeeping is unreliable due to driver fragmentation). Provides reserve / release / report APIs the Ensemble manager calls before every weight load. Alerts at 28 GB; refuses new loads at 29 GB - 512 MB safety margin.
</context>

<input_context_files>
  <file purpose="vram_budget_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#component_contracts (VramBudget)</file>
  <file purpose="vram_table">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#vram_budget_table</file>
  <file purpose="cudarc_pattern">crates/context-graph-mejepa-embedders/Cargo.toml (cudarc dep)</file>
  <file purpose="error_taxonomy">crates/context-graph-mejepa-embedders/src/error.rs</file>
  <file purpose="registration">crates/context-graph-mejepa-embedders/src/registration.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-004 complete (`VramBudgetReport`)</check>
  <check>TASK-EMBED-005 complete (`EmbedError::VramBudgetExceeded`)</check>
  <check>cudarc compiles on the build target (or feature gate `gpu` opts in)</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct VramBudget { cap_bytes: u64, safety_margin_bytes: u64, alert_threshold_bytes: u64, resident_total: u64, per_embedder: BTreeMap<EmbedderId, u64>, lora_adapters_bytes: u64 }`
    - `pub fn new(cap_bytes: u64, safety_margin_bytes: u64) -> Self` defaulting to `(29 * 2^30, 512 * 2^20)` and alert at `28 * 2^30`
    - `pub fn reserve(&mut self, embedder_id: EmbedderId, bytes: u64) -> EmbedResult<()>`
      1. Query `cuMemGetInfo` → `(free_bytes, total_bytes)`
      2. `effective = min(cap_bytes - resident_total, free_bytes - safety_margin_bytes)`
      3. If `bytes > effective` → `EmbedError::VramBudgetExceeded { needed_bytes: bytes, available_bytes: effective, currently_loaded: per_embedder.keys() }`
      4. `cudaMalloc(bytes)` probe → if fails → `EmbedError::VramBudgetExceeded` (fragmentation, EC-EMBED-10)
      5. `cudaFree(probe_ptr)` immediately
      6. Update `resident_total += bytes`; `per_embedder[embedder_id] = bytes`
    - `pub fn release(&mut self, embedder_id: EmbedderId)` subtracts bytes; idempotent on missing entry
    - `pub fn track_lora_bytes(&mut self, delta: i64)` for LoRA adapter accounting
    - `pub fn report(&self) -> VramBudgetReport` snapshot with `cuMemGetInfo` overhead
    - `pub fn cuda_mem_info() -> EmbedResult<(u64, u64)>` wrapper around cudarc returning `(free, total)` bytes; on `feature = "gpu"` calls real cudarc; otherwise stubbed via env var `CG_MEJEPA_VRAM_FAKE_FREE_BYTES` for CI tests
    - Logging: `tracing::warn!` when `resident_total > alert_threshold_bytes` (28 GB)
    - Tests:
      - reserve up to cap succeeds; one byte over → VramBudgetExceeded
      - reserve when `free_bytes - safety_margin < bytes` → VramBudgetExceeded with `available_bytes` correctly reported
      - release idempotent
      - report sums per_embedder + lora_adapters_bytes
      - alert log emitted at 28 GB but reserve still allowed up to 28.5 GB
      - probe-then-free path simulated via stub returning `cudaMalloc` failure → VramBudgetExceeded
  </in_scope>
  <out_of_scope>
    - Calling `reserve()` from Ensemble (TASK-EMBED-009 wires it)
    - Real GPU integration test (gated `--feature gpu`; FSV harness in TASK-EMBED-024 runs it)
    - Eviction policy for E15-E21 idle (mentioned in spec but actual scheduler is out of scope; manager exposes API)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/vram_budget.rs">
      use std::collections::BTreeMap;
      use chrono::Utc;
      use tracing::{debug, warn};

      use crate::EmbedderId;
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::VramBudgetReport;

      pub const DEFAULT_CAP_BYTES: u64 = 29 * (1 &lt;&lt; 30);          // 29 GiB
      pub const DEFAULT_SAFETY_MARGIN_BYTES: u64 = 512 * (1 &lt;&lt; 20); // 512 MiB
      pub const DEFAULT_ALERT_BYTES: u64 = 28 * (1 &lt;&lt; 30);          // 28 GiB

      pub struct VramBudget {
          cap_bytes: u64,
          safety_margin_bytes: u64,
          alert_threshold_bytes: u64,
          resident_total: u64,
          per_embedder: BTreeMap&lt;EmbedderId, u64&gt;,
          lora_adapters_bytes: u64,
      }

      impl VramBudget {
          pub fn new(cap_bytes: u64, safety_margin_bytes: u64) -&gt; Self;
          pub fn with_defaults() -&gt; Self;
          pub fn reserve(&amp;mut self, embedder_id: EmbedderId, bytes: u64) -&gt; EmbedResult&lt;()&gt;;
          pub fn release(&amp;mut self, embedder_id: EmbedderId);
          pub fn track_lora_bytes(&amp;mut self, delta: i64);
          pub fn report(&amp;self) -&gt; VramBudgetReport;
          pub fn resident_total(&amp;self) -&gt; u64;
          pub fn is_alert(&amp;self) -&gt; bool;
      }

      /// Wraps cudarc cuMemGetInfo on `feature = "gpu"`; otherwise reads
      /// `CG_MEJEPA_VRAM_FAKE_FREE_BYTES` env var for deterministic tests.
      pub fn cuda_mem_info() -&gt; EmbedResult&lt;(u64, u64)&gt;;

      /// Wraps cudaMalloc probe-then-free; returns Ok on success, Err on
      /// fragmentation. Stubbed under non-gpu builds via env override
      /// `CG_MEJEPA_VRAM_FAKE_PROBE_FAIL=1`.
      pub fn cuda_malloc_probe(bytes: u64) -&gt; EmbedResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - Default cap 29 GiB; default safety margin 512 MiB; default alert 28 GiB (per REQ-EMBED-05 + TECH §vram_budget_table).
    - reserve() MUST query `cuda_mem_info()` every call — no cached free_bytes.
    - reserve() MUST run probe-then-free when `feature = "gpu"`; under non-gpu builds the stub respects the env override.
    - VramBudgetExceeded includes `currently_loaded: Vec<EmbedderId>` (slot_number u8 mapped from EmbedderId per error.rs schema).
    - tracing::warn at alert threshold; tracing::debug per reserve/release.
    - NO unwrap_or default on `cuda_mem_info()` failure — propagate the error.
    - per_embedder BTreeMap keeps deterministic iteration order for report.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders vram_budget::tests` passes (using env override stubs).
    - reserve up-to-cap path covered; 1-byte-over rejected.
    - release idempotent.
    - report sums correctly across 5 simulated reserves.
    - Alert triggers at 28 GiB exactly.
    - `cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
vram_budget.rs:
  - VramBudget struct with fields above.
  - reserve():
      let (free, _total) = cuda_mem_info()?;
      let cap_avail = self.cap_bytes.saturating_sub(self.resident_total);
      let free_avail = free.saturating_sub(self.safety_margin_bytes);
      let effective = cap_avail.min(free_avail);
      if bytes > effective { return VramBudgetExceeded; }
      cuda_malloc_probe(bytes)?;
      self.resident_total += bytes;
      self.per_embedder.insert(embedder_id, bytes);
      if self.resident_total > self.alert_threshold_bytes { warn!(...); }
      Ok(())
  - release(): if let Some(b) = self.per_embedder.remove(&embedder_id) { self.resident_total -= b; }
  - report(): VramBudgetReport { resident_total_bytes, per_embedder_bytes, lora_adapters_bytes, cuda_driver_overhead_bytes: total - free - resident, captured_at: Utc::now() }
  - cuda_mem_info():
      #[cfg(feature = "gpu")] { call cudarc::driver::result::mem_get_info() }
      #[cfg(not(feature = "gpu"))] { read CG_MEJEPA_VRAM_FAKE_FREE_BYTES + CG_MEJEPA_VRAM_FAKE_TOTAL_BYTES }
  - cuda_malloc_probe(): similar stub.

lib.rs:
  - pub mod vram_budget;
  - pub use vram_budget::{VramBudget, DEFAULT_CAP_BYTES, ...};
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/vram_budget.rs">VramBudget + cuda_mem_info wrapper + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod vram_budget;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>reserve at cap succeeds; over cap returns VramBudgetExceeded</criterion>
  <criterion>fragmentation probe failure returns VramBudgetExceeded</criterion>
  <criterion>release idempotent on missing entry</criterion>
  <criterion>alert log emitted at 28 GB threshold</criterion>
  <criterion>report sums correctly across multiple reserves</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders vram_budget</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
