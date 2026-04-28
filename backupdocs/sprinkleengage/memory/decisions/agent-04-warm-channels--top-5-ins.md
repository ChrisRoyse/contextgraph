---
namespace: decisions
agent_id: agent-04-warm-channels
timestamp: 2026-04-19T13:30:00Z
status: active
tags: top-5-ins, warm-channels, decision, intro-paths
---

# Top 5 Intro Paths (probability × leverage)

Ranked by `response_probability × org_leverage × thesis-fit`. Only intros with verifiable public contact surface + direct thesis resonance are listed.

## #1 — Elvis Dohmatob (Concordia, ex-FAIR)
- **Path**: Direct email (e.dohmatob@meta.com legacy or Concordia faculty email).
- **Probability of reply**: VERY HIGH. Public on X (@dohmatobelvis), writes his own accessible posts about his papers, is a recently-appointed professor (hungry to build network).
- **Leverage**: Dohmatob is the *first author of "Strong Model Collapse" and "A Tale of Tails"* — the two most-cited 2024/2025 collapse papers after Shumailov's Nature piece. A positive technical reply from him is the single highest-value citation/endorsement.
- **Offer**: "We have a paper addressing *scope-of-applicability* of your model-collapse result to an external-supervision construction (DDA). Would you be willing to read a draft / co-author the empirical pairwise-MI audit (EXP-2)?"
- **Why #1**: highest response probability in the entire 45-person map; highest thesis alignment; lowest orthogonal risk.
- Source: [Concordia faculty](https://www.concordia.ca/faculty/elvis-dohmatob.html) · [arXiv 2410.04840](https://arxiv.org/abs/2410.04840)

## #2 — Omar Khattab (MIT EECS & CSAIL)
- **Path**: `okhattab@mit.edu` (public on his CV PDF at omarkhattab.com); alternative — submit to his Late Interaction Workshop @ ECIR 2026; alternative — DSPy GitHub issue/discussion.
- **Probability of reply**: HIGH. Academic, public X @lateinteraction, runs workshops explicitly for outside contributors. Just joined MIT Fall 2025 — still in network-building posture.
- **Leverage**: ColBERT + DSPy author; the paper's 13-embedder panel is *literally* a late-interaction fusion pattern. Khattab is the single best reader on earth for the multi-embedder framing. If he endorses, Stanford NLP (Potts + Zaharia) opens behind him.
- **Offer**: "Your late-interaction pattern generalizes to N>2 approximately-independent embedders. We quantify this as `n·(N+C(N,2))` structured supervision signals (paper §0.2). Would a workshop paper or ECIR submission interest you?"
- **Why #2**: High probability, and unlocks the strongest *academic-credibility* vector for the paper.
- Source: [omarkhattab.com/okhattab_cv.pdf](https://omarkhattab.com/okhattab_cv.pdf) · [lateinteraction.com](https://www.lateinteraction.com/)

## #3 — Ethan Perez (Anthropic, adversarial robustness lead) — via MATS Summer 2026
- **Path**: MATS Summer 2026 mentor application (Perez is listed as "Megastream" mentor). Alternative — direct X reply @EthanJPerez on a thesis-relevant post.
- **Probability of reply**: HIGH for MATS; MEDIUM for cold X.
- **Leverage**: Highest-accessibility Anthropic researcher. Leads red-team / sleeper-agent work; TCT's per-output verifiability is a direct alignment-testing primitive. ICML 2024 Best Paper = strong endorser if he engages.
- **Offer**: "Our constellation framework (TCT) produces per-output geometric verifiability, which looks like a natural substrate for debate-style adversarial alignment. Can we share the paper draft?"
- **Why #3**: The best available back-door into Anthropic without going through Kaplan (who is a Tier-1 target). MATS is a published, legitimate-applicant channel.
- Source: [MATS mentors page](https://www.matsprogram.org/mentor/megastream)

## #4 — Yunzhen Feng (NYU CDS, actively job-hunting) + Julia Kempe (NYU/EIT Oxford)
- **Path**: Email Feng directly (public on fengyzpku.github.io; X @feeelix_feng). Use him as the bridge to Kempe (his advisor).
- **Probability of reply**: VERY HIGH (Feng); MEDIUM (Kempe).
- **Leverage**: Three-paper cluster (A Tale of Tails / Strong Model Collapse / Beyond Model Collapse) sits exactly on the data-wall-and-collapse axis. Kempe's reported 2026 move to Ellison Institute Oxford (flagged unverified) places her in proximity to Gal's OATML — a potential second-order intro.
- **Offer**: "Your 'Strong Model Collapse' result assumes generator-in-loop recursion. We scope an external-supervision construction (DDA, n·(N+C(N,2)) signals per input). Would you or your advisor want to review a draft + discuss a joint EXP-2 audit?"
- **Why #4**: Feng is a final-year PhD looking for industry roles — maximum responsiveness. His endorsement + Kempe's co-sign would pair with Dohmatob's for a three-way collapse-cluster wrap.
- Source: [fengyzpku.github.io](https://fengyzpku.github.io/) · [cims.nyu.edu/~kempe](https://cims.nyu.edu/~kempe/)

## #5 — Shayne Longpre (MIT Data Provenance Initiative)
- **Path**: Direct email (MIT Media Lab); alternative — AI Tinkerers paper-club invitation (he does public paper-club sessions).
- **Probability of reply**: VERY HIGH. Does public paper-club talks, posts on Buzzsprout, small research org, topically motivated.
- **Leverage**: "Consent in Crisis" measured the actual decline of the data commons (5%→45%). TCT's fixed-corpus-with-projection-through-N-embedders is *literally* the constructive answer to his observed problem. The paper tells Longpre's thesis "yes, and here is how you do more with less".
- **Offer**: "Your 'Consent in Crisis' measured the fence going up. We propose an identity: a fixed licensed corpus projected through N frozen embedders yields up to `n·(N+C(N,2))` signals — which reframes consent-compliant training as a compositional problem, not a volume problem."
- **Why #5**: Longpre is the *policy-adjacent* warm channel; his reply would unlock both academic (MIT) and regulator/journalist-adjacent press channels. Amplifier, not acquirer, but extremely valuable for paper propagation.
- Source: [Consent in Crisis IETF slides](https://datatracker.ietf.org/doc/slides-aicontrolws-consent-in-crisis-the-rapid-decline-of-the-ai-data-commons/)

---

## Explicitly NOT in top-5 (and why)

- **Ilia Shumailov**: would be #1 by leverage, but his GDM→own-venture transition means affiliation ambiguity + bandwidth scarcity. Re-verify and re-rank in 30 days.
- **Jared Kaplan / Sam McCandlish / Dario Amodei**: Anthropic senior leadership — too-high-friction for cold contact. Route via Perez (#3) or Olah instead.
- **Demis Hassabis / Jeff Dean / Noam Shazeer**: unreachable by cold email at this stage; use only if a Delétang (A3.5) reply materializes first.
- **Sarah Guo / Elad Gil / Martin Casado**: VC-track, not research-track. Keep as Tier-2 intro targets *after* 1–2 collapse-cluster co-signs exist; their response rate to "paper only, no deal yet" is historically low without a researcher validator.

---

## Week-1 execution plan (concrete)

| Day | Action |
|---|---|
| Day 1 | Send Dohmatob cold email (#1). Attach paper PDF + LoRA HF link + one-page EXP-2 co-author proposal. |
| Day 1 | Send Khattab cold email (#2). Attach paper + highlight §0.2 `n·(N+C(N,2))` identity + offer ECIR workshop submission. |
| Day 2 | Post the paper + HF LoRA on X, tag @EthanJPerez @lateinteraction @dohmatobelvis @feeelix_feng. Do NOT beg; just publish. |
| Day 3 | Apply to MATS Summer 2026 with Perez as preferred mentor (if deadline open). |
| Day 4 | Email Feng + Kempe (single email, CC pattern). |
| Day 5 | Email Longpre, frame as policy-adjacent conversation, propose paper-club invite. |
| Day 7 | Stock-take: any reply = follow up immediately. No reply = warm up #6–#10 (Olah blog comment, Baroni, Villalobos, Gal). |

All outreach MUST include the CI disclosure and Case-scope tags (MEASURED vs PROJECTED) per project constraints.

---

## Competing Interests Disclosure
> Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
