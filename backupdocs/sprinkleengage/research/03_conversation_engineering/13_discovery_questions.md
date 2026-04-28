---
namespace: research
agent_id: agent-13-conversation-enablement
timestamp: 2026-04-19T09:00:00Z
status: active
tags: discovery, questions, listen-fors, follow-ups, audience-specific
---

# Discovery Questions — Strategic, by Audience

Each question is designed to (a) invite substantive signal, (b) preserve paper scope, and (c) position Teleox constructs without over-claiming. Every question pair = question + listen-fors + follow-up probe.

---

## 00. PRIMITIVE-EXTENSIBILITY DISCOVERY (all audiences — ask these first)

These questions probe how the target thinks about the **extensibility axis** of the primitive —
the multiplier that public embedder inventory is currently constraining. The questions surface
whether the target can define meaning-representations Chris's pool does not already contain, and
whether they see that as the real asset.

### Q-00A — "If you could define an embedder — any embedder, representing the meaning of anything a function can compute — for your own data or domain, what would the first three be?"

- **Listen-fors**:
  - Concrete examples (e.g., "clause-risk for a contract", "patient-reidentification-risk for HIPAA data", "sensor-fingerprint for industrial audio") → strong primitive-engagement signal; they understand the extensibility axis.
  - "We'd rely on public embedders" → doesn't yet see the extensibility multiplier; educate before advancing.
  - "We already build custom embedders" → probe which ones; ask whether they see the cross-correlation multiplication Chris describes.
- **Follow-up probe**: "If you had three custom embedders, and each new one multiplies correlations against every existing one in the pool, how would your current training pipeline change?"

### Q-00B — "What's the bottleneck in your training-signal supply chain today — is it raw data availability, human labeler throughput, or something else?"

- **Listen-fors**:
  - "Raw data" → direct DDA-counting-identity fit; pitch supply-side multiplier on their existing corpus.
  - "Labeler throughput" → DDA is a labeler-free supervisory yield path; lead with that.
  - "Synthetic-data quality" → Shumailov-scope angle; DDA is non-recursive by construction.
  - "We're compute-bound, not data-bound" → probe their 18-month horizon; Villalobos 2026–2032 applies.
- **Follow-up probe**: "If I showed you a way to get up to 91 supervisory signals per input on your existing corpus — clean, labeled, non-recursive — without changing your compute or acquisition posture, what would that unlock for you?"

### Q-00C — "How does your organization think about the data layer of the AI pipeline — is it a solved problem, a cost center, or a strategic frontier?"

- **Listen-fors**:
  - "Solved" → they're not in the buyer universe for a data-layer primitive; reposition toward the verifiability or sovereign-specific angle.
  - "Cost center" → direct License/Acquire comparison; Teleox primitive is the third path.
  - "Strategic frontier" → perfect prospect; lead with the extensibility story and the case studies as exploratory proofs.
- **Follow-up probe**: "If the data layer is strategic for you, would owning a primitive that reshapes it — not a product, the primitive itself — be an infrastructure asset or a differentiated offering?"

---

## 0. OPERATIONAL-SUBSTRATE DISCOVERY (all audiences, use early if "is this real?" is implied)

### Q-0A — "What's your organization's current posture on per-output auditability of AI-generated content? Is a SHA-256 provenance chain an infrastructure asset for your compliance / legal / risk team, or is it already solved by another vendor?"

- **Listen-fors**:
  - "It's solved" → ask which vendor; this tells you their infra maturity.
  - "Not solved; would be valuable" → direct open for ClipCannon's provenance chain as an enterprise-compliance anchor.
  - "Compliance doesn't care" → they are mid-market or unregulated; de-emphasise provenance.
- **Follow-up probe**: "If we ran a 48-hour PoC on your corpus with every operation chain-hashed, would that be a differentiator or table-stakes for your stakeholders?"

### Q-0B — "Have you evaluated real-time AI-clone avatars for any use case (customer-success, meeting attendance, sales outbound, internal demos)? Where did that evaluation land?"

- **Listen-fors**:
  - "Evaluated and rejected" → ask which vendor; usually HeyGen / Synthesia / ElevenLabs video — then differentiate via runtime identity-guard (ConstellationGuard) and Google Meet / Zoom native attendance (VoiceAgent).
  - "Not evaluated" → this is an expansion surface; probe their meeting culture.
  - "Actively exploring" → direct live-demo offer.
