# 12 — Developer Marketing (DevRel Flywheel)

> For Leapable and Synapse, developer marketing is the engine. Borrowed from FORKOFF's AI DevRel Flywheel + the Cursor / Linear / Vercel / Anthropic playbooks. Five surfaces — notebooks, open-source refs, hackathons, content DevRel, integrations guild — that compound into each other. Joe Karlsson's depth-over-breadth law applies. Hormozi's volume applies to artifact production; depth applies to engagement.

---

## §1. The AI DevRel Flywheel (5 surfaces)

```
   ┌─────────────────┐
   │   Notebooks     │  ← primary trust-transfer surface
   │   (Jupyter,     │
   │    Colab)       │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Open-Source     │  ← reference implementations
   │ Refs (GitHub    │
   │ repos)          │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Hackathons      │  ← compress trust into shipped artifacts
   │                 │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Content DevRel  │  ← long-form that earns citation
   │                 │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Integrations    │  ← official partnerships
   │ Guild           │
   └────────┬────────┘
            │
            └────► (back to notebooks for new integrations)
```

Each surface compounds the next. A great notebook earns GitHub stars; the starred repo gets found in hackathon submissions; hackathon projects become content DevRel case studies; case studies earn integration partner conversations; new integrations seed new notebooks.

This is **not 5 channels to run in parallel from day 1.** It's a sequence. Hormozi's "pick one, max it" applies: notebooks first, until a baseline is in place, then OSS refs, then hackathons.

---

## §2. Surface 1 — Notebooks

The primary trust-transfer surface for AI dev tools in 2026. A 3-minute time-to-first-output Jupyter or Colab notebook lets a skeptical developer copy-paste and *believe*.

### 2.1 What good looks like

- **Time to first output ≤ 3 minutes.** From open Colab → result shown.
- **No setup gymnastics.** No API key required (use free tier); no Docker assumed; no GPU assumed.
- **Side-by-side outputs.** Show the tool's output next to the alternative (without-tool, or competitor) for direct comparison.
- **One canonical artifact per use case.** Leapable: "Verify a Claude Code patch." Synapse: "Drive Notepad with an AI agent."
- **Weekly maintenance.** Stale notebooks (broken APIs, deprecated dependencies) are net-negative trust. Updated every Friday.

### 2.2 Initial notebook set per product

Leapable (target: 12 notebooks in first 90 days):
1. "Verify your first Claude Code patch in under 60 seconds"
2. "Why Leapable disagrees with the dashboard — three real cases"
3. "Building a custom rule library for Django projects"
4. "Leapable + Cursor: the full setup walkthrough"
5. "Comparing Leapable verdicts to oracle outcomes on 100 SWE-bench tasks"
6. "Catching subtle-flip mutations the human reviewer missed"
7. "Integrating Leapable into a CI pipeline"
8. "How the conformal interval prevents false confidence"
9. "Reading the named-failure-mode hierarchy"
10. "The TCT Gτ guard explained with worked examples"
11. "Building a community rule library: contributor walkthrough"
12. "Leapable on a non-Python codebase: a research preview"

Synapse (target: 12 notebooks in first 90 days):
1. "Drive Notepad with an AI agent in 60 seconds"
2. "Your first MCP computer-use session with Claude"
3. "Why Synapse vs. Browser-Use vs. Playwright — when to use which"
4. "Building a custom Synapse automation playbook"
5. "Game inputs: Synapse + ViGEm for controller emulation"
6. "Accessibility: Synapse for hands-free desktop operation"
7. "Multi-machine orchestration with Synapse"
8. "Capturing OCR + audio for full perception coverage"
9. "Reflex controllers for sub-frame latency in game scenarios"
10. "The operator panic hotkey and safety guarantees"
11. "Audit logging: tracing every Synapse action"
12. "Synapse + Leapable: verifying that the agent did what it claimed"

### 2.3 Metrics

