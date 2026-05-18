---
namespace: reports
created: 2026-05-08T23:44:31-05:00
updated: 2026-05-08T23:45:53-05:00
status: active
tags: report, session, ast-parser, toolchain, fsv
---

# Session Report: 2026-05-09 - AST Parser Toolchain Install

## Task: install missing AST/parser harness dependencies for every supported language
## Outcome: complete for host toolchains; repo-level cargo check not completed due native build cost/concurrent workload

## Summary
Audited the harness-supported languages in `reality-loop.rs` and `dynamicjepa.rs`: Rust, Python, JavaScript, TypeScript, TSX, Go, Java, C, C++, C#, Ruby, and PHP. Missing tools were `javac`, `dotnet`, `tsc`, and `pyright`.

Installed OpenJDK 21 JDK, .NET SDK/runtime coverage for 8/9/10, and npm global TypeScript/Pyright. Added `/usr/local/bin` symlinks for `tsc`, `tsserver`, `pyright`, and `pyright-langserver` so noninteractive processes do not depend on NVM shell startup.

Full state verification used a real multi-language workspace under `/tmp/contextgraph-ast-toolchain-fsv`. The evidence logs read back compiled artifacts, class files, DLLs, package locks, pyc files, and edge-case result logs from disk.

## Files Created or Modified in the Repo
- `memory/discoveries/2026-05-09--ast-parser-toolchains-installed-and-verified.md` - toolchain inventory and FSV source of truth.
- `memory/journal/2026-05-09--failure-ast-toolchain-fsv-fixture-and-native-build.md` - failures/lessons from verification.
- `memory/reports/session-2026-05-09--ast-parser-toolchain-install.md` - this report.

## Files Written or Updated in ./memory/
- `discoveries/2026-05-09--ast-parser-toolchains-installed-and-verified.md`
- `journal/2026-05-09--failure-ast-toolchain-fsv-fixture-and-native-build.md`
- `reports/session-2026-05-09--ast-parser-toolchain-install.md`

## Decisions Made
- Install .NET SDKs 8, 9, and 10 so C# `dotnet build` can handle common target frameworks for current Ubuntu 24.04-supported .NET releases.
- Keep TypeScript/TSX semantic indexing project-local (`node_modules/.bin/tsc`) per harness invariant; global `tsc` is only supplemental CLI availability.

## Discoveries
- Ubuntu built-in feed on this machine provided .NET 8 and 10; .NET 9 required the Ubuntu .NET backports PPA.
- Clean PATH readback works for `tsc`, `pyright`, `go`, `javac`, and `dotnet` using `/usr/local/bin:/usr/bin:/bin`.

## Failures
- TSX fixture initially failed real type-checking due module-scoped JSX declarations; fixed the fixture and reran successfully.
- Repo-level isolated cargo check was stopped while building native RocksDB under concurrent workload; not a missing parser dependency.

## Successes
- Installed and verified OpenJDK 21 JDK.
- Installed and verified .NET SDKs/runtimes 8, 9, and 10.
- Installed and verified TypeScript `6.0.3` and Pyright `1.1.409`.
- Synthetic happy paths passed for all harness-supported languages.
- Three edge cases were recorded with before/after state.

## Open Blockers
- None for host parser/toolchain dependencies.
- Repo-level `cargo check` should be rerun later when native build pressure is lower.

## Tests / Build
- Tests added: none in repo.
- Tests passing: all `/tmp/contextgraph-ast-toolchain-fsv` toolchain checks passed.
- Build status: host toolchain verification passing; repo-level cargo check not completed.

## Resume Here Next Session
- Start by reading: `memory/discoveries/2026-05-09--ast-parser-toolchains-installed-and-verified.md`.
- Watch out for: TypeScript/TSX must still have project-local `node_modules/.bin/tsc`; global `tsc` is not enough for that path.
- Priority: rerun `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/contextgraph-ast-toolchain-target cargo check -p context-graph-cli --no-default-features --bin reality-loop` when concurrent native builds settle.
- Open question: none for installed dependencies.

## Confidence: high
Every installed dependency was verified by command execution plus separate physical readback of produced artifacts or package state.
