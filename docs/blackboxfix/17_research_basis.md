# Research Basis

This planning set uses current public standards and project patterns as
design input. The goal is not to copy any one framework, but to align
BlackBoxFix with how buyers already think about AI risk, security,
provenance, observability, and tool integration.

## External References

### NIST AI Risk Management Framework

Reference: <https://www.nist.gov/itl/ai-risk-management-framework>

Design implication:

- map BlackBoxFix controls to Govern, Map, Measure, and Manage;
- produce evidence for design, deployment, use, and evaluation of AI systems;
- support risk management instead of only debugging.

### OWASP Top 10 For LLM And GenAI Applications

References:

- <https://owasp.org/www-project-top-10-for-large-language-model-applications/>
- <https://genai.owasp.org/llm-top-10/>

Design implication:

- treat prompt injection, sensitive data disclosure, supply chain, improper
  output handling, excessive agency, and unbounded consumption as first-class
  risk labels;
- build tool authorization and source-of-truth verification around excessive
  agency risks.

### MITRE ATLAS

Reference: <https://atlas.mitre.org/>

Design implication:

- classify security incidents using an adversarial AI taxonomy;
- give CISOs and SOC teams familiar threat labels;
- connect agent incidents to real-world attack patterns when applicable.

### OpenTelemetry

Reference: <https://opentelemetry.io/docs/what-is-opentelemetry/>

Design implication:

- export traces, metrics, and logs to existing observability stacks (Phase 7);
- keep the BlackBoxFix ledger as the authoritative evidence source;
- use semantic attributes for run, step, tool, verifier, connector, risk tier.

### W3C PROV

Reference: <https://www.w3.org/TR/prov-dm/>

Design implication:

- model provenance with entities, activities, and agents;
- represent tool calls as activities and source-of-truth records as entities;
- preserve responsibility and derivation across runs.

### OpenLineage

Reference: <https://openlineage.io/docs/>

Design implication:

- borrow the run/job/dataset lineage pattern for data workflows;
- use facets or facet-like extensions for verifier evidence;
- make lineage export possible for data-agent workflows.

### in-toto

Reference: <https://in-toto.io/>

Design implication:

- use supply-chain style metadata for who did what, in what order;
- make action chains transparent;
- consider signed attestations for evidence packets (Phase 4/5).

### SLSA Provenance

Reference: <https://slsa.dev/spec/v1.0/provenance>

Design implication:

- describe how artifacts were produced;
- let consumers verify that production matched expectations;
- apply provenance thinking to agent-produced code, config, and reports.

### Model Context Protocol

Reference: <https://modelcontextprotocol.io/specification/2025-06-18>

Design implication:

- support MCP as a standard way for agents to access tools and context
  (Phase 6);
- expose verified tool wrappers through MCP;
- treat tool descriptions as untrusted unless from trusted servers.

### SQLite WAL And Triggers

References:

- <https://www.sqlite.org/wal.html>
- <https://www.sqlite.org/lang_createtrigger.html>

Design implication:

- WAL journaling + `synchronous=FULL` is enough for a tamper-evident local
  ledger when combined with `BEFORE UPDATE`/`BEFORE DELETE` `RAISE(ABORT)`
  triggers; Phase 1 uses exactly this pattern.

## Internal Project References

- `docs/upgradeplancombine/09_fsv_extensions.md` — FSV protocol primitives
  (the broader Context Graph FSV doctrine).
- `docs/upgradeplancombine/28_bundle_training_corpus_exporter.md` — JSONL
  exporter contract using state/action/expected/readback/verifier rows.
- `docs/5090jepa/13_mcp_and_cli_surface.md` — strict MCP/CLI tool surface
  (the surface BlackBoxFix mirrors).
- `docs/5090jepa/16_troubleshooting_and_failure_taxonomy.md` — typed failure
  and remediation discipline.
- `crates/context-graph-cli/src/commands/export_dynamicjepa.rs` — the
  reference Rust pattern for the Phase 5 episode exporter.
- `crates/context-graph-cli/tests/dynamicjepa_bundle_export_fsv.rs` — the
  reference FSV test pattern (real input → real output → physical readback).

## Synthesis

The strongest product shape is:

- OpenTelemetry-compatible, but not limited to telemetry.
- PROV/OpenLineage-inspired, but focused on agent actions.
- in-toto/SLSA-inspired, but generalized beyond software builds.
- NIST/OWASP/MITRE-aligned, but operational enough to generate evidence.
- MCP-compatible, but independent from any one agent framework.

Phase 1 already commits to the foundational primitives: append-only
hash-chained storage, canonical JSON, deterministic redaction, typed errors.
Every higher-level standard above maps onto those primitives without
requiring a new storage backend.
