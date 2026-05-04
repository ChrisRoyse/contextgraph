# 28 - Constellation Guard For ME-JEPA Outputs

## Goal

Wrap every ME-JEPA inference output (predict, plan, surprise, decoder
head) with a **per-output Constellation Guard** that rejects
out-of-manifold outputs at runtime, exactly as PAPER.md §5 specifies for
the TCT method. After this work, every prediction the system emits has
a deterministic geometric acceptance test against a frozen multi-modal
centroid set.

This is the **per-output verifiability** property that scalar-reward
alignment (InstructGPT, DPO, Constitutional AI) does not provide in the
same technical sense.

## Why Per-Output Verifiability At Inference

Smokedoc 06 already specifies VICReg, paired tests, and ablation
protocols. None of those guard a *single output*. They are aggregate
training-time properties.

Production deployments need **per-output rejection**:

- A career-domain prediction that drifts to "hallucinated occupation"
  must be rejected, not merely logged.
- A plan trace whose selected action leaves the policy manifold must be
  flagged, not silently executed.
- A clone (voice / video / text style) that drifts off-identity must
  fail, not stream.
- A multi-step rollout (doc 24) whose intermediate $\hat z_{t+H}$ falls
  off-manifold must abort, not compound.

A frozen geometric test does this in $O(M \cdot d_{\max})$ per output,
where $M$ is the number of guarded modalities. No learned gate. No drift.

## Architecture

### Centroid Set Construction

Per domain pack, declare a set of **frozen centroids** computed from a
trusted reference subset of the training data (or a curated reference
set). For career taxonomy:

```toml
[constellation.career_taxonomy_v1]
reference_set_path = "configs/dynamicjepa/centroids/career_taxonomy_reference.jsonl"
reference_set_sha256 = "..."
modalities = [
  { name = "latent",      embedder = "jepa_target_encoder", weight = 1.0,  threshold = 0.65 },
  { name = "skill",       embedder = "skill_panel",         weight = 0.5,  threshold = 0.55 },
  { name = "dda_e1",      embedder = "context_graph_e1",    weight = 0.3,  threshold = 0.45 },
  { name = "dda_e10",     embedder = "context_graph_e10",   weight = 0.3,  threshold = 0.45 },
]
acceptance_rule = "all"   # all | weighted_majority
```

Each centroid is the L2-normalized mean of the reference embeddings for
that modality. Computed once at domain-pack registration and **frozen**
in `dj_constellation_centroids` CF, hash-pinned.

### Guard Predicate

For an output $\hat y$:

$$
G_\tau(\hat y; C_P) = \mathbb{1}\!\left[\bigwedge_m \cos(\phi_m(\hat y), c_m) \geq \tau_m\right]
$$

If `acceptance_rule = "all"`, every modality must pass. If
`weighted_majority`, the weighted sum of pass indicators must exceed a
configured threshold (used when modalities disagree harmlessly).

The guard is **deterministic, parameter-free, and frozen**. It admits
no learned tuning at inference.

### Integration Points

The guard wraps four existing ME-JEPA endpoints:

| Endpoint | Guard Behavior |
| --- | --- |
| `predict` | Encode $\hat z_{t+1}$ + decoded heads through each modality embedder; reject if $G_\tau$ fails; persist the guard verdict to `dj_predictions` |
| `plan` | Per candidate action, compute $G_\tau$ on the predicted $\hat z_{t+1}$; rejected candidates excluded from utility ranking; recorded in `dj_guard_decisions` |
| `record-surprise` | Compute $G_\tau$ on the *observed* outcome panel; if observed is out-of-manifold while predicted was in-manifold, that's a structurally meaningful surprise (the world drifted, not the model) |
| Decoder heads | Per emitted top-k label set, compute $G_\tau$ over the reconstructed skill vector; reject the head's emission if it fails |

### New CFs

```text
dj_constellation_centroids        # one row per (domain, modality), frozen
dj_constellation_guard_decisions  # one row per output, with per-modality cosines and final verdict
```

`dj_constellation_guard_decisions` schema:

```rust
pub struct GuardDecision {
    pub output_id: Uuid,                          // links to prediction or plan trace
    pub domain_id: String,
    pub centroid_set_hash: [u8; 32],
    pub modality_cosines: Vec<(String, f32)>,     // ("latent", 0.71), ...
    pub modality_thresholds: Vec<(String, f32)>,
    pub modality_verdicts: Vec<(String, bool)>,
    pub final_verdict: bool,
    pub computed_at_unix_ms: u64,
}
```

