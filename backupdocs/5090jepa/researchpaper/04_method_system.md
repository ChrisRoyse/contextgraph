# 3. The Dynamic/ME-JEPA Runtime

The runtime is a single Rust binary, `context-graph`, with the
DynamicJEPA subcommand surface enumerated in §3.6. The core abstraction
is a *typed transition* over a multi-instrument latent panel:

$$ (s_t, a_t, c_t, g_t) \;\longrightarrow\; \hat{s}_{t+1} $$

where $s_t$ is the panel-encoded state, $a_t$ is a typed action, $c_t$
is contextual scaffolding, and $g_t$ is an optional goal panel. The
target $s_{t+1}$ is the panel produced by *frozen, externally-defined
instruments* declared by the domain pack; the trainable predictor
$P_\psi$ produces the prediction $\hat{s}_{t+1}$.

## 3.1 Domain Packs

A domain pack is a strict TOML manifest registering a domain into the
DynamicJEPA RocksDB column families. The fields required are:
`[domain] id, version, schema_version`; `[time]` field; `[[state.fields]]`;
`[[action.fields]]`; `[[outcome.fields]]`; `[[entity.fields]]`. Field
kinds are: `categorical{variants}`, `i64{min,max}`, `f64{min,max}`,
`vector{dim, min, max}`, `string`, and `bool`. Validation is strict:
out-of-range values, missing required fields, undeclared categorical
variants, or vector dimension mismatch produce typed errors with field
paths. There is no fallback or coercion.

Three domain packs ship with the artifact:

`counter_world.v1` — A two-state counter with five candidate actions; 480
trainable parameters. Used as the smallest sanity domain.

`gridworld_5x5.v1` — A 5×5 navigation grid with four directional
actions; 16{,}960 trainable parameters. Used to verify compositional
next-state prediction.

`career_taxonomy.v1` — An O\*NET-30.2-grounded public-taxonomy career
benchmark (§4 specifies the schema); 15{,}344 trainable parameters at
A1 and on the headline panel.

The full TOML for `career_taxonomy.v1` is reproduced in App. A. The
runtime registers the pack via `dynamicjepa register-domain --file
<path>` and physically reads it back via `list-domains` and
`get-domain`.

## 3.2 Storage: 25 Typed Column Families + Audit Log

DynamicJEPA stores data in a dedicated RocksDB instance with 25
column families, all prefixed `dj_*`:

```text
dj_domain_packs           dj_normalized_states     dj_dataset_shards
dj_domain_pack_by_nv      dj_actions               dj_training_runs
dj_instrument_registry    dj_outcomes              dj_model_artifacts
dj_adapter_registry       dj_transitions           dj_predictions
dj_raw_events             dj_adapter_runs          dj_skill_policies
dj_instrument_readings    dj_latent_panels         dj_plan_traces
dj_bindings               dj_bindings_by_entity    dj_guard_decisions
dj_trajectories                                    dj_surprise_events
                                                   dj_verification_runs
                                                   dj_audit_log
```

Every mutating subcommand writes to one or more of these CFs and
*always* writes a corresponding `dj_audit_log` record. Reading a CF
back is a separate code path (`inspect-counts`, `inspect-cf`,
`get-*`) that opens the DB read-only and re-decodes the bytes; this
prevents the writer from also being the verifier. After the smoke-B
investigation, read-only opens were hardened so they cannot
accidentally create or repair a missing DB
([reports/20260501_local_smoke_b_tier_fsv.md](../smokedocs/reports/20260501_local_smoke_b_tier_fsv.md)).

For a 1{,}000-event career bundle the persisted CF row counts are:

```text
dj_raw_events            = 1004
dj_adapter_runs          = 1004
dj_normalized_states     = 1000
dj_instrument_readings   = 10000
dj_latent_panels         = 1000
dj_transitions           = 1000
dj_outcomes              = 1000
dj_dataset_shards        = 3       (train=699, val=200, test=100)
dj_training_runs         = 1
dj_model_artifacts       = 1
dj_predictions           = 6
dj_plan_traces           = 1
dj_guard_decisions       = 5
dj_verification_runs     = 1
dj_audit_log             = 3019
```

These exact counts are reproduced from `db_counts_after.json` of every
release-C seed bundle and were confirmed by independent `inspect-counts`
calls in §6 ([reports/20260501_release_c_repro_artifact_fsv.md](../smokedocs/reports/20260501_release_c_repro_artifact_fsv.md)).
The +4 discrepancy between `dj_raw_events`/`dj_adapter_runs` and
`dj_normalized_states` corresponds to the four edge-case events (empty
input, missing license, invalid action, wrong skill-vector dim,
out-of-bounds job zone — minus the empty-input case which writes only
audit rows).

## 3.3 Multi-Instrument Latent Panel

For each declared event, the runtime materializes a `LatentPanel` whose
slots are the outputs of declared *instruments*. An instrument is a
frozen function from a typed event to a structured feature output (a
vector, scalar, one-hot, or pairwise feature). Career-taxonomy uses
ten instruments at A1 (occupation cluster one-hot, job zone scalar,
skill vector, action kind one-hot, action effort scalar, outcome
cluster one-hot, next skill vector, plus three pairwise features); the
A2 panel adds a transition-distance instrument (11 slots, input dim 31,
target dim 31, latent dim 24); the A4 panel adds occupation-move-pair
one-hot pairwise features (12 slots, input dim 51, target dim 51,
latent dim 24, 16{,}688 parameters).

