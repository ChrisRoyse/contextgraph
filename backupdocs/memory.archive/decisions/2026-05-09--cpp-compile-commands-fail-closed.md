---
namespace: decisions
created: 2026-05-09T03:20:50Z
updated: 2026-05-09T03:20:50Z
status: active
severity: high
tags: cpp, mejepa, compile-commands, fail-closed, semantic-index
---

# C++ ME-JEPA Semantics Require Structured Compile Commands

## Decision
C++ ME-JEPA AST/action semantics must fail closed unless a real `compile_commands.json` with structured `arguments` is available. Parser-only C++ evidence, guessed compiler invocations, and shell-string `command` entries are not acceptable substitutes for action control or semantic-index evidence.

## Rationale
C++ semantic reality depends on the actual translation-unit command: include paths, `-D` macros, language standard, target, compiler driver, and working directory. Official Clang documentation defines `compile_commands.json` as the replay format for compilation actions and states that `arguments` is preferred because shell escaping is a source of errors. Clangd's design notes also call out that incorrect or fallback compile commands cause severe parsing problems, especially missing include paths or parsing C++ as C.

For headers, the source of truth is the real translation units in the compilation database, because headers are usually not compiled standalone. The harness should compile/check those real TUs rather than inventing a header-only command.

## Consequences
- Missing `compile_commands.json` returns `RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_MISSING`.
- Shell-string `command` entries return `RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_SHELL_COMMAND_REFUSED`.
- Structured `arguments` entries are rebased to the semantic snapshot and run with `-fsyntax-only`.
- stdout/stderr command evidence is written to files, recorded as `file:` source-of-truth strings, and read back during FSV.

## Related
- Discovery: ./memory/discoveries/2026-05-09--cpp-action-range-whitespace.md
- Report: ./memory/reports/session-2026-05-09--cpp-ast-harness-hardening.md
- Tags: cpp, compile_commands, fail-closed, semantic-index
