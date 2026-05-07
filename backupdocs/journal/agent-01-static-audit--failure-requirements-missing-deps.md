---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:32:00Z
status: active
severity: medium
tags: exp2, requirements, dependencies, spec-mismatch
---

# Defect: requirements.txt missing pydantic, click, mcp, hypothesis, pytest-cov, npeet

## Where
`scripts/exp2/requirements.txt`

## Spec
`docs/exp2/05_technical_spec.md` §3 dependencies list:
```
numpy>=1.26
scipy>=1.11
scikit-learn>=1.4
torch>=2.2
torchvision>=0.17
npeet @ git+https://github.com/gregversteeg/NPEET.git@master  # KSG
matplotlib>=3.8
seaborn>=0.13
pyyaml>=6.0
pydantic>=2.5
pandas>=2.1
pyarrow>=15.0
datasets>=2.16
pytest>=8.0
pytest-cov>=4.1
hypothesis>=6.95
click>=8.1
tqdm>=4.66
mcp>=1.0
```

## Observed
`requirements.txt` contains only:
- numpy, scipy, scikit-learn, torch, matplotlib, seaborn, pyyaml, pandas, pytest, datasets, tqdm

Missing per spec:
- torchvision (used? Not in any imports — OK to drop)
- npeet (replaced by sklearn_direct in implementation; OK to drop, but document)
- pydantic (no pydantic models in code; OK to drop)
- pyarrow (used by pandas internally; conditional)
- pytest-cov (coverage reporting; missing)
- hypothesis (property-based testing; not used in code, missing)
- click (no @click.command in code; tech-spec called for click CLI but implementation uses argparse; OK to drop)
- mcp (this is the Python `mcp` MCP-protocol package; *not* used; the implementation rolls its own JSON-RPC client. OK to drop, but document.)

## Why it matters
- C5: pre-registered hyperparams/dependencies must match. The spec's `requirements.txt` enumerated specific versions; the implementation's deviates without a documented reason.
- An open-source replicator running `pip install -r requirements.txt` succeeds but doesn't get all the tools the spec promised.

## Recommended fix
Either:
1. Add comments to `requirements.txt` explaining each omission ("# pydantic NOT used; spec ref only"; "# npeet replaced by sklearn_direct").
2. Update `05_technical_spec.md` §3 to match the actual minimal dependencies.

## Severity
MEDIUM. Won't crash anything; affects open-source release credibility.
