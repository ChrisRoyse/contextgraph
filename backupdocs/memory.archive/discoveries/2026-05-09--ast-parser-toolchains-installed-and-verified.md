---
namespace: discoveries
created: 2026-05-08T23:44:31-05:00
updated: 2026-05-08T23:45:53-05:00
status: active
tags: ast-parser, toolchain, dependencies, fsv, dynamicjepa, mejepa
---

# AST Parser Toolchains Installed And Verified

## What Happened
Audited the ME-JEPA/DynamicJEPA AST parser harness languages and installed the missing external toolchains:

- Java: installed `openjdk-21-jdk`; `javac` is now `/usr/bin/javac`, version `21.0.10`.
- C#: installed `.NET` SDKs/runtimes for 8, 9, and 10.
  - Built-in Ubuntu feed: `dotnet-sdk-8.0 8.0.126`, `dotnet-sdk-10.0 10.0.107`.
  - Ubuntu .NET backports PPA: `dotnet-sdk-9.0 9.0.116`.
  - PPA source readback: `/etc/apt/sources.list.d/dotnet-ubuntu-backports-noble.sources`.
- JavaScript/TypeScript/Python static tooling: installed npm global `typescript@6.0.3` and `pyright@1.1.409`.
- Stable noninteractive symlinks:
  - `/usr/local/bin/tsc`
  - `/usr/local/bin/tsserver`
  - `/usr/local/bin/pyright`
  - `/usr/local/bin/pyright-langserver`

Already-present and verified:

- Rust: `cargo 1.95.0`, `rustc 1.95.0`, `rust-analyzer 1.95.0`, `rust-src`.
- Python: `python3 3.12.3`.
- Go: `go1.26.3`.
- C/C++: `gcc/g++ 13.3.0`, `clang 18.1.3`, `cmake 3.28.3`, `ninja 1.11.1`.
- Ruby: `ruby 3.2.3`.
- PHP: `php 8.3.6`.
- Tree-sitter crates: runtime `tree-sitter v0.26.8` with grammars for Rust, Python, JavaScript, TypeScript/TSX, Go, Java, C, C++, C#, Ruby, and PHP.

## Full State Verification
Source of truth:

- `/tmp/contextgraph-ast-toolchain-fsv/toolchain-fsv.log`
- `/tmp/contextgraph-ast-toolchain-fsv/dotnet-9-readback.log`
- Filesystem paths under `/usr/local/bin`, `/usr/bin`, `/usr/lib/dotnet`, `/usr/local/go`, and `/etc/apt/sources.list.d`.

Evidence hashes:

- `970f2e475c580160e1848ab0b26f6321f6b13abd874b8ceed4f7273b41a8715c  /tmp/contextgraph-ast-toolchain-fsv/toolchain-fsv.log`
- `b74a09f041e244ef4430871839c891f5c5d941aea4844aa6a9a00390188b91a6  /tmp/contextgraph-ast-toolchain-fsv/dotnet-9-readback.log`

The synthetic FSV workspace compiled or checked real source for every parser language:

- Rust: `cargo check`, `cargo test`.
- Python: `python3 -m py_compile`, `pyright --outputjson`.
- JS/TS/TSX: project-local `./node_modules/.bin/tsc --noEmit`.
- Go: `go test -v ./...`.
- Java: `javac -d classes ...`, then `java -cp classes CalculatorCheck` prints `4`.
- C: `gcc -fsyntax-only`, then compiled executable.
- C++: `g++ -fsyntax-only`, then compiled executable.
- C#: `dotnet build` produced `net8.0`, `net9.0`, and `net10.0` DLLs.
- Ruby: `ruby -c`, then program prints `4`.
- PHP: `php -l`, then program prints `4`.

## Edge Cases
- Invalid syntax: bad Python file fails with `SyntaxError: '(' was never closed`; before/after state and result log are in `/tmp/contextgraph-ast-toolchain-fsv/edge-invalid/`.
- Empty input: `/tmp/contextgraph-ast-toolchain-fsv/edge-empty/` discovers zero supported source files.
- TypeScript local toolchain invariant: `/tmp/contextgraph-ast-toolchain-fsv/edge-no-local-tsc/` has global `tsc` but no `node_modules/.bin/tsc`, proving the harness's project-local TS fail-closed requirement remains meaningful.

## Why It Matters
The parser harness can now exercise compiler/static-check paths for all supported languages instead of failing due missing host dependencies. TypeScript remains intentionally project-local for TypeScript/TSX semantic indexing; the global `tsc` is for JavaScript fallback and general CLI availability, not a substitute for repo-local TypeScript evidence.

## Related
- Prior Go install: `memory/discoveries/2026-05-09--go-toolchain-installed-system-wide.md`
- Prior tree-sitter ABI discovery: `memory/discoveries/2026-05-09--tree-sitter-abi-runtime-and-mejepa-fsv-hardening.md`
- Tags: ast-parser, toolchain, java, csharp, typescript, pyright
