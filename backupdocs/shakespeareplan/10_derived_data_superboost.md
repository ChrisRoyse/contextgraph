# 10 Derived Data Superboost

## Core Distinction

The 249k+ Shakespeare datapoints are not just "more examples." They are a
measured style manifold: real cleaned corpus content plus frozen-embedder
projections, meaning labels, register labels, meter labels, style vectors,
cross-view correlations, graph structure, and contrastive relationships.

That is different from synthetic model-generated training text.

Use generated text only for evaluation probes or isolated fixture tests unless it
has external provenance and a hard verification trail. Do not recursively train
on outputs from the candidate model.

## What The Derived Signals Make Possible

### 1. Style-Only Adapter Training

Treat base Gemma as the source of modern knowledge and the LoRA as a style
adapter. The training objective should mostly steer diction, cadence, syntax,
rhetoric, and register without overwriting world knowledge.

Use the derived labels to select and weight real Shakespeare passages:

- high-style-confidence passages get more weight,
- contaminated raw spans are excluded as assistant targets,
- register buckets are balanced,
- topic clusters are balanced so the adapter does not equate Shakespeare style
  with only love/death/kingship,
- long exact-play spans are used for centroid/statistics, not direct SFT targets.

### 2. Multi-Constellation Style Lattice

Build separate anchors for:

- sonnet lyric,
- soliloquy,
- comic prose,
- tragic command,
- political rhetoric,
- philosophical dialectic,
- plain explanatory prose,
- high-metaphor verse,
- low-metaphor direct answer.

At runtime, route by prompt intent or blend anchors. This lets the model answer
"explain bcrypt" in a clear Shakespearean explanatory register instead of
hallucinating a sonnet or scene.

### 3. Meaning-Preserving Style Mining

Use E1/E10/E14 and semantic labels to find passages with compatible meaning but
different stylistic signatures. This supports DPO pairs where the preference is
style quality, not topic memorization.

Rules:

- content views decide whether two records are comparable,
- style views decide which side is preferred,
- contamination gates decide whether either side may become a target,
- exact-overlap gates prevent memorized passage leakage.

### 4. Register Control Without User-Facing Controls

The model does not need exposed style knobs. Internally, register labels can
teach the adapter that Shakespeare has many modes:

- tender lyric,
- legal argument,
- insult/comedy,
- grief,
- counsel,
- oath,
- philosophical reflection,
- compact factual answer.

The served model can choose the register automatically from the user prompt.

### 5. Derived Logit Priors

E13/SPLADE and token-level views can produce a style lexicon stronger than a
hand-written archaic-word list.

Use it for runtime priors:

- boost period function words early,
- boost syntactic/rhetorical connectors,
- never suppress modern nouns needed for factual answers,
- track how often the prior fires,
- disable or soften it inside strict JSON/code blocks.

### 6. Style Coverage Map

The 249k signals can identify holes before training:

- underrepresented registers,
- works or genres that dominate too much,
- prompt categories with weak style anchors,
- modern-topic prompts whose nearest style clusters are unstable,
- outputs that score high on style but low on content preservation.

Training should be driven by these gaps, not by simply adding more rows.

### 7. Guard Calibration

Use real held-out Shakespeare, modern controls, v2.1 accepted outputs, and v4.1
loop failures to calibrate thresholds. Fixed thresholds such as 0.45/0.30 are
temporary until each embedder and constellation has a measured distribution.

The guard should make conjunctive decisions:

```text
hard gates pass
style vote passes
content vote passes
format/parser checks pass
loop checks pass
memorization checks pass
```

### 8. Collapse-Resistant Training Loop

The safe loop is:

```text
real Shakespeare corpus
  -> frozen embedder measurements
  -> derived labels/scores/weights
  -> cleaned target selection
  -> small style-adapter update
  -> held-out production FSV
```

The unsafe loop is:

```text
candidate model output
  -> accepted because it sounds good
  -> reused as training target
  -> repeated over generations
```

Avoid the unsafe loop unless a human/external source supplies ground truth and
the record is explicitly provenance-labeled.

## Immediate Engineering Consequence

The large `shakespeare_sft_all_signals.jsonl` artifact should be treated as a
derived-signal catalog, not as a ready SFT target file. Its labels and metadata
are valuable; any assistant target inside it still has to pass the same hard
cleanliness, length, overlap, and contamination gates as every other target.

The pre-training FSV harness enforces this distinction by copying only existing
records that pass gates into smoke files and by writing a SQLite evidence
database that can be queried after the run.
