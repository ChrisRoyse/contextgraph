# 5090JEPA — AI Prompt Library

**Purpose**: Ready-to-use, copy-paste prompts that an AI (Claude, ChatGPT, Gemini) can use to produce world-class assets for the 5090jepa project. Every prompt assumes the AI has been given `5090JEPA_MASTER_CONTEXT_GUIDE.md` as preceding context.

**How to use**:

1. Open a new AI conversation.
2. Paste the **entire master context guide** (`docs2/5090JEPA_MASTER_CONTEXT_GUIDE.md`) as the first message. Tell the AI: "This is your authoritative context. Quote facts from it; do not invent."
3. Paste the **chosen prompt** from this library as the next message.
4. The AI returns a deck/infographic/post/email ready for human review.

Every prompt explicitly tells the AI:
- The audience.
- The goal.
- The structure / number of slides / word count.
- The visual style.
- The key data points to anchor on.
- The call-to-action.
- The forbidden mistakes (no inventing numbers, no jargon dumping, etc.).

If a prompt requires images, infographics, or diagrams, the AI is instructed to produce **mermaid diagrams**, **SVG**, or **HTML/CSS** that the human can paste into a deck or render. Where slide markup is required, the AI is told to produce **Marp / Reveal.js / Slidev / Google Slides text** as appropriate.

---

## TABLE OF CONTENTS

A. The universal context block (paste before any prompt)
B. Pitch decks (5)
C. Sales decks (4)
D. Technical decks (4)
E. Infographics (15)
F. Long-form content (5)
G. Short-form social content (8)
H. Sales enablement (6)
I. PR / launch announcements (5)
J. Regulator / insurer specific (3)
K. Internal documents (4)
L. Brand / visual style guide
M. Meta-prompt (let the AI invent new prompts)

---

## A. THE UNIVERSAL CONTEXT BLOCK

> **Paste this once at the start of any AI conversation**, *after* you have already pasted the full master context guide. It tells the AI how to behave throughout the session.

```text
You are a senior strategic communications operator helping me produce assets for
a new venture called 5090jepa. The master context guide I just gave you is the
canonical source of truth — every fact, number, file path, regulation, and
player name comes from it. Rules that override anything else:

1. NEVER invent numbers, customer names, dates, or quotes. If the master context
   guide does not contain a fact, say "[fact not in source — operator to confirm]"
   inline rather than fabricate.
2. Anchor every claim in section references to the master guide. Use the form
   "(per Master Context §11.2)" so the human can audit your work.
3. Match tone to audience. Investors get crisp numbers + comparable exits.
   Regulators get precise standards mapping. Engineers get architecture and code
   paths. Customers get ROI and case-study-shaped narrative.
4. Default to economy. Cut sentences that do not change the reader's behavior.
5. Default to evidence. When you make a claim, name the source-of-truth.
6. When you produce slide markup, use Marp markdown unless told otherwise.
7. When you produce diagrams, use mermaid (preferred) or SVG.
8. When you produce tables, use markdown tables.
9. When you produce code or commands, use real syntax — no placeholder pseudo-code.
10. If the prompt asks you to make assumptions (timing, customer names, prices),
    list every assumption at the bottom of the response under "Assumptions".

Confirm you have read the master context guide and understand these rules by
echoing back: "Context loaded. Sections 1-20. Universal rules acknowledged."
```

---

## B. PITCH DECKS

### B.1 — Seed investor pitch (10 slides)

```text
Produce a 10-slide seed-investor pitch deck for 5090jepa. Output as Marp markdown.

Audience: pre-seed / seed VC partner who has heard of EU AI Act but not of
DynamicJEPA. Time slot: 30 minutes. Goal: secure a $2-5M round at $20-40M post.

Required slide list (use exactly these titles, in order):
1. Title — "5090jepa: The evidence chain regulators and insurers actually want"
2. The forcing function — Aug 2 2026, EU AI Act high-risk enforcement, fines up
   to 7% of global revenue
3. Why current "AI guardrails" are insufficient — wrapper architecture limits
4. Our structural insight — runtime ownership produces evidence as a side effect
5. Architecture in one diagram — the Layer 3-4 stack vs Layer 5 wrappers
   (use a mermaid diagram from Master Context §3 + §8)
6. Demo proof — counter_world + gridworld_5x5 on RTX 5090, FSV passing
7. Go-to-market — open-source runtime, paid Compliance Pack, insurance
   distribution
8. Acquisition comps — Cisco/Robust Intelligence $400M, F5/CalypsoAI $180M
9. Team & traction — what already exists (108 MCP tools, 73 CFs, 6,527 tests),
   what we ship in the next 90 days
10. The ask — $2-5M for 12-18 month runway to close 1 BFSI lighthouse + 1
    insurance partnership; close-the-deal CTA

Each slide:
- Title 1 line.
- ≤ 5 bullet lines OR 1 diagram OR 1 chart description.
- 1 speaker note paragraph (≤ 3 sentences) for the founder.

Style:
- No clip art. No stock photos. The reader trusts numbers.
- Use Master Context §17 numbers verbatim — do not paraphrase or round.
- Cite the master context section in parentheses on every factual claim.
- Slide 5 must include a mermaid diagram showing Layers 0-7 with our position
  highlighted at Layer 3-4.
- Slide 8 must include a markdown comparison table of comps.

Forbidden:
- The phrase "AI safety" without naming a specific regulation.
- Customer names that are not in Master Context (none yet — be honest).
- Generic "we believe" claims with no anchor.
```

### B.2 — Series A pitch (15-20 slides)

```text
Produce a 15-20 slide Series A pitch deck for 5090jepa. Output as Marp markdown.

Audience: tier-1 enterprise infra VC (Lightspeed / Greylock / Sequoia / a16z).
Goal: $15-35M raise at $80-150M post. Lead investor decides this Friday.

Constraints: Series A means "show me the lighthouse + the insurance MoU + the
unit economics". Where you do not yet have these, mark "[demonstrated in next
90 days]" and present the 90-day plan as the close.

Slide list:
1. Title.
2. Problem — Annex IV / SR-11-7 / NIST AI RMF / ISO 42001 / AIBOM all converging.
3. Why wrappers cannot solve it (Master §9).
4. Our wedge — runtime evidence chain.
5. Demo — counter_world + gridworld FSV walk-through (one-screen artifact).
6. Architecture — the 3-layer stack + ecosystem position (Master §3, §8).
7. Lighthouse customer — [name or "in-flight, signs Q3 2026"], use case, ARR.
8. Insurance partnership — [carrier name or "in MoU phase"], premium discount
   mechanism (Master §13.2).
9. Pipeline — top of funnel logos, % qualification rate, conversion math.
10. Pricing & unit economics — Starter $30K, Pro $90K, Enterprise $250K-$1M;
    CAC, LTV, payback (Master §14).
11. Competitive landscape — full table including pricing, funding, position
    (Master §11.3).
12. Acquisition comps — $400M / $180M / range $300-700M for us in 36 months.
13. Roadmap — Q2 → Q4 2026 → 2027 milestones.
14. Team — founders, advisors, key hires planned.
15. Hiring plan with proceeds — VP Sales, VP GTM Ops, 3 AEs, 4 senior engineers.
16. Use of funds — % to engineering, % to sales, % to compliance/SOC2/ISO,
    % to insurance carrier integrations.
17. Risk register — top 5 risks + mitigation (Master §6.6 in game theory analysis).
18. Why now — the EU AI Act clock + acquisition window closing.
19. Ask — $X at $Y post, board seat, [milestones].
20. Appendix — backup slides on FSV, evidence chain, regulatory cross-walk
    (only show if asked).

Each slide:
- Headline that delivers the one thing the reader must remember.
- Bullets ≤ 30 words each.
- 1 chart or 1 table per data slide.
- Speaker notes 1 paragraph.

Use Marp directives for slide layout. Use a consistent type ramp (h1 = headline,
h3 = section, body = bullets). Cite Master Context sections.

Forbidden:
- "AI is going to change the world" preambles.
- Numbers not in Master Context.
- Slide bullets > 7 words longer than necessary.
```

