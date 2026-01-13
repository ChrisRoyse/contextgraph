# TASK-IDENTITY-001: Add IdentityCritical variant to ExtendedTriggerReason

```xml
<task_spec id="TASK-IDENTITY-001" version="1.0">
<metadata>
  <title>Add IdentityCritical variant to ExtendedTriggerReason</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>19</sequence>
  <implements><requirement_ref>REQ-IDENTITY-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>1</estimated_hours>
</metadata>

<context>
ExtendedTriggerReason enum needs a new variant for Identity Continuity crisis triggers.
When IC drops below 0.5, the system should trigger dream consolidation.
Constitution: AP-26, AP-38, IDENTITY-007
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/types.rs (existing enum)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.1)
</input_context_files>

<scope>
<in_scope>
- Add IdentityCritical { ic_value: f32 } variant to ExtendedTriggerReason
- Update Display impl for new variant
- Update Serialize/Deserialize if needed
- Add documentation referencing constitution
</in_scope>
<out_of_scope>
- TriggerManager logic (TASK-IDENTITY-003)
- DreamEventListener (TASK-DREAM-003)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/dream/types.rs

/// Extended trigger reasons for dream consolidation.
///
/// Priority order (highest to lowest):
/// 1. Manual - User-initiated
/// 2. IdentityCritical - IC < 0.5 threshold (AP-26, AP-38)
/// 3. GpuIdle - GPU < 80% eligibility
/// 4. HighEntropy - Entropy threshold exceeded
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExtendedTriggerReason {
    /// User manually triggered consolidation
    Manual,

    /// Coherence threshold exceeded
    CoherenceThreshold { current: f32, threshold: f32 },

    /// Time-based trigger
    TimeBased { interval_secs: u64 },

    /// Memory pressure trigger
    MemoryPressure { used_mb: u64, threshold_mb: u64 },

    /// GPU became idle (< 80% eligibility threshold)
    GpuIdle { utilization: f32 },

    /// High entropy in graph structure
    HighEntropy { current: f32, threshold: f32 },

    /// **NEW**: Identity Continuity crisis (IC < 0.5)
    /// Constitution: AP-26, AP-38, IDENTITY-007
    IdentityCritical {
        /// The IC value that triggered the crisis (must be < 0.5)
        ic_value: f32,
    },
}

impl std::fmt::Display for ExtendedTriggerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing variants ...
            Self::IdentityCritical { ic_value } => {
                write!(f, "IdentityCritical(IC={:.3})", ic_value)
            }
        }
    }
}
```
</signatures>
<constraints>
- Variant MUST be named IdentityCritical
- ic_value field MUST be f32
- Display impl MUST show IC value with 3 decimal places
- Documentation MUST reference AP-26, AP-38, IDENTITY-007
</constraints>
<verification>
```bash
cargo check -p context-graph-core
cargo test -p context-graph-core trigger_reason
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/dream/types.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core types
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Why IdentityCritical?

Identity Continuity (IC) measures how consistent the system's "sense of self" is.
When IC drops below 0.5, the system is experiencing an "identity crisis" and needs
dream consolidation to re-integrate memories and restore coherence.

### Priority Ordering

IdentityCritical is second priority (after Manual) because:
1. Identity crisis is serious and requires immediate attention
2. But user manual triggers should always take precedence
3. GPU idle and entropy are opportunistic, not critical

### Serialization

The variant must serialize/deserialize correctly for:
- State persistence
- MCP responses
- Logging/monitoring
