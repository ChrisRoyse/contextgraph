# 07 — Content Factory

> The production pipeline. Six layers (research → brief → draft → distribute → AI-search visibility → attribution), tuned for Hormozi-volume on owned surfaces, channel-respectful cadence on third-party surfaces, GEO/SEO native, brand-voice-gated. One anchor piece × N derivatives × N platforms = Hormozi's "450 pieces per week" without any one channel seeing more than its rate cap.

---

## §1. The pipeline (Knowlee 6-layer)

Adapted from the AI marketing agent fleet pattern. Each layer is owned by one or more agent classes from `05_AGENT_FLEET_DESIGN.md`:

```
RESEARCH ──► BRIEF ──► DRAFT ──► DISTRIBUTE ──► AI-SEARCH VISIBILITY ──► ATTRIBUTION
   S5/O1     P4         P1/P2     D1            O2                       O7/M7
```

| Layer | Owner agents | Output | Time per artifact |
|---|---|---|---|
| Research | S5 Competitive Intel + O1 SEO + S2 Signal Listener + Leapable wiki query | `topic_brief.yaml` | ~5 min |
| Brief | P4 Brief Generator | Structured brief with H1/H2 outline, target keyword, target reader, CTAs, evidence pointers, voice notes | ~3 min |
| Draft | P1 Content Writer (or P2 Technical Writer) | Long-form markdown anchor; brand-voice scored | ~10 min |
| Distribute | D1 Multi-Platform Publisher | 8 short-form derivatives × N platforms, scheduled per channel cards | ~15 min |
| AI-search visibility | O2 GEO Optimizer | Structured FAQ + comparison tables + citations to maximize ChatGPT/Claude/Perplexity quotation | ~5 min |
| Attribution | O7 Cohort Health + M7 Evaluator | SoT readback: did the artifact move the goal predicate? | ongoing |

Total: ~40 minutes of agent compute per anchor piece. Cost per anchor at default model routing: ~$1.50–$3.00. Hormozi-recommended cadence is one anchor per day for serious operators.

---

## §2. Hormozi volume reconciled with platform respect

> Hormozi: "We put out 450 pieces of content per week."
> Joe Karlsson DevRel: "Pick one or two community channels and go deep. Not five."

These are not contradictory. They are two different surfaces:

