# 2026-05-12 — Phase C Corpus Expansion FSV

Implemented Phase C corpus expansion for `context-graph-mejepa-corpus`.

Root causes fixed during manual real-toolchain generation:
- Generic `subtle_flip` treated syntax tokens like Rust `->` as boolean comparison candidates. Candidate replacements now must remain tree-sitter parseable before selection.
- Generic `swap_variable` allowed declarations, type names, method parameters, and Go `:=` declaration lines as candidate variables/replacements. It now restricts candidates to likely use sites.
- Generic `delete_test_call` used substring matching and deleted `expected` bindings or C# helper declarations. It now requires assertion-token boundaries and skips declaration lines.
- Go fixtures could become invalid after assertion deletion or variable swapping because Go rejects unused locals. The fixture now keeps `t` and `expected` independently used.

Durable source of truth:
- Python corpus: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511`
- Non-Python Phase C corpus: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512`
- Combined evidence: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`

Final persisted evidence:
- Non-Python index complete: 5,120 entries, 640 fixture tasks, 10 languages.
- Non-Python filesystem: 5,120 mutation files and 5,120 toolchain report JSON files.
- Non-Python RocksDB read-only inspection: 5,120 `mejepa_mutation_corpus` rows, 5,120 `mejepa_oracle_verdicts` rows, 1 `mejepa_corpus_provenance` row.
- Stats gate: 512 entries per non-Python language; `non_python_fixture_gate.passes=true`.
- Combined Phase C evidence: `passes=true`; Python 2,400 entries, non-Python 5,120 entries, combined split task count 940, bucket sizes `[752, 94, 94]`, leakage check passed.

Manual edge checks:
- Output under `/tmp` failed before writing; after state path did not exist.
- `--tasks-per-language 0` failed before writing; after state output path did not exist.
- `phase-c-verify --min-entries-per-non-python-language 999` against the 80-entry smoke corpus failed closed and wrote evidence with 10 explicit per-language failures.

Verification:
- `cargo clippy -p context-graph-mejepa-corpus --all-targets -- -D warnings`
- `cargo test -p context-graph-mejepa-corpus`
- `cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus`
