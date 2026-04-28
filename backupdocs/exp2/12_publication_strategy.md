# EXP-2 Publication Strategy

**Where and how EXP-2 results get used after Day 21.** Every surface has a different audience; the content shifts accordingly. Steve's compliance gate applies to all of them.

---

## 1 · Audience map

| Audience | Document | Timing | Compliance gate |
|----------|----------|--------|-----------------|
| Chris + Steve | Full internal report | Day 20 | self-review |
| Patent counsel | Full report + patent disclosure paragraph | Day 20–21 | counsel privileged |
| Perez (Anthropic Alignment Science) | Memo revision + exec one-pager | Day 22+ (post-internal review) | mandatory Steve gate |
| Dohmatob (Meta FAIR) | Follow-up email with results | Day 14 (coincides with action list) | mandatory Steve gate |
| NeurIPS (if accepted) | Supplementary materials | Notification (2026-09-24) | mandatory Steve gate |
| Public (arXiv / HF / Teleox blog) | Full report + reproducibility artifacts | After patent priority-date lock + Chris approval | mandatory Steve gate |

---

## 2 · Perez memo revision paragraph — pre-approved template

The Perez memo is sent Day 10 **without** EXP-2. The revision incorporates EXP-2 results once ready. Paragraph to insert:

### Strong outcome ($N_\text{eff} \ge 70$)

> **Update on pair independence**: Since the initial memo, I have run a pre-registered pairwise mutual-information audit on the 13-embedder panel using MINE, InfoNCE, and KSG estimators on $n = 1000$ production inputs. The realized floor of effective supervisory signals per input is $\hat{N}_\text{eff} = X$ with 95 % bootstrap CI [L, U], against the constructive ceiling of $N + \binom{N}{2} = 91$. The dominant redundancy is the E1–E8 pair, which by design shares the e5-large-v2 backbone; after this known reparameterization, residual pair redundancy is small. The audit protocol, estimator-by-estimator results, and reproducibility artifacts are attached (exp2_report_n1000.pdf, preregistration SHA <hash>).

### Moderate outcome (25 ≤ $N_\text{eff}$ < 70)

> **Update on pair independence**: The pairwise MI audit described in the initial memo is now complete. Measured effective signal count per input at $N = 13$ is $\hat{N}_\text{eff} = X$ (95 % CI [L, U]); constructive ceiling is 91. Panel delivers $\hat{N}_\text{eff}$-fold supervisory signal multiplication over a single-encoder baseline on the production distribution. Pair redundancy is concentrated in shared-family embedders (E1 / E8 / E10 in the e5 family; E6 / E13 twin-SPLADE); cross-family pairs remain approximately independent. The Axiom 2 independence claim in the original counting identity is a first-order approximation; the realized count is what the audit measures. Protocol and artifacts attached.

### Weak outcome ($N_\text{eff}$ < 25)

> **Update from the pairwise MI audit**: Measured $\hat{N}_\text{eff} = X$ (95 % CI [L, U]) against the constructive 91 ceiling. The realized floor is substantially below the structural ceiling and is consistent with Huh et al. 2024's Platonic Representation Hypothesis on learned SSL encoders. The headline reframes accordingly: the substrate's structural value is the three-role composition (retriever, labeller, $G_\tau$ per-output cosine predicate) operating over a lower-dimensional effective substrate than the constructive count suggests. Each additional embedder contributes $O(N)$ new pair signals; the super-linear growth curve with $N$ remains the durable claim. Protocol and artifacts attached; the unexpected result makes EXP-2 itself methodologically useful for the broader research community.

---

## 3 · Dohmatob follow-up

