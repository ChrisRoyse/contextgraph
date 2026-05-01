# Source Material Synthesis

This document maps the Steve plan and the current Context Graph repository into the ME-JEPA build.

## From The Steve Docs

The supplied strategy docs describe a rare corpus:

- 20-30 years of recruiting CRM data.
- Candidates, clients, jobs, placements, compensation, emails, calls, notes, resumes, and outcomes.
- Conversation-grade and outcome-labeled, not merely profile-grade.
- Heavy PII and client-confidentiality risk.
- Strong fit for Context Graph's DDA, constellation, guard, and audit primitives.

The final document, `14_the_single_most_powerful_thing.md`, reframes the opportunity:

```text
Do not build 17 disconnected recruiting products.
Build ME-JEPA once.
Expose the 17 products as heads/read-outs.
```

This plan follows that instruction.

## From The Current Repo

Current Context Graph already has:

- Rust workspace with core, storage, embeddings, MCP, CLI, agents, benchmark crates.
- `SemanticFingerprint` with E1-E14 content-side embedders.
- RocksDB teleological storage with 41 teleological column families.
- Source metadata, audit log, training records, learning events, typed edges, constellations.
- Existing docs for Full State Verification in `docs/bigunlock/07_full_state_verification.md`.
- Proven Shakespeare FSV evidence showing physical RocksDB counts for fingerprints, content, source metadata, embedding registry, training records, constellations, and audit logs.

The important architectural decision:

```text
Do not append recruiting Erec1-Erec5 directly to the existing SemanticFingerprint wire format.
Create a separate ME-JEPA trajectory-panel layer that can reference existing content records.
```

Reason:

- E1-E14 are stable content-side fingerprints.
- Recruiting Erec1-Erec5 are domain-specific.
- ME-JEPA sequences need subject, event, consent, temporal, and outcome metadata.
- Storage verification is cleaner when career trajectories have their own source-of-truth rows.

## Current Concepts To Reuse

| Existing primitive | ME-JEPA use |
| --- | --- |
| `SemanticFingerprint` E1-E14 | Base content projection for event text/doc chunks |
| `CF_CONTENT` | Source event/chunk content when allowed |
| `CF_SOURCE_METADATA` | Provenance and raw source trace |
| `CF_EMBEDDING_REGISTRY` | Embedder versions and model hashes |
| `CF_TRAINING_RECORDS` | Existing content training export, referenced or extended |
| `CF_CONSTELLATIONS` | Existing constellation concept, but ME-JEPA may need separate domain CFs |
| `CF_AUDIT_LOG` / `CF_AUDIT_BY_TARGET` | Audit pattern for every mutation and guard decision |
| `CF_TYPED_EDGE_RECORDS` | Candidate-company-role-outcome graph edges |
| `CF_LEARNING_EVENTS` | Pattern for outcome event storage |

## New Concepts Needed

- `MejepaSubject`: pseudonymous person/company/recruiter/hiring-manager identity.
- `MejepaEvent`: typed career or recruiting event at a timestamp.
- `MejepaPanelRecord`: materialized E1-E14 + Erec1-Erec5 latent panel for one event.
- `MejepaPairwiseRecord`: materialized pairwise interaction features.
- `MejepaTrajectory`: ordered event sequence for a subject or multi-actor interaction.
- `MejepaSplit`: train/validation/test split that prevents subject and temporal leakage.
- `MejepaTrainingRun`: predictor training run with source dataset IDs and artifact hashes.
- `MejepaGuardDecision`: G_tau decision with constellation versions and cosine traces.

## Immediate Implication

The first implementation milestone is not model training. It is a physically verifiable data plane:

```text
source records -> privacy gate -> typed events -> latent panels -> pairwise features
-> trajectories -> split manifests -> training dataset export
```

Only after that passes FSV should the predictor be trained.

