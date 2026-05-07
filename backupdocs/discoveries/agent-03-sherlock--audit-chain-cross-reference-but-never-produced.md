---
namespace: discoveries
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:18:00Z
status: active
severity: medium
tags: exp2, audit-chain, b2, write-distribution, dangling-reference
---

# AuditChain wiring: 1 producer + 1 consumer, NEVER linked

## Producers (where audit_chain.json could be created)
```
$ grep -n "AuditChain" scripts/exp2/*.py
scripts/exp2/audit_chain.py:12:class AuditChain:
scripts/exp2/verify_environment.py:15:from scripts.exp2.audit_chain import AuditChain
scripts/exp2/verify_environment.py:98:    chain = AuditChain(cfg["run_id"], out / "audit_chain_environment.json", ...)
```

Single instantiation site. Output filename hardcoded to `audit_chain_environment.json` (NOT `audit_chain.json`).

## Consumers (where audit_chain.json is referenced)
```
scripts/exp2/write_distribution.py:38: ... and `artefacts/exp2/audit_chain.json`
```

Bare-text reference in a Markdown blog template that is OUTPUT by `write_distribution.py`. The script doesn't programmatically read `audit_chain.json` — it just embeds the path string into the generated paper-patch text.

## Impact

Net: agent-01 B-2 + agent-02 refinement are both correct.
- The chain class exists.
- It is invoked exactly once (only for env verifier).
- The master `audit_chain.json` is never produced, even though the published paper-patch points to it.
- A reader following the paper to `artefacts/exp2/audit_chain.json` would find a 404.

## Cross-link

- Agent-01 blocker: `memory/blockers/agent-01-static-audit--no-audit-chain-built-anywhere.md`
- Agent-02 refinement: `memory/journal/agent-02-tester--verified-blocker-b2-audit-chain-instantiated-only-in-verify-environment.md`
- Spec C7 (audit chain mandatory): `docs/exp2/01_constitution.md`

## Verdict

This is a wiring bug, not a logic bug. The fix per agent-01 recommendation is:
1. `run_exp2.py` constructs an AuditChain at the start.
2. Each phase script appends events.
3. Final phase calls `chain.finalize()` and writes the master `audit_chain.json`.

Estimated effort: 30-60 minutes; touches 4-5 scripts. The simplifier should treat this as Tier 1.

## Confidence: HIGH
