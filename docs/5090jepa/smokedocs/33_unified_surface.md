# 33 - Unified Surface

## Goal

Specify **one external surface** for the monolith: one CLI binary, one
MCP server, one HTTP API. Every capability — DDA enrichment, ME-JEPA
train/predict/plan, voice cloning, video cloning, Constellation Guard,
bundle hash, MI audit, agent runtime — is reachable from each surface.

The internal architecture (smokedoc 26) is layered. The **external
surface is flat**.

## Why Flat

Multiple surfaces fragment the user experience and the documentation.
Today the operator runs ~150 MCP tools across three subsystems
(Context Graph 75 + ClipCannon 58 + ME-JEPA emerging). One namespace
makes that surface usable by any Claude-grade assistant or any
DevOps-grade operator.

## Decision: Single Process, Two Languages, One Boundary

Per smokedoc 32 §"How Subsystems Interoperate", two architectures are
candidates. **Pick the supervisor architecture** for v1.0:

```text
context-graph mcp serve
   │
   ├── spawns: target/release/context-graph (Rust subprocess; CG + ME-JEPA)
   ├── spawns: uv run python -m clipcannon.server (Python subprocess; CC + Phoenix + VoiceAgent)
   └── routes MCP tool calls to the correct sub-server transparently
```

Why supervisor over PyO3:

- ClipCannon's heavy Python deps (PyTorch + diffusers + demucs +
  insightface) load slowly; cold start would block the Rust process.
- A Python subprocess crash does not crash the Rust subprocess.
- PyO3's GIL constraints would require careful threading work for
  every Python call from concurrent Rust handlers.
- The supervisor pattern is what ClipCannon already does internally
  (MCP server + license server + dashboard as 3 processes).

Migration to PyO3 is possible later if performance demands; v1.0 ships
supervisor.

The **boundary is in-host**, not network: stdio MCP between supervisor
and each sub-server. No network hop in the inner loop.

## Surface 1 — One CLI Binary

```bash
context-graph <verb> <object> [--flags]
```

The verbs cover every capability:

| Verb | Object | What It Does |
| --- | --- | --- |
| `init` | (project) | Initialize a workspace |
| `ingest` | `event`, `corpus`, `clip` | Ingest typed events / corpora / video clips |
| `enrich` | `corpus` | DDA enrichment: project corpus through panel |
| `register` | `domain`, `centroids` | Register domain pack or centroid set |
| `materialize` | `panel`, `trajectory`, `dataset` | ME-JEPA panel/dataset compilation |
| `train` | `predictor`, `decoder`, `lora` | Train ME-JEPA / decoder head / LoRA |
| `predict` | (state) | ME-JEPA prediction |
| `plan` | (state, candidates) | ME-JEPA planning |
| `surprise` | (state) | Record surprise event |
| `generate` | `voice`, `video`, `text` | Identity-locked generation |
| `guard` | `evaluate`, `build` | Constellation Guard ops |
| `audit` | `mi`, `bundle`, `chain` | MI audit, bundle hash, chain inspect |
| `bundle` | `hash`, `recompute`, `archive` | Bundle ops |
| `reproduce` | (run-id) | Reproduce a past run from bundle |
| `serve` | `mcp`, `http`, `dashboard` | Run a service |
| `inspect` | `cf`, `counts`, `artifact` | Storage inspection |

This is **one binary** invoking either the Rust subsystem directly or
the supervisor's Python sub-server transparently. The user does not
care which language the work runs in.

Examples:

```bash
# DDA enrichment of a text corpus
context-graph enrich corpus \
  --input ./onet_30_2.zip \
  --panel default-13 \
  --output tmp/dda_bundle_v1 \
  --json

# Train ME-JEPA on a domain pack with DDA panel
context-graph train predictor \
  --domain career_taxonomy \
  --panel career_taxonomy_dda_v2 \
  --seed 42 \
  --json

# Generate identity-locked voice with Constellation Guard
context-graph generate voice \
  --centroid alice_v1 \
  --text "Hello, world." \
  --out alice_hello.wav \
  --provenance alice_hello.provenance.json

# Build centroid set from a reference corpus
context-graph guard build \
  --domain career_taxonomy \
  --reference-set ./career_reference.jsonl \
  --centroid-id career_taxonomy_v1

# Reproduce a past run
context-graph reproduce paper_full_50k_v1 --seeds 42,43,44,45,46
```

## Surface 2 — One MCP Server

```bash
context-graph serve mcp
```

