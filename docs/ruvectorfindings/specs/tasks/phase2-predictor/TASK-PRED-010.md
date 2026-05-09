# TASK-PRED-010 — VRAM budget enforcement (cudarc `cuMemGetInfo` + 8 GB target / 9 GB warn)

```xml
<task_spec id="TASK-PRED-010" version="1.0">

<metadata>
  <title>Implement VRAM probing via cudarc cuMemGetInfo + check_vram_steady_state + 9 GB threshold raise MEJEPA_PRED_VRAM_EXCEEDED</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-PRED-13">Steady-state VRAM ≤ 8 GB at batch 64 on RTX 5090</item>
    <item ref="REQ-PRED-14">Crossing 9 GB warn threshold raises MEJEPA_PRED_VRAM_EXCEEDED</item>
    <item ref="component:VramProbe">vram module + check_vram_steady_state + VramReport</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-001</task_ref>
    <task_ref>TASK-PRED-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified, 1 modified (predictor/mod.rs adds delegating method)</estimated_files>
</metadata>

<context>
The VRAM check is the surface that lets the FSV harness (TASK-PRED-011) and Phase 2 DoD CLI
(TASK-PRED-012) verify REQ-PRED-13 (≤ 8 GB steady state) and the BR-PRED-04 escalation rule
(crossing 9 GB → raise MEJEPA_PRED_VRAM_EXCEEDED, do not auto-evict embedders).

Implementation: query the CUDA driver via cudarc's `result::mem_get_info()` (the safe wrapper
around `cuMemGetInfo`) which returns `(free, total)` bytes. Compute `resident = total - free`,
compare against `VRAM_WARN_THRESHOLD_BYTES` (9 GB; from TASK-PRED-001 const), and:
  - `resident < 8 GB`: Ok(VramReport)
  - `8 GB ≤ resident < 9 GB`: Ok(VramReport) + log warning (BR-PRED-03 — within budget but elevated)
  - `resident ≥ 9 GB`: Err(PredictorError::VramExceeded { vram_resident_bytes, threshold_bytes })

Per TECH-PREDICTOR `<note category="vram">`: poll BETWEEN training steps (not in hot path); the
caller is responsible for cadence. This task implements the pure synchronous probe.

Per EC-PRED-08: if the device is not Cuda (or sm_120a unavailable), the function fails closed
with `PredictorError::DeviceUnavailable` — NO silent CPU fallback (constitution `<anti_patterns>`
"no silent fallback").

Iron Rule observed: this task does NOT call MeJepaPredictor::forward, does NOT call
vicreg_loss, does NOT call run_gradient_hook. It exposes the probe as a pure function +
delegating method on `MeJepaPredictor`.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets vram_steady_state ≤ 29 GB total; failure_taxonomy MEJEPA_PRED_VRAM_EXCEEDED; anti_patterns "no silent fallback")</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-13, REQ-PRED-14; AC-PRED-05-01..02; BR-PRED-03..04; ERR-PRED-03; EC-PRED-08 sm_120a unavailable)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (api_contracts MeJepaPredictor::check_vram_steady_state; implementation_notes "vram"; vram_budget_diagram)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-001.md (VRAM_STEADY_STATE_TARGET_BYTES=8GB, VRAM_WARN_THRESHOLD_BYTES=9GB constants)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (PredictorError::VramExceeded + DeviceUnavailable variants)</file>
  <file purpose="cudarc_api">cudarc workspace dependency — `cudarc::driver::result::mem_get_info()` returns (free: usize, total: usize)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/vram.rs (empty placeholder)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001 + TASK-PRED-003 complete</check>
  <check>cargo build --release -p context-graph-mejepa succeeds before this task</check>
  <check>cudarc is in workspace dependencies</check>
  <check>RTX 5090 with sm_120a available for cuda-gated tests; CPU-host fallback test verifies the DeviceUnavailable refusal path</check>
</prerequisites>

<scope>
  <in_scope>
    - Fill `crates/context-graph-mejepa/src/vram.rs` with: `VramReport` struct + `check_vram_steady_state` free function + `synchronize_device` helper
    - Add `MeJepaPredictor::check_vram_steady_state(&self) -> Result<VramReport, PredictorError>` delegating method
    - `VramReport` carries: `free_bytes`, `total_bytes`, `resident_bytes`, `warn_threshold_bytes`, `target_bytes`, `device: String`, `dod_pass_8gb: bool`, `crossed_warn_threshold: bool`
    - VramReport derives Serialize for FSV evidence (TASK-PRED-011)
    - Add `pub fn vram_resident_bytes() -> Result<u64, PredictorError>` convenience function (for `bin/mejepa.rs` DoD output)
    - Synchronize device before probe per TECH-PREDICTOR `<note category="vram">`: ensures we measure post-step steady-state, not transient peak
    - Add 5 unit tests: VramReport.dod_pass_8gb classification at 7.99/8.01/8.99/9.01 GB synthetic values; CPU device → Err(DeviceUnavailable); within-budget path returns Ok; over-warn-threshold path returns Err(VramExceeded); serde round-trip on VramReport
  </in_scope>
  <out_of_scope>
    - Phase 3 training-loop polling cadence / "every 10 steps" caching (Phase 3 — out of Phase 2 scope)
    - Auto-eviction of E15-E21 (constitution forbids per BR-PRED-04 — operator decides)
    - VRAM measurement DURING forward pass (this task probes BETWEEN steps; constitution `<note category="vram">` "Poll BETWEEN training steps, not in the hot path")
    - Gradient hook (TASK-PRED-009)
    - VICReg loss (TASK-PRED-007)
    - FSV evidence file writing (TASK-PRED-011)
    - Gradient checkpointing toggle (Phase 2 default off; flag only — TASK-PRED-001 + TASK-PRED-005)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/vram.rs">
use candle_core::Device;
use serde::Serialize;

use crate::config::{VRAM_STEADY_STATE_TARGET_BYTES, VRAM_WARN_THRESHOLD_BYTES};
use crate::error::PredictorError;

#[derive(Debug, Clone, Serialize)]
pub struct VramReport {
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub resident_bytes: u64,
    pub warn_threshold_bytes: u64,
    pub target_bytes: u64,
    pub device: String,
    pub dod_pass_8gb: bool,           // resident_bytes &lt; VRAM_STEADY_STATE_TARGET_BYTES
    pub crossed_warn_threshold: bool, // resident_bytes >= VRAM_WARN_THRESHOLD_BYTES
}

/// Synchronize the device + query cudarc cuMemGetInfo + assemble VramReport.
/// Returns Err(VramExceeded) when resident_bytes >= 9 GB warn threshold (per BR-PRED-04).
/// Returns Err(DeviceUnavailable) when device is not Cuda (no silent CPU fallback).
pub fn check_vram_steady_state(device: &amp;Device) -&gt; Result&lt;VramReport, PredictorError&gt;;

/// Convenience: just the resident_bytes value. Used by bin/mejepa.rs DoD CLI's stdout.
pub fn vram_resident_bytes(device: &amp;Device) -&gt; Result&lt;u64, PredictorError&gt;;

/// Synchronize device prior to VRAM probe — ensures steady-state measurement.
fn synchronize_device(device: &amp;Device) -&gt; Result&lt;(), PredictorError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/predictor/mod.rs">
use crate::vram::{check_vram_steady_state, VramReport};

impl MeJepaPredictor {
    pub fn check_vram_steady_state(&amp;self) -&gt; Result&lt;VramReport, PredictorError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - **Device validation**:
        - If device is `Device::Cpu`: return `Err(PredictorError::DeviceUnavailable { detail: "VRAM probe requires Cuda; cpu has no GPU memory" })`
        - If device is `Device::Cuda(ordinal)`: proceed (cudarc API is per-context; the active CUDA context corresponds to the candle device)
        - Other future variants (e.g. Metal): `Err(PredictorError::DeviceUnavailable { detail: "..." })`
    - **Synchronize before probe**: per TECH-PREDICTOR `<note>` "Poll BETWEEN training steps"; in candle 0.9.x use `device.synchronize()?` (returns `candle_core::Result<()>` mapped to PredictorError::Candle via #[from])
    - **cudarc call**: `cudarc::driver::result::mem_get_info()` returns `Result<(usize, usize), DriverError>`. Map `DriverError` to `PredictorError::DeviceUnavailable { detail: format!("cudarc cuMemGetInfo: {e}") }`
    - **Field computation**:
        - free_bytes = free as u64
        - total_bytes = total as u64
        - resident_bytes = total_bytes.saturating_sub(free_bytes) (defensive: cudarc reports usize; saturating prevents underflow on weird hardware)
        - target_bytes = VRAM_STEADY_STATE_TARGET_BYTES (8 GB)
        - warn_threshold_bytes = VRAM_WARN_THRESHOLD_BYTES (9 GB)
        - dod_pass_8gb = resident_bytes < target_bytes
        - crossed_warn_threshold = resident_bytes >= warn_threshold_bytes
        - device = format!("{:?}", device)  // e.g. "Cuda(0)"
    - **Decision** (after constructing VramReport):
        - If `crossed_warn_threshold`: return `Err(PredictorError::VramExceeded { vram_resident_bytes: resident_bytes, threshold_bytes: warn_threshold_bytes })`
        - Else: log a `tracing::warn!` if `dod_pass_8gb == false` (between 8-9 GB elevated reading per BR-PRED-03), then `Ok(report)`
    - **Cudarc-feature flag**: the cudarc call MUST be inside a `#[cfg(feature = "cuda")]` block; the non-cuda fallback compiles to a function that returns `Err(PredictorError::DeviceUnavailable { detail: "cuda feature not enabled" })`. This keeps the crate buildable on non-CUDA hosts while preserving fail-closed behavior.
    - File length stays &lt; 500 lines
    - No `unwrap()` outside test code; all cudarc errors propagate via mapping
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 61 tests passing (this task adds 5)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Test: VramReport with resident_bytes = 7.99 GB has dod_pass_8gb == true; crossed_warn_threshold == false
    - Test: VramReport with resident_bytes = 8.01 GB has dod_pass_8gb == false; crossed_warn_threshold == false
    - Test: VramReport with resident_bytes = 9.01 GB has crossed_warn_threshold == true (decision-side check_vram_steady_state would Err here)
    - Test: check_vram_steady_state(Device::Cpu) → Err(DeviceUnavailable)
    - Test: VramReport serde round-trip is byte-equal
    - On RTX 5090 (cuda feature gated): probe returns Ok with resident_bytes &gt; 0 and total_bytes ≈ 32 GB
  </verification>
