# TASK-DREAM-003: Wire DreamEventListener to TriggerManager

```xml
<task_spec id="TASK-DREAM-003" version="1.0">
<metadata>
  <title>Wire DreamEventListener to TriggerManager</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>24</sequence>
  <implements><requirement_ref>REQ-DREAM-003</requirement_ref></implements>
  <depends_on>TASK-21, TASK-23</depends_on>
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
DreamEventListener receives IdentityCritical events from the EventBus and
forwards them to TriggerManager for dream triggering. This wires together
the IC monitoring with the dream consolidation system.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/triggers.rs (TriggerManager from TASK-IDENTITY-003)
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/gpu_monitor.rs (GpuMonitor from TASK-DREAM-002)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 5.2)
</input_context_files>

<scope>
<in_scope>
- Create listeners/dream.rs in GWT module
- Implement DreamEventListener struct
- Implement handle_event() for IdentityCritical
- Wire to TriggerManager for trigger checking
- Implement signal_dream_consolidation() callback
- Handle GPU monitor errors per AP-26
</in_scope>
<out_of_scope>
- EventBus implementation
- DreamConsolidator implementation
- MCP server integration (TASK-DREAM-004)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/gwt/listeners/dream.rs
use crate::dream::triggers::TriggerManager;
use crate::dream::gpu_monitor::GpuMonitor;
use crate::dream::types::ExtendedTriggerReason;
use crate::gwt::workspace::events::WorkspaceEvent;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Dream event listener that triggers consolidation on IC crisis.
///
/// Constitution: IDENTITY-006, AP-40
pub struct DreamEventListener<G: GpuMonitor> {
    trigger_manager: Arc<Mutex<TriggerManager<G>>>,
}

impl<G: GpuMonitor> DreamEventListener<G> {
    /// Create a new dream event listener.
    pub fn new(trigger_manager: Arc<Mutex<TriggerManager<G>>>) -> Self;

    /// Handle a workspace event.
    ///
    /// # Panics
    /// Panics on lock failure per AP-26 (no silent failures).
    pub async fn handle_event(&self, event: WorkspaceEvent);
}
```
</signatures>
<constraints>
- GPU monitor errors MUST cause panic (AP-26)
- Lock failures MUST cause panic (AP-26)
- IdentityCritical events MUST update TriggerManager
- Trigger results MUST be logged
</constraints>
<verification>
```bash
cargo test -p context-graph-core dream_event_listener
cargo test -p context-graph-core test_ic_crisis_triggers_dream
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-core/src/gwt/listeners/mod.rs
- crates/context-graph-core/src/gwt/listeners/dream.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/gwt/mod.rs (add listeners module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core listeners
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Event Handling Flow

```rust
pub async fn handle_event(&self, event: WorkspaceEvent) {
    match event {
        WorkspaceEvent::IdentityCritical { ic_value, timestamp } => {
            tracing::warn!(
                "Identity crisis detected: IC={:.3} at {:?}",
                ic_value, timestamp
            );

            let mut manager = self.trigger_manager.lock().await;
            manager.update_identity_coherence(ic_value);

            match manager.check_triggers() {
                Ok(Some(reason)) => {
                    tracing::info!("Dream trigger activated: {:?}", reason);
                    self.signal_dream_consolidation(reason).await;
                }
                Ok(None) => {
                    tracing::debug!("No dream trigger (cooldown or ineligible)");
                }
                Err(e) => {
                    // AP-26: GPU monitoring failure is NOT silent
                    panic!(
                        "GPU monitor error during IC crisis: {}. \
                         Cannot proceed without GPU status (AP-26).",
                        e
                    );
                }
            }
        }
        // Handle other events...
        _ => {}
    }
}
```

### Why Panic on GPU Error?

Per AP-26, the system must not silently degrade. If we can't determine GPU status during an IC crisis:
1. We can't safely start a dream (might conflict with active work)
2. We can't safely skip a dream (might miss critical consolidation)
3. Therefore, fail-fast is the only safe option

### Consolidation Signal

The actual consolidation mechanism (channel, callback) is left to implementation.
This task only ensures the trigger is fired.
