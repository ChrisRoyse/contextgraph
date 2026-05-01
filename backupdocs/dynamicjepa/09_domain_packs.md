# Domain Packs

A domain pack is how DynamicJEPA becomes universal without becoming unstructured.

## Domain Pack Contents

```text
DomainPack {
  domain_id,
  name,
  event_kinds[],
  action_kinds[],
  entity_kinds[],
  outcome_kinds[],
  adapters[],
  required_instruments[],
  optional_instruments[],
  invariants[],
  verifier_policies[],
  allowed_use_policies[],
  fsv_harnesses[],
  benchmark_suites[]
}
```

## First Domain Packs

| Domain | Why |
| --- | --- |
| ME-JEPA recruiting/career | already planned, strong longitudinal data shape |
| Code/world-of-repo | source files, commits, tests, tools, clear source of truth |
| Shakespeare/text tutor | existing verified corpus and learner-state FSV docs |
| Robotics/video sandbox | aligns with V-JEPA and action-conditioned planning |
| Finance/time-series | clear state/action/outcome but high risk |
| Scientific papers/lab workflows | strong graph and document structure |

## Domain Pack Acceptance

A domain pack is accepted only after:

- adapter FSV passes,
- required instruments register,
- sample panel materializes,
- trajectory builds,
- at least three edge cases fail correctly,
- benchmark suite runs,
- no fallback paths exist.

## Cross-Domain Transfer

Cross-domain transfer happens through shared abstractions:

- time,
- entity,
- action,
- outcome,
- graph relation,
- uncertainty,
- goal,
- invariant,
- guard decision.

The system should transfer mechanics, not pretend domain facts are interchangeable.

Example:

```text
code test failure -> action -> observed result
robot grasp failure -> action -> observed result
candidate placement failure -> action -> observed result
```

Different domains, same transition pattern.