### B.3 — Customer sales pitch (BFSI CISO/CRO)

```text
Produce a 12-slide customer sales deck for a BFSI prospect (mid-tier US bank,
$10-50B assets, 5-15 high-risk models in production). Output Marp markdown.

Audience: CISO + CRO + Head of Model Risk Management in a 60-minute meeting.
Goal: get to a 90-day pilot at $50-75K to instrument 1 high-risk model.

Slide list:
1. Title — co-branded with prospect logo placeholder.
2. We have one slide on us, 11 slides on you.
3. Your problem (with their numbers): $X/yr on MRM FTE, Y high-risk models in
   scope for SR-11-7 + EU AI Act, Z hours per quarterly model review.
   (Mark numbers as [discovery call data] if pre-call.)
4. Annex IV / SR-11-7 / NIST AI RMF cross-walk — what evidence each requires.
5. The reality of "AI guardrails" today — why your current Lakera / Fiddler /
   Credo investment does not produce Annex IV evidence.
6. The 5090jepa runtime — our evidence chain, generated as a side effect.
7. Cross-walk: each of your existing high-risk model documentation requirements
   mapped to a DynamicJEPA record + CF (Master §7.2).
8. The 90-day pilot — scope, timeline, deliverables, success metrics.
9. Insurance partnership — Munich Re aiSure / HSB premium-reduction angle.
10. ROI — license + pilot fee vs FTE savings + premium reduction; 3-10× IRR.
11. Risk register — vendor consolidation risk, deployment risk, integration
    risk (and how the 90-day pilot de-risks each).
12. Next step — 30-min discovery call with our solutions architect this week.

Style:
- Conservative. Banks hate hype. Use specific dollar figures and standards numbers.
- Lead with cross-walks (slides 4 + 7) — that is what they will photocopy.
- Use markdown tables. Avoid mermaid (banks prefer simple).
- Pull every regulation reference verbatim from Master Context §10.

Forbidden:
- "Disrupt" / "revolutionary" / "next-generation".
- Anything that compares us to OpenAI / Anthropic — wrong layer.
- Promising specific premium-discount percentages we have not yet got in writing
  from a carrier.
```

### B.4 — Customer sales pitch (insurance carrier)

```text
Produce a 10-slide pitch for a Tier-1 AI insurance carrier (Munich Re / Swiss Re
/ HSB / AIG). Goal: sign an evidence-acceptance MoU.

Audience: Head of AI Underwriting + Chief Actuary.

Slide list:
1. Title — "5090jepa: A reduced-uncertainty risk pool for AI liability"
2. Why AI liability is hard to underwrite — opaque models, ad-hoc evidence,
   tail-risk uncertainty.
3. The evidence chain we produce (with example record types).
4. How that reduces underwriting variance — measurable model-update lineage,
   measurable surprise rate, measurable drift detection.
5. Comparable structural products — cyber rider underwriting requires SIEM
   evidence; we are the SIEM equivalent for AI runtime.
6. The carrier's win — premium-discount product offering distinguishes the
   carrier on AI Liability shelf.
7. Our win — credible third-party signal that beats all wrapper competitors.
8. Joint product structure — discount ladder tied to deployment depth.
9. Pilot — 5 mutual customers, 12 months, joint loss-ratio analysis.
10. Next — 90-day MoU, joint actuarial deep dive.

Style:
- Actuarial. Speak in variance, expected loss, claim frequency, claim severity.
- Show a hypothetical loss-ratio reduction calc: "Customer with $10M policy
  paying 5% premium = $500K. With evidence chain reducing claim probability
  from 1.5% to 0.8%, expected annual loss drops from $150K to $80K. Premium
  could drop 10-20% and still improve carrier loss ratio."
  Mark the 1.5% / 0.8% as illustrative.
- Use Master Context §10 (regulatory) and §11.6 (insurance) for anchor points.
```

### B.5 — Strategic acquirer briefing (Cisco / F5 / IBM / Datadog)

```text
Produce an 8-slide strategic-acquirer briefing for the corporate development
team of a top-5 enterprise security incumbent.

Audience: VP Corp Dev + their AI-product GM.

Slide list:
1. Title — "Why 5090jepa is a category-defining acquisition target"
2. Their gap — what their existing AI security/governance product cannot do
   (depending on acquirer: distribution but no runtime evidence chain).
3. Our fit — runtime evidence chain integrates into their distribution.
4. Comparable acquisitions — $400M / $180M precedent; what their multiple
   should be.
5. Customer overlap — co-sell motion to top-2000 enterprises.
6. Integration paths — three concrete options (channel partner, OEM, full
   acquisition).
7. Diligence dossier — what we will share in a 30-day NDA window.
8. Next step — exec dinner.

Style:
- M&A vocabulary. Use accretion math. Estimate target ARR + multiple range.
- Do not act eager — present the offer as one of several options.
- Anchor on §11.2 acquisition comps verbatim.
- Use a markdown table for §11.2.
```

---

## C. SALES DECKS

### C.1 — 90-day pilot deck for a regulated enterprise

```text
Produce a 6-slide 90-day pilot proposal deck. Output Marp.

Audience: prospect's project sponsor (Director of Model Risk).
Goal: get the PO signed for $50-75K to instrument one high-risk model.

Slides:
1. Pilot definition — model selected, evidence to produce, success criteria.
2. Week 1-2 — runtime deployment in their VPC, domain pack authoring.
3. Week 3-6 — instrumentation, first evidence packet generated.
4. Week 7-10 — integration with their MRM workflow, regulator-mode export.
5. Week 11-13 — joint readout with insurance carrier, carrier issues
   premium-discount letter.
6. Post-pilot — Pro contract conversion path + co-authored case study.

Style:
- Project-management precision. Gantt-style schedule (use mermaid gantt).
- Each week has named owners (placeholder names in [brackets]).
- Pricing breakdown: software license + deployment services + carrier consult.
- One slide must contain the "Operator Sign-Off Checklist" adapted from
  `docs/5090jepa/00_TASK.md` §12 so the prospect knows what "done" means.
```

