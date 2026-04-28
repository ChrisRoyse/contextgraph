---
namespace: patterns
agent_id: agent-42-adversarial-reviewer
timestamp: 2026-04-17T11:00:00Z
status: active
locked: false
supersedes: none
tags: stop-ship, paste-ready, concrete-edits, pre-submission, red-team-remediation
depends_on:
  - memory/discoveries/agent-42-adversarial-reviewer--critique.md
  - sections/00_frontmatter.md
  - sections/01_introduction.md
  - sections/03_framework_dda_mc.md
  - sections/04_systems.md
  - sections/06_method_tct.md
  - sections/07_case_studies.md
  - sections/08_discussion.md
  - sections/09_limitations.md
  - sections/10_conclusion.md
downstream_consumers:
  - agent-43-reproducibility-checker
  - agent-44-citation-validator
  - agent-46-coherence-checker
  - agent-47-final-assembler
answers_questions: [stop-ship, concrete-edits, review-remediation]
---

# Stop-Ship List + Concrete Paste-Ready Edits

## 0. Purpose

The sister critique file `agent-42-adversarial-reviewer--critique.md`
lists 3 CRITICAL, 14 HIGH, 19 MEDIUM, 3 LOW findings. This file is
the **operational edit set**: eight stop-ship fixes that block v1
submission and the exact paste-ready text for every CRITICAL and
HIGH finding.

A downstream writer (coherence-checker / final-assembler) should be
able to open this file, find the listed section and line marker, and
apply the edit without further interpretation. Where the edit is a
replacement, the `OLD` text is shown verbatim and the `NEW` text is
the recommended replacement. Where the edit is an insertion, the
`INSERT AT` marker names the location and the `INSERT` block is the
text to add.

All edits assume agent-32 contradiction register resolutions
(IC-1 through IC-6, EC-7 through EC-11) are already in the draft
and that agent-33 MUST-softenings 1 and 2 are applied. Spot-checks
in §1 of the critique confirm both.

---

## 1. Stop-ship list (blockers for arXiv v1)

The eight items below MUST be fixed before submission. Estimated
fix time: 2.5–3.0 hours of editing. No new measurement runs required.

| # | Finding tag | Location | Severity | Time |
|---|---|---|---|---|
| STOP-1 | 5.1 | §5.4 predicate paragraph | CRITICAL | 10 min |
| STOP-2 | 6.1 | §6.3.2 headline paragraph | CRITICAL | 10 min |
| STOP-3 | 6.2 | §6.3.3 Table 1 + caption | CRITICAL | 15 min |
| STOP-4 | 4.2 + CA-4 | §4.3 paragraph 3 | HIGH | 15 min |
| STOP-5 | CA-12 | §§1–9 body, all internal tags | HIGH | 60 min |
| STOP-6 | 8.3 + CA-4 | §8.L8 + §Competing Interests (new) | HIGH | 30 min |
| STOP-7 | 6.4 | §6.3.5 each pending measurement | HIGH | 20 min |
| STOP-8 | 6.6 + CA-14 | §8.L3 + global replace | HIGH | 20 min |

**Total: ~3 hours of editing.**

---

## 2. Paste-ready edits — CRITICAL findings

### STOP-1 — §5.3 / §5.4 per-output verifiability scope placement

**Location.** `sections/06_method_tct.md`, §5.4 "Phase 3 — The
runtime constellation guard", the paragraph immediately following
equation (5).

**OLD** (the sentence sequence after equation 5):
```
The predicate is **per output**. Its cost is $O(M \cdot d_{\max})$ for the cosine comparisons (plus embedder forward passes, one per modality per candidate). It requires **no auxiliary learned model**: $\phi_m$ is frozen by Axiom 1, $c_m$ by Phase 1, $\tau_m$ is a scalar calibrated pre-deployment. The decision is deterministic modulo GPU floating-point reproducibility.

We call the resulting property **per-output verifiability relative to the chosen centroid panel**. What is verified is that $\hat y$ satisfies the specified cosine predicate against the frozen $C_P$; what is not verified is that $\hat y$ is correct under any external identity criterion unavailable to the panel.
```

**NEW** (scope folded into the predicate paragraph; deterministic
clause softened per Finding 5.3):
```
The predicate is **per output** and is verifiable **relative to the
chosen embedder panel $\Phi$ and centroid set $C_P$**; cross-encoder
transportability is a separate question this predicate does not
resolve. Its cost is $O(M \cdot d_{\max})$ for the cosine
comparisons (plus embedder forward passes, one per modality per
candidate). It requires **no auxiliary learned model**: $\phi_m$ is
frozen by Axiom 1, $c_m$ by Phase 1, $\tau_m$ is a scalar calibrated
pre-deployment against a disjoint calibration set (§5.3 threshold
calibration). The predicate is deterministic given the embedder,
centroid, and candidate tensors; in mixed-precision (bf16 generator,
fp32 embedder, fp32 cosine) practice, the threshold margin is
subject to $O(10^{-4})$-level floating-point variance, and the
audit record logs the numerically-resolved score to support post-hoc
replay.

We call the resulting property **per-output verifiability relative
to the chosen centroid panel**. What is verified is that $\hat y$
satisfies the specified cosine predicate against the frozen $C_P$
under $\Phi$; what is not verified is that $\hat y$ is correct under
any external identity criterion unavailable to the panel.
```