- **Follow-up probe**: "Would a 30-minute live demo of VoiceAgent attending a Google Meet as an AI-clone with per-frame guard be useful?"

---

## A. RESEARCHER (Dohmatob / Perez / Khattab / Kaplan / Kempe / Feng / Shumailov / Olah)

### Q-R1
> "Is the §3.3 scope characterization of your generator-in-loop result accurate — that DDA is structurally outside the recursion regime by precondition, not by evading the theorem?"

- **Listen-fors**:
  - **Endorsement**: "Yes, that's the right precondition." → opens co-author channel.
  - **Partial**: "Close, but you'd need to verify the approximate-independence assumption on your panel." → signals EXP-2 is the right instrument; invite them onto EXP-2.
  - **Rejection**: "No, my theorem extends to non-generative loops if [X]." → immediately log and treat as reviewer-grade objection for paper v2.
- **Follow-up probe**: "What's the one load-bearing assumption you'd want me to verify empirically before claiming the scope argument?"

### Q-R2
> "Would the EXP-2 pairwise mutual-information audit (§9) on a 13-embedder panel falsify the up-to-91 upper bound in a way you'd consider co-authorable — or does the audit design need revision first?"

- **Listen-fors**:
  - Interest in audit design = co-author path is open.
  - Suggestion of alternative estimator = they are engaging technically; note the estimator and offer to run both.
  - Disinterest = shift to endorsement-only ask.
- **Follow-up probe**: "Which MI estimator would you trust on 1024-dim dense vectors — MINE, InfoNCE lower bound, or KSG?"

### Q-R3
> "On the meaning-compression framing in §3.2 — does the unit-pair distinction (signals-per-raw-data-unit at prep vs. bits/params/memory at inference) strike you as a taxonomically coherent fourth category, or as a re-label of data augmentation?"

- **Listen-fors**:
  - "Coherent but needs a formal information-theoretic grounding" → EXP-2 addresses this; offer co-auth.
  - "Re-label" → reframe as "we don't claim novelty of the augmentation act; we claim the unit pair is semantically distinct."
  - Silence / defer → do not push; note as pending.
- **Follow-up probe**: "If we added a rate-distortion style grounding for the per-input signal yield, does that move you from 'maybe' to 'yes'?"

### Q-R4
> "For Case 3 (§6.3), I'm disclosing encoder-matched scope verbatim — within-WavLM-family, N=10, one speaker, English — with ECAPA-TDNN triangulation pending as EXP-1. Is that the right disclosure posture, or should the paper defer the 0.961 number until EXP-1 is done?"

- **Listen-fors**:
  - "Disclose with the scope is the right posture" → validates current framing.
  - "Defer" → they are signalling the number is weighty; consider moving it to appendix until EXP-1 lands.
- **Follow-up probe**: "If you saw this in review, would the scope disclosure in §8.L1 be enough, or would you want the number moved out of §6.3?"

---

## B. SOVEREIGN (MGX — Al Idrissi / Humain — Amin / QIA — Al-Sowaidi / GIC — Jaensubhakij)

### Q-S1
> "What's your current per-output audit posture on sovereign-data training — is `G_τ`-style cosine-trace provenance (§5.4) something your compliance or regulatory stakeholders would value, or is aggregate audit sufficient today?"

- **Listen-fors**:
  - Per-output interest → `G_τ` is directly aligned; 48h PoC is next.
  - Aggregate-is-fine → fall back on DDA-as-signal-multiplier framing.
  - Mixed → scope `G_τ` to specific regulated domains (finance, health, defence).
- **Follow-up probe**: "Is audit posture set at the fund level or by portfolio company? (Informs who owns the PoC decision.)"

### Q-S2
> "Is non-recursive training on your own licensed or sovereign corpora a stated requirement, a strong preference, or not yet policy?"

- **Listen-fors**:
  - "Requirement" → DDA's non-recursive construction (§3.3) is a perfect match; lead with it.
  - "Preference" → still strong fit; lead with scope argument.
  - "Not policy" → opens a category-education conversation (Villalobos / data-wall context).