### C.2 — Demo script (45-minute live walkthrough)

```text
Produce a 45-minute live-demo script. Output as a numbered timeline:
mm:ss / what to do / what to say / what to show / what could go wrong.

Audience: 3 prospects on a Zoom call.
Goal: get their hands-on-keyboard reaction to the FSV evidence packet.

Demo sequence:
00:00 — RTX 5090 box visible. Run env probe (`nvidia-smi`, `nvcc --version`).
03:00 — `cargo build --workspace --release` (skip if pre-built; show binary).
04:00 — `dynamicjepa register-domain --file counter_world.v1.toml` and explain
        the TOML.
07:00 — `dynamicjepa ingest-event` with happy-path JSONL; show count deltas.
10:00 — `dynamicjepa materialize-panel --all-pending`; show panel JSON.
13:00 — `dynamicjepa compile-trajectories` then `compile-dataset` ; show shard.
16:00 — `dynamicjepa train`; tail nvidia-smi during 30s training.
19:00 — `dynamicjepa get-artifact --verify-files`; show hash recompute.
22:00 — `dynamicjepa predict` and `dynamicjepa plan`; show plan trace JSON.
26:00 — Mutate model bytes; rerun predict; show ArtifactHashMismatch error.
30:00 — Run `verify-counter-world`; show evidence bundle on screen.
33:00 — Switch domain to gridworld; same code, new TOML, repeat shorter loop.
38:00 — Walk through the evidence packet → Annex IV cross-walk doc.
42:00 — Q&A.
45:00 — Close + next-step CTA.

For each step, include the actual CLI command (real, not pseudo), the expected
output highlights, and the talking-point sentence the founder says.

Reference Master Context §6.5 (FSV harness), §15 (runbook).
```

### C.3 — Objection handling cheatsheet (1 page)

```text
Produce a 1-page sales objection-handling cheatsheet. Output as a markdown
table with three columns: Objection / 30-sec response / Backup data point.

Cover at least these objections (drawn from Master §19):

1. "We already use Lakera / Guardrails / Credo / Fiddler."
2. "AWS Bedrock Guardrails is free."
3. "What if Cisco buys you?"
4. "Compliance is procurement's problem, not security's."
5. "You can't be a SOC 2 vendor, you're too small."
6. "We'd rather wait for Microsoft / IBM to ship this."
7. "Our LLM provider already has built-in safety."
8. "How do we evaluate without sharing prod data?"
9. "What's your roadmap for X?"
10. "We're not hiring AI governance staff yet."
11. "EU AI Act enforcement will be delayed."
12. "Insurance discounts sound too good to be true — how does the math work?"
13. "We've never heard of 5090jepa."
14. "Aren't you just a wrapper?"
15. "Why open-source?"

Each response anchors on a specific master-guide section and a comparable
data point. Length per cell ≤ 40 words.
```

### C.4 — RFP response template

```text
Produce a generic RFP response template for an enterprise AI governance RFP.
Output as markdown headings the responder fills in, with example placeholder
text drawn from Master Context.

Sections required (in this order, with example responses):

1. Company overview — open-source-first, named insurer partner, Series A
   funding stage.
2. Solution overview — DynamicJEPA runtime + Compliance Pack.
3. Evidence chain capability matrix — Annex IV / SR-11-7 / NIST AI RMF / ISO
   42001 / AIBOM cross-walks (this is a verbatim copy of Master §7.2).
4. Architecture — Layer 3-4 position; integrations with cloud providers,
   identity providers, SIEMs, ticketing.
5. Security — SOC 2 Type 1/2 status, ISO 42001 certification status, data
   handling, encryption, key management.
6. Pricing — Starter / Pro / Enterprise; insurance partnership rebate.
7. Implementation services — 90-day pilot pattern.
8. References — open-source community + named pilot customers + insurance
   carrier reference.
9. SLA — uptime, response times by severity, incident process.
10. Support — channels, hours, escalation.
11. Roadmap — public roadmap pointer + customer council.
12. Contractual terms — open-source license + commercial license + DPA.

Each section has a 1-2 paragraph example answer using Master Context facts.
Where Master Context is silent (e.g. SLA percentages, support hours), insert
"[operator to confirm — see RFP playbook]" as a placeholder.
```

---

## D. TECHNICAL DECKS

### D.1 — Architecture deep-dive (45-minute conference talk)

```text
Produce a 30-slide technical deep-dive deck. Audience: principal engineers and
ML platform leads. Output Marp.

Cover:
1. Title.
2. Problem framing — Annex IV + AIBOM in one slide.
3. JEPA recap — encoder + predictor + EMA target + VICReg (Master §6.3).
4. Why a Rust runtime (not a Python wrapper) — performance, single-writer,
   strict typing.
5. The 14-embedder Context Graph foundation — what it stores, how, why.
6. The 73 + 25 column families.
7. Domain pack TOML walkthrough — counter_world example.
8. Adapter algorithm — line-by-line walkthrough of `json_event` (Master §8 of
   the 5090jepa demo plan).
9. Instrument execution — scalar / categorical / onehot / time.
10. Latent panel materialization — slot order, hash, mask.
11. Trajectory compiler — segmentation + ordering invariants.
12. Dataset shard compiler — split, leakage report, negatives.
13. Tiny JEPA architecture diagram.
14. Loss formulation — MSE + VICReg variance + covariance.
15. Training loop pseudocode.
16. EMA target + collapse audits.
17. Artifact lifecycle — write → hash → register → load-with-hash-check.
18. Prediction record + uncertainty.
19. Plan trace + guard decision.
20. Surprise event recording.
21. Verification harness — happy path + edge cases.
22. Storage atomic batches — what is one batch, what isn't.
23. Single-writer safety — flock + pid file.
24. Security model — content hashes everywhere.
25. Performance numbers — train time, VRAM, build time on RTX 5090.
26. Comparison vs PyTorch reference — disk + VRAM.
27. Open-source repo tour — how to read the codebase in 30 minutes.
28. Roadmap — Phase 11+ context_graph_memory pack, real-customer data flows.
29. Q&A.
30. Backup slides — full record-type tables.

Style:
- Heavy on diagrams (mermaid).
- Show 3+ real code snippets (Rust / TOML / JSON).
- Speaker notes are conversational (this is a talk).
- Anchor every claim on Master Context sections.
```

