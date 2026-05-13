# Session Report — Phase C Corpus Expansion FSV

Implemented and verified Phase C corpus expansion.

Changed corpus crate behavior:
- Added `generate-fixtures` CLI for deterministic Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, and PHP fixture corpora.
- Added `phase-c-verify` CLI that validates Python 300x8 evidence, non-Python per-language minimums, toolchain report counts, RocksDB CF counts, and combined split leakage.
- Added non-Python fixture generation with real toolchain validators and timed subprocess execution.
- Added per-entry language to the corpus index while keeping old Python indexes readable with a Python default.
- Extended stats with `by_language`, `by_language_category`, and the non-Python fixture ship gate.
- Hardened generic non-Python mutation operators to avoid syntax tokens, declaration sites, type names, helper declarations, and substring assertion false positives.

Web research applied:
- Tree-sitter parser docs: validate parse trees and reject ERROR/MISSING nodes rather than trusting text substitutions.
- Rust `std::process` docs and cargo-mutants timeout docs: keep subprocess validation timeout-bound and output-drained.

Final source-of-truth evidence:
- Non-Python corpus: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512`
- Non-Python verify evidence: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/verify-evidence.json`
- Combined Phase C evidence: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`

Final counts:
- Python: 2,400 entries, 2,400 mutation CF rows, 2,400 verdict CF rows, Python ship gate passed.
- Non-Python: 5,120 entries, 5,120 mutation CF rows, 5,120 verdict CF rows, 5,120 toolchain reports.
- Per non-Python language: 512 entries each for C, C#, C++, Go, Java, JavaScript, PHP, Ruby, Rust, TypeScript.
- Combined split: 940 tasks, bucket sizes `[752, 94, 94]`, leakage check passed.

Commands verified:
- `target/release/mejepa-corpus generate-fixtures --output /mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512 --tasks-per-language 64 --seed 17 --tool-timeout-secs 45`
- `target/release/mejepa-corpus phase-c-verify --python-corpus /mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511 --non-python-corpus /mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512 --evidence /mnt/d/contextgraph/fsv/corpus-fsv/split_full.json --tasks-dir tasks/swebench-lite --min-entries-per-non-python-language 500`
- `cargo clippy -p context-graph-mejepa-corpus --all-targets -- -D warnings`
- `cargo test -p context-graph-mejepa-corpus`
- `cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus`
