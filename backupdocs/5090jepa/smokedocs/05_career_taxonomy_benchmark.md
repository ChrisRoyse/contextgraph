# 05 - Career Taxonomy Benchmark

This is the recommended 5090-feasible ME-JEPA demo domain when licensed
recruiting records are not available.

## Data Sources

Preferred public sources:

- O*NET 30.2 occupation, job zone, skill, and related-occupation database.
- ESCO occupations and skills classifications as a later crosswalk/replication
  source.
- Optional public career trajectory datasets only if license and provenance are
  acceptable and recorded.

The smoke harness must record source URLs, license text/hash, local file hash,
row counts, and allowed-use notes in `data_manifest.json`.

Current implemented source:

- `tmp/onet_probe/db_30_2_text.zip`, downloaded from the O*NET Resource Center;
- required ZIP members: `Occupation Data.txt`, `Job Zones.txt`, `Skills.txt`,
  and `Related Occupations.txt`;
- generated smoke rows: 120;
- selected source occupation rows with complete data: 218;
- selected related-occupation edges: 330;
- source occupation rows in O*NET 30.2: 1016.

## Domain ID

```text
career_taxonomy.v1
```

This is a public-taxonomy career dynamics benchmark. It is not a real recruiting
production model.

## Event Shape

One event represents a subject or synthetic public-taxonomy trajectory step:

```json
{
  "trajectory_id": "public_taxonomy_path_0001",
  "step": 0,
  "source": {
    "dataset": "onet",
    "license": "O*NET 30.2 Database CC BY 4.0",
    "source_hash": "sha256:<source_zip_sha256>"
  },
  "state": {
    "occupation_code": "15-1252.00",
    "job_zone": 4,
    "skill_vector": [0.0, 0.1, 0.2],
    "experience_years": 3.0,
    "wage_band": "mid"
  },
  "action": {
    "kind": "skill_gain",
    "target_skill_cluster": "software_development",
    "effort_months": 6.0
  },
  "outcome": {
    "next_occupation_code": "15-1253.00",
    "next_job_zone": 4,
    "next_skill_vector": [0.1, 0.2, 0.3],
    "mobility_label": "near_transition"
  }
}
```

The current DynamicJEPA JSON adapter supports scalar, categorical, string, bool,
and fixed vector fields. The first smoke domain should keep fields within those
types.

## State Fields

Recommended minimal state:

| Field | Type | Notes |
| --- | --- | --- |
| `occupation_cluster` | categorical | coarse group derived from occupation code |
| `job_zone` | i64 | bounded O*NET-style zone |
| `skill_vector` | vector[N] | fixed public taxonomy skill features |
| `experience_years` | f64 | bounded |
| `wage_band` | categorical | low/mid/high/unknown |

Keep `skill_vector` small for smoke: 8, 16, or 32 dimensions derived from public
skill clusters. Do not start with hundreds of skills.

## Action Fields

Recommended minimal action:

| Field | Type | Variants/Bounds |
| --- | --- | --- |
| `kind` | categorical | `skill_gain`, `role_move`, `experience_gain`, `credential_gain`, `noop` |
| `target_skill_cluster` | categorical | finite public skill clusters |
| `effort_months` | f64 | 0..60 |

Planning can enumerate a small candidate set:

- gain adjacent skill;
- gain high-demand skill;
- move to nearby occupation;
- gain experience;
- noop.

## Outcome Fields

Recommended minimal outcome:

| Field | Type | Notes |
| --- | --- | --- |
| `next_occupation_cluster` | categorical | target cluster |
| `next_job_zone` | i64 | 1..5 or configured range |
| `next_skill_vector` | vector[N] | expected next skill features |
| `mobility_label` | categorical | `same`, `near_transition`, `far_transition`, `unknown` |

## Entity Fields

Use:

```text
trajectory_id: string
```

For real data later, this must be pseudonymous and must not expose raw PII.

## Instruments

Smoke panel:

- occupation cluster one-hot;
- job zone scalar;
- current skill vector;
- action kind one-hot;
- action effort scalar;
- outcome cluster one-hot;
- next skill vector.

Paper-small panel:

- add skill-gap vector;
- add occupation relatedness scalar;
- add wage-band one-hot;
- add transition-distance scalar.

## Dataset Construction

Candidate trajectory sources:

1. Public taxonomy graph walk:
   - nodes are occupation clusters;
   - edges are related occupations or skill-overlap transitions;
   - action is a skill/experience/role movement;
   - outcome is next occupation/skill state.
2. Public dataset trajectories if licensed:
   - map titles to O*NET/ESCO;
   - persist provenance;
   - run privacy gate if human-level records exist.

The public taxonomy graph walk is controlled but not a mock if it is generated
from source-taxonomy rows, persisted through the real DynamicJEPA pipeline, and
the generator/source hashes are recorded.

The current generator starts at O*NET-SOC `15-1252.00` Software Developers and
walks the core computer-occupation related graph. Each generated row records a
stable source ZIP hash in the event and a per-edge hash in
`career_taxonomy_onet_walk.csv`, so the same transition can be traced back to
the local O*NET ZIP and generated walk evidence.

## Edge Cases

Required:

- missing source license/provenance: fail before ingest;
- invalid action kind: raw + failed adapter, no normalized rows;
- skill vector wrong dimension: adapter failure, no normalized rows;
- out-of-bounds `job_zone`: adapter failure;
- empty JSONL: audit failure only.

## Paper-Safe Language

Use "career taxonomy benchmark" for this domain. Reserve "ME-JEPA on real career
trajectories" for a run with licensed subject-level data and privacy gate FSV.