### D.2 — Regulator briefing (20-minute closed-door)

```text
Produce a 12-slide regulator briefing for the EU AI Office, NIST, BoE PRA, or
Federal Reserve.

Audience: senior policy staff. Goal: have us referenced in upcoming guidance
or reference architectures.

Slide list:
1. Title.
2. Who we are — open-source AI runtime + commercial compliance pack.
3. The Annex IV / NIST AI RMF / SR-11-7 evidence problem from a regulator's
   perspective: documents that say what AI should do, not what it does.
4. Our evidence chain — structural, hash-verified, append-only.
5. Cross-walk to your regulation — make this slide regulator-specific.
6. Reproducibility — same code, two unrelated domains, identical FSV pattern.
7. Open-source posture — every byte of evidence format is public; auditable.
8. Insurance partnership — third-party validation of evidence credibility.
9. Reference architecture proposal — please reference us by name.
10. Voluntary pilot offer — 3-month pilot with a regulated entity in your
    jurisdiction.
11. Contact details.
12. Backup — full Annex IV table (Master §7.2).

Tone: regulatory respect, not sales. Use the regulator's terminology. Include
specific paragraph references from their published documents.
```

### D.3 — Open-source community talk (NeurIPS / Strata / FOSDEM)

```text
Produce a 25-slide community talk for an open-source / ML conference.

Audience: ML engineers and researchers.
Goal: GitHub stars, PRs, recruiting pipeline.

Slide list:
1. Title.
2. The community problem — every team writes the same evidence pipeline.
3. JEPA refresher — focus on action-conditioned variant.
4. The runtime gap in JEPA papers — they don't ship deployable infra.
5. Demo: counter_world + gridworld training in 90 seconds on a 5090.
6-15. Architecture walkthrough (mermaid diagrams + code snippets).
16. The compliance angle — why this happens to be useful for regulators.
17. The insurance angle — this is also useful for premium reduction.
18. How to contribute — domain pack contribution guide.
19. Repo tour.
20. Roadmap and good-first-issues.
21. The compliance pack is paid; the runtime is and stays free.
22. Acknowledgements (existing tests + audits + the FSV discipline).
23. Recruiting — we're hiring Rust + GPU + storage engineers.
24. Q&A.
25. Links: GitHub, Discord, blog.

Tone: peer-to-peer engineer. Self-deprecating about complexity. Heavy code.
```

### D.4 — Internal engineering all-hands

```text
Produce a 15-slide internal engineering all-hands deck.

Audience: 5-50 engineers (most pre-Series A team). Goal: align everyone on the
quarter's priorities.

Slide list:
1. Title — "Q3 2026 Engineering Priorities".
2. Where we are vs the master plan (which Phase 0-10 are done; which are open).
3. Q3 OKR 1 — 5090jepa demo passing FSV on a clean clone.
4. Q3 OKR 2 — first-customer pilot deployment.
5. Q3 OKR 3 — insurance carrier integration prototype.
6. Q3 OKR 4 — SOC 2 Type 1 readiness (controls, evidence pipelines).
7. R-rules reminder (R1-R8, no fallback, no mocks, FSV everywhere).
8. The 6 quality gates each PR must pass (build, test, FSV, simplifier,
   audit, evidence).
9. Recently shipped highlights (commits + tests + audits).
10. Top 5 risks this quarter + mitigation owners.
11. Hiring plan — 4 roles open, refer-a-friend bonus.
12. Open-source roadmap — what we'll publish externally.
13. Customer-engagement window for engineers (lighthouse pilot).
14. Quarterly retro — what we want to keep / start / stop doing.
15. Q&A.

Tone: peer leadership. Specific. No corporate fluff.
```

---

## E. INFOGRAPHICS (15 TYPES)

> Each infographic prompt outputs **(a) a mermaid or SVG diagram** and **(b) a one-paragraph caption** the human can paste into a deck/blog/social.

### E.1 — Architecture stack diagram (Layer 0-7)

```text
Produce a layered-stack diagram (mermaid `graph TD`) showing 8 layers:
- Layer 0 Compute (Blackwell GPUs, CUDA, Candle)
- Layer 1 Foundation models
- Layer 2 Memory/retrieval substrate (Context Graph)
- Layer 3 Action-conditioned world-model runtime (DynamicJEPA)
- Layer 4 Compliance / evidence chain (5090jepa Compliance Pack)
- Layer 5 Wrapper guardrails / observability (named competitors)
- Layer 6 Agent orchestration
- Layer 7 Application UX

Highlight Layer 3-4 with a different color and a thick border. Use exact
incumbent names from Master Context §8.

Caption: 3 sentences explaining why Layer 3-4 is structurally cheaper to
own than Layer 5 wrapping.
```

### E.2 — Evidence chain flow diagram

```text
Produce a `graph LR` mermaid diagram showing the full evidence chain:

RawDomainEvent → AdapterRunRecord → NormalizedState/Action/Outcome/Transition
→ InstrumentReadings → LatentPanel → PredictionRecord → PlanTraceRecord
→ GuardDecisionRecord(s) → SurpriseEventRecord → DjAuditRecord

Add side annotations on each node showing the source of truth (column family
name) and the cryptographic guarantee (content_hash, model_artifact_hash, etc).

Caption: 3 sentences positioning this as the "structural side effect" of the
runtime.
```

### E.3 — Competitive landscape map (2-D positioning)

```text
Produce a 2-D positioning chart (use SVG, not mermaid).

X-axis: "Wrapper depth" → "Runtime depth" (left = inspects boundary; right =
owns data plane).
Y-axis: "Generic" → "Compliance-specialised".

Plot every player from Master §11:
- Lakera (left, mid)
- Guardrails AI (left, low)
- Credo AI (left-mid, high)
- Fiddler (mid, mid)
- Arize (mid, low)
- Robust Intelligence (now Cisco AI Defense) (mid-right, mid)
- CalypsoAI (now F5) (mid-right, mid)
- AWS Bedrock Guardrails (left, low)
- IBM watsonx.governance (mid, high)
- 5090jepa (far right, far high) — highlight with a star

Caption: 2 sentences explaining the empty quadrant we occupy.
```

### E.4 — Regulatory timeline

```text
Produce a horizontal timeline (mermaid `timeline` syntax) covering Aug 2024 →
Aug 2027:
- 2 Feb 2025: EU AI Act prohibited practices in force
- 2 Aug 2025: GPAI obligations in force
- Dec 2023: ISO/IEC 42001 published
- Jan 2023: NIST AI RMF 1.0 published
- 2024: Cisco / Robust Intelligence acquisition ($400M)
- 2025: F5 / CalypsoAI acquisition ($180M)
- Sep 2025: HSB AI Liability for SMBs launches
- 2 Aug 2026: EU AI Act high-risk obligations in force ← STAR
- 2 Aug 2027: Embedded high-risk provisions

Caption: 2 sentences pointing at the Aug 2026 forcing function.
```