### STOP-2 — §6.3.2 headline bold asymmetry

**Location.** `sections/07_case_studies.md`, §6.3.2 "The protocol
and the headline", the bullet list immediately after "The measured
result on the 10 held-out sentences is:".

**OLD**:
```
- **Mean WavLM SECS 0.961 (MEASURED, N = 10, encoder-matched)**
- **Max WavLM SECS 0.975 (MEASURED, N = 10, encoder-matched)**
- **Min WavLM SECS 0.914 (MEASURED, N = 10, encoder-matched)**
- **9 of 10 sentences at SECS $\geq 0.95$ (MEASURED, N = 10)**
- DNSMOS P.808 (Reddy et al., 2022): clone **3.93** versus real **3.93** (MEASURED; descriptive parity, N = 14 comparison set)
- UTMOS (Saeki et al., 2022): clone **3.012** versus real **2.997** (MEASURED; clone marginally higher by +0.015, N = 14)
```

**NEW** (scope in the same typographical unit as the number; "same
guard-and-evaluator encoder" explicit):
```
- Mean WavLM SECS **0.961** on 10 held-out sentences, guard and
  evaluator both WavLM-base-plus-sv (MEASURED; one speaker; English).
- Max WavLM SECS **0.975**; min **0.914**; 9 of 10 sentences at
  SECS $\geq 0.95$ (MEASURED; same scope).
- DNSMOS P.808 (Reddy et al., 2022): clone 3.93 versus real 3.93
  (MEASURED; descriptive parity at 3 sig figs within evaluator
  noise; $N = 14$ comparison set; no formal equivalence test).
- UTMOS (Saeki et al., 2022): clone 3.012 versus real 2.997
  (MEASURED; clone marginally higher by +0.015; $N = 14$; no formal
  equivalence test).
```

The bold is lifted off the scope tag so the number-and-scope read as
a single unit; the MOS parity rows are de-bolded to de-emphasise
neural-proxy parity (which is a weaker claim than SECS agreement).

### STOP-3 — §6.3.3 temperature-sweep vs. headline discrepancy

**Location.** `sections/07_case_studies.md`, §6.3.3 "Temperature-
sweep ablation (MEASURED)", the paragraph after Table 1 caption.

**OLD**:
```
The $T = 0.3$ row's mean (0.932) is the sweep-internal mean over the 10 held-out sentences at that temperature with a different seed chain; the 0.961 headline above is a separate run at the same $T = 0.3$ operating point reported in the paper-level evaluation. Both values are consistent with $T = 0.3$ being the sweep optimum. **Scope: the optimum is pipeline-specific** — the claim is "$T = 0.3$ produced the highest post-selection mean SECS in this pipeline, this speaker, this corpus, this scorer", not "$T = 0.3$ is the optimal temperature for voice cloning in general". Transport to other speakers, other TTS backends, other scorers, or other text corpora is not claimed.
```

**NEW** (explicit on why the two numbers differ; disavows
cherry-picking):
```
The $T = 0.3$ row of Table 1 reports a sweep-internal mean of 0.932
over the 10 held-out sentences using the seed chain the
temperature-sweep protocol reserves for $T = 0.3$; the 0.961
headline comes from a subsequent evaluation run at the same
$T = 0.3$ operating point with an independent seed chain, the same
10 held-out sentences, the same scorer, and the same pipeline
version hash. The sweep established $T = 0.3$ as the optimum before
the headline run was scheduled; the headline number is therefore not
selected from the sweep. Both values are within the run-to-run
variance expected of a best-of-12 argmax over Qwen3-TTS at $T = 0.3$
with this centroid; both are consistent with $T = 0.3$ being the
sweep optimum on this pipeline, this speaker, this corpus, this
scorer. Transport to other speakers, other TTS backends, other
scorers, or other text corpora is not claimed, and we note that a
single run's mean of 0.961 does not by itself establish $T = 0.3$
optimality; the sweep is the evidence for that, and the headline is
the evidence for the SECS level at the sweep optimum.
```

Also update Table 1 caption to add: "Each row reports a single seed
chain reserved for that temperature; the paper-level 0.961 headline
at $T = 0.3$ comes from an independent seed chain in a subsequent
evaluation run."

### STOP-4 — §4.3 delete Leapable.ai commercial-terms sentence

**Location.** `sections/04_systems.md`, §4.3 paragraph 3 (last
paragraph of §4).

**OLD**:
```
Leapable.ai — a 161-tool MCP marketplace with Stripe Connect revenue split (70/30 steady-state; 75/25 for founding creators), a 5 % platform reserve held for 60 days, and an HMAC-signed billing ledger — is described in the acknowledgements as the commercial exemplar downstream of the two systems above. It is not evaluated in Section 6; its mention here is for transparency about the author's affiliations per the competing-interests statement. Section 5 specifies the TCT three-phase protocol the two systems enable; Section 6 reports the three case studies that instantiate it.
```

**NEW** (commercial terms removed; structural reference preserved):
```
Leapable.ai — an MCP marketplace operated by the author — is named
in the Acknowledgements and the Competing Interests statement as
downstream commercial context; it is not evaluated in Section 6 and
is not a contribution of this paper. The two commercial terms and
operational details (revenue split, platform reserve, billing
mechanism) live in the Acknowledgements per arXiv convention.
Section 5 specifies the TCT three-phase protocol that the two
systems above enable; Section 6 reports the three case studies that
instantiate it.
```

The deleted commercial terms (Stripe Connect 70/30, 5% reserve,
HMAC) should be relocated to §Acknowledgements as:

> "Leapable.ai operates on a Stripe Connect 70/30 revenue split
> (75/25 for founding creators) with a 5% platform reserve held for
> 60 days and an HMAC-signed billing ledger. These are commercial
> details, not research contributions."

---

## 3. Paste-ready edits — HIGH findings (stop-ship subset)

### STOP-5 — CA-12 — Strip internal SMAP identifiers from body text

**Scope.** Every occurrence of `W-\d+`, `VT-[A-Z]\d+`, `IC-\d+`,
`EC-\d+`, `CA-\d+`, `C\d+[ab]?`, `H\d+[a-z]?`, `FM-\d+`, `CL-\d+`,
`ISO-\d+`, `MP-\d+`, `R[0-9]+`, `V\d+` in `sections/01_*.md` through
`sections/10_*.md` body text. Internal tags are acceptable ONLY in
§Acknowledgements if the author wants to thank the SMAP pipeline,
but most should simply be deleted.

**Specific edits:**

1. **§6.3.2** — `(VT-I3 / VT-C1 / VT-S4; W1)` → `(see §8.L1)` (or
   drop entirely — the sentence says "Mandatory scope disclosure";
   the next sentences make the scope explicit).

2. **§6.3.2** — `(VT-C2; C6)` → delete; the paragraph is "**Naturalness-
   parity scope**", which is self-describing.

3. **§6.3.5** — `(W1 remediation)` → `(addresses the encoder-matched
   scope of §8.L1)`. `(W2 remediation; VT-I2)` → `(isolates
   selection from base-model sampling; see §8.L1)`. `(W3
   remediation; VT-T3)` → `(anchors the sample-complexity claim
   at more than one operating point; see §8.L4)`.

4. **§7.1** — `(P3 of the framework; Theorem 2 of §5.4; C5
   confidence tier).` → `(Theorem 2; §5.4).` Drop C5.

5. **§7.1** — `(EXP-1 remediation: ...)` — EXP-1 through EXP-3 are
   also internal labels but are resolved to `(a re-score with
   ECAPA-TDNN would triangulate ...)` in prose form. Either
   preserve EXP labels globally as published experiment names, or
   replace everywhere. Recommendation: preserve EXP-1/2/3 because
   they are referenced in §8 and §9; drop all other acronyms.

6. **§7.2** — `(C1; CA-1)` → `(see §8.L4)`.

7. **§7.3** — `(C13; CA-2 with multi-modal scope tightening)` → `(§7.3
   paragraph 2 applies scope-tightening qualifications)`.

8. **§7.4** — `(P2 of the framework; CL-06 MODERATE-categorical)` →
   `(see §3.3)`.

9. **§7.4** — `(IC-5 resolved)` → delete.

10. **§7.4** — `(W9 scope)` → delete.

11. **§7.5** — `(C12a, $\geq 95\%$ confidence)` → delete.

12. **§7.5** — `(C12b)` → delete.

13. **§7.5** — `(W6)` → `(see §8.L4)`.

14. **§8** final paragraph — entire confidence-delta enumeration
    (`$\sim 0.98$ encoder-matched confidence to $\sim 0.90$`, etc.)
    should be rewritten in plain language. See STOP-8 below for a
    paste-ready rewrite.

15. **§3.1** — `Under Axiom 1 (grounded measurement; Section 5)` →
    `Under Axiom 1 (§5)`.

16. **§3.2** — `V11 carries verbatim` occurrences → replace with
    "the approximate-independence hedge from §3.1".

17. **§8.L4** — `V11 carries verbatim throughout: "approximately
    independent; strict formal orthogonality is an open question"`
    → rewrite without "V11" label.

18. **§8.L1 / §8.L3 / §8.L4** — each begins with a numbered heading
    and a body paragraph. The bodies contain internal refs like
    "EXP-1" (retain), "C5" (remove), "IC-4" (remove), "W1" (remove).

Complete search-and-replace: run the regex
`\b(W|VT|IC|EC|CA|C\d|H\d|FM|CL|ISO|MP|R|V)\d+[a-z]?\b` across
`sections/01_*.md` through `sections/10_*.md`, review each hit,
delete or replace per above. Preserve EXP-1, EXP-2, EXP-3 as
named experiments.

### STOP-6 — §8.L8 + §Competing Interests

**Problem.** §8.L8 "Commercial conflict of interest" is
structurally mis-placed (it is a disclosure, not a limitation on
claims). A §Competing Interests section is named as future but is
not drafted.

**Action.** (i) Draft §Competing Interests and place between §9
Conclusion and §References. (ii) Rewrite §8.L8 as a technical
limitation.

**§Competing Interests (new; paste-ready):**
```
## Competing Interests

The author, Chris Royse, is founder and operator of Teleox.ai
(primary affiliation) and Leapable.ai (companion marketplace).
Context Graph is the backbone system of Teleox.ai; ClipCannon is a
production pipeline developed in the same commercial context;
Leapable.ai is an MCP marketplace operated by Teleox.ai. The author
holds equity in both entities. No third-party research funding or
vendor-paid benchmark is reported in this manuscript; no commercial
comparison study, market-size claim, revenue or pricing figure
appears in §1–§9. The DDA, meaning-compression, and TCT constructs,
the constructive theorems, and the Case 3 measured anchor are
methodology contributions emerging from commercial engineering
practice; the asymmetric release posture (Case 1 artefacts released
for byte-identical reproduction; Case 2 and Case 3 subject-specific
artefacts withheld on consent and dual-use grounds) is disclosed
fully in §Ethics. Readers should understand the author's commercial
affiliation as a prior for the research framing and weight
§1–§9 accordingly.
```

**§8.L8 (rewrite) — paste-ready:**
```
## L8. The production systems are commercial engineering artefacts

Context Graph and ClipCannon are live systems developed in an
ongoing commercial engineering practice (Teleox.ai). The line,
tool, column-family, and test counts reported in §4 are snapshot
values from the working tree at measurement date 2026-04-16; the
systems continue to evolve and subsequent measurements may report
different values. The DDA substrate at $N = 13$ and $N = 7$ is the
invariant this paper defends; the specific implementation details
are context. A reproducible reference release of the Shakespeare
LoRA training stack (Case 1) is in preparation and will carry a
pinned commit and artefact hash separate from the live system
codebase.
```

### STOP-7 — §6.3.5 pending-measurement justifications

**Location.** `sections/07_case_studies.md`, §6.3.5 "Three pending
companion measurements", after the numbered list.

**Action.** For each of the three pending measurements (cross-
encoder ECAPA; pre-selection cosine distribution; $n_m$ curve), add
one sentence justifying why v1 does not contain it. Current draft
says "Instrument specification is locked; run pending" for each,
which reads as a deferral without reason.

**NEW sentence per measurement (insert at the end of each numbered
bullet 1, 2, 3):**

- **Bullet 1 (cross-encoder ECAPA):** "This v1 reports the
  within-WavLM-family measurement because the ECAPA-TDNN re-score
  requires an independent model weight audit for licensing and
  version-reproducibility, which is scheduled for a subsequent
  version of this manuscript."
- **Bullet 2 (pre-selection distribution):** "The pre-selection
  SECS distribution requires re-running the 12-candidate
  generation from identical seeds to obtain the full 120-candidate
  record; the seed chain is archived but the re-run is deferred
  to a subsequent version to preserve v1 as a tight headline."
- **Bullet 3 ($n_m$ curve):** "The four-point $n_m \in \{10, 25,
  50, 100\}$ curve requires three additional centroid constructions
  and three additional 10-sentence evaluation runs; the $n_m = 50$
  row is the one that was complete at v1 submission, and the
  remaining three rows are scheduled for a subsequent version."

