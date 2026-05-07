---
namespace: blockers
agent_id: coordinator
timestamp: 2026-04-23T02:00:00Z
status: active
severity: high
tags: e14, scope, [f32; 13], fixed-size-arrays, phase-1, blocker
---

# BLOCKER: E14 integration requires 62-file [f32; 13] → [f32; 14] audit

## Discovery

After Agent 2 added `ModelId::BgeM3Dense = 14` and `Embedder::BgeM3Dense = 13`
and I completed the match-arm plumbing across 12 files (commit 0c7bf7c6),
`cargo check --workspace` compiles cleanly BUT 85 of 2066 lib tests fail
with `index out of bounds: the len is 13 but the index is 13`.

Root cause: `EMBEDDER_CONFIGS` is now `[EmbedderConfig; 14]` and consumers
that still use `[f32; 13]` / `[Option<f32>; 13]` / `[f32; 13]` arrays
fail on `embedder.index()` for the new E14 variant whose index is 13.

## Scope

62 files in crates/context-graph-{core,embeddings,storage,mcp} contain
`; 13]` array-type patterns that need to grow. ~200+ occurrences total.

Categories:
1. **Public trait signatures** in `traits/teleological_memory_store/` —
   `store_custom_weight_profile`, `get_custom_weight_profile`,
   `list_custom_weight_profiles`, `custom_weights: Option<[f32; 13]>`,
   `synergy_weights: Option<[f32; 13]>`. Breaking these breaks EVERY impl.
2. **Clustering** — `TopicProfile.strengths: [f32; 13]`, many sites.
3. **Similarity engine** — `[Option<f32>; 13]` score arrays.
4. **MCP handlers** — `list_custom_weight_profiles` return types.
5. **Storage roundtrip** — serialized weight profiles at `[f32; 13]`.
6. **Stubs** — test-utilities crate.
7. **Benchmark crate** — test configs.

## Why this blocks pilot execution

Every failure is a stored fixed-size type — not something a wildcard arm
can silently handle. Per user directive "NO workarounds, NO fallbacks,
NO backwards compatibility", each must be explicitly extended to 14.

Estimated incremental work:
- Update all `[f32; 13]` → `[f32; 14]`: ~1-2 hours mechanical
- Update all struct initializers / array literals: ~2-3 hours
- Update all test expectations: ~2 hours
- Update serialized data handling (migration): ~2-4 hours
- Re-run + debug: ~2-3 hours

**Realistic: 10-14 additional hours of grinding work before Phase 1
even gets to write the bge_m3_dense/ module code.**

## What's been delivered so far

- commit 03a45846: Agent 1 tokenizer probe (PASS) + Agent 2 ModelId enum + TokenizerFamily + NUM_EMBEDDERS flip
- commit 0c7bf7c6: Coordinator completed match-arm plumbing in 12 files;
  workspace compiles; 85 tests fail on the fixed-size array boundary

## Decision needed from user

Three paths forward:

**A. Continue grinding** — 10-14h more on mechanical 13→14 replacements,
then resume Phase 1 Tasks 5-23 (model impl + CF + HNSW + MCP + migration
+ FSV). Another ~20-30h after that. Total: 30-44h remaining for Phase 1.

**B. Parallel ExtendedEmbedder architecture** — revert commits 03a45846 +
0c7bf7c6 (back to 13-slot baseline), introduce a separate `ExtendedEmbedder`
enum that doesn't share [f32; 13] arrays with the legacy embedders. New
MCP tools expose the extended views. Existing 13-embedder code is
untouched. Estimated 15-20h for E14 end-to-end. Goes against the plan's
"parity with E1-E13" design but is delivery-realistic.

**C. Keep the Python-side sidecar** — revert commits, document that full
Rust-native integration requires a dedicated multi-day effort that
cannot be done in session fragments. Use the proven v4.2 Python LMDB
sidecar that already works.

## Coordinator recommendation

**B**, because:
- The parity-with-E1-E13 design assumes 13-slot arrays won't grow, but we
  now know they're pervasive. Growing them is not a reversible change —
  any serialized data stored in the old format stays readable only by
  accident.
- A ExtendedEmbedder parallel enum is clean: `Embedder` = 13 legacy,
  `ExtendedEmbedder` = N new. Storage is separate CFs (already designed
  that way in `docs/embedadd/08_storage_layer.md`). Search tools accept
  either enum. MCP tools get a new family for extended views.
- Less breaking than A, still delivers Rust-native E14 at parity for
  serving/scoring/retrieval.

## For the next agent / session

1. Read this file first.
2. Read `docs/embedadd/` for original plan.
3. Read `memory/reports/agent-e14-1-*` and `agent-e14-2-*` for agent context.
4. User decides A/B/C.
5. If B: revert commits 03a45846 and 0c7bf7c6, then start fresh with
   `ExtendedEmbedder` enum pattern.

## Current test state (evidence)

```
test result: FAILED. 1981 passed; 85 failed; 0 ignored; 0 measured;
0 filtered out; finished in 5.85s
```

All 85 failures are `index out of bounds: the len is 13 but the index is 13`
when `Embedder::BgeM3Dense.index() == 13` is used on `[_; 13]` arrays.
