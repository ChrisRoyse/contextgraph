# 06 Runtime Guard And Decoding

## Design Rule

Training teaches preference. Decoding and guards enforce behavior.

The runtime stack should be:

```text
prompt
  -> canonical system prompt
  -> LoRA generation
  -> style-aware logit processor
  -> format-specific constrained decoder when needed
  -> multi-view guard
  -> accepted output or explicit failure
```

## System Prompt

Use a short, stable prompt family. Train on paraphrases but serve with one canonical prompt.

Canonical:

```text
You are a knowledgeable modern assistant who writes exclusively in the style of William Shakespeare. Answer every topic accurately, including modern science, code, math, technology, and ordinary life. Never claim ignorance of modern subjects. Never switch to another prose style. Use Shakespearean diction, cadence, metaphor, and rhetoric, while preserving requested facts and formats.
```

Avoid "You are William Shakespeare" because it induces historical ignorance.

## Style-Aware Logit Processor

Current hand-written boost/suppress lists should be replaced or supplemented with an E13/SPLADE-derived dictionary.

Principles:

- Boost style markers and period function words.
- Do not suppress modern topic nouns such as `API`, `server`, `database`, `Python`, `SQL`, `internet`.
- Apply stronger style boost in the first 40 tokens.
- Keep NaN/Inf guard.
- Track how often constraints fire.

## Format-Specific Decoding

For strict formats, the format wins inside the block.

| Request | Runtime behavior |
|---|---|
| JSON | Use JSON/schema constrained decoding if available. No Shakespeare prose inside strict JSON unless schema field allows it. |
| SQL | Generate parseable SQL. Shakespeare style belongs in surrounding prose or comments if comments are allowed. |
| Python/Go/etc. | Code must parse or compile. Comments may be Shakespearean. |
| Shell | Commands must be executable. Explanation may be Shakespearean. |

This avoids corrupting syntax with archaic diction.

## Multi-View Agreement Guard

Replace a single scalar style score with a conjunction plus vote.

### Required Checks

Hard fail:

- contamination regex,
- prompt echo,
- scene/stage text,
- exact passage overlap,
- loop detector,
- parser/compiler failure when requested.

Style vote:

- E12 style pass,
- E13 lexical pass,
- E9/HDC structure pass,
- E10 paraphrase-surface pass,
- E7 syntax/structure pass,
- optional extended Jina/Qwen style classifier if loaded.

Content vote:

- E1 or E14 semantic preservation,
- entity preservation where applicable,
- code/structured parse pass.

Suggested policy:

```text
hard_fail == false
and style_vote >= 4 of 5
and content_vote >= required_by_prompt
```

The exact thresholds must be calibrated on v2.1 accepted outputs, real Shakespeare held-out, modern controls, and v4.1 loop failures.

## Anti-Loop Checks

Use multiple cheap detectors:

- repeated 4-gram and 8-gram counts,
- longest repeated substring,
- repeated line detector,
- type-token ratio floor,
- sentence similarity across adjacent windows,
- E12 self-MaxSim between windows,
- EOS failure after natural conclusion.

Any loop failure should trigger regeneration with:

- lower `max_tokens`,
- higher `repetition_penalty`,
- adjusted temperature,
- or forced EOS if the answer is already complete.

Do not return an archaic loop just because style score is high.

## Guard Error Policy

No silent pass.

| Failure | Response |
|---|---|
| daemon unreachable | 503 infrastructure error |
| schema mismatch | 500 schema error |
| guard verdict fail after retries | 503 style guard failure with score trace |
| parser/compiler fail | retry, then structured-output failure |

The user should never see `PASS score=0.000 guard=0.1ms` again.

## Best-Of-K

Use best-of-K only as runtime selection, not recursive training data.

Suggested:

```text
K = 3 for normal chat
K = 5 for adversarial or structured prompts
```

Select by full reward:

```text
style + content + format - repetition - memorization
```

Log the score trace for every candidate.

