# 2026-04-30 Manual Edge-Case FSV Report

## Source Of Truth

Bundle:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1`

Career edge evidence:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/fsv_evidence/career_taxonomy/97908837-fa89-427a-aa1a-91ef6ed80059/edge_00.json`
- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/fsv_evidence/career_taxonomy/97908837-fa89-427a-aa1a-91ef6ed80059/edge_01.json`
- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/fsv_evidence/career_taxonomy/97908837-fa89-427a-aa1a-91ef6ed80059/edge_02.json`
- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/fsv_evidence/career_taxonomy/97908837-fa89-427a-aa1a-91ef6ed80059/edge_03.json`
- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/fsv_evidence/career_taxonomy/97908837-fa89-427a-aa1a-91ef6ed80059/edge_04.json`

## Edge Results

| Edge | Expected | Before | After | Outcome |
| --- | --- | --- | --- | --- |
| Empty input | no raw row, no adapter row, no normalized state, audit +1 | raw 0, adapter 0, state 0, transition 0, audit 1 | raw 0, adapter 0, state 0, transition 0, audit 2 | typed `ADAPTER_FAILED`; no invalid data persisted |
| Missing source license | raw +1, adapter +1 failed, no normalized state, no transition, audit +2 | raw 0, adapter 0, state 0, transition 0, audit 2 | raw 1, adapter 1, state 0, transition 0, audit 4 | field path `entity.source_license`; no normalized data persisted |
| Invalid action | raw +1, adapter +1 failed, no normalized state, no transition, audit +2 | raw 1, adapter 1, state 0, transition 0, audit 4 | raw 2, adapter 2, state 0, transition 0, audit 6 | field path `action.move`; unsupported variant rejected |
| Wrong skill-vector dimension | raw +1, adapter +1 failed, no normalized state, no transition, audit +2 | raw 2, adapter 2, state 0, transition 0, audit 6 | raw 3, adapter 3, state 0, transition 0, audit 8 | field path `state.skill_vector`; expected vector[4], got vector[3] |
| Out-of-bounds job zone | raw +1, adapter +1 failed, no normalized state, no transition, audit +2 | recorded in edge evidence | recorded in edge evidence | out-of-range numeric field rejected |

## Manual Finding

All inspected invalid career inputs failed closed. The system persisted enough
source-of-truth evidence to diagnose each failure, but did not write normalized
states or transitions for invalid rows.

## Operator Note

When manually inspecting RocksDB CFs, use sequential `inspect-cf` commands per
DB path. A parallel read attempt against the same DB path correctly produced a
RocksDB lock error with typed `STORAGE` output and remediation text.

