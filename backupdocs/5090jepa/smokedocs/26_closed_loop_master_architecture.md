# 26 - Monolith Architecture

This is the architecture of the **single mega-system** at v1.0. Not a
composition of four products. Not a closed loop linking separate
codebases. **One system** with internal layers, internal contracts,
and one external surface.

The previous version of this doc framed Context Graph + ClipCannon +
ME-JEPA + TCT as four systems composing into a closed loop. That
framing is gone. They are layers of one runtime now, vendored together
in a single repo (smokedoc 32).

## What The Monolith Is

```text
                          External surface
                          (one CLI, one MCP, one HTTP API)
                                  │
   ┌──────────────────────────────┴───────────────────────────────┐
   │                  THE MONOLITH (one process)                   │
   │                                                               │
   │  ┌──────────────────────────────────────────────────────────┐ │
   │  │  Output layer                                            │ │
   │  │  ─ Predictions, plans, surprises, voice, video, text     │ │
   │  │  ─ Every output passes Constellation Guard               │ │
   │  │  ─ Every output carries provenance hash                  │ │
   │  └────────────────────────▲─────────────────────────────────┘ │
   │                           │                                   │
   │  ┌────────────────────────┴─────────────────────────────────┐ │
   │  │  Inference + alignment layer                             │ │
   │  │  ─ ME-JEPA predictor (action-conditioned, latent JEPA)   │ │
   │  │  ─ Decoder heads (top-k occupation, skill reconstruct)   │ │
   │  │  ─ Voice generation (Qwen3-TTS + LoRA per-identity)      │ │
   │  │  ─ Video generation (Phoenix avatar engine)              │ │
   │  │  ─ Constellation Guard (per-output cosine vs centroids)  │ │
   │  └────────────────────────▲─────────────────────────────────┘ │
   │                           │                                   │
   │  ┌────────────────────────┴─────────────────────────────────┐ │
   │  │  Training layer                                          │ │
   │  │  ─ ME-JEPA training (CUDA, candle 0.10.2, RTX 5090)      │ │
   │  │  ─ TCT 3-phase: panel build, geometric loss, runtime gd  │ │
   │  │  ─ LoRA fine-tuning per-identity                         │ │
   │  │  ─ Frozen centroid construction from reference subset    │ │
   │  └────────────────────────▲─────────────────────────────────┘ │
   │                           │                                   │
   │  ┌────────────────────────┴─────────────────────────────────┐ │
   │  │  DDA panel layer                                         │ │
   │  │  ─ 13 frozen text embedders (E1..E13) ─ Context Graph    │ │
   │  │  ─ 7 frozen video modalities ─ ClipCannon                │ │
   │  │  ─ Pairwise interaction features (78 + 21 = 99 pairs)    │ │
   │  │  ─ Up-to 91 signals/text-input + 28/video-clip           │ │
   │  └────────────────────────▲─────────────────────────────────┘ │
   │                           │                                   │
   │  ┌────────────────────────┴─────────────────────────────────┐ │
   │  │  Storage + provenance layer                              │ │
   │  │  ─ RocksDB column families (~75 across all systems)      │ │
   │  │  ─ Audit log (tamper-evident, OpenTimestamps anchored)   │ │
   │  │  ─ Bundle assembly (one schema; smokedoc 29)             │ │
   │  │  ─ FSV protocol (smokedoc 08)                            │ │
   │  └────────────────────────▲─────────────────────────────────┘ │
   │                           │                                   │
   │  ┌────────────────────────┴─────────────────────────────────┐ │
   │  │  Ingest layer                                            │ │
   │  │  ─ Domain packs (career, voice, video, text-style, ...)  │ │
   │  │  ─ Typed event adapters                                  │ │
   │  │  ─ Corpus manifest + license + hash                      │ │
   │  └──────────────────────────────────────────────────────────┘ │
   └───────────────────────────────────────────────────────────────┘
```

Six layers. One process. One repo. Every horizontal arrow is in-process
function call; no cross-service network hops in the inner loop.

## Why One Process, Not Microservices

- Latency: cross-language calls between Rust (Context Graph + ME-JEPA)
  and Python (ClipCannon) are tolerable when in-process; intolerable
  over network for streaming inference (voice / video).
- Provenance: in-process calls trivially share the same bundle handle;
  microservices need RPC + shared state, which is more code and more
  failure modes.
- Operational simplicity: one process to deploy, one process to
  monitor, one process to restart.
- Honest about scope: a "platform" is sometimes vapor for "five
  different services that share a logo". The monolith refuses that.

## Cross-Language Boundary (Rust ↔ Python)

The Rust workspace owns the storage + provenance + DDA panel + ME-JEPA
+ Constellation crate. The Python `clipcannon/` tree owns the 23-stage
video pipeline + Phoenix avatar + VoiceAgent meeting bot.

The monolith is **one process** at runtime via:

- **Embedded Python interpreter** in the Rust process via `pyo3`,
  invoked for ClipCannon stages that need Python ML libs (PyTorch,
  diffusers, faster-whisper, demucs);
- **OR**: a single MCP supervisor process spawns one Rust subprocess +
  one Python subprocess and routes; the **MCP boundary is in-host
  IPC, not network**.

