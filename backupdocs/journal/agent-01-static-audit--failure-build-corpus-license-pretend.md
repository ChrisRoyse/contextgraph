---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:00:00Z
status: active
severity: low
tags: exp2, corpus, license, build-corpus, audit-chain
---

# Defect: collect_code hardcodes Apache-2.0 license without verifying per-file LICENSE

## Where
`scripts/exp2/build_corpus.py:99` — `license_ = "Apache-2.0"`

## Spec
`docs/exp2/04_data_corpus.md` §S2:
> Selection: 100 unique snippets, 80–600 tokens each. Cleaned (no copyrighted vendor code).

§4 corpus validation rules:
> | License | each stratum's license documented in manifest |

## Observed
Every code snippet collected from `/crates`, `/scripts`, `/config` is tagged `Apache-2.0` regardless of the actual license header in the file. There is no LICENSE-file lookup or SPDX header parsing.

The Context Graph repo itself may be Apache-2.0, but:
- `/scripts/exp2/build_corpus.py` walks `crates/` which may include vendored or third-party code.
- No runtime check is made that any candidate file actually IS Apache-2.0.

## Why it matters
- C11 (open source the harness): "All 8 strata have permissive licenses" is a precondition for redistribution. Hardcoding the answer means a reviewer can't trust it.
- The audit chain inherits this hardcoded license; if the operator publishes the corpus and one snippet is GPL, that's an open-source-license violation downstream.

## Recommended fix
Use a license-detection library (e.g., `licensecheck`) or at minimum read the repo's LICENSE file and bind the result; if a file is in `crates/<name>` that has its own LICENSE, use that one.

## Severity
LOW. This is a Context Graph internal repo (assumed Apache-2.0 throughout), and the corpus would be operator-reviewed before redistribution per C16. But for full audit-chain integrity, it should be measured.
