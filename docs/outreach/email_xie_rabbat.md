# Email to Saining Xie + Mike Rabbat (AMI Labs)

**To:** `saining@nyu.edu`, `mrabbat@meta.com` *(verify — see notes)*
**Cc:** —
**From:** `chrisroyseai@gmail.com`
**Attachment:** `ME-JEPAResearchPaper.pdf` (594 KB)
**Send timing:** 5–7 days after the LeCun email if no engagement
**Word count:** 152

---

**Subject:** Draft preprint — JEPA pipeline verifier (re: AMI Scientist eval axis)

Dr. Xie, Dr. Rabbat,

Sharing a draft preprint privately. Not a representational contribution — the paper asks what infrastructure is required to make a JEPA training, planning, and provenance pipeline physically verifiable on commodity hardware.

The runtime targets the same structural question the AMI Scientist posting names as one of its research efforts ("evaluations for benchmarking world model understanding, prediction, and planning"). Headline: career-taxonomy validation cosine 0.905 ± 0.044 across 5 seeds (BCa CI [0.861, 0.931]); the action-removed ablation collapses to 0.790 — exactly as the action-conditioned predictor design requires. The full-state verifier physically re-reads every persisted output as a separate code path.

I sent this draft to Yann separately. Reaching out to you in parallel because the world-model evaluation angle is closer to your direct work — the action-conditioned-predictor design choice in §4 in particular owes the I-JEPA / V-JEPA target-encoder lineage.

Happy to grant private repo access to AMI for verification.

Private link: https://teleox.ai/ami
PDF attached. No reply expected.

Chris Royse
chrisroyseai@gmail.com

---

## Notes for sender

- **Address verification needed for Mike Rabbat:** check his current LinkedIn for posted email; try `mrabbat@amilabs.xyz` plus the NYU adjunct fallback. Hand-verify before send
- **Saining Xie:** confirmed `saining@nyu.edu` works
- "Happy to grant private repo access to AMI for verification" replaces the previous "private code grant" phrasing — less vendor, more academic
- Quoting the AMI Scientist job description verbatim still works — proves you've read their hiring page
- Naming Rabbat's direct contribution to I-JEPA / V-JEPA is the credibility signal

## What changed in this revision

| Before | After | Why |
|---|---|---|
| Subject "ME-JEPA runtime draft — relevant to AMI's world-model evaluation work?" | "Draft preprint — JEPA pipeline verifier (re: AMI Scientist eval axis)" | Specific reference to the job posting, no question mark, sharper |
| "may be directly relevant to the evaluation-infrastructure questions" | "targets the same structural question the AMI Scientist posting names" | Direct, no hedge |
| "Happy to extend a private code grant to AMI for verification if useful" | "Happy to grant private repo access to AMI for verification" | Drops the "if useful" qualifier; "private repo access" is academic, "code grant" is vendor |
| (no specific I-JEPA / V-JEPA reference) | "the action-conditioned-predictor design choice in §4 owes the I-JEPA / V-JEPA target-encoder lineage" | Naming Rabbat's actual prior work proves you've read the paper, not just the wikipedia entry |

## Why this works for AMI specifically

- Treats them as researchers first, recruiters second
- Quotes the job posting — signals you've read what they're hiring for without explicitly applying
- Names the technical lineage (I-JEPA / V-JEPA target encoder) → action-conditioned predictor → ME-JEPA — shows you've traced the design history
- "Private repo access" instead of "code grant" — academic register
- The "no reply expected" close still applies; if they want to connect, they will
