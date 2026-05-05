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
| `16_measurement_contract.md` | Runtime contract that prevents measurement-spec and boundary failures from becoming false success claims. |
| `17_problem_and_signal_reality_packets.md` | Current implementation of problem-reality capture, per-attempt signal packets, blindspot tracking, and FSV evidence. |
| `18_gemma_coding_agent_harness.md` | Current Gemma coding-agent harness, optimized Ollama settings, governed tool surface, line-window edit interface, tool-use reward signals, and FSV evidence. |
| `19_harness_transition_reality.md` | Current implementation of the harness-as-reality-loop transition contract: every governed tool call becomes a verified state/action/observation/outcome row. |
| `20_openai_outer_optimizer.md` | Current implementation of the OpenAI/Codex-grade outer optimizer and one-attempt smoke contract before long continuous loops. |
| `21_grand_scale_reality_autoresearch.md` | How Karpathy-style AutoResearch ideas are generalized into ContextGraph's two-level reality-loop system. |

## Current Project Mapping

| Capability | Current Location |
|---|---|
| Local Gemma/Qwen/Mistral frontend and tool harness | `gemmafrontend/` |
| Verified reward compiler spec | `gemmafrontend/reward_spec.verified_outcome.json` |
| Rust-owned SWE reality loop | `target/debug/reality-loop` from `crates/context-graph-cli/src/bin/reality-loop.rs` |
| Official SWE-bench Docker readback | `target/debug/reality-loop` invoking the official SWE-bench harness directly |
| Governed SWE agent tool gateway | `crates/context-graph-cli/src/bin/reality-loop.rs` |
| Deterministic reality prompt compiler | `crates/context-graph-cli/src/bin/reality-loop.rs` |
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

The current model-facing tool surface is governed by ContextGraph. Early Phase 1 tools were read-only, and the current Rust loop adds `apply_structured_edit` as the only model-facing candidate patch submission path. Patch mutation still enters only through ContextGraph's verified patch/evaluator path.

## Phase 2 Implementation Snapshot

Phase 2 prompt reinjection is now owned by the Rust loop. The previous Python prompt compiler was removed so there is one implementation path.

Current source-of-truth artifacts:

- compiler implementation: `crates/context-graph-cli/src/bin/reality-loop.rs`;
- initial prompt manifest: `<attempt_root>/prompt-manifest.json`;
- refinement prompt candidate: `<task_root>/prompt-candidates/attempt-NNN/`;
- compiler readback files: Rust prompt, request, response, parse report, and failure-evidence artifacts;
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

The Rust binary is the only supported SWE-bench reality loop:

```bash
cargo build -p context-graph-cli --bin reality-loop
target/debug/reality-loop \
  --model gemma4:e4b \
  --task-manifest /tmp/contextgraph-swe-lite-refinement/swe_lite_refine_1777940680618314859_801dcad0/tasks/psf__requests-2317/qwen3-8b/task-manifest.json \
  --runtime-root /tmp/contextgraph-rust-reality-loop \
  --official-eval-each-attempt \
  --run-until-resolved
```

This mode requires official SWE-bench evaluation and stops only when the official report places the instance id in `resolved_ids`. Candidate failures become next-prompt evidence. Infrastructure, model-runtime, ledger, compiler, or official-harness failures still fail closed with typed Rust errors.

The Rust binary uses Rust-owned orchestration, typed failure artifacts, JSON readback checks, SQLite ledger readbacks, GPU VRAM budget checks through `nvidia-smi`, a runtime `measurement-contract.json`, and BlackBoxFix reward/evidence compilation. The official SWE-bench evaluator remains an external Python/Docker verifier because SWE-bench itself is distributed that way; ContextGraph no longer owns a Python implementation of the refinement loop.

The Rust loop now fails closed if official SWE-bench readback is disabled. Local patch validation can produce useful failure evidence, but it cannot become a SWE success claim.

Current hardening adds two first-class model-facing reality packets:

- `<run_root>/problem-reality-packet.json` captures the durable reality of the SWE problem before attempt 1: dataset, instance id, base commit, FAIL_TO_PASS/PASS_TO_PASS oracle, problem terms, candidate production files, source/test boundary, and root-cause discovery protocol.
- `<attempt_root>/reality-signal-packet.json` captures each attempt's verified outcome: validation result, official test progress when available, failure class, patch statistics, tool grounding, BlackBoxFix score/reward, ledger counts, and reinjection directive for the next prompt.

The final candidate-patch handoff is now tool-mediated. The model must call `apply_line_window_edit` or `apply_structured_edit`; ContextGraph compiles the edit into a patch artifact, reads the patch file and SHA-256 back from disk, and then validates that artifact. The old second-step raw JSON final answer contract has been removed from the live path.

Gemma coding-agent mode is now explicit. In this architecture, the governed coding-agent harness is the inner reality loop itself: it is the sensor/actuator layer where Gemma reads problem reality, navigates source, submits edits, receives verified failures, and gets reward-facing signals for the next attempt.

- Ollama request options are persisted in each tool-round request: `temperature=0`, `top_p=0.9`, `num_ctx=32768`, `num_predict=4096`.
- Default tool-round budget is `8` so Gemma has enough turns to inspect, navigate, open a source window, and submit a patch.
- `apply_line_window_edit` is the preferred edit tool because it replaces line ranges from an `open_file_window` readback and requires the observed file SHA-256.
- Every governed tool call now writes `harness-transition.json`, reads it back, and has BlackBoxFix verify it with the request/result artifacts.
- Reality-signal packets now include `harness_reality` and `tool_reward_signals`, which score intermediate coding-agent behaviors before official SWE success exists.
- `--smoke-test --max-attempts 1 --openai-optimizer-each-attempt` is the required short path for validating the Gemma inner loop plus OpenAI outer optimizer before a long `--run-until-resolved` execution.

Latest reality-signal FSV output:

```text
/tmp/contextgraph-tool-mediated-fsv-10/rust_reality_1777970781993552749_69ff234d/
```

That run used the real `scikit-learn__scikit-learn-25638` SWE-bench Lite task and real Gemma/Ollama output. Gemma inspected reality through governed tools but did not submit a candidate patch through `apply_structured_edit`. The loop persisted and read back the problem packet, tool transcript, failed validation, reality-signal packet, reward compilation, BlackBoxFix ledger rows, and final summary instead of claiming success.

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
