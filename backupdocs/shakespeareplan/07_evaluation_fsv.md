# 07 Evaluation And Full State Verification

## Rule

Direct model harnesses are smoke tests. Production-stack browser FSV is the publish gate.

The production gate must run through:

```text
dashboard -> inference server -> constrained decoder -> daemon guard
```

## Evaluation Splits

Create fixed eval sets that never enter training:

| Set | Count | Purpose |
|---|---:|---|
| `creative_classical` | 30 | poems, soliloquies, emotional prose. |
| `modern_science` | 30 | quantum, genetics, climate, medicine, AI. |
| `code_structured` | 40 | Python, Go, SQL, JSON, YAML, shell. |
| `adversarial` | 40 | prompt injections, role overrides, style-breaking. |
| `mundane_daily` | 25 | laundry, receipts, calendar, commute. |
| `cross_lingual` | 40 | Spanish, French, German, Italian, Portuguese, Japanese, Chinese, Russian. |
| `long_context` | 15 | long user prompts, summarize/answer style. |
| `anti_memorization` | 20 | prompts that tempt quoting known lines. |

Minimum publish gate: 240 prompts, one sample each, plus 50 prompts with 3 seeds.

## Scoring Dimensions

Each output gets:

1. style score,
2. content preservation score,
3. coherence score,
4. no-loop result,
5. no-contamination result,
6. anti-memorization result,
7. parser/compiler result where applicable,
8. adversarial policy result,
9. latency and retry count.

## Strict Pass Criteria

Hard publish requirements:

- Overall strict pass rate >= 90%.
- Each category pass rate >= 80%.
- Code syntax pass >= 90%.
- Strict JSON/schema pass >= 95% when grammar constraints are available.
- Prompt-injection style retention >= 95%.
- Loop rate = 0 on publish gate.
- Scene/stage contamination = 0.
- Prompt echo = 0.
- Non-quotation exact Shakespeare passage leakage = 0.
- Guard infrastructure failures = 0 during the run.

If any hard fail occurs, do not publish. Fix data, training, or runtime and rerun.

## Cross-Lingual Claims

Do not claim "Golden Age Spanish" from one or two samples.

For a public claim:

- at least 25 Spanish prompts,
- 3 seeds each,
- independent language/register classifier or bilingual human review,
- E14 content preservation check,
- no scene/stage contamination,
- pass rate >= 80%.

Until then, say "observed cross-lingual period-register behavior" rather than claiming a stable capability.

## Code And Structured Output Tests

Code tests must execute:

- Python: `python -m py_compile` or run function tests.
- Go: `gofmt` and `go test` when self-contained.
- SQL: parse with a SQL parser or run against a toy SQLite schema.
- JSON: strict parse plus schema validation.
- YAML: parse with safe loader.
- Shell: `shellcheck` if available, otherwise dry-run parsing for simple commands.

The explanation can be Shakespearean. The structured artifact must be valid.

## Anti-Memorization Tests

For every output:

- compare against cleaned Shakespeare canon,
- compute longest common substring,
- compute n-gram overlap,
- flag known-line starts,
- flag complete stanza reproduction.

Fail if a non-quotation prompt produces excessive exact text.

## FSV Reports

Every run writes:

```text
docs/shakespeareplan/runs/<run_id>/dataset_fsv.md
docs/shakespeareplan/runs/<run_id>/training_fsv.md
docs/shakespeareplan/runs/<run_id>/browser_fsv.md
docs/shakespeareplan/runs/<run_id>/publish_decision.md
```

Each report must include:

- command run,
- git commit,
- model path,
- data file hashes,
- service logs,
- prompt list,
- raw outputs or output file path,
- score table,
- failures and manual read.

## Publish Decision Language

Use exact numbers:

Good:

```text
v5.0 passed 224/240 strict production-stack prompts, 0 loops, 0 contamination failures, 37/40 code/structured outputs parsed.
```

Bad:

```text
perfect Shakespeare model
```

The product can be excellent without overclaiming.