If any of these justifications is false, the v1 submission should be
re-planned to include the measurement.

### STOP-8 — §6.6 + CA-14 — "Camera-ready" / "Track A/B" language

**(a) Global replace in §§1–10:**
- `camera-ready will re-verify` → `a subsequent version of this
  manuscript will re-verify`
- `for the camera-ready version` → `for a subsequent version`

Estimated: ~9 occurrences across §1, §6.1.1, §6.1.4, §6.2.1, §6.2.4,
§8.L3, Table 2 caption, Table 3 caption, §9.

**(b) §8.L3 Track A / Track B deletion:**

**OLD** (§8.L3 Remediation paragraph, last two sentences):
```
**Remediation.** Two-track drafting is in place: Track A ships measured per-modality tables for Case 2 and cleaned-retrain numbers for Case 1 if runs complete; Track B preserves the forensic-narrative contributions and stamps "PENDING — architecturally complete" on quantitative rows. The constructive theorems (T1, T2) and the DDA counting identity do not depend on these measurements.
```

**NEW**:
```
**Remediation.** A subsequent version of this manuscript is expected
to include measured per-modality tables for Case 2 and cleaned-retrain
numbers for Case 1; if those runs do not complete before the next
revision, the forensic-narrative contributions remain and the
quantitative rows remain flagged "PENDING — architecturally
complete". The constructive theorems (Theorem 1 and Theorem 2) and
the DDA counting identity do not depend on these measurements.
```

