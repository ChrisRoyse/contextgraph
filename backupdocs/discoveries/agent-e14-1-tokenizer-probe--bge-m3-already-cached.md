---
namespace: discoveries
agent_id: agent-e14-1-tokenizer-probe
timestamp: 2026-04-22T15:30:00Z
status: active
tags: e14, bge-m3, tokenizer, hf-cache, xlm-roberta, sentencepiece
---

# Discovery: BAAI/bge-m3 already in HF cache — no download needed

## Summary

BAAI/bge-m3 is already fully downloaded in the local HuggingFace Hub cache
from prior Python v4.2 work. Agent 3 (model-impl) does NOT need to wire
up `hf-hub` crate or any HTTP downloader — can follow the existing pattern
of loading from a local path.

## Paths

**Hub root:** `~/.cache/huggingface/hub/models--BAAI--bge-m3/`

**Snapshots present:**
- `5617a9f61b028005a4858fdac845db406aefb181/` — the "full" snapshot with:
  - `tokenizer.json` (17,098,108 bytes) <- probe uses this
  - `tokenizer_config.json`
  - `special_tokens_map.json`
  - `sentencepiece.bpe.model` (raw SPM file)
  - `config.json`
  - `pytorch_model.bin`
  - `colbert_linear.pt` (dense/sparse/colbert heads all together)
  - `sparse_linear.pt`
  - `1_Pooling/`, `onnx/`, `modules.json`, etc.
- `9a0624b896d81da7492a910ffa53731274b6cf3d/` — lighter snapshot, only `model.safetensors`

**For Agent 3 (model-impl):** the safetensors snapshot is the one to use
for weight loading (HF's "safetensors" revision). Tokenizer, config, and
any `.pt` head weights live in the first snapshot.

## XLM-RoBERTa tokenizer details (verified)

- **Format:** HF `tokenizer.json` v1.0 (standard tokenizers-crate loadable)
- **Vocab size:** 250,002 (confirms XLM-RoBERTa-Large base)
- **Special tokens (verified by probe):** `<s>`=0, `<pad>`=1, `</s>`=2, `<unk>`=3
- **Tokenization style:** SentencePiece BPE with `▁` prefix for word-initial
  subwords (XLM-RoBERTa convention)
- **Example:** `"Shall I compare thee to a summer's day?"` →
  `["<s>", "▁S", "hall", "▁I", "▁compare", "▁the", "e", "▁to", "▁a", "▁summer", "'", "s", "▁day", "?", "</s>"]`
  (15 tokens total)

## Implication for Task 7 (tokenizer wrapper)

The tokenizer itself is plug-and-play with the existing v0.22 `tokenizers`
crate — no custom SentencePiece integration needed. Agent 3 can mirror
`semantic/loader.rs` line 55 verbatim with only the path changed.

Max length handling: the tokenizer file has `"truncation": null` and
`"padding": null`, so the loader will need to configure truncation at 8192
via `tokenizer.with_truncation(Some(TruncationParams { max_length: 8192, ... }))`
the same way existing BERT-family loaders do.