Day 14 follow-up (per action list ACTION #9) can reference EXP-2 depending on whether it is complete by then (it will NOT be — EXP-2 completes Day 21). Default Day 14 language:

> Eric — following up on my 2026-04-19 note. I have been running a pre-registered pairwise MI audit on the 13-embedder Context Graph panel, results expected week of 2026-05-12. The protocol (MINE/InfoNCE/KSG, 78 pairs, $n=1000$) and the preregistration are shared at <link>. Would you be interested in a co-author or corresponding-author split on the extension — running the same protocol on Meta FAIR's internal embedder panels would be a natural generalization, and pair-MI on collapse-regime encoders connects directly to your recent work. Happy to share the full audit once results finalize May 14.

---

## 4 · Patent disclosure paragraph

For counsel inclusion in the provisional + 12-month non-provisional filings:

> The system comprises an embedder panel containing $N \ge 3$ distinct embedder slots whose outputs are composed via a three-role retriever / labeller / per-output cosine predicate operator. Per input, the composition emits at most $N + \binom{N}{2}$ distinct supervisory signals as the sum of $N$ per-embedder outputs and $\binom{N}{2}$ pairwise cross-correlation outputs (the "counting identity"). The measured realized effective signal count, as estimated by three independent mutual-information estimators (MINE, InfoNCE, and KSG) under a pre-registered protocol, is a signal-count quantity that indexes system capability distinct from model parameter count and training-data volume. The measured realized count is bounded above by the constructive count; the constructive count is an invariant of the panel size $N$ and the choice of pairwise cross-correlations. The compositional operator that emits $\binom{N}{2}$ cross-correlation signals is claimed as a structural element of the system. The per-output cosine-similarity predicate operator $G_\tau$ applied element-wise to each emitted signal is claimed as an additional structural element.

Counsel refines claim scope based on: (a) the EXP-2 realized number (narrower claims if Weak), (b) USPTO examiner feedback, (c) Alice §101 fragility concerns documented in H6 of the probability recalibration.

---

## 5 · NeurIPS supplementary materials

If accepted (P ≈ 8–12 % per red-team recalibration):

- Full report (markdown + PDF)
- Reproducibility artifacts (anonymized corpus .npz, PCA matrices, all seeds)
- Estimator reference implementations (MINE, InfoNCE, KSG) under `scripts/exp2/lib/`
- Preregistration SHA quoted in the header
- Per-pair statistics machine-readable (JSONL)

If rejected: post same materials to arXiv as a technical report. Priority timestamp remains.

---

## 6 · Public comms — Teleox blog / HF

After patent priority-date lock (Day 14+) and Chris approval:

### Short version (Teleox blog post, ~600 words)

- Opening: the counting identity in English (no math).
- What we measured and why.
- The constructive 91 ceiling and the $\hat{N}_\text{eff}$ realized floor together.
- One heatmap figure (the $13 \times 13$ MI matrix).
- Reproducibility link to the GitHub / arXiv release.
- One paragraph on what this means for DDA: the panel-as-substrate-for-supervisory-signal-multiplication thesis is either supported (Strong / Moderate) or requires reframing (Weak).
- No discussion of commercial implications; this is a research-integrity post, not a pitch.

### HF Case 1 model card update (per action list ACTION #13)

Add a dedicated section "Pairwise MI on the 13-embedder panel" with the headline $N_\text{eff}$ number, 95 % CI, and link to the report. Keep the BSL license and NeurIPS submission reference as primary.

---

## 7 · What we DO NOT publish

- **Do not publish** the raw production corpus texts (privacy).
- **Do not publish** the memory IDs in plaintext (use one-way hashes).
- **Do not publish** seed-by-seed critic checkpoints (reproducibility via fresh training from serialized PCA + corpus suffices).
- **Do not publish** the exact commercial framing / pitch language here; that lives in the Perez memo and banker docs.
- **Do not publish** anything before the patent filing on Day 14 without counsel sign-off.
- **Do not grant any party exclusivity** on the EXP-2 results before banker mandate (per WINNING_PLAYBOOK).

---

## 8 · Handling an independent-publication collision (from arXiv/USPTO monitoring)

If the Day-5 monitoring flags a paper from a lab doing a similar measurement (see R-10 in [10_risk_register.md](10_risk_register.md)):

1. **Within 48 hours**: Chris + Steve assess: is this prior art, parallel, or downstream?
2. **If prior art** (published first, describing exactly the same measurement): narrow patent claims, add the reference to the EXP-2 report as a prior-art citation. Do not claim primacy we don't have.
3. **If parallel** (concurrent, different panel composition): cite the paper in the EXP-2 report with comparison. Still publish.
4. **If downstream** (builds on our cited ideas): continue as planned; note the reference.

---

## 9 · Open-science conditionality

Per the playbook, open-science binding is Q4 of the Q1–Q7 blocker list. If Chris commits to open-science on EXP-2 (likely), the publication timing changes:

- Pre-patent-lock (before Day 14): cannot publish EXP-2 results externally. Internal report only.
- Post-patent-lock: free to publish under the BSL + NeurIPS-supplementary pattern already in place for Case 1.
- If NeurIPS accepts: supplementary materials are the canonical public release.
- If NeurIPS rejects: arXiv + Teleox blog is the canonical release.

---

## 10 · Sign-off chain for each surface

- **Internal report (Day 20)**: Chris + Steve
- **Patent counsel surface (Day 21)**: Chris + Steve + counsel
- **Perez memo revision**: Chris + Steve (compliance) + one trusted reader outside the team if possible
- **Dohmatob follow-up**: Chris + Steve (compliance)
- **Public arXiv / blog**: Chris + Steve + counsel (patent scope review) + Chris approval on tone

Any external surface without the required sign-offs is a protocol violation and logged as an incident in the preregistration amendment log.
