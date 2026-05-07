---
namespace: decisions
agent_id: agent-e14-1-tokenizer-probe
timestamp: 2026-04-22T15:30:00Z
status: active
tags: e14, dependencies, hf-hub, cargo-toml
---

# Decision: Do NOT add `hf-hub` crate for E14 integration

## Context

The Task 1 prompt suggested either `hf_hub` crate or `reqwest`-to-HF-mirror
for downloading `tokenizer.json`. I investigated Cargo.lock and the
existing codebase.

## Findings

1. `grep "^name = " Cargo.lock` — neither `hf-hub`, `reqwest`, `ureq`, nor
   any other HTTP client is a dep of the workspace. Adding one would pull
   in tokio-native-tls/rustls and significantly expand the dep graph.
2. All 9 existing embedder loaders (semantic, kepler, sparse, code,
   causal, contextual, graph, late_interaction, plus `train_causal.rs`)
   use the same pattern:
   ```rust
   let tokenizer_path = self.model_path.join("tokenizer.json");
   let tokenizer = Tokenizer::from_file(&tokenizer_path)?;
   ```
   They NEVER download; the operator is expected to have the model on disk.
3. BAAI/bge-m3 is already fully cached locally (see
   `discoveries/agent-e14-1-tokenizer-probe--bge-m3-already-cached.md`).

## Decision

Follow the existing pattern. Point E14's loader at the local HF Hub cache
(or a configurable `model_path`), same as E1-E13. Operators who don't have
bge-m3 cached run `huggingface-cli download BAAI/bge-m3` — outside the
daemon's responsibility.

## Rationale

- **Consistency:** matches all 9 existing embedders
- **No new deps:** avoids pulling in `reqwest`/`rustls`/`hyper` (~50+ extra
  crates, meaningful compile time)
- **Fail-fast:** missing model fails loudly at load time, not silently at
  download time
- **Already works:** the probe test confirms end-to-end without downloads

## Impact on later agents

- **Agent 2 (enum plumbing):** nothing new; `ModelId::BgeM3Dense` fits the
  same `model_path`-keyed factory pattern
- **Agent 3 (model-impl):** use `self.model_path.join("tokenizer.json")`
  verbatim from `semantic/loader.rs:54-55`. No new crate
- **Operator runbook (Task 19 / 23):** doc note that operators must run
  `huggingface-cli download BAAI/bge-m3` once before migration

## Revisit if

An automated CI/test environment is added that can't assume pre-cached
models. At that point we add `hf-hub` to `dev-dependencies` only, not
production.