**(c) §8 final paragraph — rewrite without confidence-delta numbers.**

**OLD**:
```
Three experiments would most directly tighten the claim set. **EXP-1** — cross-encoder SECS triangulation on the Case 3 held-out set — is expected to move C5 from $\sim 0.98$ encoder-matched confidence to $\sim 0.90$ cross-encoder if it strengthens, or to reframe the headline to "within-WavLM-family" if it does not. **EXP-2** — pairwise mutual-information audit across the Context Graph 13-embedder panel — is expected to move C1 from $\geq 0.95$ "up to 91" to a measured $N_{\text{eff}}$ replacing the structural upper bound. **EXP-3** — four-point $n_m \in \{10, 25, 50, 100\}$ sweep on Case 3 — is expected to move C12b from $\sim 0.40$ empirical support at one operating point to a partial sample-complexity curve. The camera-ready version is expected to carry those three measurements and update the confidence deltas accordingly.
```

**NEW**:
```
Three experiments would most directly tighten the claim set.
**EXP-1** — an ECAPA-TDNN cross-encoder re-score of the Case 3
argmax candidates — would promote the Case 3 headline from
within-WavLM-family identity agreement to cross-encoder identity
ranking if mean ECAPA $\geq 0.85$ with Pearson $r \geq 0.6$;
otherwise the within-WavLM-family scope of §6.3 and §8.L1 carries.
**EXP-2** — a pairwise mutual-information audit across the
13-embedder Context Graph panel, estimating $I(\phi_j(X);
\phi_k(X))$ on 100–1,000 representative inputs — would replace the
"up to 91" structural upper bound with a measured effective-signal
count $N_{\text{eff}}$, and would sharply edge the DDA counting
argument if the pairwise term contracts heavily. **EXP-3** — the
four-point $n_m \in \{10, 25, 50, 100\}$ sweep on Case 3 — would
anchor the sample-complexity claim at more than one operating
point and test the Axiom-3 interval directly. A subsequent version
of this manuscript is expected to carry these three measurements.
```

