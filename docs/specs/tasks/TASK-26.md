# TASK-DREAM-005: Wire IC monitor to emit IdentityCritical events

```xml
<task_spec id="TASK-DREAM-005" version="1.0">
<metadata>
  <title>Wire IC monitor to emit IdentityCritical events</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>26</sequence>
  <implements><requirement_ref>REQ-DREAM-005</requirement_ref></implements>
  <depends_on>TASK-25</depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The Identity Continuity monitor calculates IC from Kuramoto order parameter and
personality vectors. When IC drops below threshold, it must emit IdentityCritical
event to the EventBus for DreamEventListener to handle.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/server.rs (from TASK-DREAM-004)
- /home/cabdru/contextgraph/crates/context-graph-core/src/gwt/listeners/dream.rs (from TASK-DREAM-003)
</input_context_files>

<scope>
<in_scope>
- Create IdentityContinuityMonitor in GWT module
- Implement IC calculation: IC = cos(PV_t, PV_{t-1}) * r(t)
- Implement threshold checking (< 0.5)
- Emit WorkspaceEvent::IdentityCritical on crisis
- Wire to KuramotoStepper for order parameter
</in_scope>
<out_of_scope>
- PersonalityVector storage
- EventBus implementation
- Full GWT workspace integration
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/gwt/monitors/identity_continuity.rs

/// Identity Continuity monitor.
///
/// Calculates IC from personality vector cosine similarity and Kuramoto order parameter.
/// Constitution: IDENTITY-007, AP-38
pub struct IdentityContinuityMonitor {
    /// Previous personality vector
    prev_pv: Option<Vec<f32>>,
    /// IC threshold for crisis detection
    threshold: f32,
    /// Event sender for crisis notifications
    event_tx: tokio::sync::mpsc::Sender<WorkspaceEvent>,
}

impl IdentityContinuityMonitor {
    /// Create a new IC monitor.
    pub fn new(threshold: f32, event_tx: tokio::sync::mpsc::Sender<WorkspaceEvent>) -> Self;

    /// Update IC with new personality vector and order parameter.
    ///
    /// Emits IdentityCritical event if IC < threshold.
    ///
    /// # Arguments
    /// * `current_pv` - Current personality vector
    /// * `order_parameter` - Kuramoto order parameter r(t)
    pub async fn update(&mut self, current_pv: &[f32], order_parameter: f32);

    /// Calculate IC value.
    ///
    /// IC = cos(PV_t, PV_{t-1}) * r(t)
    fn calculate_ic(prev_pv: &[f32], current_pv: &[f32], order_parameter: f32) -> f32;
}
```
</signatures>
<constraints>
- IC calculation MUST use cosine similarity
- IC MUST be multiplied by order parameter
- Event MUST be emitted when IC < threshold
- prev_pv MUST be updated after calculation
</constraints>
<verification>
```bash
cargo test -p context-graph-core identity_continuity_monitor
cargo test -p context-graph-core test_ic_crisis_emits_event
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-core/src/gwt/monitors/mod.rs
- crates/context-graph-core/src/gwt/monitors/identity_continuity.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/gwt/mod.rs (add monitors module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core monitors
```
</test_commands>
</task_spec>
```

## Implementation Notes

### IC Calculation

```rust
fn calculate_ic(prev_pv: &[f32], current_pv: &[f32], order_parameter: f32) -> f32 {
    // Cosine similarity
    let dot: f32 = prev_pv.iter().zip(current_pv.iter()).map(|(a, b)| a * b).sum();
    let norm_prev: f32 = prev_pv.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_curr: f32 = current_pv.iter().map(|x| x * x).sum::<f32>().sqrt();

    let cos_sim = if norm_prev > 0.0 && norm_curr > 0.0 {
        dot / (norm_prev * norm_curr)
    } else {
        1.0 // If either is zero vector, assume continuity
    };

    // IC = cosine_similarity * order_parameter
    cos_sim * order_parameter
}
```

### Event Emission

```rust
pub async fn update(&mut self, current_pv: &[f32], order_parameter: f32) {
    if let Some(prev) = &self.prev_pv {
        let ic = Self::calculate_ic(prev, current_pv, order_parameter);

        if ic < self.threshold {
            let event = WorkspaceEvent::IdentityCritical {
                ic_value: ic,
                timestamp: std::time::SystemTime::now(),
            };

            if let Err(e) = self.event_tx.send(event).await {
                tracing::error!("Failed to emit IdentityCritical event: {:?}", e);
            }
        }
    }

    // Update previous PV
    self.prev_pv = Some(current_pv.to_vec());
}
```

### Integration Point

This monitor should be called after each Kuramoto step:
1. Get order parameter from KuramotoStepper
2. Get current personality vector from SelfEgoNode
3. Call update() with both