The panel for the ablation matrix:

| Ablation | Panel slots | Input dim | Target dim | Latent dim | Action dim | Params |
|---|---:|---:|---:|---:|---:|---:|
| A0 (taxonomy only) | 8 | 22 | 22 | 24 | 5 | 14{,}832 |
| A1 (+ skill vector) | 10 | 30 | 30 | 24 | 5 | 15{,}344 |
| A2 (+ transition dist) | 11 | 31 | 31 | 24 | 5 | 15{,}408 |
| A3 (= A2, action removed) | 11 | 31 | 31 | 24 | 5 | 15{,}408 |
| A4 (= A2 + pairwise) | 12 | 51 | 51 | 24 | 5 | 16{,}688 |
| A5 (= A2, no VICReg) | 11 | 31 | 31 | 24 | 5 | 15{,}408 |

Numbers read from
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/table_ablation_multiseed.csv`.

## 3.4 Predictor and Objective

The predictor $P_\psi$ is an MLP over the input-panel representation
producing the predicted target panel:

$$ \hat{s}_{t+1} = P_\psi(s_t, a_t) $$

Training minimizes the composite loss

$$ L_{\text{total}} = \alpha\,L_{\text{panel}} + \beta\,L_{\text{pairwise}} + \gamma\,L_{\text{action}} + \delta\,L_{\text{constellation}} + \varepsilon\,L_{\text{anti-pole}} + \zeta\,L_{\text{uncertainty}} + \eta\,L_{\text{invariant}} $$

(see [backupdocs/dynamicjepa/07_dynamic_predictor_and_objectives.md](../../backupdocs/dynamicjepa/07_dynamic_predictor_and_objectives.md))
implemented inside `context-graph-dynamicjepa::train_dataset` with
weights persisted as part of the model-artifact config so a reviewer
can read them back from `dj_model_artifacts`. VICReg variance and
covariance terms are weighted by separate scalars in the persisted
config; A5 is the ablation in which both are forced to zero.

For the present paper-small results the dominant terms are
$L_{\text{panel}}$ (per-slot MSE/cosine to the frozen target panel) and
$L_{\text{action}}$ (action-conditioning regularizer); $L_{\text{pairwise}}$
is active only in A4 (which adds the pairwise-feature panel slot
explicitly).

## 3.5 Planning, Guard, and Surprise

`dynamicjepa plan` enumerates the five candidate actions
(`skill_gain_adjacent`, `skill_gain_high_demand`, `role_move_near`,
`experience_gain`, `noop`), runs $P_\psi$ for each, evaluates a
constellation/anti-pole guard, and writes a `dj_plan_traces` row plus
five `dj_predictions` and five `dj_guard_decisions` rows. The selected
action is the one whose guarded utility maximizes a domain-pack-declared
scalar. `dynamicjepa record-surprise` compares an observed outcome to
the corresponding prediction; if the surprise exceeds the threshold a
`dj_surprise_events` row is persisted. All of these are
artifact-hash-verified through `verify_artifact_files`.

## 3.6 Subcommand Surface

The DynamicJEPA subcommand surface is exactly 29 commands:

```text
register-domain     get-trajectory          plan
list-domains        list-trajectories       record-surprise
get-domain          compile-dataset         get-prediction
inspect-counts      get-dataset-shard       get-plan-trace
inspect-cf          inspect-dataset         get-surprise
ingest-event        train                   verify-counter-world
run-adapter         get-training-run        verify-gridworld
materialize-panel   get-artifact            verify-career-taxonomy
get-panel           predict                 research-smoke
list-instrument-readings                    hp-sweep
bind  list-bindings  compile-trajectories   multiseed-aggregate
                                            capture-reference
                                            check-reference
                                            check-release-aggregate-reference
```

(Source:
[crates/context-graph-cli/src/commands/dynamicjepa.rs](../../../crates/context-graph-cli/src/commands/dynamicjepa.rs)
lines 75–149.) Of these, `research-smoke` is the headline paper command:
it composes domain registration, ingest, panel materialization,
trajectory/dataset compilation, training, prediction, planning,
ablation runs, edge-case verification, artifact hashing, paper-table
generation, and FSV evidence emission into one immutable bundle.

## 3.7 Hardware

All experiments run on a single host:

| Component | Observed |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090 |
| GPU memory | 32{,}607~MiB |
| Compute capability | 12.0 (`CUDA_COMPUTE_CAP=120`) |
| Driver | 596.21 |
| CUDA | 13.2, V13.2.78 |
| Rust | rustc 1.95.0 (59807616e 2026-04-14) |
| Cargo | cargo 1.95.0 (f2d3ce0bd 2026-03-21) |
| CPU | AMD Ryzen 9 9950X3D 16-Core (30 logical) |
| RAM | 110~GiB visible to WSL2 |
| Kernel | Linux 6.6.87.2-microsoft-standard-WSL2 |

(Source:
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42/hardware.json`
and `environment.json`.) `docs2/systemspecs.md` is hashed into every
research-smoke bundle (`acdc549a5913c59f46376414c08373a8363d646fd3c36d1fc6289a193c0e1205`),
so reviewers can verify the host description against the bundle.