### E.5 — Market sizing chart

```text
Produce a horizontal bar chart (SVG) comparing market sizes:

- AI Governance Platforms (Gartner): 2026 $492M, 2030 $1B+
- AI Model Risk Management: 2026 $7.17B, 2030 $15B
- BFSI AI Governance Spend: 39% of total
- AI Insurance liability premiums (Munich Re aiSure et al.): [estimate $X]

Each bar labeled with absolute number + CAGR.

Caption: 1 sentence on TAM convergence on $15B by 2030.
```

### E.6 — Value-prop one-pager

```text
Produce a 1-page value-proposition infographic in HTML/CSS (single file). Print
size: US Letter (8.5"×11") portrait.

Layout:
- Header band: "5090jepa — The evidence chain regulators and insurers want."
- 3 columns:
  - "Without us": manual evidence assembly, 5 FTE-years per high-risk model,
    fines exposure, premium not negotiable.
  - "With wrappers": prompt-level guardrails, no world-state evidence, vendor
    lock-in risk.
  - "With 5090jepa": runtime evidence chain, 10× FTE reduction, insurer
    discount, model-provider neutral.
- Bottom band: 5 numbers in big type (use Master §17 verbatim).
- Footer: GitHub URL placeholder + contact email placeholder.

Use a serif headline font + sans-serif body. Color palette: deep navy + warm
accent. No clip art. No icons that aren't simple geometric shapes.
```

### E.7 — Customer journey diagram

```text
Produce a customer-journey diagram (mermaid `journey` syntax):

1. Awareness — reads HN front-page launch post.
2. Interest — runs the demo on their laptop.
3. Consideration — schedules 30-min discovery call.
4. Pilot — 90-day, $50-75K, 1 high-risk model instrumented.
5. Insurance — Munich Re premium-discount letter issued.
6. Expansion — Pro contract, all high-risk models.
7. Reference — co-authored case study, conference talk.

Score each stage 1-7 for {customer effort, customer benefit, our work}.

Caption: 2 sentences on the conversion lever (steps 4-5).
```

### E.8 — Pricing comparison table

```text
Produce a markdown table comparing our pricing to the competitive set:

Plan         | 5090jepa   | Lakera | Guardrails | Fiddler | Credo | AWS Bedrock | Datadog LLM
Free tier    | OSS full   | demo   | OSS        | free    | -     | included    | $120/day
Self-serve   | -          | -      | -          | $0.002/trace | - | -           | -
Mid-market   | $30K       | est $50K | -        | est $25K | est $40K | -        | est $50K
Enterprise   | $250K-$1M  | $250K+   | $200K+    | $250K+   | $200K+   | bundled  | bundled
Insurance discount | YES   | NO     | NO         | NO      | NO    | NO          | NO

Annotate each row with notes. Caption: 1 sentence on insurance-discount as the
unique line item.
```

### E.9 — Buy vs build calculator

```text
Produce an SVG infographic showing:

- Build path: 5 FTEs × $200K × 18 months = $1.5M; 70% chance of failure-to-pass-audit
- Buy path: $90K Pro license + $50K integration; deploys in 90 days; insurance
  discount $100K/yr; net annual cost minus discount = -$60K (negative = customer
  is paid to deploy)

Include break-even chart over 36 months.

Caption: 2 sentences. Anchor on Master Context §17 figures.
```

### E.10 — Runbook flowchart

```text
Produce a mermaid `flowchart` of the FSV verification harness:

Wipe DB → register-domain → ingest-event → materialize-panel → compile-trajectories
→ compile-dataset → train → predict → plan → record-surprise → write VerificationRunRecord

Branch each step into "happy path" + "edge case 1/2/3" with the expected count
deltas.

Caption: 2 sentences on FSV discipline.
```

### E.11 — Domain pack lifecycle diagram

```text
Produce a mermaid `stateDiagram-v2` showing a domain pack's lifecycle:

[*] → Authored → Registered → Active → Bumped → Deprecated → Retired → [*]

Annotate each transition with the responsible CLI command and the audit row
written.

Caption: 2 sentences on TOML-as-source-of-truth and source_hash discipline.
```

### E.12 — Compliance mapping matrix (large)

```text
Produce a markdown table where rows are regulations (EU AI Act Annex IV items,
NIST AI RMF Govern/Map/Measure/Manage, SR-11-7, ISO 42001 controls) and columns
are DynamicJEPA records or CFs.

Cells: ✓ if covered, ✗ if not, "via Compliance Pack" if requires the paid tier.

Use Master §7.2 verbatim. Add a footer note on the OSS vs commercial split.
```

### E.13 — ROI calculator visual

```text
Produce a calculator-style infographic in HTML/CSS the operator can paste into
a sales page.

Inputs (sliders):
- Number of high-risk models in production
- Avg compliance FTE per model
- Avg AI liability policy size
- Avg current premium %

Outputs:
- Current cost: FTEs × $200K + premium
- 5090jepa cost: license + integration + reduced premium
- Net savings, IRR, payback months

Use vanilla HTML+CSS+JS. Color palette: same as Master §E.6 one-pager.
```

### E.14 — Acquisition comps chart

```text
Produce a horizontal bar chart (SVG) plotting acquisition values over time:

- Aug 2024: Cisco / Robust Intelligence — $400M
- Sep 2025: F5 / CalypsoAI — $180M
- (Projected) 2027: 5090jepa — $300-700M ← shaded confidence band

Annotate each bar with acquirer's strategic motive.

Caption: 1 sentence positioning ourselves in the comp band.
```

### E.15 — Before / after side-by-side (regulatory audit)

```text
Produce an infographic (HTML/CSS) showing two columns:

Left "Before 5090jepa":
- Auditor receives: Word docs, PDFs, screenshots
- Time to answer questions: weeks
- Source of truth: hard-to-reconstruct logs across multiple systems
- Cost: 2-5 FTEs at $200K/yr per high-risk model

Right "After 5090jepa":
- Auditor receives: signed JSON evidence bundle, regulator portal access
- Time to answer questions: minutes
- Source of truth: 25 RocksDB column families + content hashes
- Cost: $90K-$1M license / yr enterprise-wide

Caption: 1 sentence on auditor-friendliness as a differentiator.
```

---

## F. LONG-FORM CONTENT

### F.1 — Whitepaper (15-25 pages)

