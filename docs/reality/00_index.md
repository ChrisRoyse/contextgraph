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

## Current Project Mapping

| Capability | Current Location |
|---|---|
| Local Gemma/Qwen/Mistral frontend and tool harness | `gemmafrontend/` |
| Verified reward compiler spec | `gemmafrontend/reward_spec.verified_outcome.json` |
| SWE-bench local refinement loop | `gemmafrontend/swe_lite_refinement_loop.py` |
| Official SWE-bench Docker readback bridge | `gemmafrontend/swe_lite_official_eval.py` |
| Smoke benchmark suite definition | `gemmafrontend/swe_smoke_benchmark_suite.json` |
| Current SWE-bench status doc | `docs/contextgraph_swe_bench_refinement_loop.md` |
| BlackBoxFix ledger/evidence CLI | `target/debug/context-graph blackbox ...` |
| DynamicJEPA state/action/surprise machinery | existing ContextGraph/DynamicJEPA modules and generated run artifacts |

## Research Sources

- Ralph Loop docs and repository: https://ralphloop.sh/ and https://github.com/pageai-pro/ralph-loop
- Anthropic Ralph Loop plugin page: https://claude.com/plugins/ralph-loop
- Karpathy AutoResearch repository: https://github.com/karpathy/autoresearch
- Karpathy Software 2.0 essay: https://karpathy.medium.com/software-2-0-a64152b37c35
- SWE-agent paper: https://arxiv.org/abs/2405.15793
- SWE-agent ACI docs: https://github.com/SWE-agent/SWE-agent/blob/main/docs/background/aci.md
- SWE-bench Lite and evaluation docs: https://www.swebench.com/lite.html and https://www.swebench.com/SWE-bench/guides/evaluation/
- Reflexion paper: https://arxiv.org/abs/2303.11366
