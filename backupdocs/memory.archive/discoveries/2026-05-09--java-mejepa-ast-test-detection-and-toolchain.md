---
namespace: discoveries
agent_id: codex-java-hardening
timestamp: 2026-05-09T02:54:57Z
status: active
tags: mejepa,java,ast-parser,dynamicjepa,fail-closed,test-detection
---

# Java ME-JEPA AST Harness Missed Standard Test Signals

## What Happened

The live ME-JEPA action extractor in `crates/context-graph-cli/src/bin/reality-loop.rs`
had no Java-aware `path_is_test` patterns, so edits in files such as
`src/test/java/com/acme/CalculatorTests.java` or `CardValidatorIT.java`
were classified as ordinary Java methods unless the method name started
with `test`.

The semantic-index extractor in `crates/context-graph-cli/src/commands/dynamicjepa.rs`
had a narrower Java check (`*Test.java`) but still missed common Surefire
and Failsafe patterns such as `Test*.java`, `*Tests.java`, `*TestCase.java`,
`IT*.java`, `*IT.java`, and `*ITCase.java`.

The live action compiler path also returned parser-only evidence with
`toolchain_missing` when Java lacked `javac`. For the Java hardening scope,
that is a fallback that can hide missing compiler evidence from ME-JEPA
prediction records.

## Why It Matters

ME-JEPA's Java action panel needs to distinguish production edits from
test edits and needs to know when compiler-backed evidence is actually
unavailable. Misclassifying Java tests as production methods corrupts
`E_AST`, `E_Test`, patch-intent, and verifier-target features. Returning
parser-only Java evidence when `javac` is missing creates a false sense
that the Java compiler/LSP slice was observed.

## Root Cause

Java test detection was implemented as generic Python/JS/TS path heuristics
plus a single narrow `*Test.java` suffix in one code path. It did not encode
the Java ecosystem's standard Maven Surefire/Failsafe naming conventions
or JUnit annotation signals.

## Related

- Official Maven Surefire defaults: `Test*.java`, `*Test.java`, `*Tests.java`,
  `*TestCase.java`.
- Official Maven Failsafe defaults: `IT*.java`, `*IT.java`, `*ITCase.java`.
- JUnit Jupiter marks test methods with `@Test`, `@ParameterizedTest`,
  `@RepeatedTest`, `@TestFactory`, or `@TestTemplate`.

## Verification

- `cargo test -p context-graph-cli java -- --nocapture` exercises the Java
  semantic-index and live action harnesses.
- Semantic-index FSV writes real Java source files and a
  `java-semantic-fsv.json` evidence file, then re-reads the JSON before
  asserting `adds` and `computes` are test symbols.
- Live action FSV re-reads the compiler snapshot path named in
  `RUST_REALITY_DYNAMICJEPA_JAVA_TOOLCHAIN_MISSING` when `javac` is absent,
  proving the candidate after-source exists in the snapshot and the repo source
  remains unchanged.
