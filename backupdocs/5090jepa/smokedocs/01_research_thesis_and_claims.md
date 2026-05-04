# 01 - Research Thesis And Claims

## Research Thesis

Existing JEPA work shows that useful world representations can be learned by
predicting latent targets instead of reconstructing raw observations. The
5090 ME-JEPA demo should contribute a systems result:

```text
JEPA-style latent prediction + typed domain packs + source-of-truth storage
+ artifact hashing + planning/surprise records = auditable world-model runtime
```

The novelty is not that a tiny model beats large career models. The novelty is
that every stage of a domain-portable latent dynamics system is physically
verifiable on commodity high-end hardware.

## Claims We Can Make With Current Runtime

1. **End-to-end world-model loop**: domain pack -> event ingest -> adapter ->
   latent panel -> trajectory -> dataset -> CUDA training -> artifact registry
   -> prediction -> plan trace -> surprise -> verification row.
2. **Domain portability**: counter_world, gridworld, and a career-taxonomy domain
   can use the same storage, CLI, trainer, artifact registry, and FSV pattern.
3. **Auditability**: every persisted behavior has decoded RocksDB readback and
   every model artifact has recomputed file hashes.
4. **Single-GPU feasibility**: the complete smoke program can run on one RTX
   5090 with 32GB VRAM.
5. **Career-domain feasibility**: public occupation/skill taxonomies can be
   represented as typed career events and used to train/evaluate latent
   transition predictors.

## Claims We Must Not Make Without More Data

- Do not claim validated recruiting prediction accuracy without licensed real
  recruiting/career records.
- Do not claim compliance readiness without privacy/legal gate evidence.
- Do not claim production deletion propagation unless the deletion FSV is run on
  a real subject/vault source of truth.
- Do not claim broad labor-market validity from a controlled public taxonomy
  benchmark alone.

## Paper-Framing Language

Use:

- "5090-feasible research demonstrator"
- "public-taxonomy career transition benchmark"
- "source-of-truth verified latent dynamics runtime"
- "controlled career-domain task grounded in public occupational taxonomies"

Avoid:

- "production recruiting model"
- "predicts human careers" without qualification
- "fully compliant" unless legal/privacy FSV evidence exists
- "real-world hiring decisions" unless the data and governance support that

## Core Contributions

1. A domain-pack abstraction that makes latent world modeling domain portable.
2. A strict full-state verification protocol for model training, inference,
   planning, and artifact registry operations.
3. A compact CUDA JEPA implementation that can train action-conditioned latent
   predictors on a single RTX 5090.
4. A career-domain benchmark design that captures occupation, skill, transition,
   and action effects without private recruiting data.
5. A research bundle schema that makes paper results reproducible and auditable.