```text
Produce a 15-25 page whitepaper. Output as a single markdown document with
section numbers, tables, mermaid diagrams.

Title: "The Evidence Chain: A Reference Architecture For Auditable AI Runtimes"

Structure:
1. Executive summary (1 page)
2. Problem framing — Annex IV / SR-11-7 / NIST AI RMF / ISO 42001 / AIBOM (3 pages)
3. Why current approaches fail (3 pages, includes wrapper-vs-runtime analysis)
4. Reference architecture — DynamicJEPA + Context Graph stack (4 pages)
5. The evidence chain — record types and cross-walk tables (4 pages, full §7.2)
6. Reproducibility — counter_world + gridworld FSV walk-through (3 pages)
7. Insurance distribution and the signal-game equilibrium (2 pages)
8. Open-source vs commercial split (1 page)
9. Path to deployment in regulated environments (2 pages)
10. References & further reading (1 page)

Include 5 figures (mermaid), 4 tables, 30+ inline citations to Master Context §X.

Tone: technical-marketing. Reads like a Google whitepaper or HashiCorp deep dive.
```

### F.2 — Technical blog post (HN front-page launch)

```text
Produce a 1500-2500 word technical blog post in markdown.

Title: "We trained a JEPA on a 5090 in 90 seconds — and it produces an EU AI
Act audit trail as a side effect"

Structure:
- Lede: the operator runs `verify-gridworld` and shows the evidence packet.
- Backstory: why we built this (the regulatory clock + wrapper inadequacy).
- Architecture walk-through — keep it concrete, with code snippets.
- The evidence chain explanation — how it satisfies Annex IV.
- The insurance angle — why insurers care.
- Demo reproduction guide — `git clone …; nvidia-smi …; cargo build …`
- The open-source / commercial split.
- What's next.
- Call to action: GitHub stars, Discord, hiring.

Voice: technical, slightly contrarian, anti-hype. First-person plural ("we").
Heavy on real terminal commands and JSON snippets.

Reference Master §3-7 for architecture, §10 for regulatory anchor, §13 for
strategy.
```

### F.3 — Customer case study template

```text
Produce a 2-page customer case-study template (markdown).

Sections:
1. Customer overview (name placeholder, industry, size, compliance regime).
2. Challenge — what model risk problem they faced (specific dollar pain).
3. Solution — how 5090jepa was deployed (90-day pilot pattern).
4. Results — quantified: FTE saved, audit time reduced, premium reduction %.
5. Quotes — placeholder for two operator quotes (CISO + CRO).
6. Architecture diagram — anonymised but specific (data sources, models,
   evidence flows).
7. What's next — expansion roadmap.

Length: 800-1200 words.

Style: B2B, restrained, numbers-first. Two pull-quote callouts. One diagram.
```

### F.4 — Industry analysis post (LinkedIn long-form)

```text
Produce a 1200-1800 word LinkedIn long-form post.

Title: "The AI Governance Market Is Consolidating — Here's What That Means For
Regulated Enterprises"

Structure:
1. Opening hook — Cisco $400M + F5 $180M + EU AI Act = market re-shape in 24
   months.
2. The wrapper-startup era is closing (name 5+ startups by category).
3. Hyperscaler bundles are commoditising the prompt-safety layer.
4. What this means for buyers — the procurement risk diagram.
5. The next moat — evidence-chain depth + insurance partnerships.
6. Practical advice for CISOs and CROs — RFP questions to ask.
7. Soft CTA — happy to chat.

Voice: industry analyst. No selling. The reader infers our authority.

Pull every fact from Master §10-§12. Cite vendor pages. Use markdown.
```

### F.5 — Regulatory commentary post

```text
Produce a 800-1200 word commentary post on EU AI Act Annex IV.

Title: "Annex IV Is Specific. Most AI Vendors Aren't."

Structure:
1. Annex IV's actual text — paraphrase the 8 documentation requirements.
2. What "general-purpose AI guardrails" tools document. (Gap analysis.)
3. What is structurally needed (evidence chain) and why hand-built docs fail.
4. Reference architecture pointer (link to whitepaper).
5. Practical guidance — what to put in your RFI today.

Voice: regulator-respectful. Precise. No marketing claims.

Pull regulatory facts from Master §10. Anchor architectural claims to §7.
```

---

## G. SHORT-FORM SOCIAL CONTENT

### G.1 — LinkedIn post (announcement)

```text
Produce 3 alternative LinkedIn posts (300-600 chars each) announcing the
open-source release of 5090jepa.

Variant A — punchy / contrarian: lead with "Most AI guardrail tools miss the
evidence regulators actually need."

Variant B — authoritative / industry analyst: lead with the EU AI Act Aug 2026
date.

Variant C — engineering / tech-detail: lead with "We trained a JEPA on a 5090
in 90 seconds and it generates an Annex IV evidence packet by default."

Each ends with a single CTA (GitHub link placeholder). No emojis (operator pref).
```

### G.2 — LinkedIn post (regulatory milestone reaction)

```text
Produce a 200-400 char LinkedIn post reacting to a real upcoming regulatory
milestone (Aug 2 2026 EU AI Act high-risk enforcement).

Tone: industry analyst, not founder. Make a specific prediction (e.g.,
"60% of enterprises won't have Annex IV evidence in place by Aug 2 2026").

Anchor on Master §10.1.
```

### G.3 — X / Twitter thread (5-12 tweets)

```text
Produce a 7-tweet X thread for tech audience.

Tweet 1: hook — "Regulators don't want your prompt logs. They want your
runtime's evidence chain." (≤ 240 chars)
Tweet 2: the gap.
Tweet 3: what evidence chain means.
Tweet 4: counter_world demo screenshot description.
Tweet 5: gridworld demo screenshot description.
Tweet 6: open-source repo link.
Tweet 7: "Hiring Rust + GPU + storage engineers — DM."

Each tweet ≤ 280 chars. Use line breaks. No emojis.
```

### G.4 — X thread (insurance angle)

```text
Produce a 6-tweet X thread on the insurance distribution mechanism.

Tweet 1 hook — "Munich Re will pay your customer 10-20% premium discount to
deploy your AI governance tool. Here's how to be that tool."
Tweet 2-5 — mechanism design walkthrough (Master §13.2).
Tweet 6 — link to game-theory analysis blog post.
```

### G.5 — Hacker News launch post

```text
Produce an HN launch post ("Show HN: …") for the 5090jepa open-source release.

Title (≤ 80 chars): "Show HN: 5090jepa – action-conditioned JEPA runtime that
emits an EU AI Act audit trail by default"

Body (250-500 words): backstory, what it does, why it matters, repro
instructions (`git clone ...`), what we'd love feedback on.

Anti-patterns to avoid:
- Marketing tone.
- Buzzwords ("revolutionary", "disruptive").
- Claims you can't substantiate in the comments.

Voice: peer engineer, honest about what's done and what's open.
```

### G.6 — Newsletter blurb (200-400 words)

```text
Produce a 200-400 word newsletter blurb for an enterprise AI weekly.

Lead: "If you're a CISO or CRO at a bank or insurer, here's the calendar item
that should be on your wall."
- Date: Aug 2 2026.
- Why: EU AI Act high-risk enforcement.
- The procurement window is closing.
- What 5090jepa does and why it's open-source.
- 2-sentence link to demo.
```

