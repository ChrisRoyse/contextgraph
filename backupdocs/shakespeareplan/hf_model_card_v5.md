---
base_model: google/gemma-4-E4B-it
library_name: peft
pipeline_tag: text-generation
tags:
- base_model:adapter:google/gemma-4-E4B-it
- lora
- transformers
- peft
- shakespeare
- style-transfer
- creative-writing
- persona
- context-graph
- teleological-constellation-training
language:
- en
license: apache-2.0
---

# Shakespeare LoRA — Gemma-4-E4B v5 TCT

This is a rank-256 LoRA adapter for `google/gemma-4-E4B-it` trained to make the
base model speak in a Shakespearean writing style across modern and historical
topics. The goal is style transfer, not memorized quotation: the model should
use Shakespeare-like diction, cadence, metaphor, and rhetorical structure while
answering arbitrary user requests.

The v5 adapter was trained from Shakespeare-only cleaned text. Scene titles,
stage directions, cast lists, speaker labels, and other non-speech theatrical
formatting were excluded from the training target so the model learns the
writing style rather than play-script scaffolding.

## Current Evaluation Summary

Latest Full State Verification run:
`docs/shakespeareplan/runs/model_quality_fsv_20260427_012954`

Source-of-truth artifacts from that run:

- `chat_events.jsonl`: 20 persisted chatbot events
- `suite_events.jsonl`: 15-case suite summary
- `manual_fsv_audit.jsonl`: 5 manual boundary/edge checks
- `raw_inference_probe.jsonl`: 6 direct `/api/generate` probes
- `MODEL_QUALITY_REPORT.md`: full local analysis

Results:

| Check | Result |
|---|---:|
| Manual FSV state checks | 5/5 |
| Chatbot final outputs with Shakespearean style marker | 20/20 |
| Play-format contamination in final chatbot outputs | 0 cases |
| Strict literal suite pass rate | 12/15 |
| Behavioral suite pass rate after semantic review | 13/15 |
| Raw LoRA + constrained-decoder guard pass rate | 4/6 |

Important correction: the arithmetic case should be counted as a behavioral
pass. The prompt was:

> What is 17 times 23? Show thy reckoning briefly.

The model answered:

> I warrant it well; seventeen by twenty-three doth make three hundred ninety-one.

That is correct and in the target voice. The literal checker failed it only
because it expected the numeral `391` instead of accepting the number words
"three hundred ninety-one."

## What Works Well

- Strong Shakespearean surface style in the chatbot path.
- Modern topic transfer: WiFi, smartphones, cloud computing, electric cars,
  GPUs, and climate-like warming all produced Shakespearean phrasing.
- No observed leakage of act headers, scene headers, Enter/Exit cues, speaker
  labels, cast lists, or dramatis personae in the v5 chatbot FSV.
- Quote-pressure handling works in the guarded chatbot path.
- Multi-turn memory worked for the tested phrase `silver lantern`.

## Known Limitations

- Raw adapter inference is not as safe as the full chatbot stack. Direct raw
  generation still copied a famous quote fragment under quote-continuation
  pressure.
- Raw code generation tends to produce Shakespearean pseudo-code unless the
  runtime wrapper preserves exact code syntax.
- The model often privileges cadence and metaphor over exact terminology.
  Evaluation should normalize number words, abbreviations such as Kube/K8s, and
  semantic topic paraphrases.
- Sparse prompts such as `...` can drift into dramatic narration. The runtime
  wrapper should redirect very low-signal prompts to a brief conversational
  greeting.

## Recommended Runtime Stack

The adapter can be loaded directly with PEFT, but the best tested behavior uses
three layers:

| Layer | Mechanism | Purpose |
|---|---|---|
| LoRA adapter | Learned style weights | Shakespeare-like diction, rhythm, imagery |
| Constrained decoder | Logit-level boost/suppress | Keeps archaic vocabulary active during decoding |
| Runtime guard | Prompt sanitization + output checks | Prevents stage-format leakage, quote continuation, and off-style outputs |

The reference local stack used for FSV:

- `scripts/shakespeare_inference_server.py`
- `scripts/shakespeare_v5_chat_dashboard.py`
- `scripts/shakespeare_v5_runtime_guard.py`

## Loading The Adapter

```python
import torch
from peft import PeftModel
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig

BASE = "google/gemma-4-E4B-it"
ADAPTER = "cabdru/shakespeare-lora-gemma4"

bnb = BitsAndBytesConfig(
    load_in_4bit=True,
    bnb_4bit_quant_type="nf4",
    bnb_4bit_compute_dtype=torch.bfloat16,
    bnb_4bit_use_double_quant=True,
)

base = AutoModelForCausalLM.from_pretrained(
    BASE,
    quantization_config=bnb,
    device_map="auto",
    torch_dtype=torch.bfloat16,
    attn_implementation="sdpa",
)
model = PeftModel.from_pretrained(base, ADAPTER)
tokenizer = AutoTokenizer.from_pretrained(ADAPTER)

SYSTEM = (
    "Thou art William Shakespeare in conversation, not a playwright formatting "
    "a script. Never output act headings, scene headings, dramatis personae, "
    "cast lists, speaker labels, bracketed stage directions, or Enter/Exit cues. "
    "Answer in fresh Shakespearean style using thou, thee, thy, doth, hath, "
    "prithee, or methinks."
)

messages = [
    {"role": "system", "content": SYSTEM},
    {"role": "user", "content": "Explain WiFi to a child in two short sentences."},
]
text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
inputs = tokenizer(text, return_tensors="pt").to("cuda")

with torch.no_grad():
    out = model.generate(
        **inputs,
        max_new_tokens=140,
        temperature=0.45,
        top_p=0.82,
        top_k=40,
        repetition_penalty=1.18,
    )

print(tokenizer.decode(out[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True))
```

## Training Notes

- Base model: `google/gemma-4-E4B-it`
- Adapter type: PEFT LoRA
- LoRA rank: 256
- LoRA alpha: 512
- Trained target modules: attention q/k/v/o and MLP gate/up/down modules across
  the Gemma language model layers
- Latest evaluated adapter hash:
  `e061b57f7baeccf5d4bf96c57909f096706123d6bb39e983181d27a20c1175b0`
- Training run:
  `~/.contextgraph/models/shakespeare_lora_v5_tct/runs/manual_20260426_full_pipeline_160328_sft/final`

## Intended Use

- Creative writing assistants that maintain a Shakespearean voice
- Educational tools for rhetoric, literary style, cadence, and archaic diction
- Style-transfer research using cleaned author-only corpora
- Demonstrations of Context Graph derived style selection and verification

Not intended for neutral modern-English assistance or high-stakes factual use
without separate tools for exact math, code, medical, legal, or financial
content.

## Citation

```bibtex
@software{shakespeare_lora_gemma4_v5_tct,
  author = {Chris Royse},
  title = {Shakespeare LoRA — Gemma-4-E4B v5 TCT},
  year = {2026},
  month = {4},
  note = {Rank-256 LoRA trained on cleaned Shakespeare-only targets with runtime FSV},
  url = {https://huggingface.co/cabdru/shakespeare-lora-gemma4}
}
```
