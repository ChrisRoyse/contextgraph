# 4. The `career_taxonomy.v1` Public-Taxonomy Benchmark

`career_taxonomy.v1` is a public benchmark for action-conditioned
latent transition prediction over occupational data. The benchmark is
*deterministically generated from the O\*NET 30.2 text database* under
CC BY 4.0 with the verbatim USDOL/ETA non-endorsement clause. It is
*not* a real recruiting trajectory dataset; treating it as one is
expressly prohibited by the artifact license (§11).

## 4.1 Source

The single source ZIP is the official O\*NET 30.2 text database from
the O\*NET Resource Center, which we fetched once and verified by
SHA-256:

| Field | Value |
| --- | --- |
| Source | https://www.onetcenter.org/dl_files/database/db_30_2_text.zip |
| Local path | `tmp/onet_probe/db_30_2_text.zip` |
| SHA-256 | `b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741` |
| License | Creative Commons Attribution 4.0 International (CC BY 4.0) |
| License URL | https://www.onetcenter.org/license_db.html |
| Download page | https://www.onetcenter.org/database.html |

The runtime preflights this archive on every research-smoke run and
fails closed if any of the following invariants fail:

- archive entry count = 42, file count = 41;
- root prefix = `db_30_2_text/`;
- release marker text "O\*NET 30.2 Database" is present;
- license marker "Creative Commons Attribution 4.0 International License" is present;
- per-file row counts: `Occupation Data.txt` = 1{,}016, `Job Zones.txt` = 923, `Skills.txt` = 62{,}580, `Related Occupations.txt` = 18{,}460;
- per-file required header columns are present;
- no archive member path may escape the archive root (path-traversal guard).

The persisted preflight evidence is at `onet_source_preflight.json`
inside every bundle; row counts above reproduce verbatim from
`data_manifest.json` of `20260501T_release_c_gate_v2_paper_small_seed42`.

## 4.2 Walk Generator

`research-smoke --onet-text-zip ...` reads the four required files,
selects the *core computer-occupation related-occupation graph*
starting from O\*NET-SOC `15-1252.00` Software Developers, and walks
until the requested number of career events is reached. Of the 1{,}016
source occupations, **218** are selected as in-graph; **330**
related-occupation edges are selected. For 1{,}000-event paper-small
runs the generator emits 1{,}000 events (seeds {42, 43, 44, 45, 46}
each producing a different deterministic walk under the same selected
edge set). Each generated event's `source_hash` is bound to the
source-ZIP SHA-256, and each transition is recorded with a per-edge
hash in `career_taxonomy_onet_walk.csv`. The walk CSV has 1{,}001 lines
(header + 1{,}000 transitions). The first generated rows are anchored
to Software Developers `15-1252.00` and Database Architects
`15-1243.00`.

The full attribution boundary recorded in every generated bundle is:

> "O\*NET 30.2 Database, Creative Commons Attribution 4.0 International
> License (CC BY 4.0). Generated fixture modifies O\*NET database
> information for a public taxonomy benchmark; USDOL/ETA has not
> approved, endorsed, or tested these modifications."

(Source:
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42/data_manifest.json`
field `sources[0].license`.)

## 4.3 Schema

State (six required fields):

| Field | Type | Constraint |
| --- | --- | --- |
| `occupation_cluster` | categorical | `{software, data, systems, management}` |
| `job_zone` | i64 | `1 ≤ x ≤ 5` |
| `skill_vector` | vector[4] | `0.0 ≤ x_i ≤ 1.0` |
| `experience_years` | f64 | `0.0 ≤ x ≤ 40.0` |
| `wage_band` | categorical | `{low, mid, high}` |
| `step_index` (time) | i64 | `0 ≤ x ≤ 1{,}000{,}000` |

Action (single required field):

| Field | Type | Constraint |
| --- | --- | --- |
| `move` | categorical | `{skill_gain_adjacent, skill_gain_high_demand, role_move_near, experience_gain, noop}` |

Outcome (four required fields):

| Field | Type | Constraint |
| --- | --- | --- |
| `next_occupation_cluster` | categorical | `{software, data, systems, management}` |
| `next_job_zone` | i64 | `1 ≤ x ≤ 5` |
| `next_skill_vector` | vector[4] | `0.0 ≤ x_i ≤ 1.0` |
| `mobility_label` | categorical | `{same, near_transition, far_transition}` |

Entity (`trajectory_id`, `source_dataset`, plus declared metadata).

The full TOML domain pack is in App. A. The schema choices are
deliberately small (4-D skill vector, 4-cluster occupation, 5-action
discrete space) so the entire ablation matrix fits in a 1{,}000-event
paper-small bundle on the RTX 5090 in well under one hour while still
producing meaningful action contrast.

## 4.4 Splits, Provenance, and Edge Cases

The 1{,}000 generated transitions are deterministically split into
train = 699, val = 200, test = 100 (read from
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42/db_counts_after.json`
plus the `dj_dataset_shards` decoded readback of paper-small 1K
[reports/20260430_paper_small_1k_fsv.md](../smokedocs/reports/20260430_paper_small_1k_fsv.md)).
Splits are persisted in `dj_dataset_shards` with leakage checks; no
trajectory or transition appears in more than one shard.

Eleven edge-case rows are generated per bundle and verified against
expected before/after deltas:

| Edge | Raw Δ | Adapter Δ | State Δ | Transition Δ | Audit Δ |
| --- | ---: | ---: | ---: | ---: | ---: |
| empty input | 0 | 0 | 0 | 0 | 1 |
| missing source license | 1 | 1 | 0 | 0 | 2 |
| invalid action | 1 | 1 | 0 | 0 | 2 |
| wrong skill-vector dimension | 1 | 1 | 0 | 0 | 2 |
| out-of-bounds job zone | 1 | 1 | 0 | 0 | 2 |

(plus six additional CLI-level edge cases such as bad ZIP path,
missing `--onet-text-zip` for paper purpose, custom event count
without ZIP, unsafe ZIP member path, and incomplete ZIP file count;
see §6.2 and
[reports/20260430_onet_zip_preflight_fsv.md](../smokedocs/reports/20260430_onet_zip_preflight_fsv.md)).

## 4.5 Why a Public-Taxonomy Benchmark and Not a Real-Person Dataset?

We deliberately reject loading a real-person career trajectory dataset
into the present artifact, even where a public dataset with permissive
license exists (e.g. JobHop [Johary et al., 2025]; KARRIEREWEGE [Senger
et al., 2024]). The reasons:

1. **Audit-first scope.** The contribution of this paper is the
   *runtime* and the *FSV protocol* on a substrate that any reviewer
   can rebuild from the public ZIP and the published source-ZIP
   SHA-256. Real-person data—even when public—introduces consent,
   re-identification, and jurisdictional questions that the present
   artifact does not yet have privacy gates for.

2. **Future Phase 12 separation.** The design plan explicitly defers
   real subject-level data to a Phase 12 with privacy/consent FSV
   ([smokedocs/25_privacy_gate_real_data_phase12.md](../smokedocs/25_privacy_gate_real_data_phase12.md)).
   Mixing the two in v1.0 would muddle the evidence chain.

3. **Paper-safety language.** Calling this a "career-taxonomy
   benchmark" rather than "ME-JEPA on real career trajectories" is
   non-negotiable per the project's source-of-truth document
   ([smokedocs/01_research_thesis_and_claims.md](../smokedocs/01_research_thesis_and_claims.md)).

The benchmark is therefore reusable as a *smallest reproducible career
substrate* for future work that needs to share an FSV-friendly base
before attempting real-data extensions.
