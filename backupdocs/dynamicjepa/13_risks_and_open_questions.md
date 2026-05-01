# Risks And Open Questions

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| "Universal" becomes vague | architecture drifts | enforce domain-pack contracts |
| Silent fallback across instruments | invalid predictions | fail-fast required panels |
| Dynamic updates cause forgetting | regressions | artifact versioning, replay, eval gates |
| Binding errors | wrong entities/actions linked | explicit binding source-of-truth and FSV |
| Target leakage | false model quality | split manifests and leakage tests |
| Planner unsafe actions | real-world harm | preconditions, guards, human approvals |
| Too many domains too soon | no proof depth | start with 2-3 domains only |
| Storage explosion | cost and complexity | panel compaction, hashes, tiered storage |
| Model overclaims | credibility loss | narrow claims and benchmark evidence |
| Audit overhead | latency | batch where safe, never skip audit |

## Open Questions

1. Should DynamicJEPA be a new crate or a top-level subsystem inside existing storage/core crates?
2. Which first domain best proves generality without legal drag: code/repo world or Shakespeare tutor?
3. Should the first predictor use a Perceiver-style latent bottleneck, transformer decoder, or state-space model?
4. How should dynamic panels handle optional instruments without creating fallback behavior?
5. What simulator can serve as the first approved planning source of truth?
6. What is the artifact promotion policy from draft to active?
7. What is the minimum cross-domain benchmark that proves the runtime is universal rather than a rebranded vertical?

## Recommended Answers

- Build DynamicJEPA as a separate subsystem with clean crate boundaries.
- Use code/repo world as the first dynamic domain because triggers and outcomes are physically checkable through files, tests, and git state.
- Use ME-JEPA recruiting as the high-value domain after legal/privacy gates.
- Treat optional instruments as explicit panel variants, not hidden substitutes.
- Require every online update to create a new artifact version.

