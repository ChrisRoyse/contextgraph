# TASK-IDENTITY-003: Implement TriggerManager IC checking

```xml
<task_spec id="TASK-IDENTITY-003" version="1.0">
<metadata>
  <title>Implement TriggerManager IC checking</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>21</sequence>
  <implements><requirement_ref>REQ-IDENTITY-003</requirement_ref></implements>
  <depends_on>TASK-20</depends_on>
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
TriggerManager orchestrates dream trigger conditions with priority ordering.
It must check IC against threshold, verify GPU eligibility, and respect cooldown.
Constitution: AP-26 (fail-fast), AP-38 (IC crisis handling)
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/triggers.rs (TriggerConfig from TASK-IDENTITY-002)
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/types.rs (ExtendedTriggerReason from TASK-IDENTITY-001)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.5)
</input_context_files>

<scope>
<in_scope>
- Implement TriggerManager struct with GpuMonitor generic
- Implement new() constructor with config validation
- Implement update_identity_coherence() for IC updates
- Implement update_entropy() for entropy updates
- Implement set_manual_trigger() for manual triggers
- Implement check_triggers() with priority ordering
- Implement check_identity_continuity() helper
- Handle GPU eligibility checking
</in_scope>
<out_of_scope>
- GpuMonitor implementation (TASK-DREAM-002)
- DreamEventListener integration (TASK-DREAM-003)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/dream/triggers.rs
use crate::dream::types::ExtendedTriggerReason;
use crate::dream::gpu_monitor::{GpuMonitor, GpuMonitorError, thresholds};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Manages dream trigger conditions.
///
/// Priority order (highest first):
/// 1. Manual - User-initiated
/// 2. IdentityCritical - IC < 0.5 (AP-26, AP-38)
/// 3. GpuIdle - GPU < 80% eligibility
/// 4. HighEntropy - Entropy threshold exceeded
pub struct TriggerManager<G: GpuMonitor> {
    config: TriggerConfig,
    current_ic: Option<f32>,
    current_entropy: Option<f32>,
    gpu_monitor: Arc<Mutex<G>>,
    last_trigger: Option<Instant>,
    manual_trigger: bool,
}

impl<G: GpuMonitor> TriggerManager<G> {
    /// Create a new trigger manager.
    ///
    /// # Panics
    /// Panics if config validation fails (AP-26)
    pub fn new(config: TriggerConfig, gpu_monitor: G) -> Self;

    /// Update the current Identity Continuity value.
    pub fn update_identity_coherence(&mut self, ic: f32);

    /// Update the current entropy value.
    pub fn update_entropy(&mut self, entropy: f32);

    /// Set manual trigger flag.
    pub fn set_manual_trigger(&mut self);

    /// Check all trigger conditions and return highest priority trigger.
    ///
    /// # Returns
    /// `Some(reason)` if trigger condition met, `None` otherwise.
    ///
    /// # Errors
    /// Returns `Err` if GPU monitoring fails (no silent failure).
    pub fn check_triggers(&mut self) -> Result<Option<ExtendedTriggerReason>, GpuMonitorError>;

    /// Check identity continuity crisis status.
    pub fn check_identity_continuity(&self) -> bool;
}
```
</signatures>
<constraints>
- check_triggers() MUST respect priority order
- GPU monitor errors MUST propagate (no silent failure)
- Cooldown MUST be enforced between triggers
- IC values MUST be clamped to [0, 1] with warning for invalid
- Mutex lock failures MUST panic per AP-26
</constraints>
<verification>
```bash
cargo test -p context-graph-core trigger_manager
cargo test -p context-graph-core test_priority_ordering
cargo test -p context-graph-core test_cooldown_enforcement
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/dream/triggers.rs (add TriggerManager)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core triggers
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Priority Ordering Logic

```rust
pub fn check_triggers(&mut self) -> Result<Option<ExtendedTriggerReason>, GpuMonitorError> {
    // Check cooldown first
    if let Some(last) = self.last_trigger {
        if last.elapsed() < self.config.cooldown {
            return Ok(None);
        }
    }

    // Priority 1: Manual
    if self.manual_trigger {
        self.manual_trigger = false;
        self.last_trigger = Some(Instant::now());
        return Ok(Some(ExtendedTriggerReason::Manual));
    }

    // Priority 2: IdentityCritical
    if let Some(ic) = self.current_ic {
        if ic < self.config.ic_threshold {
            let gpu_eligible = self.gpu_monitor.lock()?.is_eligible_for_dream()?;
            if gpu_eligible {
                self.last_trigger = Some(Instant::now());
                return Ok(Some(ExtendedTriggerReason::IdentityCritical { ic_value: ic }));
            }
        }
    }

    // Priority 3: GpuIdle
    // ... etc
}
```

### NaN/Infinity Handling

IC values might be invalid due to edge cases:
```rust
pub fn update_identity_coherence(&mut self, ic: f32) {
    let ic = if ic.is_nan() || ic.is_infinite() {
        tracing::warn!("Invalid IC value {}, clamping", ic);
        ic.clamp(0.0, 1.0)
    } else {
        ic.clamp(0.0, 1.0)
    };
    self.current_ic = Some(ic);
}
```

### Lock Failure Handling

Per AP-26, lock poisoning is a fatal error:
```rust
let mut monitor = self.gpu_monitor.lock()
    .expect("GPU monitor lock poisoned - panicking per AP-26");
```