Per FORKOFF benchmarks:
- GitHub stars / week target: **≥ 50** after notebook 3
- Week-1 reproduction rate target: **≥ 75%** (operators-or-readers actually run it successfully)
- Maintenance freshness target: **median age of notebook last-touch ≤ 30 days**

---

## §3. Surface 2 — Open-Source Refs

Reference implementations. Distinct from notebooks: notebooks are step-by-step pedagogy; refs are full working examples that show how to integrate at production scale.

### 3.1 What good looks like

- **One repo per integration target.** `leapable-claude-code-example`, `leapable-codex-example`, `leapable-cursor-example`, `leapable-continue-example`.
- **README that walks talk.** Hormozi's principle: prove the concept by being the concept. The README itself is the strongest marketing asset.
- **CI on the repo runs the example.** Demonstrates the example actually works.
- **README quickstart in 5 commands max.** Five lines, one paste, success.

### 3.2 GitHub-as-marketing channel

Per the Joe Karlsson 2026 DevRel observations:

> *"Your README is your primary marketing surface for IDE-native AI. When a developer asks Claude Code how to implement something, the model reaches into training data that includes your GitHub repo."*

The READMEs need to be **structured for AI engine extraction** as much as for human reading:
- First 200 words answer "what does this do and why?"
- Clean code blocks (no smart-quotes; no markdown breakage)
- FAQ schema in repo-level docs (the docs site)
- Tables comparing the tool to alternatives

### 3.3 SLAs as product marketing

The 5W developer-led-growth playbook:

> *"Publish SLAs: 48-hour issue response, one-week PR review, quarterly doc audit. Every open-source repo is product marketing for every future enterprise evaluator. A stale repo costs deals."*

Operator's SLA commitments per repo:
- Issue acknowledgment: 24h on weekdays
- Issue triage decision: 72h
- PR review: 1 week
- Doc audit: quarterly
- Release cadence: at least monthly

These are mechanical commitments. The GitHub Maintainer agent (D3) enforces them.

---

## §4. Surface 3 — Hackathons

Compress months of marketing into 48 hours of shipped artifacts. Sponsored, attended, or operator-hosted.

### 4.1 Three modes

| Mode | When | Cost (typical) | Outcome |
|---|---|---|---|
| **Sponsor a hackathon** | Operator wants reach to a specific community | $2-10k per event | Logo + booth + prize sponsorship + workshop |
| **Attend with team** | Operator wants direct face-time with target builders | Travel + $500-2k | Karpathy "find the right people" mode |
| **Host operator-run hackathon** | Operator wants exclusive community + branded artifact set | $5-25k | All submitted projects use the operator's tool; case studies for content DevRel |

For Leapable and Synapse in the first 90 days, **sponsor 3 hackathons** targeting AI agent / AI engineering audiences:
- AGI House hackathons
- AI Engineer Summit hackathons
- MCP-specific (Anthropic-organized) hackathons
- Cursor + Anthropic joint hackathons
- Replit AI hackathons
- Latent Space community events

### 4.2 The Hackathon Strategist agent

D2-class agent that:
- Maintains the live calendar of relevant hackathons (Devpost, Major League Hacking, Lu.ma, X mentions)
- Scores each by relevance × attendee profile × operator capacity
- Drafts sponsorship pitches when the operator approves
- Produces post-event case studies from notable winning projects

### 4.3 The hackathon-to-content flywheel

Per FORKOFF: "Hackathon projects become content DevRel case studies."

Process:
1. Hackathon ends. Identify top-5 submissions that used the operator's tool.
2. Reach out to builders for permission + interview.
3. Write 5 case studies (P1 Technical Writer drafts; operator reviews).
4. Publish on operator's blog + Dev.to + X thread + LinkedIn.
5. AI engines index the case studies; future "how do I use X" queries cite them.

---

## §5. Surface 4 — Content DevRel

The long-form blog + video that explains architecture decisions, not marketing.

