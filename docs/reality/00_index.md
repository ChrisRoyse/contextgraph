# Reality-Verified Self-Optimization

This directory defines how ContextGraph can become a reality-grounded self-optimization system: any LLM proposes actions, ContextGraph executes through governed tools, durable source-of-truth readbacks verify the outcome, and the verified outcome becomes the next prompt, reward sample, benchmark row, and training/eval episode.

The product category name used here is **Reality-Verified Self-Optimization**.

## Core Claim

Most agent loops iterate on text, logs, or model confidence. ContextGraph can iterate on durable reality:

```text
LLM -> ContextGraph action gateway -> real system mutation
    -> source-of-truth readback -> evidence ledger
    -> reward/loss -> next prompt / training sample / promotion gate
```

The loop does not advance because an agent says it is done. It advances only when the source of truth proves the expected result persisted.

## Files

| File | Purpose |
|---|---|
| `01_research_findings.md` | Research notes from Ralph Loop, Karpathy AutoResearch, SWE-agent, SWE-bench, and Software 2.0/3.0. |
| `02_contextgraph_reality_loop_architecture.md` | Full system architecture for ContextGraph as a reality verification and optimization layer. |
| `03_reality_data_contract.md` | Data records that must be captured so failures can be correlated, reprompted, scored, and trained on. |
| `04_swe_bench_reality_loop.md` | How this applies to SWE-bench Lite and eventually larger SWE-bench subsets. |
| `05_auto_optimization_engine.md` | The autonomous experiment loop that keeps/discards attempts based on verified reward. |
| `06_governance_and_reward_hacking_controls.md` | Controls required so the system optimizes outcomes without learning to cheat the metric. |
| `07_implementation_roadmap.md` | Practical build plan for turning the docs into working code. |
| `08_meta_reality_loop.md` | The outer loop whose job is to create and improve the inner SWE-bench reality loop. |
| `09_smoke_suite_curriculum.md` | How to size the smoke test so it is hard enough to optimize against but small enough to run safely. |
| `10_reality_measurement_matrix.md` | Full matrix of what must be measured in reality to make optimization possible. |
| `11_loop_builder_artifacts.md` | Concrete artifacts the meta-loop is allowed to create, edit, score, keep, or discard. |
| `12_prompt_reinjection_protocol.md` | How verified failures become the next model prompt without polluting it with noise or lies. |
| `13_path_to_100_percent_swe.md` | Strategy for pushing all three local models toward 100% on the selected SWE suite. |
| `14_operational_scoreboard.md` | The scoreboard that tells whether the meta-loop is actually improving the inner loop. |
| `15_rust_reality_loop_port.md` | Current Rust-owned reality loop implementation, validation evidence, and remaining gaps. |

## Current Project Mapping

| Capability | Current Location |
|---|---|
| Local Gemma/Qwen/Mistral frontend and tool harness | `gemmafrontend/` |
| Verified reward compiler spec | `gemmafrontend/reward_spec.verified_outcome.json` |
| SWE-bench local refinement loop | `gemmafrontend/swe_lite_refinement_loop.py` |
| Rust-owned SWE reality loop | `target/debug/reality-loop` from `crates/context-graph-cli/src/bin/reality-loop.rs` |
| Official SWE-bench Docker readback bridge | `gemmafrontend/swe_lite_official_eval.py` |
| Governed SWE agent tool gateway | `gemmafrontend/swe_lite_refinement_loop.py --agent-tool-rounds` |
| Deterministic reality prompt compiler | `gemmafrontend/reality_prompt_compiler.py` |
| Smoke benchmark suite definition | `gemmafrontend/swe_smoke_benchmark_suite.json` |
| Current SWE-bench status doc | `docs/contextgraph_swe_bench_refinement_loop.md` |
| BlackBoxFix ledger/evidence CLI | `target/debug/context-graph blackbox ...` |
| DynamicJEPA state/action/surprise machinery | existing ContextGraph/DynamicJEPA modules and generated run artifacts |

## Phase 1 Implementation Snapshot

The first working reality loop now persists:

- root reality ledger: `<run_root>/reality-loop.sqlite`;
- root event stream: `<run_root>/reality-events.jsonl`;
- branch registry: `<run_root>/reality-scoreboard/reality-branches.json`;
- attempt tool transcript: `<attempt_root>/agent-tools/agent-tool-transcript.json`;
- official failure evidence: `<attempt_root>/official-failure-evidence.json`;
- next prompt candidate: `<task_root>/prompt-candidates/attempt-NNN/`.

