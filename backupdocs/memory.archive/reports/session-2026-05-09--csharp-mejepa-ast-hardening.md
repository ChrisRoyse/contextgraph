---
namespace: reports
agent_id: codex-csharp
timestamp: 2026-05-09T05:40:00Z
status: complete
tags: csharp,mejepa,dynamicjepa,ast,semantic-index,fsv,fail-closed,dotnet
---

# C# ME-JEPA AST Harness Hardening Report

## Scope

Focused only on C# AST parser/action hardening for the ME-JEPA DynamicJEPA
prediction system. Concurrent non-C# workspace changes were left untouched.

## Root Cause Summary

- C# `operator_declaration` symbols were collapsed to the generic name
  `operator`, so overloaded operators lost semantic identity.
- C# semantic-index build-target discovery accepted `.sln` and `.csproj`, but
  not `.slnx`, while the action path already allowed `.slnx`.
- C# compiler evidence used `dotnet build --no-restore`, which can fail a clean
  SDK-style project snapshot before `project.assets.json` exists.
- Live action owner attribution initially selected nested call nodes inside an
  operator body instead of the enclosing C# operator declaration.
- The host initially had no `dotnet` on `PATH`, preventing the real compiler
  happy path from being exercised.

## Fixes

- C# semantic-index and live action naming preserve operator overload identity:
  `operator +`, `operator ==`, and conversion operators such as
  `operator int`.
- C# build-target discovery accepts exactly one `.sln`, `.slnx`, or `.csproj`
  and fails closed for missing or ambiguous targets.
- C# compiler checks use `dotnet build <target> --nologo`, allowing the .NET
  CLI's normal implicit restore in clean snapshots.
- C# action attribution prefers enclosing `operator_declaration` and
  `conversion_operator_declaration` owners before nested call/assignment nodes.
- Installed and verified user-local .NET SDK `8.0.420`; added `DOTNET_ROOT` and
  `.dotnet/tools` to `/home/cabdru/.bashrc` and `/home/cabdru/.profile`.

## Research Applied

- Microsoft documents `dotnet build` as building project/solution inputs,
  including `.slnx`, and explains that build normally performs implicit restore
  unless `--no-restore` is used.
- Microsoft C# reference defines overloaded operators as explicit public
  `operator` declarations on user-defined types, so the operator token is part
  of the durable member identity.
- Tree-sitter documents anonymous operator tokens plus `ERROR` and `MISSING`
  nodes; parser recovery must be treated as diagnostics, not guessed semantics.
- Microsoft dotnet-install docs require manual `DOTNET_ROOT` and `PATH`
  exports after user-local installation; signature verification was performed
  before installing.

## Source Of Truth

Semantic FSV:

- `/tmp/contextgraph-csharp-semantic-fsv/csharp-semantic-fsv.json`
- `/tmp/contextgraph-csharp-semantic-fsv/csharp-lib-tests.log`
- source readback SHA-256:
  `3ce24cbc07e33c2a3926113af6cab76399cb1805cb9d6d386c1ff7f3b59e56ea`
- symbols read back: `Vector`, `X`, `Vector`, `operator +`, `operator int`
- call read back: `operator +` calls `Vector`

Action FSV:

- `/tmp/contextgraph-csharp-action-fsv/csharp-action-fsv.json`
- `/tmp/contextgraph-csharp-action-fsv/csharp-action-tests.log`
- checked state: `compiler_semantic_status="checked"`,
  `compiler_diagnostic_count=0`, `semantic_provider="compiler_lsp"`
- repo source readback remained unchanged after the candidate compiler snapshot.

Artifact hashes:

- semantic log:
  `77b65657cc479cfa2d6962fcbdc2e0c8ea5a3af779428f9f7e3dd5f3cbfc24f8`
- semantic JSON:
  `c1ccba1bd018b78ecb344e5806ee6394b6b65abbc563eed0dba69365d6019b1e`
- action log:
  `9eef7f083cf04e53fd4617b49e4ba92bd07e66ac76320b5df5effbeb509899db`
- action JSON:
  `3e43a49c5d1a3f3c3ea675cec4c2f8ec71150309401132cdb87f0ad1384b739b`

## Verification Commands

- `target/debug/deps/context_graph_cli-425ba30a0168ed8d csharp --nocapture --test-threads=1`
- `target/debug/deps/reality_loop-fd57860eb98cbc76 csharp --nocapture --test-threads=1`
- `cargo fmt --all --check`
- `git diff --check`
- `bash -lc 'command -v dotnet && dotnet --version && dotnet --info | sed -n "1,28p"'`
- `jq '{source_of_truth, source_readback_sha256, symbols: [.symbols[] | {name, semantic_kind, visibility, kind}], calls, dataflow_edges, type_facts}' /tmp/contextgraph-csharp-semantic-fsv/csharp-semantic-fsv.json`
- `jq '{source_of_truth, action: .action, repo_source_readback}' /tmp/contextgraph-csharp-action-fsv/csharp-action-fsv.json`

## Manual Edge Cases

- Missing build target: before `files=[]`; after fail-closed
  `requires a real .sln, .slnx, or .csproj`.
- Ambiguous projects: before `["App.csproj", "Lib.csproj"]`; after
  fail-closed multiple-project diagnostic.
- Ambiguous solutions: before `["Lib.sln", "App.sln"]`; after fail-closed
  multiple `.sln/.slnx` diagnostic.
- Invalid C# syntax: before broken `Oops(` method; after
  `RUST_REALITY_DYNAMICJEPA_AST_PARSE_SYNTAX_ERROR` with a `MISSING ")"` tree
  diagnostic and source-of-truth file path.

## Build Notes

A full fresh `cargo test` rebuild from an isolated target directory was
blocked by the repo's CUDA-enabled llama.cpp dependency stack and concurrent
native builds. The root build-environment problem was `/usr/bin/nvcc` selecting
an old CUDA path and defaulting to `sm_52`; reruns with
`CUDACXX=/usr/local/cuda-13.2/bin/nvcc` and
`CMAKE_CUDA_ARCHITECTURES=120a-real` progressed into real `sm_120a`
compilation but exceeded the tool session limit. Final C# verification used
already-built focused test binaries from the current workspace and then
performed separate JSON/log readback from the physical source-of-truth files.

## Open Blockers

- None for C# parser/action hardening.
- A full clean cargo rebuild should be rerun later when the shared CUDA native
  build workload is quiet.

## Confidence: high

The C# semantic and action tests passed, the action path used real `dotnet`
compiler evidence, and the final state was verified by separate reads of the
JSON/log artifacts and repo source.