### CLI

```bash
target/debug/context-graph dynamicjepa build-centroids \
  --db "$DB" \
  --domain career_taxonomy \
  --constellation career_taxonomy_v1 \
  --json
```

Computes and persists the frozen centroid set; fails closed if the
reference set is missing, malformed, or too small (< 30 examples per
modality).

```bash
target/debug/context-graph dynamicjepa predict \
  --db "$DB" \
  --domain career_taxonomy \
  --jepa-artifact-id "$ART" \
  --constellation career_taxonomy_v1 \
  --observed-state "$JSON" \
  --action "$ACTION" \
  --json
```

Existing predict; guard fields appear in the response and a row in
`dj_constellation_guard_decisions`.

```bash
target/debug/context-graph dynamicjepa guard-evaluate \
  --db "$DB" \
  --constellation career_taxonomy_v1 \
  --output-id "$OUTPUT_ID" \
  --json
```

Re-evaluates an already-emitted output against (potentially newer)
centroid versions; useful for retroactive audit.

## FSV Extension

Smokedoc 08 §Stage Sources Of Truth gains:

| Stage | Source Of Truth |
| --- | --- |
| Centroid set construction | `dj_constellation_centroids` + `reference_set_sha256` |
| Per-output guard | `dj_constellation_guard_decisions` per emitted output |

Smokedoc 04 gains §Gate 14 (Constellation Guard):

> **Gate 14 — Constellation Guard**
>
> Pass criteria:
> - centroid set built and persisted with reference-set hash;
> - every prediction in `dj_predictions` has a matching guard decision row;
> - centroid set hash recomputable from the reference subset;
> - rejected outputs counted in the bundle and **not** included in
>   downstream metrics.

## Tuning Thresholds

Per-modality $\tau_m$ are calibrated by:

1. Compute cosines on the validation split (in-manifold examples).
2. Use the 5th percentile as the default threshold, exposing a
   calibrated false-rejection rate of 5%.
3. Operator can override per-deployment; override recorded in the
   bundle.

The paper appendix reports the calibration curve. Thresholds are not a
trade secret; they are part of the published artifact.

## Performance

Guard cost per output:

- Encode $\hat y$ with each modality embedder ($M$ forward passes).
- $M$ cosine computations against pre-stored centroids.
- One CF write.

For $M = 4$ at career-taxonomy scale, ~5 ms per prediction. Negligible
relative to the JEPA forward pass itself.

## Coordination With ClipCannon Constellation

ClipCannon's constellation system (per `docs2/alldata.md` §6) already
implements the same idea for video identity. **Do not fork; share**:

- Move the cosine-gate logic into a shared crate
  `crates/context-graph-constellation` consumed by both ME-JEPA
  (Rust) and ClipCannon (Python via PyO3 binding or HTTP MCP).
- Reuse the same centroid-set schema (`reference_set_sha256` +
  per-modality thresholds + acceptance rule).
- Output a uniform `GuardDecision` struct readable by either system.

This means a video clone trained in ClipCannon and a career predictor
trained in ME-JEPA share the same guard contract and the same FSV
fields. Doc 39 (unified MCP surface) wires this together.

## Edge Cases (Fail-Closed)

- Reference set has fewer than 30 examples per modality:
  `INSUFFICIENT_REFERENCE_SET`.
- Centroid set hash drifts mid-deployment (e.g. operator edits TOML):
  `CENTROID_SET_DRIFT`; existing predictions stand, new predictions
  blocked until re-registered.
- Modality embedder unavailable at inference (e.g. weights deleted):
  `MODALITY_EMBEDDER_MISSING`; no silent fallback to fewer modalities.
- Cosine NaN (zero-norm output): treat as a hard reject (not a pass);
  flag `numerical_warning`.
- Output rejected but caller did not pass `--allow-rejected`: response
  is the typed rejection; no synthesized "best effort" output.

## Acceptance

This work is done when:

- the constellation crate exists and is consumed by both ME-JEPA and
  ClipCannon;
- `career_taxonomy_v1` constellation set exists and is verified per
  Gate 14;
- guard decisions appear in every paper-cited prediction row;
- the rejection rate is reported in the bundle's
  `evaluation_report.json`;
- doc 19 §Discussion has a paragraph on per-output verifiability with
  numbers from a real bundle.
