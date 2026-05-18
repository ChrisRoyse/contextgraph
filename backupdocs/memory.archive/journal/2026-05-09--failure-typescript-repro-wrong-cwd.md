---
namespace: journal
agent_id: codex-typescript-hardening
timestamp: 2026-05-09T03:50:47Z
status: active
tags: typescript,mejepa,ast,repro,failure
---

# Failure: TypeScript Repro Ran Cargo From Temp Repo

## What Happened

The first TypeScript semantic-index reproduction created a real temporary
TypeScript git repo under `/tmp/contextgraph-ts-repro-before-*`, installed local
`typescript`, and then invoked `cargo run` while the shell was still `cd`'d into
that temp repo.

Cargo failed with:

`error: could not find Cargo.toml in /tmp/contextgraph-ts-repro-before-3020025/repo or any parent directory`

## Why It Matters

This was a harness-test setup error, not a parser-harness result. It produced no
semantic-index output and no command evidence directory, so it cannot be used as
evidence for TypeScript AST behavior.

## Lesson

When reproducing CLI behavior against a temp repo, run the ContextGraph CLI from
`/home/cabdru/contextgraph` and pass the temp repo via `--repo`.