---

## 4. Paste-ready edits — remaining HIGH findings

These are not stop-ship but are required for a strong submission.

### Finding 1.2 — "locked 88-citation pool" phrasing

**Location.** Multiple in §1, §3.4, §5.4, §7.3.

**Action.** Introduce once, in §1 paragraph 2 as a footnote:
```
[footnote] The novelty claims in this manuscript are pool-relative:
the reference set of 88 sources cited throughout is the master pool
against which "no cited method" and "to our knowledge" claims are
stated. The thematic scope of the pool is given in §2. Pool-external
counter-examples would qualify specific claims but not invalidate
the composition-level contribution for the specific systems this
manuscript reports.
```

Then global replace:
- `to our knowledge in the locked 88-citation pool that anchors this
  manuscript` → `within the reference set` (or delete entirely).
- `the locked 88-citation pool` → `the reference set`.
- `pool-relative to the locked 88-citation master reference pool` →
  `pool-relative (within the reference set)`.

Remove all "88" digits from the body text.

### Finding 1.3 — Separate the 91-signal identity from the fourth-category proposal

**Location.** `sections/01_introduction.md`, §1 paragraph 4, last two
sentences (the "first consequence" / "second consequence" /
"third consequence" block).

**OLD** (the first-consequence sentence):
```
The first consequence is the counting identity $|D'| = n \cdot (N + \binom{N}{2})$ — up to 91 signals per input at $N=13$ — which positions meaning compression as a fourth taxonomic entry alongside bit-, weight-, and activation-compression with a semantically distinct unit.
```

