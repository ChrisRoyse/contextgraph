# 06 — Guard Upgrade: Combined Utility + $G_\tau$ Predicate

This document specifies the upgrade of the existing
`dj_guard_decisions` from a utility-threshold heuristic to a combined
runtime guard that AND-s the existing utility verdict with the TCT
constellation predicate $G_\tau$ from PAPER.md §5.4.

## 1. The current `dj_guard_decisions` row

Today, `dj_guard_decisions` carries (paraphrased from
`crates/context-graph-core/src/dynamicjepa/...`):

```rust
pub struct GuardDecision {
    pub guard_id: GuardDecisionId,
    pub plan_trace_id: PlanTraceId,
    pub candidate_action_id: ActionId,
    pub utility_score: f32,
    pub verdict: GuardVerdict,           // Accept / Reject
    pub created_at_unix_ms: u64,
    pub audit_id: AuditId,
}

pub enum GuardVerdict {
    Accept,
    Reject { reason: String },
}
```

The reason field is a free-form string per-domain heuristic. For
career\_taxonomy the verdict is essentially "utility above declared
floor"; for counter\_world it is a "stay-in-bounds" check; for
gridworld it is "action-effect within board".

## 2. The upgraded row

Two new fields land:

```rust
pub struct GuardDecision {
    // existing fields ...
    pub utility_score: f32,
    pub verdict: GuardVerdict,                 // overall AND-verdict
    pub utility_verdict: GuardVerdict,         // existing utility check
    pub gtau_verdict: GuardVerdict,            // new: TCT predicate

    // new fields:
    pub constellation_uuid: Option<ConstellationId>,
    pub cosine_to_centroid_per_modality: Option<BTreeMap<InstrumentId, f32>>,
    pub tau_per_modality: Option<BTreeMap<InstrumentId, f32>>,
    pub gtau_failed_modalities: Option<Vec<InstrumentId>>,
}
```

`verdict` is the AND of `utility_verdict` and `gtau_verdict`. When a
domain pack declares no constellation (counter\_world v1, etc.), the
`Option`s are `None` and `verdict := utility_verdict` (preserving the
v1 baseline behaviour).

## 3. The combined acceptance rule

Per `02_architecture.md` §5:

```text
accept(candidate a in plan p) :=
  utility(state_p, a) >= u_min(domain_p)
    AND
  forall m in M_d:
    cos(phi_m(predicted_panel(a)), c_m^{(d, subject_p)}) >= tau_m
```

where:
- `M_d` is the constellation's modality set
- `subject_p` is the constellation subject for the plan (default
  global; subject-specific iff TOML declares it)
- `tau_m` is read from `dj_threshold_calibrations`

## 4. Computation flow

The plan execution code path (existing `plan_next_action` in
`crates/context-graph-dynamicjepa/src/...`) extends to:

```text
plan_next_action(s_t, domain_pack):
  for each candidate action a in K_d:
    1. predicted_panel := P_psi.predict(s_t, a)             [unchanged]
    2. utility := domain_pack.utility(s_t, a)               [unchanged]
    3. utility_verdict := utility >= u_min                  [unchanged]
    4. constellation := select_constellation(domain_pack, s_t)  [new]
       // returns the subject-specific C_P if state matches a declared
       // subject value; else returns the global C_P
    5. for each modality m in constellation.modalities:     [new]
         cosine_m := cos(phi_m(predicted_panel), c_m)
         tau_m := dj_threshold_calibrations[domain, subject, m].tau
         pass_m := cosine_m >= tau_m
    6. gtau_verdict := all(pass_m)                          [new]
    7. verdict := utility_verdict AND gtau_verdict          [new logic]
    8. persist guard decision row with all fields populated
```

### Cost analysis

For career\_taxonomy at $K_d = 5$, $\lvert M_d \rvert = 10$:

- Step 5 cost: 5 candidates × 10 modalities × $d_m$-dim cosine
  $\approx$ 50 cosine evaluations of vectors with $d_m \in [4, 64]$.
  Total $\sim 2{,}500$ multiply-adds, well under 1 ms.
