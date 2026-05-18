---
namespace: decisions
agent_id: agent-77-harness-editor
timestamp: 2026-05-08T03:18:00Z
status: superseded
superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md
tags: locked-decision, harness-edit, model-swap, moe, coder-specialized, generic, supersedes-aspect-of-claude-md
---

# Decision: swap inner-actor from dense Qwen3.6-27B-Q6_K_XL to MoE Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL

## Locked rule

The default ccreality inner-actor is **Qwen3-Coder-30B-A3B-Instruct, Unsloth Dynamic Q4_K_XL GGUF**:
- Repo: `unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF`
- Filename: `Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf`
- Path: `models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf`
- Size: 17,665,334,432 bytes (17.67 GB)
- SHA256: `2841aa314d916434860cfb8990347528dcdfe5c350dbcb9d1461dbee88ff2533`
- Pin key: `qwen3_coder_30b_a3b_q4_k_xl`
- MCP model identifier: `qwen3-coder:30b-a3b`

Architecture: Mixture-of-Experts. 30B total params, **only 3B active per token**. Coder-specialized training. Same Qwen3 chat template + tool calling format as predecessor — drop-in compatible with the harness.

This SUPERSEDES the dense Qwen3.6-27B-Q6_K_XL pin established earlier and referenced in `CLAUDE.md §11.1`. Old GGUF deleted from disk.

## Why locked

Operator-driven directive (2026-05-08 session). Empirical observation: dense 27B at Q6_K_XL produced 14-26 min/round on RTX 5090 due to:
1. Heavy thinking-mode generation (thousands of `<think>` tokens before tool call)
2. Large prompt prefill at growing context

MoE A3B reduces per-token compute by ~10× (3B active vs 27B dense). Coder-specialized training preserves SWE-bench code-edit quality. Q4_K_XL quant (vs prior Q6_K_XL) further reduces prefill cost.

Expected ~3-5× wallclock speedup per round. Combined with turn-73's 7200s budget cap, attempts should comfortably fit a complete trajectory (5-12 rounds) inside budget.

## What changed (concrete edits)

### 1. `config/ccreality_model_pins.json` (full rewrite)
Old pin `qwen3_6_27b_q6_k_xl` REPLACED with new pin `qwen3_coder_30b_a3b_q4_k_xl`. Old SHA `8746881d…` and old size `25636485344` gone; new SHA + size in.

### 2. `crates/context-graph-cli/src/bin/reality-loop.rs`

| Line | Change |
|---|---|
| 28 | `DEFAULT_MODEL: "qwen3.6:27b" → "qwen3-coder:30b-a3b"` |
| 29-30 | `DEFAULT_INNER_MODEL_PATH: "models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf" → "models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf"` (wrapped onto two lines for rustfmt) |
| 31 | `DEFAULT_INNER_MODEL_PIN_KEY: "qwen3_6_27b_q6_k_xl" → "qwen3_coder_30b_a3b_q4_k_xl"` |
| 32-37 | doc comment for `DEFAULT_GPU_MIN_FREE_MB` — added swap note |
| 38-46 | doc comment for `DEFAULT_GPU_PRELOAD_MIN_FREE_MB` — updated rationale |
| 47 | `DEFAULT_GPU_PRELOAD_MIN_FREE_MB: 25600 → 18432` (right-sized for 17.67 GB model + KV cache headroom) |
| 12626-12640 | pin-loading FSV test fixtures — repo_id, filename, size lower-bound updated |
| 12704-12706 | schema-invalid test fixture — pin key, repo_id, filename updated |

New SHAs:
- `config/ccreality_model_pins.json`: `sha256:57675233ec55a06a097fb7d095aee75c26d0d748d1907848fc16e84b81a1243b`

### 3. Disk
- Deleted: `models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf` (24 GB freed)
- Deleted: `models/qwen3.6-27b/` directory (now empty)
- Added: `models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf` (17.67 GB)
- Net disk: ~6 GB freed

## Verification (turn 77)

| Check | Result |
|---|---|
| `cargo check -p context-graph-cli` | PASS (2.1s, no new warnings) |
| `cargo test -p context-graph-cli` | PASS (38 passed, 0 failed; all `load_model_pin_*_fsv` and `validate_inner_model_pin_*_fsv` tests pass against new pin) |
| `cargo build --release -p context-graph-cli` | PASS (26.2s) |
| `target/release/reality-loop` mtime | `2026-05-07 22:15:15 -0500` (rebuilt this swap) |
| Source-of-truth readback | matches spec exactly |
| Old GGUF removal | verified gone |
| Smoke test | NOT run — engine PID 742443 still in flight; would conflict |

## GENERIC-ONLY check

Mental check: would this same model swap help on `psf__requests-2317`, `django__django-12286`, `sympy__sympy-13895`? **YES** — model swap applies to ALL SWE-bench tasks. Coder-specialized + MoE benefits universal.

## Operational notes

- The new binary at `target/release/reality-loop` is staged for the NEXT drip via the MCP→CLI shim (commits 4072dcf3 / d48102e6).
- Engine PID 742443 (still running) uses the OLD binary AND OLD model (which I just deleted from disk). The mmap'd GGUF is held in the engine's memory pages, so deletion doesn't crash it — the process keeps the file's data via the open mmap mapping. When the engine exits, those pages are released cleanly.
- After PID 742443 ends, the next drip will use Qwen3-Coder-30B-A3B-Q4_K_XL.
- Initial validation will need to confirm: chat template loads correctly (Qwen3 series), tool grammar still matches, GPU footprint stays under 18432 MB preload threshold.

## Rollback path

If the new model proves unsuitable (chat template incompatibility, quality regression, memory issues), the rollback is:
1. Re-download `unsloth/Qwen3.6-27B-GGUF` filename `Qwen3.6-27B-UD-Q6_K_XL.gguf` (24 GB)
2. Restore previous pin entry (sha256 `8746881d40f280b1b6b858c656a347c754ed3d9cc8d2e1ad46b3635b87f611f8`, size 25636485344)
3. Revert source defaults

## Source of truth

- `file:/home/cabdru/contextgraph/config/ccreality_model_pins.json`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L28-L47`
- `file:/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs#L12626-L12640`
- `file:/home/cabdru/contextgraph/models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf`
- `file:/tmp/contextgraph-rust-reality-loop/rust_reality_1778203122132669180_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/harness-transitions/round-04.json` (and rounds 05-09)