Registers tools from every subsystem under one namespace. Tool naming
convention:

```text
cg.<subsystem>.<verb>_<object>
```

Examples:

- `cg.dda.enrich_corpus`
- `cg.mejepa.train_predictor`
- `cg.mejepa.plan`
- `cg.guard.evaluate`
- `cg.voice.generate`
- `cg.video.generate`
- `cg.bundle.hash`
- `cg.audit.mi_audit`
- `cg.session.timeline`        (existing CG)
- `cg.video.analyze_clip`      (existing CC)
- `cg.video.compile_edl`       (existing CC)
- `cg.voiceagent.attend_meeting` (existing CC VoiceAgent)

Subsystem prefixes:

| Prefix | Origin |
| --- | --- |
| `cg.dda` | DDA panel ops |
| `cg.mejepa` | ME-JEPA train/predict/plan/surprise |
| `cg.guard` | Constellation Guard |
| `cg.voice` | Voice clone (ClipCannon voice + Phoenix) |
| `cg.video` | Video pipeline (ClipCannon 23-DAG, Phoenix avatar) |
| `cg.voiceagent` | Real-time meeting bot |
| `cg.session` | Session/timeline (Context Graph existing) |
| `cg.memory` | Memory/embedding (Context Graph existing) |
| `cg.audit` | Audit + provenance |
| `cg.bundle` | Bundle ops |
| `cg.system` | Health, info, metrics |

The supervisor fans out: tools prefixed `cg.video.*`, `cg.voice.*`,
`cg.voiceagent.*` route to the Python sub-server; everything else
routes to the Rust sub-server. The MCP client sees one server.

Tool count target: ~150 today (75 + 58 + ~15 ME-JEPA new). After
namespace consolidation, expect 130-140 distinct tools because some
overlap (e.g. CG and CC each have a `health` tool; consolidate to one).

## Surface 3 — One HTTP API

```bash
context-graph serve http --port 3300
```

Thin REST/JSON-RPC mirror of the MCP surface for non-LLM clients.
Every MCP tool maps to:

- `POST /v1/<namespace>/<tool>` with JSON body matching the MCP
  argument schema;
- responses include `provenance_hash` and `bundle_path` headers so
  clients can chain to follow-up calls.

Auth: bearer token via the existing ClipCannon license-server pattern
(HMAC), unified to one tenant model.

## Capability Discovery

```bash
context-graph capabilities
```

Returns the merged tool catalog as JSON: namespace + tool name + args
schema + return schema + provenance fields touched. This is what doc
consumers (clients, AI assistants, sales demos) read to learn what
the monolith does.

## Versioning

The MCP API is versioned at the server level: `cg.system.version`
returns `{api_version: "1.0", monolith_version: "1.0.0",
git_commit: "...", build_date: "..."}`.

API breaking changes bump the major version. CLI breaking changes
(verb rename, flag removal) require a deprecation cycle:

- v1.x: deprecation warning logged.
- v2.0: removal.

## Operational Decisions

- Single port for HTTP (default 3300; ClipCannon's existing 3100
  license-server stays for backward compat or moves under monolith
  router).
- Single MCP entry point on stdio (default) or SSE (when configured).
- `context-graph mcp serve` exits cleanly on SIGTERM, shutting down
  both sub-servers.
- Health endpoint `/healthz` returns aggregate health from both
  sub-servers.

## Backward Compatibility

ClipCannon's existing 58 MCP tool names are kept as **aliases** under
their new `cg.video.*` / `cg.voice.*` / `cg.voiceagent.*` prefixes for
v1.0. Old names log a one-time deprecation warning. Removed in v2.0.

Context Graph's existing 75 MCP tools migrate to `cg.session.*` /
`cg.memory.*` / `cg.audit.*` prefixes; existing names kept as aliases
for v1.0.

This protects any existing user (the operator's own AI assistant
configurations, public YouTube demos) from breaking on day one.

## Acceptance

Unified surface is **shipped** when:

- `context-graph capabilities` returns a unified tool catalog ≥130
  tools across all prefixes;
- `context-graph mcp serve` cold-starts both sub-servers in <8s and
  serves any tool from any subsystem in one MCP session;
- `context-graph serve http` exposes the same surface as REST;
- a single Claude / Codex / OpenAI client session drives DDA →
  ME-JEPA → Constellation Guard → bundle hash through the unified
  surface end-to-end without switching servers;
- doc 39 of the previous (deleted) plan's intent is fully covered
  by this doc.
