# 08 Implementation Roadmap

## Phase 0: documentation and inventory

Status: this directory.

Acceptance:

- current state documented
- source-of-truth CFs documented
- workflows documented
- FSV harness plan documented

## Phase 1: learner-state FSV harness

Objective:

Create a script that proves learner-state rows work end-to-end.

Likely file:

```text
scripts/v4_2/learner_state_bigunlock_fsv.py
```

Tasks:

1. start MCP daemon on a clean DB
2. call `get_capability_matrix`
3. call `preflight_learner_assets`
4. register learner
5. record 5 real self-report/E17 observations
6. compile learner constellation
7. read back learner state
8. inspect `ldb` counts
9. run edge cases
10. write JSON report

Acceptance:

- `learner_profile` = 1
- `fingerprints_learner` = 5
- `learner_state_history` = 5
- `learner_constellations` = 1
- edge cases do not mutate counts

## Phase 2: state-conditioned Shakespeare search FSV

Objective:

Prove learner state changes retrieval policy and search output.

Likely file:

```text
scripts/v4_2/shakespeare_state_conditioned_fsv.py
```

Tasks:

1. copy or point to verified Shakespeare DB
2. create learner with three states:
   - optimal
   - stressed
   - negative-valence
3. run identical queries under each state
4. verify selected profile
5. verify returned content IDs and hashes
6. verify no mutation of content or learner state during search

Acceptance:

- stressed state selects `affect_repair`
- negative-valence state selects `affect_priming` or documented policy
- optimal state keeps base profile
- at least one top-5 ranking differs where profile differs
- invalid state-conditioned requests error

## Phase 3: learning event and L/M trace FSV

Objective:

Prove a tutoring interaction can persist outcome and UTL state.

Likely file:

```text
scripts/v4_2/shakespeare_learning_event_fsv.py
```

Tasks:

1. run a state-conditioned Shakespeare query
2. synthesize a real test interaction from real retrieved content
3. record after-state observation
4. call `compute_L` with `persist=true`
5. call `record_learning_event`
6. read event, M trace, delta log
7. inspect CF counts

Note:

The interaction scenario may be synthetic for control, but the content must come from the real Shakespeare DB and the state rows must be real persisted learner observations. Do not use fake content.

Acceptance:

- `learning_events` +1
- `learner_delta_log` +1
- `learner_m_per_trace` +1
- event references real memory IDs
- L calculation matches components

## Phase 4: learner training export FSV

Objective:

Prove events and learner state export into a dataset usable by trainers.

Likely file:

```text
scripts/v4_2/shakespeare_learner_training_fsv.py
```

Tasks:

1. create at least 10 interaction rows
2. include useful/harmful/no-learning outcomes
3. export `reward_model` dataset
4. export `reranker` dataset
5. read previews
6. inspect physical CF count

Acceptance:

- dataset count >= 2
- preview rows non-empty
- matrix shapes match task
- row IDs trace to event/state rows

## Phase 5: runtime constellation guard prototype

Objective:

Use Shakespeare constellations as runtime guardrails.

Likely implementation:

```text
scripts/v4_2/shakespeare_constellation_guard.py
```

or MCP tools if it must be server-side.

Tasks:

1. fingerprint generated candidate text through `store_memory` or a non-persisting embed endpoint if added
2. score against target constellation
3. score nearest corpus passage for memorization risk
4. return accept/revise/abort
5. log correction-required learning event when guard blocks

Acceptance:

- target-style score available
- nearest-source score available
- blocked generation writes a learning event
- temporary probe memory is deleted or a non-persisting fingerprint API exists

## Phase 6: first empirical loop

Objective:

Show measurable improvement from real outcome events.

Tasks:

1. collect 50-100 Shakespeare learner interactions
2. freeze baseline model/retrieval policy
3. export learner pairs
4. train candidate reranker or adapter
5. evaluate held-out prompt suite
6. write comparison report

Acceptance:

- candidate improves at least one predefined metric
- no regression in memorization risk
- no increase in stress-floor violations
- every training row traces to persisted event/state rows

## Phase 7: add one physical channel

Objective:

Move beyond text/self-report into a real E15-E21 physiological channel.

Recommended first channel:

- E19 EDA, if a sensor is available
- E18 PPG/HRV, if wearable export is easier

Tasks:

1. choose hardware/data export
2. implement preprocessing script
3. produce features and raw SHA-256
4. call `record_session_observation`
5. verify state-conditioned policy changes

Acceptance:

- raw file exists and is hashed
- preprocessing version recorded
- state vector row persisted
- policy can route from this channel without text self-report

## File/module touchpoints

| Area | Files |
| --- | --- |
| MCP tool definitions | `crates/context-graph-mcp/src/tools/definitions/utl.rs`, `learning.rs`, `core.rs` |
| MCP handlers | `crates/context-graph-mcp/src/handlers/tools/utl_tools.rs`, `learning_tools.rs`, `memory_tools.rs` |
| learner core | `crates/context-graph-core/src/learner/mod.rs` |
| weights | `crates/context-graph-core/src/weights/mod.rs` |
| storage | `crates/context-graph-storage/src/teleological/rocksdb_store/learner.rs` |
| scripts | `scripts/v4_2/*.py` |
| docs | `docs/bigunlock/*.md` |

