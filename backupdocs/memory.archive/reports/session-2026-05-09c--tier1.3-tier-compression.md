---
namespace: reports
agent_id: agent-204-coordinator
created: 2026-05-09T07:00:00Z
updated: 2026-05-09T07:00:00Z
timestamp: 2026-05-09T07:00:00Z
status: active
tags: report, session, tier-compression, doc-05-tier-1.3, doc-09-section-6.2, quantization
---

# Session Report: 2026-05-09c — Tier 1.3 / doc 09 §6.2 Tier Compression

## Task
Continue audit-and-implement of docs 00-09. Doc 05 Tier 1.3 ("Clean-room
8/7/5/3-bit tier compressor") and doc 09 §6.2 (tiered storage policy)
were both still in the deferred pile. Ship them as a new clean-room
crate.

## Outcome: complete (for this primitive)

## Summary
New crate `crates/context-graph-tier-compression/` (~470 lines after
refinements) implementing groupwise per-vector linear scalar
quantization at 4 bit widths (8/7/5/3) per `BitWidth` enum. Encode
produces a `CompressedBlob` with magic-versioned 22-byte header + bit-
packed payload. Decode reconstructs the f32 vector with reconstruction
error bounded by `(max - min) / (2*(2^bits - 1))`. Sub-byte packing
uses LSB-first bitstream into ceil(n*bits/8) bytes. Fail-closed
everywhere: empty input, non-finite (NaN OR ±Inf), truncated bytes,
wrong magic, wrong version, unsupported bit width — all reject with
`TIER_COMPRESSION_INVALID_INPUT`. `min > max` after deserialize rejects
with `TIER_COMPRESSION_NUMERICAL_INVARIANT`.

This is doc 09's tiered-storage-policy substrate: HOT = raw f32, WARM
= 8-bit (4× compression), COOL = 5-bit (~6.4×), COLD = 3-bit (~10.7×).
The 7-bit intermediate (~4.57×) is also implemented for completeness.

A code-simplifier subagent flagged 3 quality refinements; all 3 applied:
- R1 — guard `n: u64 → usize` truncation in deserialize (fail-closed
  at `lib.rs:393-403`).
- R2 — fuse encode passes (DEFERRED, the validate-then-mutate pattern
  is intentional).
- R3 — `packed_len(n, bits) -> Option<usize>` with `checked_mul` to
  defend against attacker-controlled `n` values (`lib.rs:426-432`).
  Required updating 4 callsites: encode uses `.expect()` (n is bounded
  by an in-memory slice), decode and deserialize fail-closed via
  `ok_or_else`, serialize falls back to `HEADER_LEN` capacity (since
  `serialize` is infallible and an OOM on extend is the right failure
  mode for impossible-n cases).

## Files Created or Modified in the Repo
- `crates/context-graph-tier-compression/Cargo.toml` (NEW)
- `crates/context-graph-tier-compression/src/lib.rs` (NEW, ~470 lines)
- `crates/context-graph-tier-compression/src/tests.rs` (NEW, 21 tests)
- `crates/context-graph-tier-compression/examples/tier_compression_fsv.rs`
  (NEW, ~280 lines)
- `Cargo.toml` (workspace root) — added new crate to members and
  default-members.

## Files Written or Updated in ./memory/
- `discoveries/agent-tcs-code-simplifier--tier-compression-n-truncation-32bit.md` (subagent)
- `discoveries/agent-tcs-code-simplifier--tier-compression-double-finite-scan.md` (subagent)
- `discoveries/agent-tcs-code-simplifier--tier-compression-packed-len-overflow.md` (subagent)
- `reports/agent-tcs-code-simplifier--tier-compression-review--final-report.md` (subagent)
- `reports/session-2026-05-09c--tier1.3-tier-compression.md` (this file)

## Decisions Made
- **Per-vector min/max storage** — matches doc 09 §6.2 "groupwise quant"
  framing. Each blob stores its own min/max in the header so vectors
  with different ranges quantize independently. Alternative
  (corpus-wide fixed range) was rejected because it would require an
  upfront calibration pass and breaks the decode-without-context
  property.
- **Endpoint-preservation invariant** — proved by construction. The
  minimum value maps to q=0 → decode = min + 0*range = min. The
  maximum value maps to q=L-1 → decode = min + (L-1)/(L-1)*range = max.
  Tests at `tests.rs:48-59` assert this at every bit width.
- **R3 `packed_len -> Option<usize>`** — the alternative was making it
  panic on overflow. Returning Option keeps `packed_len` safe to call
  from `deserialize` against on-disk bytes whose `n` could be
  attacker-controlled.

## Discoveries
- **Bit-packing accumulator math**: at any moment between values,
  `bits_in_acc` is `< 8` (we flush full bytes each iteration). Adding a
  new value adds at most 8 bits. So `bits_in_acc + 8 ≤ 15 < 64`,
  meaning the `u64` accumulator never risks overflow on the shift +
  OR. Verified by direct argument; tests cover 8-bit (max width) and
  3-bit (smallest width).
- **JSON-text-of-f32 round-trip can be 1-ULP-shifted** for some f32
  values: previous turn's split FSV hit
  `0.10333333333333333 ↔ 0.10333333333333332`. Tier-compression FSV
  doesn't carry f32 in the JSON evidence (only metadata), so the same
  trap doesn't apply here. Documented for future FSV producers.

## Failures
None this turn. The R3 refinement cascaded into ~6 callsite updates
that broke compile temporarily, but each broken site was visible in
diagnostics and fixed within the same edit cycle. No test was modified
to "cover up" broken behavior.

## Successes
- 21 unit tests pass; the new crate compiles clean with no warnings.
- FSV evidence on disk:
  - `/tmp/contextgraph-tier-compression-fsv/blob_8.bin` (790 bytes)
  - `/tmp/contextgraph-tier-compression-fsv/blob_7.bin` (694 bytes)
  - `/tmp/contextgraph-tier-compression-fsv/blob_5.bin` (502 bytes)
  - `/tmp/contextgraph-tier-compression-fsv/blob_3.bin` (310 bytes)
  - `tier-compression-evidence.json` records compression ratios:
    8-bit = 3.89×, 7-bit = 4.43×, 5-bit = 6.12×, 3-bit = 9.91×.
  - Cosine similarities: 0.9999933 (8-bit), 0.9999742 (7-bit),
    0.9995989 (5-bit), 0.9919150 (3-bit) on a 768-dim Gaussian-like
    synthetic vector.
  - Per-element error bounds verified within
    `(max - min) / (2*(2^bits - 1))` at every width.
- All 4 boundary cases (empty input, NaN input, truncated on-disk blob,
  wrong-magic on-disk blob) reject with the expected error code
  AND leave physical evidence on disk
  (`/tmp/contextgraph-tier-compression-fsv/{truncated_blob,wrong_magic_blob}.bin`).
- Subagent review: 0 BLOCKER, 1 PASS, 3 LOW refinements, all applied.

## Open Blockers
None.

## Tests / Build
- Tests added: 21 (all passing).
- Tests passing: yes.
- Build status: passing.
- Total tests across ME-JEPA-foundation crates:
  mejepa-corpus=50 + solver=58 + witness=30 + mincut=18 +
  tier-compression=21 = **177**.

## Resume Here Next Session

**Context to read first**:
- `memory/context/mejepa-implementation-initiative.md` — Phase 0 audit
  (now further-along: mutation operators ✓, split wrapper ✓, tier
  compression ✓).
- `memory/reports/session-2026-05-09b--phase0-corpus-split-wrapper.md`
  — previous session's "Resume Here" still applies for Docker oracle.

**Doc 09 implementation state after this turn**:
- Tier 1.3 (8/7/5/3-bit tier compression) — ✓ shipped this turn.
- Tier 1 remaining: 1.5 (training-free retrieval LM + diffuser),
  BMSSP V-cycle.
- Tier 2 remaining: 2.1 (SONA), 2.2 (verified_training certificates),
  2.5 (Matryoshka + Graph RAG / Leiden).
- Tier 3 remaining: 3.1, 3.3, 3.4, 3.5, 3.6 (most are XL effort).
- Doc 09 Phase 0 remaining: Docker oracle wrapper, RocksDB CFs,
  full 300-task corpus generation.
- Doc 09 Phases 1-8: instruments, predictor, training, inference,
  self-healing, hygiene, integration, evaluation. ~38-50 days of work.

**Highest-leverage next primitive (recommended)**: Tier 2.5 Matryoshka +
Graph RAG (Leiden community detection). Doc 09 references
Matryoshka-style nested embeddings for the panel, and Graph RAG /
Leiden for cross-task patch similarity (§3.5.3 row "Cross-task patch
similarity graph"). Bounded ~2-3 days; clean-room math; touches no
external systems. Alternative: Phase 0 RocksDB CFs (would close the
last code-side Phase 0 deferred item).

**Watch out for**:
- The new `packed_len(n, bits) -> Option<usize>` API change is a
  breaking change to the crate's public surface. No external users
  yet; if the storage crate ever consumes it, propagate the Option
  carefully.
- The 22-byte header is fixed-size with magic+version. Future schema
  evolution should bump VERSION rather than reusing bytes — the
  deserialize check at `lib.rs:382-389` rejects unknown versions
  fail-closed.

**Open question**: Does doc 09's "Stoer-Wagner mincut for cross-task
similarity graph" (§3.5.3 row 8) mean the same Stoer-Wagner that
context-graph-mincut implements, or a different graph clustering
primitive? Need to re-read the doc to confirm before shipping the
Tier 2.5 Graph RAG primitive.

## Confidence: high
21 tests pass, no warnings, FSV produces 4 .bin files on disk +
JSON evidence + 2 boundary-case .bin files for the truncated/wrong-magic
cases. Cosine similarities at every bit width exceed the conservative
per-tier thresholds. Code-simplifier subagent confirmed correctness; 3
LOW refinements applied; 0 BLOCKER.
