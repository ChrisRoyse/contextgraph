---
namespace: decisions
agent_id: agent-141-coordinator
timestamp: 2026-05-08T18:55:00Z
status: active
tags: ruvector,agentdb,upstream,clean-room,reference-only,no-vendoring,no-path-dep,locked
---

# Locked: Upstream Reference-Only — Clean-Room Reimplementation Only

## The rule (locked 2026-05-08 by operator)

For both the cloned `agentdb/` repo and the cloned `ruvector/` repo:

1. **No vendoring.** Do NOT copy any of their source files into `crates/` or `vendor/`.
2. **No path-dep.** Do NOT add `path = "../agentdb/..."` or `path = "../ruvector/..."` to any `Cargo.toml`. The cloned trees never appear in our build dependency graph.
3. **No npm dep.** Do NOT add `agentdb`, `ruvector`, `@ruvector/*`, or related packages to any `package.json` we ship.
4. **No upstream tracking.** "I don't care about the things he does with ruvector moving forward." We do NOT pull updates, watch the repos, or sync to their release cadence. Whatever is in the trees today is the only snapshot we'll ever look at.
5. **Reference-only.** The cloned trees stay on disk solely as **read-only inspiration**. Read the code to understand the algorithm; reimplement in our own crates against our own design.

## What we DO with the clones

- Read the source to understand a primitive (e.g., the SHAKE-256 73-byte witness chain in `ruvector/crates/rvf/rvf-crypto/src/witness.rs:24`).
- Read the ADR docs and the comments to learn the design rationale.
- Re-derive the algorithm from first principles in our own naming, our own crate boundaries, our own error types.

## Why this rule exists

- **Operator quote (verbatim):** "Vendor (copy into ContextGraph tree) i want to go with this path lock it in. i just want to take ruvector ideas and do my own implementation of them into my own system. i dont care about the things he does with ruvector moving forward. i just want to use it as reference for the technology to see how many it could be applied to my use case with the reality loop and me jepa system"
- The operator wants ContextGraph to be self-contained: zero external Rust crates from the ruvnet ecosystem, zero npm packages from the agentdb ecosystem.
- The operator does not want to inherit upstream choices, version pins, or maintenance burden.
- The operator wants to learn from the algorithms, not adopt the codebase.

## How this overrides the prior findings docs

- **`docs/agentdbfindings/`** (8 docs, 2,069 lines): the existing recommendations (mostly "port to Rust") align with this rule and stay valid. Wherever a doc says "vendor" or implies adoption, treat it as superseded by this rule.
- **`docs/ruvectorfindings/`** (9 docs, 2,909 lines): the existing recommendations (which used "vendor" and "path-dep" as primary verbs) are SUPERSEDED. Every "Vendor `crates/X`" or "Path-dep `crates/X`" recommendation becomes "**Clean-room reimplement** the relevant idea from `crates/X` into our own crate." Effort estimates for clean-room reimplementation are roughly 2-3× the path-dep estimates in the original docs.

## How to reference RuVector / agentdb in code

When a Rust file in `crates/context-graph-*` is implementing an idea taken from these references, add a header comment in the form:

```rust
// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/witness.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
```

The comment is **provenance only** — it cites the inspiration so future agents can find the original. It is NOT a license attribution, because no code was copied.

## How this affects the future

- Future agents that read `docs/agentdbfindings/` or `docs/ruvectorfindings/` MUST honor this rule. Recommendations to "vendor" or "path-dep" in those docs are obsolete.
- Future agents proposing a new ruvnet-ecosystem dependency must either (a) cite an explicit operator override, or (b) not propose it.
- The `agentdb/` and `ruvector/` directories at the workspace root are reference checkouts. Add to `.gitignore` if not already. Do NOT commit them. Do NOT add them to workspace `members`. Do NOT add them to workspace `exclude` either — they should be invisible to `cargo`.

## Cross-references

- Findings docs: `docs/agentdbfindings/`, `docs/ruvectorfindings/`.
- Prior decisions:
  - `agent-140-coordinator--agentdb-tier1-tier2-rust-adoption.md` — predates this rule; "Rust adoption" is now interpreted as clean-room reimplementation.
- CLAUDE.md §11.4 (GENERIC-ONLY rule) — every clean-room reimplementation must satisfy the same generality test: would this same primitive help if the use case were different?
- CLAUDE.md §10.3 (Self-improvement loop) — this decision is itself an instance of "after a correction from the operator, write the lesson to ./memory/decisions/."

## Test the rule before merging anything

Before any PR that adds code under `crates/context-graph-*`, run the check:

```bash
# No path-dep on agentdb/ or ruvector/
! grep -rn 'path.*=.*"\.\./\(agentdb\|ruvector\)' Cargo.toml crates/*/Cargo.toml

# No vendored ruvector/agentdb files
! find crates -name '*.rs' -exec grep -l '@ruvector\|ruvnet/agentdb' {} \;

# No npm deps from the ruvnet ecosystem
! grep -E '"(agentdb|ruvector|@ruvector/)' package.json
```

If any check fails, the change violates this locked decision.