- **Follow-up probe**: "If you could specify one property of any LLM touching your sovereign data, would it be non-recursive, auditable per output, or something else?"

### Q-S3
> "Given the Stargate / AIP / Humain commitments, where does signal-multiplication on existing licensed corpora sit relative to compute-capacity investment on your AI-strategy roadmap?"

- **Listen-fors**:
  - "Adjacent pillar" → DDA fits; propose PoC inside one portfolio company.
  - "Subordinate to compute" → reframe DDA as compute-amplifier (more signal per GPU-hour).
  - "Haven't thought about it that way" → category-creation opportunity; offer 3-page sovereign brief.
- **Follow-up probe**: "Is the portfolio-company PoC the right entry point, or would you prefer a fund-level methodology evaluation?"

---

## C. BANKER (Qatalyst — Quattrone / LionTree — Bourkoff / Centerview / Allen)

### Q-B1
> "How are you categorizing the Scale AI / Character / Inflection structural family in 2026 — is the 49%-plus-CEO template still live, or are regulators closing that lane?"

- **Listen-fors**:
  - "Still live with regulator engagement" → Teleox fits the template if scaled; note regulator-first structuring is the pattern.
  - "Closing" → pivot to license + reverse-acquihire (Character, Windsurf) or direct M&A (Nuance, Mandiant).
  - "Mixed" → ask for the latest signal; banker reveals their read.
- **Follow-up probe**: "For a substrate-level methodology with open-weight proof and two measured anchors, which of the four structures (49% / license+acquihire / reverse-acquihire / direct M&A) is the cleanest fit?"

### Q-B2
> "Do you see category-creation plays at $3B+ as sell-able in current market conditions, or is $1–3B the realistic band for substrate-level asset families this year?"

