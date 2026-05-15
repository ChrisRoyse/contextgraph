# E14 - BGE-M3 Dense Semantic Role

## What E14 Is

E14 is the BGE-M3 dense head used as the multilingual semantic anchor in the
ME-JEPA-Code embedder panel. The runtime model identity is `BAAI/bge-m3`. The
dense vector is produced by XLM-RoBERTa-Large, uses the CLS token
representation, and is normalized into a 1024-dimensional vector. Its intended
context window is 8192 tokens.

E14 is an active content embedder. It is not a fallback, not a mock, and not a
zero-vector substitute. If the BGE-M3 snapshot is absent or malformed, provider
construction must fail closed before prediction or corpus processing begins.

## Semantic Role

E14 contributes a cross-language meaning coordinate to the frozen target-side
instrument panel. It helps ME-JEPA-Code compare code, comments, claims,
documentation, and test intent when surface language, wording, or document
length differs.

The role is especially important for:

- multilingual meaning preservation
- Cross-lingual paraphrase and translation detection
- long-context semantic matching beyond shorter dense embedders
- semantic agreement or disagreement with E1, E7, E10, E12, and E13
- claim-to-reality checks where the words changed but the intended behavior did
  not

## Fusion Profiles

The E14-heavy profiles are deliberately named so tests and operators can verify
that the role is wired into retrieval and predictor evidence:

- `multilingual_search`: E14 is a primary signal when language boundaries or
  non-English text may hide relevant evidence.
- `long_context`: E14 carries the long-document semantic signal where shorter
  encoders would truncate.
- `translation_finder`: E14 is the dominant signal for translation pairs and
  cross-language near duplicates.

## What E14 Does Not Prove

E14 does not prove code works. It is one frozen instrument in the panel, not the
verdict. It does not replace AST instruments, oracle outcomes, witness-chain
readback, import checks, type-graph features, or durable source-of-truth
verification.

E14 also does not capture exact token overlap as directly as sparse embedders,
and it does not model execution semantics by itself. Its value is strongest when
its similarities and disagreements are correlated with the other frozen
instruments.

## ME-JEPA-Code Use

For Python-first ME-JEPA training, E14 supplies a stable semantic axis for each
AST chunk and patch-context tuple. Useful reward signals include:

- E14 agrees with E7 code semantics while sparse/token embedders disagree:
  likely wording-level or identifier-level risk.
- E14 disagrees with E1/E10 on a claim: possible claim drift or description that
  does not match the edited bytes.
- E14 stays stable while AST/type/oracle instruments shift: likely semantic
  intent preserved but implementation reality changed.
- E14 moves with a patch while oracle outcomes fail: candidate failure-shape
  fingerprint for Q2 and Q3 heads.

These signals are only training inputs. The frozen target panel and durable
oracle readback remain the truth source.
