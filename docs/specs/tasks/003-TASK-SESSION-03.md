# TASK-SESSION-03: Add ConsciousnessState.short_name()

```xml
<task_spec id="TASK-SESSION-03" version="1.0">
<metadata>
  <title>Add ConsciousnessState.short_name() Method</title>
  <status>pending</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-03</requirement_ref>
  </implements>
  <depends_on><!-- None --></depends_on>
  <estimated_hours>0.5</estimated_hours>
</metadata>
```

## Objective

Add short_name() method to ConsciousnessState enum returning 3-character codes for minimal token output in PreToolUse hook.

## Context

The PreToolUse hook has a strict ~20 token budget for output. Using full state names like "CONSCIOUS" or "FRAGMENTED" wastes tokens. 3-character codes (CON, FRG, etc.) provide the same information in minimal space.

## Implementation Steps

1. Locate existing ConsciousnessState enum in state_machine/types.rs
2. Add short_name() method with #[inline] annotation
3. Return static &str for each variant
4. Add unit test for all variants

## Input Context Files

```xml
<input_context_files>
  <file purpose="enum_definition">crates/context-graph-core/src/gwt/state_machine/types.rs</file>
</input_context_files>
```

## Files to Create

None.

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/gwt/state_machine/types.rs` | Add short_name() method to ConsciousnessState impl |

## Rust Signatures

```rust
// Extension to existing ConsciousnessState

impl ConsciousnessState {
    #[inline]
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Conscious => "CON",
            Self::Emerging => "EMG",
            Self::Fragmented => "FRG",
            Self::Dormant => "DOR",
            Self::Hypersync => "HYP",
        }
    }
}
```

## Definition of Done

### Acceptance Criteria

- [ ] Returns "CON" for Conscious
- [ ] Returns "EMG" for Emerging
- [ ] Returns "FRG" for Fragmented
- [ ] Returns "DOR" for Dormant
- [ ] Returns "HYP" for Hypersync
- [ ] Method is #[inline] for zero call overhead
- [ ] Test case TC-SESSION-04 passes (all state mappings)

### Constraints

- Must be #[inline] for inlining in hot path
- Returns &'static str (no allocation)
- Exactly 3 characters per code
- All 5 variants covered

### Verification Commands

```bash
cargo build -p context-graph-core
cargo test -p context-graph-core short_name
```

## Test Cases

### TC-SESSION-04: All State Mappings
```rust
#[test]
fn test_consciousness_state_short_name() {
    assert_eq!(ConsciousnessState::Conscious.short_name(), "CON");
    assert_eq!(ConsciousnessState::Emerging.short_name(), "EMG");
    assert_eq!(ConsciousnessState::Fragmented.short_name(), "FRG");
    assert_eq!(ConsciousnessState::Dormant.short_name(), "DOR");
    assert_eq!(ConsciousnessState::Hypersync.short_name(), "HYP");

    // Verify all are exactly 3 chars
    for state in [
        ConsciousnessState::Conscious,
        ConsciousnessState::Emerging,
        ConsciousnessState::Fragmented,
        ConsciousnessState::Dormant,
        ConsciousnessState::Hypersync,
    ] {
        assert_eq!(state.short_name().len(), 3);
    }
}
```

## Exit Conditions

- **Success**: All 5 variants return correct codes
- **Failure**: Missing variant, wrong code - error out with detailed logging

## Next Task

After completion, proceed to **004-TASK-SESSION-04** (CF_SESSION_IDENTITY Column Family).

```xml
</task_spec>
```
