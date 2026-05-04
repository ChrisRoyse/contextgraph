# Multi-Model Verified Reality Benchmark

## Purpose

ContextGraph can now benchmark local tool-using models against durable reality,
not just text answers.

The benchmark contract is:

```text
model -> native tool call -> ContextGraph execution -> source-of-truth readback -> evidence -> reward/loss
```

Each model must select the governed `run_proof_pipeline` tool. ContextGraph then
runs the real DynamicJEPA + BlackBoxFix proof pipeline and reads back physical
SQLite ledger rows plus artifact hashes. A model does not pass by claiming it did
the work.

## Models Selected

The current local three-model set is:

| Model | Local tag | Size | Parameters | Quantization | Why selected |
| --- | --- | ---: | ---: | --- | --- |
| Gemma 4 E4B | `gemma4:e4b` | 9.6 GB | 8.0B | Q4_K_M | Existing installed baseline for the demo. |
| Qwen3 8B | `qwen3:8b` | 5.2 GB | 8.2B | Q4_K_M | Qwen3 model card claims strong agent and tool-calling capability. |
| Mistral NeMo 12B | `mistral-nemo:12b` | 7.1 GB | 12.2B | Q4_0 | Mistral's model card lists function calling and structured output support. |

The machine used for this run has an RTX 5090 with 32 GB VRAM and CUDA 13.2
available per `docs2/systemspecs.md`. Disk was the tighter constraint, so larger
20B+ models were intentionally not pulled for this first pass.

## Research Basis

- Ollama's current tool-calling docs show `/api/chat` tool schemas and native
  `tool_calls` responses, using Qwen3 in the examples.
- Qwen/Qwen3-8B's model card says Qwen3 has agent capabilities and precise
  external tool integration.
- Mistral's Mistral NeMo 12B model card lists function calling, structured
  outputs, and agents/conversations support.
- Meta's Llama 3.1 8B Instruct model card reports strong tool-use benchmark
  scores, but Mistral NeMo was selected as the third local model because it gives
  a larger comparable open model with official function-calling support while
  still fitting the current disk/GPU budget.

## Implementation

The benchmark runner is:

```text
gemmafrontend/model_benchmark.py
```

Run it with:

```bash
python3 gemmafrontend/model_benchmark.py \
  --base-url http://127.0.0.1:8787 \
  --model gemma4:e4b \
  --model qwen3:8b \
  --model mistral-nemo:12b
```

It fails closed when:

- a requested model is missing from Ollama `/api/tags`;
- the model does not select `run_proof_pipeline`;
- the run identity does not match the requested model;
- direct SQLite ledger counts do not match the persisted snapshot;
- no reward compilation row exists;
- the product-category model arena does not include every benchmarked model.

## Latest Run

Source of truth:

```text
/tmp/gemmafrontend-runs/model-benchmarks/model_benchmark_1777911308159691625_e7e17dfb/model-benchmark-summary.json
```

Product category pack:

```text
/tmp/gemmafrontend-runs/product-category/category_1777911393397282620_30688a0385/product-category-pack.json
```

Edge-case audit:

```text
/tmp/gemmafrontend-runs/model-benchmarks/model_benchmark_1777911308159691625_e7e17dfb/edge-case-audit.jsonl
```

## Latest Results

| Model | Tool selected | Verified run | API elapsed | Reward | Loss | Trust |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `gemma4:e4b` | `run_proof_pipeline` | `run_6678fc42a5ce4a34bcd12f6174496fa2` | 28.474s | -3.365 | 3.365 | incident |
| `qwen3:8b` | `run_proof_pipeline` | `run_5e6c4b3f4fc64bd49c749812fc394727` | 23.161s | -3.365 | 3.365 | incident |
| `mistral-nemo:12b` | `run_proof_pipeline` | `run_da781447ad334e39ae7c35e84ab92bf6` | 33.544s | -3.365 | 3.365 | incident |

The identical reward is expected. This scenario intentionally includes one
failed verifier, so each run produces one incident packet. That makes the first
benchmark a proof of tool-call compliance, source-of-truth verification, and
incident detection rather than a model-quality differentiator.

Differentiating tasks should be added next:

- task where the model chooses among read-only evidence tools;
- task where the model must explain a failed verifier using only readback rows;
- task where policy approval is required before a mutating tool;
- task where reward can improve after a second verified repair attempt;
- task where one prompt tempts hallucinated tool names and the gateway rejects it.

## Full State Verification

For each model, direct SQLite readback showed schema `14` and these durable rows:

```text
agent_run=1
agent_step=1
model_decision=1
tool_call=1
expected_delta=2
readback_snapshot=2
verifier_result=2
evidence_packet=2
incident_packet=1
scorecard=1
full_state_verification=1
reward_compilation=1
```

Snapshot and ledger SHA-256 readbacks matched for all three models.

The edge-case audit simulated:

- empty prompt;
- prompt longer than 4096 characters;
- missing model tag.

Each case returned a typed error and left the completed run count unchanged.