- Step 4 cost: one TOML field lookup + one `dj_constellations` row
  read; cached after first plan. Negligible.

The cost stays well within the existing plan-step latency budget.

## 5. Constellation selection logic

`select_constellation(domain_pack, state)` is deterministic:

```text
1. If domain_pack declares no [[constellation.subjects]]:
     return constellation(domain_pack, "global")
2. For each declared subject (in TOML order):
     if state.<subject.field> == subject.value:
       return constellation(domain_pack, subject.id)
3. Fallback (no subject matched):
     return constellation(domain_pack, "global")
```

For career\_taxonomy v2:
- A state with `occupation_cluster = "software"` selects
  `cluster.software`.
- A state with `occupation_cluster = "data"` selects `cluster.data`.
- Etc.

The fallback to global is well-defined; no exception path.

## 6. The `verdict` field's three reason kinds

The combined verdict's reason is enriched:

```rust
pub enum GuardVerdict {
    Accept,
    Reject { kind: RejectKind, detail: String },
}

pub enum RejectKind {
    UtilityBelowFloor { utility: f32, floor: f32 },
    GtauFailed { failed_modalities: Vec<InstrumentId> },
    BothFailed,
}
```

When both fail, `BothFailed` carries both reason details so a
post-hoc auditor sees the full picture.

## 7. FSV-11: $G_\tau$ verifiability

A new FSV harness verifies, for every persisted `dj_guard_decisions`
row in the bundle:

1. The `predicted_panel` referenced in the matching
   `dj_predictions` row is recomputable from the `dj_model_artifacts`
   weights + the `state_t` panel. (Existing FSV-7 already covers this.)
2. The `cosine_to_centroid_per_modality` recomputes from
   `predicted_panel` + the `dj_constellations` row referenced via
   `constellation_uuid`. Tolerance $\le 10^{-4}$.
3. The `tau_per_modality` matches the current
   `dj_threshold_calibrations` row for the same (domain, subject,
   modality).
4. The `gtau_verdict` recomputes correctly from the persisted cosines
   and taus.
5. The `verdict` recomputes correctly from `utility_verdict AND
   gtau_verdict`.
6. The recorded `instrument_artifact_hash` (via the constellation row)
   matches the current `dj_instrument_registry`.

Failure on any check writes a typed FSV row with `fsv_check =
"gtau_verifiability"`.

## 8. Plan-trace contract

The `dj_plan_traces` row's existing `selected_action_id` field
continues to refer to the maximum-utility candidate by default. With
$G_\tau$ active, the field semantic is extended:

- If at least one candidate has `verdict = Accept`,
  `selected_action_id` is the highest-utility *accepted* candidate
  (i.e., utility argmax over the Accept set).
- If no candidate is accepted, `selected_action_id` is the highest-
  utility candidate regardless of $G_\tau$ — the plan still produces
  *some* selection, but a new field
  `plan_trace.no_accepted_candidate: bool` is set true.

In the second case, an `dj_audit_log` row of kind
`plan_no_accepted_candidate` is written so the bundle's MC-ratio
aggregator can report this as a *signal* (the absence of
acceptance is itself a signal about the predictor / centroid /
threshold relationship).

## 9. Edge cases (typed-error catalog)

The combined guard adds:

| Edge | Trigger | Typed error code |
| --- | --- | --- |
| Constellation missing | Domain declares constellation but `dj_constellations` lookup empty | `GUARD_CONSTELLATION_MISSING` |
| Threshold missing | `dj_threshold_calibrations` row absent for active modality | `GUARD_THRESHOLD_MISSING` |
| Hash drift mid-bundle | `instrument_artifact_hash` on guard row $\ne$ registry | `GUARD_INSTRUMENT_HASH_DRIFT` |
| All candidates rejected | `no_accepted_candidate = true` | logged, NOT a typed error (operational signal) |
| $\tau$ retuned without supersedes | `dj_threshold_calibrations` shows two non-superseded rows for same (domain, subject, modality) | `GUARD_THRESHOLD_DUPLICATED` |

