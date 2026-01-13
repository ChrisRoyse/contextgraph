# TASK-UTL-001: Fix Johari Blind/Unknown action mapping

```xml
<task_spec id="TASK-UTL-001" version="1.0">
<metadata>
  <title>Fix Johari Blind/Unknown action mapping</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>9</sequence>
  <implements><requirement_ref>REQ-UTL-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>1</estimated_hours>
</metadata>

<context>
The Johari quadrant action mapping has Blind and Unknown actions SWAPPED.
Constitution defines:
- Blind (deltaS>0.5, deltaC<0.5) -> TriggerDream
- Unknown (deltaS>0.5, deltaC>0.5) -> EpistemicAction

Current implementation incorrectly maps:
- Blind -> EpistemicAction (WRONG)
- Unknown -> TriggerDream (WRONG)

This is a critical bug affecting uncertainty management behavior.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-utl/src/johari/retrieval/functions.rs
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 5.3)
</input_context_files>

<scope>
<in_scope>
- Swap Blind and Unknown action mappings in get_suggested_action()
- Add constitution compliance test
- Add documentation referencing constitution definitions
</in_scope>
<out_of_scope>
- Other Johari functions
- MCP tool integration (TASK-MCP-005)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-utl/src/johari/retrieval/functions.rs
use crate::johari::retrieval::action::SuggestedAction;
use crate::types::johari::quadrant::JohariQuadrant;

/// Get the suggested action for a Johari quadrant.
///
/// Constitution mapping (utl.johari):
/// - Open (deltaS<0.5, deltaC>0.5) -> DirectRecall
/// - Hidden (deltaS<0.5, deltaC<0.5) -> GetNeighborhood
/// - Blind (deltaS>0.5, deltaC<0.5) -> TriggerDream
/// - Unknown (deltaS>0.5, deltaC>0.5) -> EpistemicAction
///
/// # CRITICAL FIX (ISS-011)
///
/// Previous implementation had Blind and Unknown SWAPPED.
pub fn get_suggested_action(quadrant: JohariQuadrant) -> SuggestedAction {
    match quadrant {
        // Low surprise, high confidence -> Direct retrieval works
        JohariQuadrant::Open => SuggestedAction::DirectRecall,

        // Low surprise, low confidence -> Explore neighborhood for context
        JohariQuadrant::Hidden => SuggestedAction::GetNeighborhood,

        // High surprise, low confidence -> Need dream consolidation
        // FIXED: Was incorrectly EpistemicAction
        JohariQuadrant::Blind => SuggestedAction::TriggerDream,

        // High surprise, high confidence -> Epistemic action needed
        // FIXED: Was incorrectly TriggerDream
        JohariQuadrant::Unknown => SuggestedAction::EpistemicAction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constitution compliance test for Johari action mapping.
    #[test]
    fn test_all_quadrant_actions_match_constitution() {
        // Constitution: utl.johari.Open = "deltaS<0.5, deltaC>0.5 -> DirectRecall"
        assert_eq!(
            get_suggested_action(JohariQuadrant::Open),
            SuggestedAction::DirectRecall,
            "Open quadrant must map to DirectRecall"
        );

        // Constitution: utl.johari.Hidden = "deltaS<0.5, deltaC<0.5 -> GetNeighborhood"
        assert_eq!(
            get_suggested_action(JohariQuadrant::Hidden),
            SuggestedAction::GetNeighborhood,
            "Hidden quadrant must map to GetNeighborhood"
        );

        // Constitution: utl.johari.Blind = "deltaS>0.5, deltaC<0.5 -> TriggerDream"
        assert_eq!(
            get_suggested_action(JohariQuadrant::Blind),
            SuggestedAction::TriggerDream,
            "Blind quadrant must map to TriggerDream (ISS-011 fix)"
        );

        // Constitution: utl.johari.Unknown = "deltaS>0.5, deltaC>0.5 -> EpistemicAction"
        assert_eq!(
            get_suggested_action(JohariQuadrant::Unknown),
            SuggestedAction::EpistemicAction,
            "Unknown quadrant must map to EpistemicAction (ISS-011 fix)"
        );
    }
}
```
</signatures>
<constraints>
- Blind MUST map to TriggerDream
- Unknown MUST map to EpistemicAction
- Test MUST verify all four quadrants
- Documentation MUST reference constitution sections
</constraints>
<verification>
```bash
cargo test -p context-graph-utl test_all_quadrant_actions_match_constitution
cargo test -p context-graph-utl johari
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-utl/src/johari/retrieval/functions.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-utl
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Semantic Reasoning

Why the mapping makes sense:

**Blind (high surprise, low confidence)**:
- High surprise = unexpected information
- Low confidence = don't trust it
- Action: TriggerDream to consolidate and integrate the surprise

**Unknown (high surprise, high confidence)**:
- High surprise = unexpected information
- High confidence = trust the source
- Action: EpistemicAction to explicitly update beliefs

The swapped version was semantically incorrect because:
- Blind with EpistemicAction would update beliefs we don't trust
- Unknown with TriggerDream would sleep on information we should act on

### Regression Prevention

The test explicitly documents the constitution reference to prevent future regressions.
Any change to this mapping will fail the test and force review of constitution.