Smokedoc 33 picks one. The pyo3 path is technically cleaner; the
supervisor path is operationally simpler. Decision is in smokedoc 33.

## What Each Layer Owns

### Ingest

- Domain pack TOML schemas + adapters.
- Corpus manifest with license + hash.
- Typed event ingest with FSV at-rest.
- Code: `crates/context-graph-dynamicjepa/src/ingest/` +
  `clipcannon/src/clipcannon/pipeline/` (ingest stages of 23-DAG).

### Storage + Provenance

- RocksDB CFs across all subsystems.
- Audit log + tamper-evident chain + OpenTimestamps anchoring.
- Bundle assembly (smokedoc 29).
- FSV evaluator.
- Code: `crates/context-graph-storage/` + `clipcannon/src/clipcannon/
  storage/` (sqlite-vec for video pipeline state).

### DDA Panel

- 13 frozen text embedders (E1-E13) hosted in-process.
- 7 frozen video modalities hosted in-process.
- Pairwise interaction adapters.
- MI audit (smokedoc 30) on demand.
- Code: `crates/context-graph-embedders/` +
  `clipcannon/src/clipcannon/embeddings/`.

### Training

- ME-JEPA trainer (Rust + candle).
- LoRA fine-tuner (Python or Rust depending on model).
- TCT 3-phase orchestrator.
- Frozen centroid builder.
- Code: `crates/context-graph-dynamicjepa/src/training/` +
  `clipcannon/src/clipcannon/training/` (for video LoRAs).

### Inference + Alignment

- ME-JEPA predict / plan / surprise.
- Decoder heads.
- Voice + video generation.
- Constellation Guard.
- Code: `crates/context-graph-dynamicjepa/src/inference/` +
  `crates/context-graph-constellation/` (shared crate; smokedoc 28) +
  `clipcannon/src/clipcannon/generation/`.

### Output

- One emission contract: `(payload, provenance_hash, guard_decision)`.
- Persisted to `dj_predictions` / `dj_plan_traces` / `dj_surprise_events`
  / `dj_constellation_guard_decisions`.
- One streaming format for voice + video (chunked + per-chunk guard).

## What Lives Where In The Repo

After smokedoc 32 (merged repo structure):

```text
contextgraph/
  crates/                       # Rust workspace
    context-graph-core/
    context-graph-storage/
    context-graph-dynamicjepa/  # ME-JEPA
    context-graph-embedders/    # 13-embedder DDA panel
    context-graph-constellation/# shared CG crate (smokedoc 28)
    context-graph-cli/          # one CLI binary
    context-graph-mcp/          # one MCP server
    ...
  clipcannon/                   # Python tree (vendored 2026-04-30)
    src/
      clipcannon/               # 23-stage DAG
      phoenix/                  # avatar engine
      voiceagent/               # meeting bot
      license_server/           # billing
    ...
  configs/                      # domain packs, train configs, centroids
  docs/
    5090jepa/smokedocs/         # this directory
  paper/                        # one-paper LaTeX
  Cargo.toml                    # Rust workspace root
  Cargo.lock
  pyproject.toml                # top-level Python workspace pointer
  Makefile                      # cross-language build
  Dockerfile                    # one container
```

The Rust workspace and the Python tree coexist at the same level. A
top-level `Makefile` (smokedoc 32) orchestrates both.

## Single External Surface

Three thin wrappers over the same internal calls:

- **CLI**: `context-graph <subcommand>` covers every capability.
  ME-JEPA, voice clone, video clone, DDA enrichment, agent runtime,
  bundle hash, MI audit — all subcommands of one binary.
- **MCP server**: `context-graph mcp serve` exposes ~150 tools (75 CG
  + 58 CC + new ME-JEPA) under one namespace (smokedoc 33).
- **HTTP API**: thin REST/JSON-RPC mirror of the MCP surface for
  non-LLM clients.

Smokedoc 33 specifies the surface design.

## What Was Removed

The previous version of this doc had a "Document Map (26-41)" and
gestured toward five product docs (31-35) and a three-paper roadmap
(36). All of that was deleted on 2026-04-30 when the monolith pivot
landed. The current monolith doc map is in smokedoc 14.

## Dependencies On Other Docs

- Smokedoc 27 (DDA → ME-JEPA bridge) — internal layer plumbing.
- Smokedoc 28 (Constellation Guard for ME-JEPA) — shared crate.
- Smokedoc 29 (unified bundle schema) — output contract.
- Smokedoc 30 (MI audit) — empirical anchor.
- Smokedoc 32 (merged repo structure) — this architecture's filesystem.
- Smokedoc 33 (unified surface) — this architecture's surface.

## Acceptance

The monolith architecture is **realized** when:

- one `cargo build` + `pip install -e clipcannon` produces a working
  binary that exposes every capability;
- one `context-graph mcp serve` registers tools from every subsystem
  under one namespace;
- one `reproduce.sh` end-to-end runs the integrated pipeline against
  a sample corpus on the operator's RTX 5090;
- the bundle root hash (smokedoc 29) covers every output of every
  layer.