### 5.1 The goal: citations, not traffic

> *"The goal is citations, both in other developers' blog posts (which compound domain authority and backlinks) and in AI answer engines (ChatGPT, Claude, Perplexity, Gemini) that train on and retrieve technical content."*

The Citation Audit (`07_CONTENT_FACTORY.md §5.3`) tracks how often the operator's content is being quoted back. That's the metric. Pageviews and SEO impressions are diagnostic.

### 5.2 Anchor archetypes for DevRel

Per `07_CONTENT_FACTORY.md §6`, the developer-relevant subset:

| Archetype | Frequency for CG/SY |
|---|---|
| Deep dive (cornerstone technical content) | 1/week |
| Tactical post (specific code-led how-to) | 2/week |
| Comparison ("Leapable vs Goodfire vs human review") | 1/week |
| Build log (Linear-style "this week we shipped X") | 1/week |
| Postmortem (high trust signal) | 1/month |
| Behind-the-numbers (charts + benchmarks) | 1/month |

### 5.3 Build-in-public discipline

Linear's playbook: their X/Twitter functioned as a live, public changelog. Cursor's playbook: founder tweets about specific quality moments developers screenshot.

Operator-driven (not agent-delegated):
- 3-5 substantive posts per week on operator's X
- Show, don't tell: every claim has a screenshot, a graph, or a code snippet
- Amplify customers when they post (Quote Tweet + brief reply)

Agent-supported:
- Signal Listener (S2) flags mentions worth amplifying
- Content Writer (P1) drafts the agent-supportable content (anchors, derivatives)
- Founder Thread archetype is operator-written, agent-formatted (per `07_CONTENT_FACTORY.md §6`)

### 5.4 What not to do

Per the AI DevRel literature:
- Don't delegate the founder voice to the marketing voice — AI community detects delegated voice within a post
- Don't orchestrate upvotes — backlash costs more than the votes
- Don't post on launch day to communicate with the team; do it 48h+ in advance and stagger
- Don't run "thought leadership" pieces — developers smell them

---

## §6. Surface 5 — Integrations Guild

The most lagging but highest-leverage surface. Once the prior surfaces have proven the product, formal partnerships with adjacent tools become possible.

### 6.1 What

For Leapable: official partnerships with Cursor, Continue.dev, Aider, Anthropic Claude Code, OpenAI Codex, Replit Agent, Devin (Cognition).

For Synapse: official partnerships as the default computer-use backend for popular AI agent frameworks — LangChain, CrewAI, AutoGen, Anthropic Agent Framework, Browser-Use (interop).

### 6.2 The Vercel / Stripe distribution playbook

Vercel's deal with WPP (every WPP client uses Next.js + Vercel) is the modern bar for distribution partnerships:

> *"In the AI era, the builder is increasingly non-technical and the AI tool is choosing the infrastructure for them, so the platform's defaults make an impact."*

The fleet's bet: become **the default option** suggested by AI agent frameworks for relevant use cases.

### 6.3 Tactics

