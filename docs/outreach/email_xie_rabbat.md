# Email to Saining Xie + Mike Rabbat (AMI Labs)

**To:** `saining@nyu.edu` (CSO, also NYU faculty), `mrabbat@meta.com` *(verify — see notes)*
**Cc:** —
**From:** `chrisroyseai@gmail.com`
**Attachment:** `ME-JEPAResearchPaper.pdf` (594 KB)
**Send timing:** 5–7 days after the LeCun email if no engagement
**Word count:** 174

---

**Subject:** ME-JEPA runtime draft — relevant to AMI's world-model evaluation work?

Dr. Xie, Dr. Rabbat,

Sharing a draft preprint privately: Dynamic/ME-JEPA, a domain-portable JEPA runtime where data ingest, training, planning, and provenance are physically auditable on a single RTX 5090. Career-taxonomy validation cosine 0.905 ± 0.044 across 5 seeds (BCa CI [0.861, 0.931]); A3 action-removed ablation collapses to 0.790 exactly as predicted and verified.

The "audited execution traces" framing in §8 — every bundle is a deterministic-from-source byte slice plus a small model-output slice paired with observed targets and ablation baselines, with cryptographic provenance — may be directly relevant to the evaluation-infrastructure questions in the AMI Scientist job description ("evaluations for benchmarking world model understanding, prediction, and planning").

I shared this with Yann separately last week. Reaching out to you because the world-model evaluation angle is closer to your direct work than to his.

Private link: https://teleox.ai/ami/
PDF attached.

Happy to extend a private code grant to AMI for verification if useful. No reply expected.

Chris Royse
chrisroyseai@gmail.com

---

## Notes for sender

- **Address verification needed for Mike Rabbat:** Rabbat is at AMI Labs now (VP World Models) but his @meta.com address may still resolve. Best route: check his current LinkedIn for a posted email, or send to `mrabbat@amilabs.xyz` (likely format) plus `mike.rabbat@nyu.edu` as a fallback. He held an adjunct affiliation. Hand-verify before hitting send
- **Saining Xie:** confirmed at NYU as faculty (`saining@nyu.edu`) and CSO at AMI Labs. NYU email almost certainly works
- This is the **lateral AMI entry** — if LeCun ignores you, Xie+Rabbat are the people most directly responsible for evaluating exactly what your paper proposes
- Quote the AMI job description verbatim ("evaluations for benchmarking world model understanding, prediction, and planning") — signals you've read their hiring page, makes the connection inarguable
- "Happy to extend a private code grant to AMI for verification" is the soft commercial open without being a sales pitch
- Replace `ami` slug with whatever you deploy on teleox.ai. Consider `xie-rabbat` instead of `ami` to make the personalization more explicit

## Why this works

| Element | Effect |
|---|---|
| "Reaching out because [your work] is closer than Yann's" | Honest, accurate, signals you understand who actually does what at AMI |
| Quoting the AMI Scientist job description verbatim | Proves you've read their hiring page; positions you as a candidate without explicitly applying |
| "Happy to extend a private code grant to AMI for verification" | Commercial-friendly opening that doesn't require them to commit anything |
| "Shared with Yann separately last week" | Honest, prevents the "wait, did Yann see this?" awkwardness |

## Sequencing rationale

This email goes LAST in the three-email sequence:
1. Day 0: LeCun (privileged-access)
2. Day +2: Cho (warm intra-NYU)
3. Day +5–7: Xie + Rabbat (lateral AMI, only if no engagement on the first two)

If LeCun replies, you don't need this email at all. If Cho forwards to LeCun, you may not need it either. Reserve for the case where the first two paths don't open a conversation.
