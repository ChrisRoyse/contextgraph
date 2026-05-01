# System Architecture

ME-JEPA is a staged system. Every stage has a trigger, a source of truth, and an independent verification read.

## End-To-End Flow

```text
Legal/title/privacy gate
  -> air-gapped corpus intake
  -> Stage A privacy triage
  -> source adapters
  -> typed recruiting/career events
  -> E1-E14 content projection
  -> Erec1-Erec5 recruiting projection
  -> pairwise interaction materialization
  -> trajectory sequence builder
  -> constellation and anti-pole compiler
  -> ME-JEPA predictor training
  -> read-out head training
  -> G_tau inference guard
  -> audited product/API output
```

## Stage 0: Legal And Operational Gate

No corpus processing runs until the legal gate is recorded as passed.

Required artifacts:

- title document or sale/license document,
- privacy policy/consent review,
- client confidentiality review,
- approved data-use matrix,
- operating environment approval,
- designated source-of-truth storage location.

If any artifact is missing, the system errors. There is no "developer mode" that bypasses this for real corpus data.

## Stage A: Privacy Triage

Purpose:

- detect PII,
- tokenize identifiers,
- separate raw PII into a vault,
- classify consent state,
- tag jurisdiction,
- mark records allowed, anonymized-only, quarantined, deleted, or excluded.

The main ME-JEPA pipeline only sees records with an explicit allowed processing state.

## Stage B: Source Adapters

Adapters convert source systems into typed event streams:

- SQL tables -> candidate/company/job/placement/activity events,
- email archives -> participant-attributed conversation events,
- documents -> OCR/provenance-backed resume, JD, contract, reference events,
- audio -> transcript events only when recording consent and biometric rules allow,
- activity logs -> status/timeline events.

Adapters do not embed, train, summarize, or infer. They normalize source data into typed events and preserve provenance.

## Stage C: Latent Panel Materialization

Each allowed event is projected into:

- E1-E14 existing Context Graph content spaces,
- Erec1 skills-taxonomy space,
- Erec2 compensation-context space,
- Erec3 career-trajectory space,
- Erec4 industry-vertical space,
- Erec5 conversation/soft-skill space,
- pairwise interaction features across the 19 spaces.

Panel materialization is a persisted, inspectable intermediate. Training must not compute private latent data only in RAM and discard proof.

## Stage D: Trajectory Builder

Events are ordered into trajectories:

- per candidate,
- per company,
- per hiring manager,
- per recruiter,
- per role/search,
- multi-actor placement episode.

Each trajectory stores:

- ordered event IDs,
- time deltas,
- actor roles,
- consent envelope,
- source record hashes,
- split assignment,
- leakage constraints.

## Stage E: ME-JEPA Predictor

The predictor consumes masked trajectory panel sequences and predicts target latent panels.

Key design:

- targets are frozen E1-E14 + Erec1-Erec5 panel outputs,
- predictor weights are trainable,
- target panel is not trainable,
- masking prevents seeing target events,
- pairwise-feature loss constrains second-order structure,
- constellation loss anchors desired regions,
- anti-pole loss penalizes forbidden regions.

## Stage F: Read-Out Heads

Read-out heads are small models on top of the trained predictor:

- compensation band regression,
- match-quality reward,
- bias/anti-pole discriminator,
- hiring-manager predictor,
- trajectory predictor,
- reference summarizer,
- conversation coach,
- vendor reranker.

Heads cannot bypass consent filters or G_tau.

## Stage G: Guarded Output

Every product output passes:

```text
consent_state allows use
AND model artifact is approved
AND source trajectory records exist
AND G_tau accepts success/quality constellation threshold
AND anti-pole proximity stays below threshold
AND audit write succeeds
```

If audit write fails, output fails. No unaudited output is allowed.