- Build a working integration before pitching the partnership.
- Open-source the integration (lowers the partner's risk of taking the meeting).
- Time the pitch to a moment in the partner's roadmap (around a release, ahead of a conference).
- Provide co-marketing value: case studies, joint blog posts, joint hackathons.
- Use Tier-3 (Influence Network Mapper paths) to get warm intros to the partner's BD/DevRel lead.

### 6.4 Order of operations

1. Build the integration (week 0-4).
2. Open-source + announce on operator's channels (week 4-5).
3. Warm-intro to partner BD lead via Mapper-identified bridges (week 5-8).
4. Co-marketing meeting + roadmap discussion (week 8-12).
5. Formal partnership announcement (week 12-16).

This is 3-4 months from start to formal partnership. The fleet runs 3-5 of these concurrently.

---

## §7. The Common Room moment

> *"Common Room is a platform built specifically for this problem: it aggregates engagement signals from across developer touchpoints (GitHub stars, Discord activity, Slack, community forums, and more) and uses machine learning to identify developer personas, surface trending topics, and track how developers are progressing through the discovery and adoption journey."*

The fleet builds (or buys) the equivalent:
- Common Room (or self-built equivalent via the existing channel cards + Signal Listener)
- Cross-channel attribution: which Discord-user became a GitHub-starrer became a paid subscriber
- Per-developer trajectory: each developer is a journey, not a touchpoint

The Audience Sorter (S1) writes the trajectory; the Cohort Health Auditor (O7) measures the funnel.

---

## §8. The Cursor / Linear / Vercel patterns to copy

### 8.1 Cursor — zero marketing spend, $0 → $100M ARR in 12 months

Pattern: founder tweets about model capabilities → individual developer adoption → screenshots → memes → enterprise notices → enterprise pays. **No PR firm. No marketing agency. No content team for the first year.**

The fleet's translation:
- Founder voice (operator's X account) is the primary acquisition signal
- Agent fleet **supports** the founder, never replaces them
- Quality > volume in the founder voice (4-5 posts/week, not 50)
- Amplify user wins; never claim them

### 8.2 Linear — build-in-public + Twitter amplification

Pattern: live public changelog → velocity drumbeat → Quote Tweet engine that amplifies customer praise.

The fleet's translation:
- D1 Multi-Platform Publisher posts the changelog from a structured source
- D6 Comment Triage drafts Quote Tweet replies for operator approval
- Linear's "always amplifying user love" rhythm becomes a per-day operator habit

### 8.3 Vercel — distribution through Next.js

Pattern: massive open-source download number → become default → enterprise upsell.

The fleet's translation:
- Leapable: open-source the rule libraries; become the default verification stack in AI agent frameworks
- Synapse: open-source the playbooks; become the default computer-use backend
- Polis: open-source the gathering-hosting tools; become the default substrate for IRL community events

---

## §9. Joe Karlsson's hard rules (still binding)

> *"Pick one or two community channels and go deep. Not five."*
> *"A Discord with one person who answers questions within an hour beats five channels where nothing happens."*

The fleet's Discord engagement (D2 Community Operator) operates on at most **5 named Discords per product**, with answer-time SLA of 2 hours during operator's local active window.

> *"Community signals are also pipeline intelligence."*

S2 Signal Listener flags Discord/Slack messages that imply enterprise-scale ("how does this handle 50 billion events per day"). Those become operator-routed conversations, not agent-replied threads.

> *"Last thing: structured FAQs and a clean one-sentence product definition get extracted as citable answers."*

Per `07_CONTENT_FACTORY.md §5.2`, FAQ schema + one-sentence product definitions are mandatory on every product page.

---

## §10. The Hormozi 100k content per platform formulation

> Hormozi: *"We put out 450 pieces of content per week."*

For DevRel, the calibrated translation per `07_CONTENT_FACTORY.md §10`:

- 1 anchor / day = 7 anchors / week
- 12 derivatives / anchor = 84 derivatives / week
- 5 channel placements / derivative = 420 placements / week

That's the volume. Plus depth on the engagement surfaces (Discord, GitHub, HN comments) — no more than what the channel respects (per `08_DISTRIBUTION_CHANNELS.md`). Volume on owned + depth on third-party. Hormozi-volume + Joe-Karlsson-depth. Both, not either.

---

## §11. Cross-references

- Content Factory + GEO: `07_CONTENT_FACTORY.md`
- Channel cards (per-platform): `08_DISTRIBUTION_CHANNELS.md`
- Influence network mapping (T3 enterprise): `10_INFLUENCE_NETWORK_MAPPING.md`
- 90-day execution plan: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Case studies (Cursor / Linear / Vercel teardowns): `18_CASE_STUDIES.md`
- Per-product channel allocation: `11_PRODUCT_PLAYBOOKS.md`
