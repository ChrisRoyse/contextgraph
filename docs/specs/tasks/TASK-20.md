# TASK-IDENTITY-002: Implement TriggerConfig with IC threshold

```xml
<task_spec id="TASK-IDENTITY-002" version="1.0">
<metadata>
  <title>Implement TriggerConfig with IC threshold</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>20</sequence>
  <implements><requirement_ref>REQ-IDENTITY-002</requirement_ref></implements>
  <depends_on>TASK-19</depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>

<context>
TriggerConfig holds configuration for the TriggerManager including the IC threshold.
Constitution specifies IC < 0.5 as the crisis threshold.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/types.rs (from TASK-IDENTITY-001)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.5)
</input_context_files>

<scope>
<in_scope>
- Create TriggerConfig struct in triggers.rs
- Add ic_threshold field (default 0.5)
- Add entropy_threshold field
- Add cooldown Duration field
- Implement validate() with assertions
- Implement Default
</in_scope>
<out_of_scope>
- TriggerManager implementation (TASK-IDENTITY-003)
- GPU integration
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/dream/triggers.rs
use std::time::Duration;

/// Configuration for trigger manager.
#[derive(Debug, Clone)]
pub struct TriggerConfig {
    /// IC threshold for identity crisis (default: 0.5)
    /// Constitution: gwt.self_ego_node.thresholds.critical = 0.5
    pub ic_threshold: f32,

    /// Entropy threshold for high entropy trigger
    pub entropy_threshold: f32,

    /// Cooldown between triggers
    pub cooldown: Duration,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            ic_threshold: 0.5,
            entropy_threshold: 0.8,
            cooldown: Duration::from_secs(60),
        }
    }
}

impl TriggerConfig {
    /// Validate configuration.
    ///
    /// # Panics
    /// Panics if ic_threshold is outside [0, 1] (AP-26 fail-fast)
    pub fn validate(&self) {
        assert!(
            (0.0..=1.0).contains(&self.ic_threshold),
            "ic_threshold must be in [0, 1], got {}",
            self.ic_threshold
        );
        assert!(
            (0.0..=1.0).contains(&self.entropy_threshold),
            "entropy_threshold must be in [0, 1], got {}",
            self.entropy_threshold
        );
    }

    /// Create config with custom IC threshold.
    pub fn with_ic_threshold(mut self, threshold: f32) -> Self {
        self.ic_threshold = threshold;
        self
    }
}
```
</signatures>
<constraints>
- ic_threshold MUST default to 0.5
- validate() MUST panic on invalid thresholds (AP-26)
- Thresholds MUST be in [0, 1] range
- cooldown MUST default to 60 seconds
</constraints>
<verification>
```bash
cargo check -p context-graph-core
cargo test -p context-graph-core trigger_config
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-core/src/dream/triggers.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/dream/mod.rs (add triggers module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core triggers
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Fail-Fast Validation

Per AP-26, invalid configuration should panic immediately rather than silently proceeding:

```rust
pub fn validate(&self) {
    assert!(
        (0.0..=1.0).contains(&self.ic_threshold),
        "ic_threshold must be in [0, 1], got {}",
        self.ic_threshold
    );
}
```

This ensures configuration errors are caught at initialization, not during runtime.

### Cooldown Purpose

The cooldown prevents trigger spam:
- After a trigger, wait at least `cooldown` before next trigger
- Prevents rapid-fire dreams that could destabilize the system
- 60 seconds is a conservative default

### Builder Pattern

The `with_*` methods allow fluent configuration:
```rust
let config = TriggerConfig::default()
    .with_ic_threshold(0.4)
    .with_entropy_threshold(0.9);
```
