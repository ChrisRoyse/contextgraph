# Risks And Open Questions

## Critical Risks

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Dirty title or unusable consent | Kills the corpus | legal gate before processing |
| PII leakage into model data | Creates regulatory and reputational exposure | vault separation, FSV, output filters |
| Silent fallback to smaller panel | Invalidates ME-JEPA claim | fail-fast on missing embedder |
| Target leakage | Makes predictor look good while learning nothing | split manifests, mask specs, leakage audits |
| Predictor degeneracy | Model ignores context or memorizes shortcuts | collapse diagnostics, ablations, held-out eval |
| Existing fingerprint migration breakage | Could destabilize Context Graph | separate ME-JEPA panel record |
| Artifact provenance gaps | Makes outputs non-auditable | artifact registry and hash checks |
| Deletion after training | Creates impacted model artifacts | artifact impact tracking |
| Employment-AI regulation | Product may be high-risk | bias audit, notices, model cards, guard logs |
| Overbuilding before proof | Wastes time | tiny FSV-first predictor before scale |

## Open Architecture Questions

1. Should ME-JEPA storage be inside `context-graph-storage` or a new crate such as `context-graph-mejepa`?
2. Should ME-JEPA constellations reuse `CF_CONSTELLATIONS` or use `mejepa_constellations` for domain isolation?
3. Which Erec model family should be implemented first: skills, compensation, or trajectory?
4. Should E1-E14 vectors be copied into `mejepa_panel_records` or referenced by content fingerprint ID?
5. What is the minimum real-data subset needed for the first non-toy predictor?
6. What legal state should mark an artifact as unservable after consent revocation?
7. What is the first external benchmark that makes the research claim credible without exposing private data?

## Decision Recommendations

- Put ME-JEPA in a separate crate or module boundary, but reuse core/storage primitives.
- Keep a separate ME-JEPA storage namespace.
- Implement Erec2 compensation first for a fast, aggregate-friendly product head, and Erec3 trajectory second for the research claim.
- Copy compact normalized vectors or hashes where FSV needs immutable evidence; reference large token/sparse structures where duplication is too expensive.
- Do a tiny predictor on a controlled, persisted real-data subset before any large training run.

## Stop Conditions

Stop the build if:

- legal gate cannot be recorded,
- Stage A privacy triage cannot prove raw PII separation,
- N=19 panel cannot be physically materialized,
- training export cannot prove no leakage,
- model artifacts cannot be traced to source rows,
- guard decisions cannot be written and read before output.