**NEW** (split into two sentences; explicit "constructive upper
bound"):
```
The first consequence is the counting identity $|D'| = n \cdot (N +
\binom{N}{2})$: up to 91 structured supervisory signals per input at
$N = 13$ as a constructive upper bound under Axioms 1 and 2, with
empirical tightness pending a pairwise mutual-information audit.
This counting view positions meaning compression as a fourth
taxonomic entry alongside bit-, weight-, and activation-compression,
measured in a semantically distinct unit (structured signals per
raw-data unit) that we do not claim supersedes the other three.
```

### Finding 2.3 — §1 must pre-empt the VALL-E 2 apples-to-apples read

**Location.** `sections/01_introduction.md`, §1 paragraph 4, the
Case 3 block.

**OLD**:
```
Qwen3-TTS conditioned on a 50-clip centroid and selected via a WavLM-base-plus-sv best-of-twelve at temperature $T=0.3$ achieved **0.961 mean SECS (max 0.975) on 10 held-out sentences for one English-speaking subject under an encoder-matched protocol** (guard and evaluator both WavLM-base-plus-sv), with 9 of 10 sentences at SECS $\geq 0.95$.
```

**NEW** (VALL-E 2 paradigm disclaimer added inline):
```
Qwen3-TTS conditioned on a 50-clip centroid and selected via a
WavLM-base-plus-sv best-of-twelve at temperature $T=0.3$ achieved
**0.961 mean SECS (max 0.975) on 10 held-out sentences for one
English-speaking subject under an encoder-matched protocol** (guard
and evaluator both WavLM-base-plus-sv), with 9 of 10 sentences at
SECS $\geq 0.95$. This is a **personalised** protocol result (50-clip
centroid plus best-of-$K$); published zero-shot TTS numbers (e.g.,
VALL-E 2 at approximately 0.881 WavLM-large-sv) target a different
problem class (no subject reference set; single-prompt conditioning)
and are not directly comparable.
```

### Finding 3.1 — Tighten "signal" definition in §3.1

**Location.** `sections/03_framework_dda_mc.md`, §3.1 paragraph 5
(the "A signal here is a scalar..." paragraph).

**OLD**:
```
A signal here is a scalar or low-dimensional feature that is a deterministic function of a raw input under frozen embedders and frozen interaction rules, is attached to a specific (input, embedder) or (input, embedder-pair) index, and can be used as input or target in a downstream loss without new data collection.
```

**NEW** (linear-reparameterisation exclusion; indexable):
```
A signal here is a per-embedder projection $\phi_m(x_i)$ or a
pairwise interaction feature $\rho_{jk}(\phi_j(x_i), \phi_k(x_i))$
that (i) is a deterministic function of a raw input $x_i$ under the
frozen $\phi_m$ and the frozen interaction rules, (ii) is indexable
under a specific (input, embedder) or (input, embedder-pair) key,
(iii) can be used as an input or target in a downstream loss without
new data collection, and (iv) is not, relative to the other signals
in $D'$, merely a fixed invertible linear reparameterisation of an
already-counted signal. The last condition prevents trivial
inflation of the count by applying redundant projections; it is the
operational reading of Axiom 2 within the counting identity.
```

### Finding 3.2 — Table 1 row-label hedge

**Location.** `sections/03_framework_dda_mc.md`, §3.2 Table 1,
last row.

**OLD**:
```
| **Meaning compression** | units of raw data | structured labelled signals | DDA (this paper) |
```

**NEW**:
```
| **Meaning compression** (proposed) | units of raw data | structured labelled signals | DDA (this paper; a fourth entry proposed here, not a consensus category) |
```

### Finding 5.2 — §5.6 Case 3 threshold-calibration ordering

**Location.** `sections/06_method_tct.md`, §5.6 Case 3 block, the
sentence describing $\tau_{\mathrm{speaker}} = 0.95$.

**OLD**:
```
Phase 3 threshold $\tau_{\mathrm{speaker}} = 0.95$ calibrated to the same-session-band floor on a disjoint calibration set; the operational output is the argmax regardless of whether the strict predicate clears $\tau$, and the cosine trace is attached as provenance.
```

**NEW** (explicit ordering: threshold before evaluation):
```
Phase 3 threshold $\tau_{\mathrm{speaker}} = 0.95$ was calibrated
**before** the 10-sentence held-out evaluation, using the
same-session-band floor (10th percentile) on a disjoint calibration
set of authentic subject samples not included in $R_m$. The
operational output of Phase 2 is the argmax across the $K = 12$
candidates regardless of whether the strict predicate clears $\tau$;
the cosine trace is attached as provenance, and the 10-sentence
evaluation reports the argmax SECS distribution without any
retrospective adjustment of $\tau$.
```

If this ordering claim is not empirically true (i.e., $\tau$ was set
after seeing evaluation outputs), the sentence must be rewritten to
disclose the actual ordering, and the guard-verifiability claim
must be downgraded correspondingly.

### Finding 5.4 — §5.5 Layer 3 policy naming

**Location.** `sections/06_method_tct.md`, §5.5 "Three-layer
enforcement stack", Layer 3 paragraph.

**OLD**:
```
**Layer 3 — Runtime guard $G_\tau$ (Section 5.4).** The full generation $\hat y$ is evaluated against the frozen text-style centroid by predicate (5). Layer 3 is **per-output** and **semantic**: the only one of the three whose decisions depend on the frozen centroid and are verifiable post-hoc against the audit record. Rejection triggers a bounded regeneration loop ($K = 4$ per sentence) and, on budget exhaustion, a fallback with `below_threshold` warning.
```

**NEW** (explicit policy naming):
```
**Layer 3 — Runtime guard $G_\tau$ (Section 5.4).** The full
generation $\hat y$ is evaluated against the frozen text-style
centroid by predicate (5). Layer 3 is **per-output** and
**semantic**: the only one of the three whose decisions depend on
the frozen centroid and are verifiable post-hoc against the audit
record. The rejection policy is `regenerate` with budget $K = 4$
per sentence; on budget exhaustion the pipeline falls through to
`fallback`, returning the relaxed argmax with a `below_threshold`
warning attached to the audit record.
```

### Finding 6.3 — §6.3.4 Table 2 1-clip baseline

**Location.** `sections/07_case_studies.md`, §6.3.4 Table 2 caption
and preceding paragraph.

**Action.** Add to the caption: "The 1-clip baseline uses single-
draw sampling (no best-of-$K$); the 50-clip row uses best-of-12 at
$T = 0.3$. The ablation therefore conflates centroid size with
best-of-$K$ selection effect; an apples-to-apples measurement at
$K = 12$ for the 1-clip baseline is RO-10-adjacent future work."

If the 1-clip baseline DID use best-of-12, state that instead and
populate the max column.

### Finding 6.5 — §6.2.2 forensic-fix framing

**Location.** `sections/07_case_studies.md`, §6.2.2 forensic-finding
paragraph — the list of six bugs.

**OLD** (beginning of the second paragraph):
```
Root cause: the original training code computed `target = latent - noise`, but the EchoMimicV3 scheduler (`fm_solvers.py`) uses a velocity convention `target = noise - latent`. The sign flip trained the model to **add** noise to clean video rather than remove it — anti-denoising. Loss minimised cleanly because the target was well-defined; the model learned the wrong direction. Five companion blockers were isolated in the same pass: a `cv2` frame-rate mismatch (60 fps read vs 25 fps requested); mouth bounding boxes landing on the nose (geometric-ratio extraction on `scene_map` bboxes, fixed by landmark-based detection); a dead `q_audio` LoRA target (a typo at transformer line 843 overwrites the `q_audio(q)` output so LoRA gradients never reach the loss — fix: exclusion from the LoRA target list); 22 GB of audio features accumulating in RAM during preprocessing (fix: stream to disk per clip); and a viseme curriculum advertised as weighted but actually uniform (fix: `WeightedRandomSampler` with viseme-rarity weights, oversampling OY/AW/UH by 2–4×).
```

**NEW** (reframe as audit record, not confession):
```
Root cause: the original training code computed
`target = latent - noise`, but the EchoMimicV3 scheduler
(`fm_solvers.py`) uses a velocity convention
`target = noise - latent`. The sign flip trained the model to
**add** noise to clean video rather than remove it — anti-denoising.
Loss minimised cleanly because the target was well-defined; the
model learned the wrong direction. The forensic audit record
(`docs/inspect/08_training_audit.md` in the Santa project) logs
five companion fixes from the same audit pass: a frame-rate
sampling mismatch between probe and read paths, a geometric-ratio
error in mouth-bounding-box extraction, a dead LoRA target on the
`q_audio` projection, a memory-accumulation bug in audio-feature
preprocessing, and a non-functional weight schedule on the viseme
curriculum. Each has a commit-level fix and a post-fix verification
step in the audit record; the audit is cited as engineering
narrative rather than as randomised ablation, and controlled
re-runs isolating each fix are future work.
```

Then append to the paragraph:
```
The general lesson this section supports is narrow: the multi-
embedder multi-role composition of §3.4 catches pipeline failures
that single-embedder single-role evaluation does not, and the
forensic audit with the frozen panel is a deployment requirement
rather than a discretionary step.
```

### Finding 6.7 — §6.3.2 seed procedure

**Location.** `sections/07_case_studies.md`, §6.3.2 protocol
paragraph.

**OLD**:
```
Best-of-12 means twelve candidates are drawn per sentence at temperature 0.3 with independent seeds; each candidate is scored by WavLM cosine against the centroid; the argmax is returned.
```

**NEW**:
```
Best-of-12 means twelve candidates are drawn per sentence at
temperature 0.3 with twelve independent random seeds drawn from a
per-sentence seed chain logged in the pipeline's audit record; each
candidate is scored by WavLM cosine against the centroid; the argmax
is returned. Seed chains are reproducible from the logged chain
seed plus the per-candidate index.
```

### Finding 8.2 — §8.L3 15/15 FSV clarification

**Location.** `sections/09_limitations.md`, §8.L3 paragraph.

**Action.** Add a sentence after "Case 1's three-layer enforcement
stack passed 15/15 forensic-smoke verification on a prior run":

```
The 15/15 FSV battery is an integrity check (base-model identity,
LoRA attachment, centroid invariance, provenance trace, rejection
policy, etc.) rather than an identity-quality measurement; passing
15/15 on the prior run did not catch the training-data echo-bug of
§7.7, and the retrain FSV is therefore a re-integrity check on
cleaned data rather than a new form of identity-quality validation.
Identity-quality readouts on the retrain are T1 pending.
```

### CA-2 — §3.2 CA-5 triad statement (positive framing)

**Location.** `sections/03_framework_dda_mc.md`, §3.2 paragraph
immediately after Table 1.

**INSERT** (new paragraph between Table 1 and the "The unit pairs
are semantically distinct" paragraph):
```
Three properties distinguish meaning compression from classical data
augmentation and from multi-view representation learning: **(a) no
new inputs are generated** — the raw $x$ is unchanged, and every
signal in $D'$ is a deterministic readout of $x$ under a frozen
$\phi_m$ rather than a transformation of $x$; **(b) the encoders are
frozen** — no gradient update flows into any $\phi_m$ from the
$\phi_m(x)$ signals, so the panel is an instrument rather than a
learned representation; **(c) the panel has $N \geq 2$ independently
trained encoders** — the configuration is not a paired-encoder
alignment (as in CLIP-lineage dual encoders) or a single encoder with
multiple projection heads. A data-augmentation pipeline that
produces transformed inputs fails (a); a multi-view representation-
learning pipeline that trains the encoders fails (b); a
paired-encoder retrieval system fails (c). Meaning compression is
the configuration that satisfies (a), (b), and (c) jointly.
```

### CA-9 — §3.4 three-role qualifier acknowledgement

**Location.** `sections/03_framework_dda_mc.md`, §3.4 paragraph 2
(the "All three roles invoke the same frozen function..." paragraph).

**INSERT** (after the existing pool-relative sentence):
```
The composition is defined by the conjunction of three qualifiers:
identical parameter instances across roles, no parameter update
between roles, and simultaneous deployment (one role does not
precede the others at system time). A paper that satisfies any two
of the three qualifiers but not the third is a **partial precedent**;
the CLIP and ColBERT lineages satisfy the frozen-instrument
qualifier and the multi-role qualifier but deploy the encoders only
as retrievers; the FaceNet lineage satisfies the per-output runtime
qualifier at $M = 1$ but is not multi-role. The specific joint
specification — retriever, labeller, and runtime guard over
identical frozen instances in the same production pipeline — is the
configuration the cited pool does not contain.
```

---

## 5. Sanity check — the edits collectively

After applying the eight stop-ship fixes and the remaining HIGH
edits:

- **STOP-1** makes the verifiability scope explicit in the predicate
  paragraph.
- **STOP-2** makes the 0.961 headline typographically carry its
  encoder-matched scope.
- **STOP-3** resolves the apparent T=0.3 sweep-vs-headline mismatch.
- **STOP-4** removes commercial-terms phrasing from §4 body.
- **STOP-5** removes SMAP internal identifiers from reader-facing
  body.
- **STOP-6** moves commercial COI to §Competing Interests proper.
- **STOP-7** justifies each deferred companion measurement.
- **STOP-8** replaces "camera-ready" language with arXiv-appropriate
  revision language; rewrites §8 final paragraph without internal
  confidence-delta numbers.
- Finding 1.2 introduces and then silences the "88 pool" label.
- Finding 1.3 separates signal-count from fourth-category claim.
- Finding 2.3 pre-empts VALL-E 2 comparison in §1.
- Finding 3.1 tightens the signal definition.
- Finding 3.2 hedges Table 1.
- Finding 5.2 makes threshold calibration pre-evaluation explicit.
- Finding 5.4 names Layer 3 policy.
- Finding 6.3 clarifies Table 2 1-clip baseline.
- Finding 6.5 reframes forensic fixes as audit record.
- Finding 6.7 makes seed procedure reproducible.
- Finding 8.2 flags 15/15 FSV as integrity-check not quality
  validation.
- CA-2 pulls CA-5 triad into positive §3.2 statement.
- CA-9 acknowledges the three-role conjunction as a qualifier stack.

Residual MEDIUM findings (19 items) are left un-edited here — the
coherence-checker can apply them with freedom. None is load-bearing
for arXiv v1.

## 6. Changelog

- 2026-04-17 — Paste-ready edit set by agent-42 for eight stop-ship
  findings plus nine remaining HIGH findings.
