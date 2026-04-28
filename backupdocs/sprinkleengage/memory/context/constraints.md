---
namespace: context
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T00:00:00Z
status: active
tags: constraints, tone, scope, smap
---

# Global Constraints (non-negotiable)

## Scope Preservation (paper §8 limitations)
1. **Case 3 scope**: encoder-matched (WavLM-base-plus-sv guard + WavLM-base-plus-sv evaluator),
   N=10 held-out English sentences, one speaker, single session, single encoder family.
   **Never claim cross-encoder identity verification** without ECAPA-TDNN re-score (EXP-1 pending).
2. **DDA counting identity**: "up to n·(N+C(N,2))" — **constructive upper bound**, tightness
   pending EXP-2 pairwise mutual-information audit across the 13-embedder panel.
3. **Collapse-silent scope**: DDA is *structurally outside* Shumailov-form recursion regime by
   construction (no generator-in-loop). This is a scope argument, **not a refutation**.
4. **Meaning compression = fourth category**: semantically distinct unit pair (signals/raw-data-unit
   at dataset prep vs. bits/params/memory at inference). **Not claimed superior** to bit/weight/
   activation compression — reviewer acceptance of the taxonomic distinction is flagged as a
   scholarly judgement we do not pre-empt.
5. **TCT vs scalar-reward**: different property class (per-output verifiability relative to chosen
   centroid panel). **Not a benchmark contest** against RLHF/DPO/Constitutional AI.
6. **Case 2 Santa**: **MEASURED and operational** as of 2026-04-19 (Chris confirmed).
   ClipCannon+Phoenix full stack runs the constellation-guarded clone-video pipeline end-to-end:
   16-min source → 7-modality constellation (4,044D, vis/sem/emo/spk/pros/sent/voice) → EchoMimicV3
   LoRA (rank 256, α 512, 8 modules, 2,362 curated clips, mouth-weighted flow-matching, 6 forensic
   fixes applied) → runtime ConstellationGuard (per-frame SigLIP re-embedding + cosine threshold).
   **The paper draft §6.2 labels Case 2 "projected" because that paper revision is paper-internal;
   the operational stack Chris ships treats Case 2 as measured and demonstrable.** Subject
   artefacts (clips/weights/constellation) remain WITHHELD under dual-use posture.
7. **Case 1 Shakespeare**: MEASURED 2026-04-17; SFT loss 1.12 (98.9% token acc), DPO loss 0.47
   (98.0% rewards/acc, margins 43.62); LoRA published at huggingface.co/cabdru/shakespeare-lora-gemma4.

## Dual-Use / Release Posture (paper Ethics)
- **Case 1 (Shakespeare)**: byte-identical release — paper + LoRA + DDA-projected corpus + three-
  layer enforcement stack.
- **Case 2 (Santa identity-locked video)**: protocol-level only; subject clips/weights/constellation
  WITHHELD.
- **Case 3 (voice clone)**: protocol-level only; subject reference clips/centroid/audio WITHHELD.

## Competing Interests Disclosure (must appear in every deliverable footer)
> Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP
> marketplace). This package emerges from commercial engineering practice. Readers should weight
> content as a methodology contribution with acknowledged commercial affiliation.

## File Discipline
- Max 1500 lines per file; split to `_part1.md`, `_part2.md` with cross-references.
- YAML frontmatter required on every .md file under `memory/` and `research/`.
- Every factual external claim cites a dated, accessible URL with Tier 1/2/3 classification.
- Every paper-derived claim cites paper section (e.g., "Paper §5.4 G_τ predicate").
- MEASURED/PROJECTED tags on every Case-derived number.

## Tech Stack Decisions
- Docs location: `/home/cabdru/contextgraph/docs/sprinkleengage/research/` (numbered sub-dirs).
- Memory location: `/home/cabdru/contextgraph/docs/sprinkleengage/memory/` (SMAP 8 namespaces).
- No backwards compatibility — this is a new build. If something doesn't work, fail fast and log.

## Anti-Patterns (never do these)
1. Overclaiming Case 3 as cross-encoder verified.
2. Quoting "91 signals" without "up to" and pending-audit caveat.
3. Saying DDA "refutes" collapse.
4. Commercial TAM claim from Teleox.
5. Claiming TCT "beats" RLHF on benchmarks.
6. Citing paper sections the paper doesn't support.
7. Fabricating executive quotes or sources — if not verifiable, flag.
8. Silent failures — every failure goes to `./memory/journal/` or `./memory/blockers/`.