</definition_of_done>

<pseudo_code>
vram.rs:
    pub struct VramReport { ... }

    pub fn check_vram_steady_state(device) -&gt; Result&lt;VramReport, PredictorError&gt; {
        match device {
            Device::Cpu =&gt; return Err(PredictorError::DeviceUnavailable {
                detail: "VRAM probe requires Cuda; cpu has no GPU memory".to_string()
            }),
            Device::Cuda(_) =&gt; {},
            _ =&gt; return Err(PredictorError::DeviceUnavailable { detail: format!("unsupported device: {:?}", device) }),
        }
        synchronize_device(device)?;
        #[cfg(feature = "cuda")]
        let (free, total) = cudarc::driver::result::mem_get_info()
            .map_err(|e| PredictorError::DeviceUnavailable { detail: format!("cudarc cuMemGetInfo: {e}") })?;
        #[cfg(not(feature = "cuda"))]
        return Err(PredictorError::DeviceUnavailable { detail: "cuda feature not enabled".to_string() });

        let free_bytes = free as u64;
        let total_bytes = total as u64;
        let resident_bytes = total_bytes.saturating_sub(free_bytes);
        let target_bytes = VRAM_STEADY_STATE_TARGET_BYTES;
        let warn_threshold_bytes = VRAM_WARN_THRESHOLD_BYTES;
        let dod_pass_8gb = resident_bytes &lt; target_bytes;
        let crossed = resident_bytes &gt;= warn_threshold_bytes;
        let report = VramReport {
            free_bytes, total_bytes, resident_bytes,
            warn_threshold_bytes, target_bytes,
            device: format!("{:?}", device),
            dod_pass_8gb, crossed_warn_threshold: crossed,
        };
        if crossed {
            return Err(PredictorError::VramExceeded {
                vram_resident_bytes: resident_bytes,
                threshold_bytes: warn_threshold_bytes,
            });
        }
        if !dod_pass_8gb {
            tracing::warn!(target: "mejepa::vram",
                "VRAM elevated reading: {} bytes (target {}, warn {})",
                resident_bytes, target_bytes, warn_threshold_bytes);
        }
        Ok(report)
    }

    pub fn vram_resident_bytes(device) -&gt; Result&lt;u64, PredictorError&gt; {
        // Variant that does NOT raise on warn-threshold crossing — returns the bytes for caller inspection.
        // Useful for FSV evidence at-end-of-dryrun probe in TASK-PRED-011.
        // ... mirror logic but skip the Err on crossed_warn_threshold; return resident_bytes.
    }

    fn synchronize_device(device) -&gt; Result&lt;(), PredictorError&gt; {
        device.synchronize().map_err(PredictorError::Candle)
    }