### G.7 — Conference talk abstract (250-word CFP submission)

```text
Produce a 250-word conference abstract for NeurIPS / Strata / Money 20/20 /
RSA / FOSDEM / DevOpsDays.

Title (≤ 12 words).
Abstract: 200 words.
Audience: 30 words.
Speaker bio placeholder: 30 words.

Make 4 versions, one per conference type, adjusting the angle (research,
data infra, fintech, security, devops, OSS).
```

### G.8 — Investor email (cold)

```text
Produce a 4-email cold-investor outreach sequence (each 80-150 words):

E1: subject line + 4-line intro + ask for 30 min.
E2: follow-up 4 days later with one specific data point (e.g., Cisco $400M
acquisition reference).
E3: follow-up 8 days later with the demo video link.
E4: break-up email 14 days later — "no further contact unless interested".

Each email opens with a personalisation placeholder ([their portfolio company X]
or [their recent post Y]) for the operator to fill in.

No emojis. No "I hope this finds you well".
```

---

## H. SALES ENABLEMENT

### H.1 — Cold-outbound email sequences

```text
Produce four 5-email cold-outbound sequences (3-day intervals), one per ICP:

Sequence A — BFSI CISO / CRO (model risk lens).
Sequence B — Insurance carrier (underwriter lens).
Sequence C — Healthcare CTO (FDA AI/ML SaMD lens).
Sequence D — F500 manufacturing CISO (predictive-maintenance lens).

Each email 80-150 words. Each has:
- Personalisation hook (placeholder bracketed).
- A specific data point from Master Context.
- A 1-sentence CTA.
- A signature block.

The 5-email cadence:
1. Pattern interrupt + data point.
2. Architecture preview (1 paragraph + diagram link).
3. Customer case study tease (placeholder if no signed customer yet).
4. ROI calculator.
5. Break-up.
```

### H.2 — Demo discovery questions

```text
Produce a 30-question discovery checklist a sales engineer asks during a
60-minute pre-pilot discovery call. Categorise into:

A. AI footprint (5 questions: model count, frameworks, providers, deployment
modes, data sensitivity).
B. Compliance footprint (5: SR-11-7? EU AI Act exposure? ISO 42001? SOC 2?
GDPR? FDA?).
C. Current tooling (5: Lakera/Guardrails/Credo/Fiddler/Arize?).
D. Insurance footprint (5: AI liability rider? carrier? policy size? claims
history? renewal date?).
E. Procurement (5: budget cycle? signing authority? procurement timeline?
preferred deployment?).
F. Success criteria (5: what does success look like 90 days in? 12 months in?
who reviews? what's the renewal trigger?).

Each question short, answerable in ≤ 2 minutes.
```

### H.3 — ROI calculator spec

```text
Produce a precise spec for an ROI calculator embedded on the marketing site.

Inputs (operator-controlled):
- # of high-risk AI models in production
- # of compliance FTEs per model
- Average AI liability policy premium ($)
- Average current AI liability premium % of policy
- Compliance team annual budget

Outputs (auto-calculated):
- Current annual cost = FTEs × $200K × models + (premium × premium %)
- 5090jepa annual cost = chosen tier license + estimated integration ($X)
  + (premium × reduced premium %)
- Net annual savings
- 3-year cumulative IRR
- Payback in months

Specify the exact formula for each output. Include 3 sample personas with
filled inputs and computed outputs.
```

### H.4 — One-page sales sheet

```text
Produce a 1-page (US Letter) sales sheet in HTML/CSS.

Top: tagline + logo placeholder.
Middle: 3 columns (Problem / Approach / Outcome) with bullets.
Bottom: 5 numeric proof points + CTA.

Color palette navy + amber. Print-friendly. No icons.
```

### H.5 — Battle card vs Lakera

```text
Produce a 1-page battle card comparing 5090jepa vs Lakera.

Sections:
- Snapshot (positioning, funding, founded).
- Where they win (named scenarios).
- Where we win (named scenarios).
- "What to say if asked X" (5 anticipated objections).
- Migration story for prospects already using Lakera.

Anchor on Master §11.3 + §9.

Produce identical battle cards for: Guardrails AI, Fiddler, Credo, Arize,
Cisco AI Defense, F5 ADSP, IBM watsonx.governance, AWS Bedrock Guardrails,
Datadog LLM Observability.
```

### H.6 — Mutual NDA + DPA template

```text
Produce a starter mutual NDA template and a starter DPA (Data Processing
Agreement) template for use with regulated-enterprise prospects.

Each ≤ 2 pages. Markdown. Plain English. Bracketed placeholders for jurisdiction
and signatories. State the operator must have legal counsel review before signing.

Include the standard data-classification + breach-notification clauses.
```

---

## I. PR / LAUNCH ANNOUNCEMENTS

### I.1 — Press release: open-source launch

```text
Produce a 500-word press release for the open-source launch of 5090jepa.

Headline (≤ 12 words): start with a specific outcome, not a verb.
Subhead (≤ 25 words): the elevator-pitch sentence.
Dateline: [city, date].
Lead paragraph: who/what/when/where/why.
Body 1: the problem (Annex IV / Aug 2 2026).
Body 2: what 5090jepa does (architecture in 3 sentences).
Body 3: the open-source / commercial split.
Body 4: a quote from the founder (placeholder).
Body 5: a quote from a customer or insurance carrier (placeholder).
Boilerplate.
Contact info placeholder.

Use AP style. Include 3 numeric proof points from Master §17.
```

### I.2 — Press release: Series A funding

```text
Produce a 600-word Series A funding announcement.

Headline: include $ amount and use of funds.
Lead: investor names, total raised, valuation if disclosed.
Body: traction proof points, named lighthouse customers (or "5 BFSI design
partners"), insurance partnership.
Quotes: founder, lead investor, insurance partner.
Boilerplate.

Mark every customer / valuation number as placeholder if unconfirmed.
```

### I.3 — Press release: insurance partnership

```text
Produce a 500-word press release for the first AI insurance carrier
partnership.

Lead: carrier name + 5090jepa + premium-reduction product.
Body: the mechanism (Master §13.2), expected customer benefit, joint pilot.
Quotes: 5090jepa founder, carrier head of underwriting.

Pre-flight: emphasise "joint pilot", not "joint product launch", until the
carrier signs off on the latter.
```

### I.4 — Press release: lighthouse customer win

```text
Produce a 500-word press release for the first publicly-named BFSI customer.

Lead: customer's name + use case + Q3 2026 EU AI Act readiness.
Body: integration shape (which high-risk model), evidence packet outcome,
insurance discount issued.
Quotes: customer CISO, customer CRO, founder.

Mark every detail as placeholder if not yet signed off by customer comms.
```

### I.5 — Founder LinkedIn post: 1-year reflection