The current model-facing tools are read-only and governed: `list_files`, `read_file`, `search_repo`, `inspect_repository_context`, and `read_failure_evidence`. Patch mutation still enters only through the verified patch/evaluator path.

## Phase 2 Implementation Snapshot

Phase 2 adds a first-class prompt compiler so prompt reinjection is a deterministic artifact, not scattered string assembly inside the loop.

Current source-of-truth artifacts:

- compiler implementation: `gemmafrontend/reality_prompt_compiler.py`;
- loop integration: `gemmafrontend/swe_lite_refinement_loop.py`;
- initial prompt manifest: `<attempt_root>/prompt-manifest.json`;
- refinement prompt candidate: `<task_root>/prompt-candidates/attempt-NNN/`;
- compiler readback files: `prompt.txt`, `prompt-compiler-output.json`, and `prompt-quality.json`;
- task BlackBoxFix ledger: `<task_root>/blackbox.sqlite`;
- root reality ledger: `<run_root>/reality-loop.sqlite`.

The compiler currently emits these deterministic prompt templates:

- `swe_repair_initial_v2`;
- `swe_repair_local_failure_v2`;
- `swe_repair_official_failure_v2`.

The compiler fails closed with typed errors when required source-of-truth inputs are missing, an unknown prompt kind is requested, or hidden solution/gold patch content leaks into the prompt.

Latest verified Phase 2 smoke output:

```text
/tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777939166629468299_801dcad0/
```

That run proves the compiler generated a second-attempt official-failure prompt candidate from durable SWE-bench/BlackBoxFix evidence. The instance remained unresolved, which is the correct reality signal for optimization: the loop recorded the failure, generated reward/incidents, and produced the next prompt candidate instead of claiming success.

## Continuous Reality Loop

The legacy Python loop can run until official source-of-truth success:

```bash
python3 gemmafrontend/swe_lite_refinement_loop.py \
  --model qwen3:8b \
  --instance-id psf__requests-2317 \
  --official-eval-each-attempt \
  --run-until-resolved
```

This mode requires official SWE-bench evaluation and stops only when the official report places the instance id in `resolved_ids`. Candidate failures become next-prompt evidence. Infrastructure, model-runtime, ledger, compiler, or official-harness failures still fail closed with typed errors.

The Rust port now owns the same core loop for Gemma-first smoke testing:

```bash
cargo build -p context-graph-cli --bin reality-loop
target/debug/reality-loop \
  --model gemma4:e4b \
  --task-manifest /tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777940680618314859_801dcad0/tasks/psf__requests-2317/qwen3-8b/task-manifest.json \
  --runtime-root /tmp/contextgraph-rust-reality-loop \
  --official-eval-each-attempt \
  --run-until-resolved
```

The Rust binary uses Rust-owned orchestration, typed failure artifacts, JSON readback checks, SQLite ledger readbacks, GPU VRAM budget checks through `nvidia-smi`, and BlackBoxFix reward/evidence compilation. The official SWE-bench evaluator remains an external Python/Docker verifier because SWE-bench itself is distributed that way; the reality loop does not implement its control flow in Python.

Latest Rust smoke output:

```text
/tmp/contextgraph-rust-reality-loop-smoke-v4/rust_reality_1777959982988939074_e525987d/
```

That run used real Gemma/Ollama output and real `psf/requests` source. It intentionally does not claim success: attempt 1 produced a corrupt patch, attempt 2 violated the raw JSON contract, and BlackBoxFix persisted two verifier failures, two incident packets, and two reward compilations.

## Research Sources

- Ralph Loop docs and repository: https://ralphloop.sh/ and https://github.com/pageai-pro/ralph-loop
- Anthropic Ralph Loop plugin page: https://claude.com/plugins/ralph-loop
- Karpathy AutoResearch repository: https://github.com/karpathy/autoresearch
- Karpathy Software 2.0 essay: https://karpathy.medium.com/software-2-0-a64152b37c35
- SWE-agent paper: https://arxiv.org/abs/2405.15793
- SWE-agent ACI docs: https://github.com/SWE-agent/SWE-agent/blob/main/docs/background/aci.md
- SWE-bench Lite and evaluation docs: https://www.swebench.com/lite.html and https://www.swebench.com/SWE-bench/guides/evaluation/
- Reflexion paper: https://arxiv.org/abs/2303.11366