predictor/mod.rs:
    impl MeJepaPredictor::check_vram_steady_state(&amp;self) -&gt; Result&lt;VramReport, PredictorError&gt; {
        check_vram_steady_state(&self.device)
    }

tests:
    test_vram_report_dod_classification (synthetic VramReport at 7.99 / 8.01 / 8.99 / 9.01 GB)
    test_vram_check_cpu_returns_device_unavailable
    test_vram_report_serde_round_trip
    test_vram_check_within_budget_returns_ok    (cuda-gated; uses real probe)
    test_vram_check_over_warn_returns_err       (synthetic via mocking — or skip if mocking is heavyweight; cuda-gated synthetic over-allocation)
</pseudo_code>

<files_to_create>
  <!-- none; modifying placeholder -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/vram.rs">Fill empty placeholder with VramReport + check_vram_steady_state + vram_resident_bytes + synchronize_device + 5 unit tests</file>
  <file path="crates/context-graph-mejepa/src/predictor/mod.rs">Add MeJepaPredictor::check_vram_steady_state delegating method</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">`pub use vram::{check_vram_steady_state, vram_resident_bytes, VramReport};`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">Add `cudarc = { workspace = true, optional = true }` and `tracing = { workspace = true }` if not present; declare `[features] cuda = ["candle-core/cuda", "candle-nn/cuda", "dep:cudarc"]`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings (default features; non-cuda)</criterion>
  <criterion>cargo build --release -p context-graph-mejepa --features cuda succeeds on RTX 5090 host</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 61 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>check_vram_steady_state(Device::Cpu) → Err(PredictorError::DeviceUnavailable)</criterion>
  <criterion>VramReport.dod_pass_8gb classification matches expectation at 7.99/8.01/8.99/9.01 GB synthetic values</criterion>
  <criterion>VramReport.crossed_warn_threshold == true at 9.01 GB</criterion>
  <criterion>check_vram_steady_state on RTX 5090 (cuda-gated) returns Ok with total_bytes ≈ 32 GB</criterion>
  <criterion>VramReport serde round-trip is byte-equal (Serialize derive)</criterion>
  <criterion>No silent CPU fallback — all non-Cuda devices return DeviceUnavailable</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo build --release -p context-graph-mejepa --features cuda</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa vram</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