Three new typed errors plus one operational signal.

## 10. Backwards-incompatibility note

Per CLAUDE.md no-backwards-compat rule, v2.0 RocksDB stores written by
v1.0 binaries are not read by v2.0 binaries (and vice versa). The
schema-version bump on the bundle JSON manifest enforces this:
`run_manifest.json` gains a top-level `bundle_schema_version: 2`;
v2.0 readers refuse to load a bundle whose value is `1` and v1.0
readers refuse to load `2`. Test plan in `14_test_plan.md` §4 covers
these edges.

## 11. What v2.0 does *not* add to the guard

Deliberately deferred:

- **Phase-2 conditioning** ($\mathcal{L}_{\mathrm{TCT}}$ from PAPER.md
  §5.3): the predictor is **not** trained against the constellation
  in v2.0. v2.1 axis.
- **Best-of-$K$ resampling on guard rejection**: the predictor is
  *not* re-invoked when $G_\tau$ rejects in v2.0. v2.1 axis.
- **Per-modality $\tau_m$ adaptation** to per-bundle calibration
  drift: $\tau$s are pinned at the constellation-build time and do
  not float.
- **Cross-encoder triangulation**: PAPER.md §6.3.5 EXP-1; deferred
  because no second-encoder family is in scope for the in-scope
  domain packs.

These deferrals keep v2.0 a *pure runtime guard* upgrade, isolating
the cost and benefit of the constellation predicate from any
training-objective change. v2.1 layers Phase-2 conditioning and
best-of-$K$ on top.

## 12. Worked example (career\_taxonomy A1, hypothetical state)

State: `{occupation_cluster: "software", job_zone: 4, skill_vector:
[0.6, 0.3, 0.1, 0.0], experience_years: 3.0, wage_band: "mid"}`.

Plan candidate `a = skill_gain_high_demand` produces predicted
next-state with skill\_vector $\to$ `[0.65, 0.32, 0.13, 0.02]`,
occupation\_cluster $\to$ `software`, etc.

Guard execution:

```text
- Selected constellation: cluster.software (10 modalities).
- For each m in M:
    cosine = cos(phi_m(predicted_panel), c_m^{cluster.software})
    tau    = dj_threshold_calibrations[career_taxonomy, cluster.software, m].tau
    pass   = cosine >= tau
  Example modality results:
    occupation_cluster_onehot:    cosine=1.000, tau=0.95, pass=true
    job_zone_scalar:              cosine=0.987, tau=0.90, pass=true
    skill_vector:                 cosine=0.962, tau=0.92, pass=true
    action_kind_onehot:           cosine=1.000, tau=0.85, pass=true
    action_effort_scalar:         cosine=0.94,  tau=0.80, pass=true
    outcome_cluster_onehot:       cosine=1.000, tau=0.90, pass=true
    next_skill_vector:            cosine=0.95,  tau=0.92, pass=true
    pair_oc_x_skill:              cosine=0.91,  tau=0.85, pass=true
    pair_oc_x_zone:               cosine=0.94,  tau=0.85, pass=true
    pair_skill_x_zone:            cosine=0.88,  tau=0.80, pass=true
- gtau_verdict = Accept
- utility = 0.78, u_min = 0.5; utility_verdict = Accept
- verdict = Accept
- persist row with all 10 (cosine, tau, pass) triples
```

Compare to candidate `a = noop`, predicted to leave the state
unchanged. Cosines all near 1.000, but utility may fall below floor
(`utility_verdict = Reject { UtilityBelowFloor { 0.20, 0.50 } }`),
so the overall `verdict = Reject { UtilityBelowFloor }` even though
$G_\tau$ would have accepted. The combined guard correctly reflects
both axes.

This worked example becomes a regression test in `14_test_plan.md`.