- **Listen-fors**:
  - "$3B+ is sellable with right proof stack" → keeps $3B thesis alive; they are signalling their banker-floor.
  - "$1–3B is the band" → lower-bound anchor is real; design engagement accordingly.
  - "Depends on the counterparty" → invite a specific list (3 names they'd run against first).
- **Follow-up probe**: "Who are the three counterparties you'd run first for a substrate-level asset with an open-weight proof and one MEASURED identity-verification case?"

### Q-B3
> "What's the minimum engagement floor for your firm on a sell-side mandate, and what does the proof-stack need to look like for you to take a mandate?"

- **Listen-fors**:
  - Explicit floor ($1B / $2B / $3B) → qualifies their band.
  - "Depends on substrate maturity" → Case 1 open-weight + Case 3 MEASURED is the substrate; ask if that clears the bar.
  - "We'd need a term sheet on paper" → they are not the first-mover banker; move to next banker.
- **Follow-up probe**: "If the open-weight LoRA, MEASURED voice result, and Context Graph source repo clear the substrate bar, what's the next artefact you'd want to see before engaging?"

---

## D. HYPERSCALER EXEC (Alphabet / MSFT / Meta / Amazon / Apple / Nvidia / Anthropic / OpenAI)

### Q-H1
> "How is your org positioning against Villalobos 2026–2032 exhaustion — licensing, synthetic generation, or methodology investments in DDA-style signal multiplication?"

- **Listen-fors**:
  - Licensing-dominant → DDA is additive, not competitive; position as corpus-amplifier.
  - Synthetic-heavy → share §3.3 scope argument and the generator-in-loop risk.
  - Open to methodology → lead with Kaplan 2020 → DDA counter-example framing.
- **Follow-up probe**: "Where does your org draw the line between 'generator-in-loop synthetic' (Shumailov-regime) and 'non-recursive signal multiplication' (DDA-regime)?"

### Q-H2
> "Where does per-output verifiability sit relative to your current alignment roadmap — is it a 'nice-to-have' beside RLHF/Constitutional AI, or a first-class property you're actively scoping?"

- **Listen-fors**:
  - First-class → `G_τ` (§5.4) directly lands; request deep-dive with alignment lead.
  - Nice-to-have → reframe as audit / compliance property for regulated verticals.
  - Not scoped → category-creation conversation; cite §7.3 "different property class".
- **Follow-up probe**: "Who inside your org is the natural stakeholder for per-output geometric verifiability — safety, product, or compliance?"

### Q-H3
> "What's your current posture on adopting academic substrate from a non-lab author — is open-weight reproducibility (e.g., the Shakespeare LoRA on HuggingFace) enough to enter a technical deep-dive, or do you require a full benchmark suite first?"

- **Listen-fors**:
  - "Open-weight reproducibility is enough" → Case 1 HF LoRA opens the channel.
  - "Benchmark suite required" → they are signalling a longer path; propose 48h PoC as the on-ramp.
  - "Depends on the author's academic credential" → map this to Dohmatob/Kaplan endorsement path.
- **Follow-up probe**: "If one collapse-cluster researcher co-signs the §3.3 scope argument, does that change your bar?"

---

## E. VENTURE CAPITAL (Casado / Gil / Guo / Midha / Grady / Khosla)

### Q-V1
> "How does your fund evaluate 'pool-relative novelty' claims (paper §2.4) in a crowded retrieval / alignment space — what's the diligence artefact that flips 'plausibly new' to 'demonstrably new' for you?"

- **Listen-fors**:
  - "Researcher endorsement" → Dohmatob path is the diligence unlock.
  - "Measured benchmark" → Case 3 MEASURED is the anchor; offer full protocol disclosure.
  - "Production substrate" → Context Graph (~370K LOC Rust) is the anchor; offer source walk-through.
- **Follow-up probe**: "If we delivered one collapse-cluster endorsement plus the HF LoRA plus Context Graph source, does that clear pool-relative-novelty diligence?"

### Q-V2
> "What's your fund's current thesis on methodology-level (substrate) investments vs. application-level investments in 2026 — is the capital flowing to substrate or application?"

- **Listen-fors**:
  - Substrate-heavy → Teleox fits; ask about check size and round composition.
  - Application-heavy → reframe Context Graph + ClipCannon as applications on Teleox substrate.
  - Balanced → offer the blended pitch.
- **Follow-up probe**: "If a substrate play had an open-weight proof and a MEASURED identity-verification anchor, would it qualify for your Infrastructure fund or your American Dynamism fund?"

### Q-V3
> "What's your read on 'category-creation' stories at the substrate layer — still the high-variance / high-reward move, or has 2025–2026 tightened the market toward proven taxonomies?"

- **Listen-fors**:
  - "Still high-variance" → positions meaning-compression fourth-category (§3.2) as intended.
  - "Market tightened" → frame as "we don't need reviewer acceptance of the fourth-category claim for the commercial thesis to hold" — isolates paper risk from commercial risk.
- **Follow-up probe**: "Separating the paper's academic reception risk from the commercial substrate risk — which of the two is your fund actually underwriting?"

---

## F. COMMON CROSS-AUDIENCE (can drop into any conversation)

### Q-X1
> "What would a minimum-viable 48-hour PoC on your data look like — 1 corpus plus 3 embedders, or something richer?"

- **Listen-fors**: willingness to share any corpus at all; if yes → PoC channel is open.
- **Follow-up probe**: "Does the corpus leave your environment at any point, or must it stay fully on-prem? (Informs PoC architecture.)"

### Q-X2
> "If this paper were being reviewed at NeurIPS / ICML / COLM, which one reviewer in your network would you want on the panel?"

- **Listen-fors**: the name itself is a warm-channel candidate; log and follow up.
- **Follow-up probe**: "Would you be willing to make that intro if I shared the paper with you first?"

### Q-X3
> "Is there a second conversation after this one where we should include your chief scientist / compliance head / corp-dev lead?"

- **Listen-fors**: "Yes" with a name = win. "Maybe" = propose structured 48h PoC instead.
- **Follow-up probe**: "What's the one artefact they'd most want to see before that second conversation?"

---

## Pre-ask checklist (every question, every conversation)

- [ ] Question does not presuppose Teleox superiority ("beats", "SOTA").
- [ ] Question does not over-claim Case 3 (no "cross-encoder verified").
- [ ] Question does not require quoting "91 signals" without "up to" + EXP-2 hedge.
- [ ] Listen-fors written BEFORE conversation — not constructed mid-stream.
- [ ] Follow-up probe scoped narrow enough to fit in the remaining call time.
- [ ] Every answer logged to `memory/journal/` within 24h.

---

**Competing Interests**: Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