```text
Produce a 600-word LinkedIn long-form by the founder, 1 year after launch.

Tone: reflective, lessons-learned, slightly self-deprecating. Specific
on what we got wrong + corrected.

Topics:
- The R7 lesson: assume the task is wrong.
- The day we mutated a byte and discovered ArtifactHashMismatch worked.
- The first insurance partnership conversation.
- The first audit a regulator referenced our format in.
- What we were wrong about.
- What's next.

End with a recruit-the-next-cohort CTA.
```

---

## J. REGULATOR / INSURER SPECIFIC

### J.1 — EU AI Office briefing memo

```text
Produce a 4-page internal memo (markdown) for sharing with EU AI Office staff.

Sections:
1. Submitter identity + open-source posture.
2. Annex IV mapping (verbatim Master §7.2).
3. Reproducibility — domain-pack TOML examples.
4. Conformity assessment alignment.
5. Pilot offer for an EU regulated entity.
6. Consent for our reference architecture to be cited in published guidance.

Tone: regulatory-respectful. Specific paragraph-level citations of EU AI Act.
```

### J.2 — Munich Re aiSure underwriter briefing

```text
Produce an 8-page underwriter briefing for Munich Re's AI underwriting team.

Sections:
1. The reduction-of-uncertainty thesis.
2. Worked example: a hypothetical $10M AI Liability policy at 5% premium with
   1.5% claim probability vs reduced 0.8% claim probability under our evidence
   chain.
3. Operational requirements for the carrier (data ingest, claim-evidence flow).
4. Joint pilot proposal — 5 mutual customers, 12 months, blinded loss-ratio
   data sharing.
5. Pricing-discount ladder structure.
6. Renewal evidence cadence.
7. Joint marketing terms.
8. Next steps.

Tone: actuarial. Use carrier-friendly terminology (combined ratio, frequency,
severity, reserves, attachment point).
```

### J.3 — Federal Reserve / OCC supervisory briefing

```text
Produce a 5-page memo for use in a Fed / OCC examiner conversation.

Cover SR-11-7 cross-walk, model-risk-management workflow integration,
audit-trail format, regulator-mode portal access.

Tone: factual, conservative, no marketing.
```

---

## K. INTERNAL DOCUMENTS

### K.1 — 12-week sprint plan

```text
Produce a 12-week sprint plan, week-by-week, for an engineering team of 4-6
shipping the 5090jepa demo to production-ready open-source release.

Each week:
- Top 3 deliverables.
- Owner.
- Definition of done (FSV-passing).
- Blocking risks.
- Demo at end-of-week.

Weeks 1-3 = Phase 0-1 (fixtures, core types).
Weeks 4-6 = Phase 2-3 (storage data plane + CLI).
Weeks 7-9 = Phase 4-6 (adapter, panels, datasets).
Weeks 10-12 = Phase 7-10 (model, prediction, planning, FSV harness).

Reference `docs/dynamicjepa/implementationplan/14_phase_task_breakdown.md`.
```

### K.2 — Hiring scorecards (4 roles)

```text
Produce 4 hiring scorecards (1 page each):

A. Founding Engineer — Rust + GPU + storage.
B. Founding Sales (BFSI / regulated enterprise).
C. Compliance Officer (SOC 2 + ISO 42001 lead).
D. Founding GTM Ops (data-driven sales+marketing).

Each scorecard:
- Mission (1 sentence).
- 5 outcomes the hire will deliver in first 6 months.
- 8-12 competencies, 5-point scale.
- Disqualifiers.
- Compensation band placeholder.
- Sample interview loop (4 stages).
```

### K.3 — Quarterly OKR template

```text
Produce a quarterly OKR template (1 page).

Format:
- Quarter, year, theme.
- 4 objectives.
- Each objective has 3 key results (each must be measurable, time-bounded).
- Each KR has an owner.
- Each KR has a confidence score (0-1).

Example: Q3 2026 OKRs aligned to the 12-month operating plan.
```

### K.4 — Board update template (monthly)

```text
Produce a 4-page monthly board update template.

Sections:
1. TL;DR — 5 bullets.
2. Pipeline — funnel stages, names, conversion %.
3. ARR + cash runway.
4. Hiring.
5. Product — what shipped, what's blocked.
6. Risks — top 5 + mitigation owner.
7. Asks of the board.

Tone: candid. Numbers > narrative.
```

---

## L. BRAND / VISUAL STYLE GUIDE

```text
Produce a 4-page brand & visual style guide.

Sections:
1. Brand voice — calm, technical, regulator-respectful, slightly contrarian.
2. Vocabulary — preferred terms ("evidence chain", "structurally", "by default")
   and forbidden terms ("disruptive", "revolutionary", "AI safety" used alone).
3. Color palette — navy primary (#0B1F3A), amber accent (#F0B53A), neutral grays.
4. Typography — IBM Plex Serif for headlines, IBM Plex Sans for body, IBM Plex
   Mono for code/CLI.
5. Logo guidelines — min size, clear space, do/don't.
6. Diagram style — minimal lines, 1px strokes, 3-color max, no shadows.
7. Slide template — Marp, 16:9, slide title 36pt, body 18pt, code 14pt.
8. Photography — none, ever. Use diagrams instead.
9. Iconography — Heroicons outline only.
10. Tone-by-audience matrix (regulator / engineer / buyer / press).
```

---

## M. META-PROMPT (Let The AI Invent New Prompts)

```text
You are an expert at prompt engineering for AI-driven content. Given the
master context guide for 5090jepa and the existing prompt library above,
generate 10 new prompts I have not yet considered for assets that would
support our go-to-market strategy.

For each new prompt:
1. Name the asset type (e.g., "Acquisition diligence one-pager").
2. State the audience.
3. State the goal (specific business outcome).
4. Provide the full prompt text in a code block, ready to copy-paste.
5. Anchor on relevant Master Context sections.

Bias toward assets that:
- Compress operator effort (one prompt produces a deliverable a person can ship).
- Cover gaps in our current library (e.g., partner-channel enablement,
  competitive teardowns, customer-advisory-board kit).
- Ladder up to the next milestone (Series A, lighthouse customer, insurance
  MoU).

End your response by asking me which 3 of the 10 I want to commit to the
library.
```

---

## CLOSING NOTES

This library is meant to be amended, not frozen. When a new asset type proves
useful, add a new prompt block. When a prompt produces lazy output, sharpen
its constraints. When the Master Context Guide changes, audit prompts for
broken references.

The two documents (`5090JEPA_MASTER_CONTEXT_GUIDE.md` and this one) together
form a self-contained content factory: an AI with both can produce strategy
memos, sales decks, technical talks, regulator briefings, social posts,
press releases, customer one-pagers, and recruiting collateral — all
anchored in the same source of truth.

Update cadence: quarterly. Owner: founder + GTM lead.