- **Owned surfaces (Hormozi's volume target):** the operator's website, blog, podcast, YouTube channel, newsletter. No rate cap. Volume = leverage.
- **Third-party platforms (DevRel "go deep"):** GitHub, Discord, Reddit, HN. Volume here = automated-looking. Depth, not breadth.

The factory produces volume on owned surfaces and **derivative cadence** on third-party platforms. One anchor piece per day, refactored into many formats, distributed at each channel's natural pace.

### 2.1 The one-to-many amplifier

```
1 daily anchor (long-form, ~2000 words)
  │
  ├── 1 YouTube long (10-15 min)
  ├── 1 podcast episode (audio strip)
  ├── 1 X thread (8-12 tweets)
  ├── 1 LinkedIn post (1200 chars)
  ├── 1 newsletter blurb (300 words)
  ├── 1 Dev.to / Hashnode cross-post (technical anchor)
  ├── 1 Mastodon post
  ├── 1 Bluesky post
  ├── 4 short-form vertical clips (TikTok / IG Reels / YT Shorts / X video)
  ├── 1 carousel (LinkedIn or Instagram)
  ├── 1 infographic
  └── 3 quote graphics (per high-leverage line in the anchor)
```

Total artifacts per anchor: **18+**. Per week with daily anchor: **126+**. Per month: **540+**. This is Hormozi-volume, achieved through aggressive repurposing, not through agent spam.

Joe Karlsson's rule still binds: of those 540 artifacts, the **depth** investments — actual responses in Discord, comments under others' posts, GitHub issue triage, podcast appearances — get the deep treatment. The repurposing is bulk; the engagement is hand-shaped.

---

## §3. Brand voice spec

The single most-violated thing in autonomous content systems is brand voice. The factory's defense is a structured brand voice document plus a fine-tuned evaluator.

### 3.1 The brand voice doc

`brandvoices/<product>.md`:

```yaml
---
product: contextgraph
last_updated: 2026-05-26
calibration_kappa: 0.78  # against operator labels
---

# ContextGraph Brand Voice

## Stance
- Technical first. We assume the reader is a working AI engineer.
- Direct. No throat-clearing. Lead with the claim; back with evidence.
- Verifiable. Every quantitative claim must come from a cited measurement.

## Voice
- Confident but not arrogant. We benchmark against ourselves and against named alternatives.
- We do not say "delve," "tapestry," "in the realm of," "the journey," "unleashing," "navigate the complexities." (AI-tell phrases.)
- We do not say "we're excited to announce." We just announce.
- We do not say "thought leadership."

## Voice negative space
- No emoji in long-form. (Sparing in short-form.)
- No exclamation points outside celebratory operator-personal posts.
- No em-dashes (operator preference); use semicolons or restructure.
- No begging phrases ("hope you enjoyed!", "let me know what you think!").

## Structural
- Headlines: claim-first ("ME-JEPA-Code closes 85% of patches Cursor leaves open"), not curiosity-bait ("You won't believe what we just found...").
- Numbers preferred over adjectives. "12.4× faster" not "much faster."
- Code preferred over English when explaining technical mechanics.

## Forbidden phrases (regex deny)
- (?i)\b(unleash|delve|tapestry|navigate the complexities|in the realm of|game-?chang(er|ing))\b
- (?i)\bsynerg(y|ies|istic)\b
- (?i)\brevolutionar(y|ize)\b

## Reference exemplars
- examples/2026-04_blog_post_evaluated_excellent.md
- examples/2026-05_thread_evaluated_excellent.md
```

### 3.2 The evaluator

The Brand Voice Scorer (`05_AGENT_FLEET_DESIGN.md §6.3`) is fine-tuned on the operator's approved historical content. It produces a 0–1 score on every draft. Below the goal-spec threshold (default 0.85) the draft is rejected and the Writer retries with the score's diagnostic feedback.

Quarterly recalibration:
- Operator reviews 50 random draft scores blind, labels them.
- Kappa against the scorer must hold above 0.6 (G-Eval calibration target for marketing copy).
- Below 0.6 → re-train the scorer on the new labels.

This is the same calibration discipline as `15_METRICS_AND_OBSERVABILITY.md §7`.

---

## §4. The Hormozi Value Equation in every artifact

Hormozi (`$100M Offers`):

> Value = (Dream Outcome × Perceived Likelihood) ÷ (Time Delay × Effort & Sacrifice)

Every content brief carries a Value Equation block. Drafts are checked against it.

```yaml
value_equation_block:
  dream_outcome: "Ship AI-generated patches that pass Docker oracle on first try"
  perceived_likelihood: "ME-JEPA-Code reaches 0.95 oracle correlation on Python SWE-bench Lite"
  time_delay: "Seconds per patch (vs minutes per human review)"
  effort_sacrifice: "No new tools to learn — drop-in MCP server"
target_reader_states_post_consumption:
  - "I now believe ContextGraph + ME-JEPA-Code can actually verify what Claude/Codex produces"
  - "I have a specific reason to try it this week (not someday)"
  - "I know the next step (install + run heal-drill demo)"
```

If a draft doesn't explicitly hit the dream outcome, perceived likelihood, time delay, and effort/sacrifice — the brief is rewritten. This is mechanical. The factory doesn't ship copy that fails to express the value equation any more than it ships copy that fails the brand voice score.

Hormozi's one-liner template for warm and cold outreach:

> *"I help (ideal customer) get (dream outcome) in (time period) without (effort and sacrifice) and (increase perceived likelihood of achievement)."*

The Outreach Drafter (D4) uses this exact structure. Every cold message has all four blanks filled.

---

## §5. SEO + GEO simultaneously

### 5.1 SEO (still matters)

Programmatic SEO via the Programmatic SEO Engineer (O1) generates landing pages at scale for long-tail queries the audience actually searches:

- ContextGraph: `"how to verify <patch type> with ME-JEPA"` × 60 patch types
- Synapse: `"<task> with MCP computer use"` × ~150 tasks
- Polis: `"third place in <city>"` × top-100 cities; `"how to meet <demographic> in person <city>"`

Each template page is operator-approved (the *template*, not each page). Pages auto-generate from the data spine; the Evaluator's hallucination check ensures every claim is grounded in either operator-provided data or verifiable third-party data.

### 5.2 GEO (the bigger lever in 2026)

ChatGPT serves 800M WAU; Gemini 750M MAU; Perplexity 45M; Claude 30M. Combined dwarf Google for many queries. **Citation in their answers is the new ranking signal.**

The GEO Optimizer (O2) does:
- **Answer-first content structure** — the answer to the query appears in the first 200 words.
- **TL;DR sections** at the top of long-form (AI engines extract these preferentially).
- **Cited statistics** with primary sources (AI engines prefer content with explicit numbers + citations).
- **FAQ schema** (`schema.org/FAQPage` JSON-LD) — direct ingestion path for AI engines.
- **Comparison tables** — AI engines lift table rows verbatim when answering "X vs Y" queries.
- **Allow AI crawlers** in `robots.txt` (`GPTBot`, `ClaudeBot`, `Google-Extended`, `PerplexityBot`).

Per `2026-04 LLMrefs guide`: SEO and GEO winners share only ~20% overlap (down from ~70% in 2024). They need different optimization.

### 5.3 GEO measurement

The fleet runs a weekly Citation Audit: queries the AI engines via their APIs (where available) and via Synapse computer-use for the rest with a basket of ~100 product-relevant questions. The audit produces:

- Citation share: % of questions where the operator's content was cited
- Position: first / second / N-th cited source
- Quoted-text fidelity: does the AI engine quote the operator's words verbatim or paraphrase
- Competitor citation share: who else is being cited

GEO performance is a Tier-2 metric on the per-goal scoreboard.

---

## §6. The anchor archetypes

The factory produces seven anchor archetypes. Each has its own template, voice, and evaluator weight:

| Archetype | Purpose | Length | Cadence |
|---|---|---|---|
| **Deep dive** | Cornerstone technical content, the kind that earns HN front page | 2,500-5,000 words | 1 / week |
| **Tactical post** | Specific how-to, code-led | 800-1,500 words | 2 / week |
| **Comparison** | "X vs Y" content (high GEO yield) | 1,500-2,500 words | 1 / week |
| **Build log** | Karpathy/Linear-style "this week we shipped X" | 600-1,000 words | 1 / week |
| **Postmortem** | "We tried X. It failed. Here's why." High trust signal. | 1,000-2,500 words | 1 / month |
| **Founder thread** | Operator's personal narrative; not delegated to agent | 500-2,500 chars | 2-3 / week (operator-written) |
| **Behind-the-numbers** | Charts + analysis on benchmark/usage/growth | 1,000-2,000 words | 1 / month |

The Planner picks the archetype per the goal's current need: behind on awareness → comparison / deep dive. Behind on trust → postmortem / behind-the-numbers. Behind on conversions → tactical post.

---

## §7. Hallucination defense

Three layers:

1. **Evidence pinning** — every quantitative claim in a draft must reference a numbered evidence item in the brief. The brief carries the evidence items as `{id, source, quote, url}`. The Writer is allowed to use only those items; the Evaluator deny-lints any unsourced number.

2. **Reverse citation check** — the Evaluator runs a programmatic check: for each cited quote, fetch the URL and verify the quote exists. Misquotes are blocked before publish.

3. **Operator hallucination audit** — weekly sample of 20 published artifacts is operator-reviewed for factual integrity. Any hallucination triggers a Reflector post-mortem and an update to the writer's reflection memory.

---

## §8. Persona handling

When the operator runs multiple personas (a personal account, a product account, optionally one or more pen-name personas), each anchor is written for **one** persona and only repurposed within that persona's channel set. The Persona Carrier (P5) enforces — content tagged `persona: chris` cannot be published from `persona: contextgraph_team`.

Each persona has:
- Own brand voice doc
- Own forbidden phrases list
- Own posting cadence per platform
- Own publication history (used for "we already covered this" dedupe)

The Wiki Gardener (M10) detects content drift across personas and flags it.

---

## §9. The repurposing rules

The Multi-Platform Publisher (D1) takes the anchor and produces derivatives by **applying transformation rules**, not by free-form rewriting. Each rule preserves the value equation and the brand voice; only format and length change.

| Source | Target | Transformation |
|---|---|---|
| 2000-word anchor | X thread | Take the H1s as tweet 1; turn each H2 into 1-3 tweets; carry over the numbers; CTA at end |
| 2000-word anchor | LinkedIn post | First 250 chars must be a complete hook (LI cuts at "see more"); use 3-paragraph structure; CTA at end |
| 2000-word anchor | TikTok / Reels script | 8-15s vertical; visual + voiceover script; hook in first 1.5s |
| 2000-word anchor | Newsletter blurb | TL;DR + 3 takeaways + "read the full piece" CTA |
| 10-minute YouTube | 3-5 vertical clips | Pull the highest-engagement 15-30s segments; auto-captions; vertical re-frame via Synapse |
| Quote line | Quote graphic | One-liner + operator headshot; brand-template auto-generated |

Rules-based transformation keeps derivatives consistent with the anchor's argument. Free-form rewriting drifts.

---

## §10. The Hormozi Rule of 100 (calibrated)

Hormozi's daily prescription for an operator getting started:

- 100 outreaches/day (warm + cold)
- 100 minutes spent making one post
- $100/day on ads
- 100 minutes improving ads

For the autonomous fleet, the calibrated version per active goal:

| Activity | Daily target | Owner | Notes |
|---|---|---|---|
| Outreaches (combined warm + cold) | 30-100 per active goal | D4 Outreach Drafter | Bounded by per-account rate caps; operator-approved per net-new |
| Substantive post production | 1 anchor + ~12 derivatives | P1 Writer + D1 Publisher | 100 min of human-equivalent compute; ~5 min of agent wall-clock |
| Paid ads spend | Within goal's `daily_velocity_cap_usd` | O4 Ad Operator | Default $100-500/day during validation; up to budget ceiling once channel proves out |
| Ad improvement iterations | 5-20 per channel | O4 Ad Operator | Per-channel A/B variant generation + test |

The Hormozi prescription was designed for a single human operator. The fleet scales the same prescription to whatever the budget supports, with the Cost Pacer keeping it honest.

---

## §11. Cross-references

- Channel cards: `08_DISTRIBUTION_CHANNELS.md`
- Brand voice evaluator: `15_METRICS_AND_OBSERVABILITY.md §7`
- GEO measurement: `15_METRICS_AND_OBSERVABILITY.md §4`
- Hallucination check: `05_AGENT_FLEET_DESIGN.md §3`
- Persona policy: `02_FIRST_PRINCIPLES.md §19`
- Programmatic SEO + GEO playbooks: `12_DEVELOPER_MARKETING.md §6`
