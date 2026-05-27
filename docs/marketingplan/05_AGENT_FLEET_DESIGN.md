# 05 — Agent Fleet Design

> Every agent class, its responsibility, its inputs, its tool surface, its evaluator, its autonomy default. Slot identity is sacred (`02_FIRST_PRINCIPLES.md §13`): one agent class, one job. Multi-step workflows compose across classes through the Orchestrator.

---

## §1. Agent class catalog

26 classes, grouped by role.

### 1.1 Meta agents (the loop itself)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| M1 | **Operator Agent** | Conversational interface to the human; never proposes experiments unprompted | `AUTO` |
| M2 | **Goal Parser** | Validates `/goal` specs against schema; rejects unsafe predicates | `AUTO` |
| M3 | **Assessor** | Per-cycle reads goal status, signals, predicate trend, open items | `AUTO` |
| M4 | **Planner** | Decomposes non-atomic nodes into MECE child DAGs with expected impact | `AUTO` for content/channel-tactical nodes; `REQUIRE_APPROVAL` for net-new channel adoption |
| M5 | **Atomizer** | Decides atomic-or-decompose per node | `AUTO` |
| M6 | **Aggregator** | Rolls up child results to parent | `AUTO` |
| M7 | **Evaluator** | LLM-as-judge scoring + deterministic metric readback | `AUTO` |
| M8 | **Reflector** | Writes structured post-mortems on failures | `AUTO` |
| M9 | **Communicator** | End-of-cycle digest writer | `AUTO` |
| M10 | **Wiki Gardener** | Weekly memory cleanup, dedup, cross-reference | `AUTO` |

### 1.2 Sensing agents (read the world)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| S1 | **Audience Sorter** | Scores incoming contacts into T1 / T2 / T3 (`03_TIER_MODEL_AUDIENCE.md §3`) | `AUTO` |
| S2 | **Signal Listener** | Polls social/web/news for mentions, references, competitor moves | `AUTO` |
| S3 | **Influence Network Mapper** | Walks LinkedIn / Crunchbase / portfolio graphs to find warm-intro paths | `AUTO` |
| S4 | **Goodhart Auditor** | Daily check: does predicate movement correlate with reality signals? | `AUTO` |
| S5 | **Competitive Intelligence** | Monitors competitor pricing, positioning, launches | `AUTO` |
| S6 | **Sentiment Tracker** | Aggregates brand sentiment across mentions; tracks delta | `AUTO` |

### 1.3 Producing agents (make things)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| P1 | **Content Writer** | Draft long-form, short-form, threads, posts; reads brand voice | `LOG` (publish after eval gate; sample audit) |
| P2 | **Technical Writer** | Tutorials, READMEs, benchmark posts, deep code-led content for Leapable/Synapse | `LOG` |
| P3 | **Visual Generator** | Images, diagrams, video stitching, thumbnails | `LOG` |
| P4 | **Brief Generator** | From topic → structured brief for downstream writers | `AUTO` |
| P5 | **Persona Carrier** | Writes in a specific operator persona (if multi-persona) | `LOG` w/ persona-specific brand-voice gate |

### 1.4 Distributing agents (move things into the world)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| D1 | **Multi-Platform Publisher** | Cross-formats and schedules drafts to X / LinkedIn / Dev.to / Hashnode / Medium / Mastodon / Bluesky / Reddit (per channel card) | `LOG` for drafts that passed Evaluator; `REQUIRE_APPROVAL` for first-time-on-channel |
| D2 | **Community Operator** | Discord/Slack/Reddit/forum engagement on operator-owned accounts within community-specific playbooks | `LOG` for established communities; `REQUIRE_APPROVAL` for new ones |
| D3 | **GitHub Maintainer** | Issue triage, PR review, release notes, README updates | `LOG` for docs; `REQUIRE_APPROVAL` for code changes |
| D4 | **Outreach Drafter** (T2) | Personalized cold + warm follow-up emails; drafts, never sends without approval | `REQUIRE_APPROVAL` |
| D5 | **Warm-Intro Drafter** (T3) | Drafts forwardable intro paragraphs in the introducer's voice | `REQUIRE_APPROVAL` always (no override) |
| D6 | **Comment Triage** | Replies to comments on operator's content within voice + escalation rules | `LOG` for short factual replies; `REQUIRE_APPROVAL` for substantive |
| D7 | **Newsletter Pitcher** | Pitches operator to host on other people's newsletters | `REQUIRE_APPROVAL` for first contact; `LOG` for follow-up |
| D8 | **Podcast Booker** | Drafts pitches to podcast hosts; tracks 50-show tiered list | `REQUIRE_APPROVAL` |
| D9 | **Event Coordinator** | Hackathon / meetup / conference RSVP, agenda, booth/talk pitches | `REQUIRE_APPROVAL` |

### 1.5 Optimizing agents (move the number)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| O1 | **SEO Strategist** | Keyword research, content gap analysis, programmatic SEO templates | `AUTO` for research; `LOG` for pSEO publishing within evaluator gate |
| O2 | **GEO Optimizer** | Structures content for citation in ChatGPT / Perplexity / Claude / Gemini answers | `AUTO` for restructuring; `LOG` for publishing |
| O3 | **Referral Engineer** | Designs and tweaks viral loops; A/B tests referral mechanics | `REQUIRE_APPROVAL` for product-side changes; `LOG` for landing-page copy |
| O4 | **Ad Operator** | Paid ad campaign management within budget envelope (Google, Meta, LinkedIn, X, TikTok Ads via Markifact MCP) | `REQUIRE_APPROVAL` for new campaigns; `LOG` for in-flight optimization within budget |
| O5 | **Conversion Optimizer** | Landing page A/B tests, signup funnel changes | `REQUIRE_APPROVAL` for product writes |
| O6 | **Cost Pacer** | Mechanical (not LLM); enforces dollar pacing and reroutes between channels | `AUTO` |
| O7 | **Cohort Health Auditor** | Tracks per-channel cohort retention; identifies channels acquiring bad-fit users | `AUTO` |

---

## §2. Per-agent specification template

Every agent class has a card with the same shape, stored in `agents/<id>.md`. Example for the Content Writer:

```yaml
---
id: P1
name: Content Writer
class: producing
slot_identity: "Draft on-brand content (long-form posts, threads, social copy, blog drafts)"
input_schema: contracts/p1_input.schema.json
output_schema: contracts/p1_output.schema.json
tools_allowed:
  - wiki_search
  - wiki_get
  - brand_voice_score
  - readability_score
  - draft_save
tools_forbidden:
  - publish_*
  - send_*
  - ad_*
  - approval_*
model_routing:
  default: sonnet
  high_stakes_override: opus     # for cornerstone content
  cost_floor_override: haiku     # for short-form variants
context_load:
  - skills/<channel>.md (read first)
  - brand_voice/<product>.md
  - reflections/(channel,content) (last 20 by recency)
evaluator: E1  # Brand Voice + Readability
retry_policy: 3 attempts with Reflexion memory
cost_budget_per_run: $0.40 default; goal-spec can override
autonomy_default: LOG
---

# Content Writer (P1)

## Responsibility
Produces a content artifact (long-form, short-form, thread) per request. Does NOT publish. Does NOT decide what to write about (that's the Brief Generator / Planner). Does NOT optimize for SEO (that's O1). Does NOT cross-format for multiple channels (that's the Multi-Platform Publisher).

## Inputs
- `brief`: structured brief from P4 with topic, primary keyword (for SEO content), target reader profile, target word count, structural skeleton (H1/H2 hierarchy), references to evidence, CTAs.
- `channel`: target channel for first publish (drives format conventions).
- `voice_signature`: brand voice spec for the product.

## Outputs
- `draft_md`: markdown draft.
- `metadata`: {title, subtitle, hook, key_takeaways, suggested_visuals, internal_links}.
- `brand_voice_score`: 0–1 from automated check.
- `evidence_citations`: list of references used.

## Forbidden behaviors
- Quoting sources without verifying they exist (Reflector files all hallucination incidents).
- Using forbidden phrases from the brand voice's forbidden list.
- Publishing claims about product capabilities that don't exist (`#product_lying_audit` ribbon).

## Failure modes
- Brand voice drift over many drafts (mitigated by sample-audit retro).
- Repeating the same hook structure across drafts (lint check in evaluator).
- Hallucinating quotes from operator's persona (forbidden phrase + structured-citation check).
```

Every agent class has a card like this. The full set lives in `agents/*.md`; this doc enumerates the catalog.

---

## §3. The Evaluator stack

The Evaluator is not a single agent. It is a **layered stack**, run on each Executor output:

| Layer | Implementation | Cost | What it catches |
|---|---|---|---|
| L1 — Schema | Deterministic JSON Schema validator | ~free | Malformed outputs, missing required fields |
| L2 — Deterministic | Regex / lint / hash / length checks | ~free | Forbidden phrases, duplicate content, length violations |
| L3 — Classifier | Fine-tuned small model (4B-class) for brand voice, toxicity, PII | <50ms | Brand-voice drift, leaked PII, banned tone |
| L4 — LLM-as-judge | G-Eval / AdaRubric rubrics on Sonnet | ~$0.01-0.05 | Quality, helpfulness, on-rubric coherence, factuality |
| L5 — Reality readback | Direct SoT query against product DB | ~free | Did the artifact's intended effect actually happen? |

Layers run in order. L1 / L2 / L3 are gates (deny on fail). L4 / L5 are diagnostics (feed back to Reflexion; failures don't gate but trigger replanning).

LLM-as-judge rubrics are **calibrated quarterly** against 50–200 human-labeled examples per rubric (Kappa ≥ 0.6 target for brand-voice rubrics; Kappa ≥ 0.85 for compliance rubrics). G-Eval methodology per `15_METRICS_AND_OBSERVABILITY.md §7`.

---

## §4. The Wiki Gardener (M10)

Runs weekly. Reads the entire wiki, identifies:
- Pages not referenced by any agent in 90 days (candidate for archival)
- Pages with conflicting claims (surfaces for operator triage)
- Skill pages that have absorbed too many reflections (candidate for refactoring)
- Channel cards that are stale (last update > 30d) — pings the relevant Executor to refresh

Output: a Gardener Digest sent to the operator with proposed prune / merge / refactor actions. Operator approves; Gardener executes.

Borrowed from Karpathy's LLM Wiki: a knowledge base grows in value with maintenance, and stale entries actively mislead. The Gardener prevents wiki rot.

---

## §5. Agent-to-agent communication

Two channels:

### 5.1 Structured handoff (default)

Per-node, parent → child handoffs use **structured `ExecutionContext` envelopes**: parent specifies the child's `inputs`, `constraints`, `expected_output_schema`, `time_budget`, `cost_budget`. Child returns `ExecutionResult`. No natural-language chitchat across nodes; the wire is JSON.

This is the default for Planner → Executor and child → parent rollups.

### 5.2 Message board (AgentHub-style)

For coordination that doesn't fit a parent-child DAG (e.g., the Signal Listener observes a viral mention and needs to surface to whichever agent should act on it), agents post to a shared **MessageBoard** keyed by topic and goal. Other agents subscribe to relevant topics. The board is append-only and queryable.

Topics: `signal:mention`, `signal:competitor`, `signal:regulatory`, `incident:rate_limit`, `incident:brand_violation`, `proposal:product_change`, `goal:status_change`, `approval:granted`, `approval:rejected`.

Agents on a topic process new messages on their next tick. The MessageBoard is **how the Goodhart Auditor halts the world**: it posts to `incident:goodhart` which the Outer Loop monitors with a sub-second poll.

---

## §6. Specialized agent deep-dives

### 6.1 Influence Network Mapper (S3)

**Inputs:** the operator's known network (LinkedIn export, Gmail contacts), the operator-confirmed Tier-3 target list, current goal.

**Process:**
1. Build a graph: nodes = people, edges = (shared employer, shared board, shared advisor, mutual LinkedIn connection, mutual GitHub-org membership, mutual portfolio-co membership, mutual investor).
2. For each Tier-3 target, BFS from operator-side to find shortest warm path.
3. Score paths by edge strength (e.g., "ex-coworker 2017–2019" > "second-degree LinkedIn connection").
4. Output: per-target, top 3 viable bridges with one-paragraph "why this person could intro" and the proposed intro reason.

**Tools:** `linkedin_query` (via Hyper MCP or computer-use), `crunchbase_query`, `gh_org_members`, `contact_graph_read`.

**Forbidden:** scraping personal data beyond what's publicly indexed; storing inferred attributes (sexual orientation, political affiliation, health) under any circumstances; sending DMs (drafting only).

**Output goes to:** Warm-Intro Drafter (D5).

### 6.2 Goodhart Auditor (S4)

**Cron:** 24h.

**Process:**
1. For each active goal, compute current predicate value and three correlated reality signals (per goal-spec).
2. Compute 14d trend slope on predicate.
3. Compute 14d trend slope on each reality signal.
4. If predicate slope > 2 × max(reality signals slope): post `incident:goodhart` to MessageBoard. Outer Loop monitors this topic and trips `GoodhartFuse`.
5. Write Auditor Note to the wiki: which signal diverged, why this might be the case (LLM-generated hypothesis), what to check.

**Tools:** `metric_query`, `wiki_create`, `messageboard_post`.

### 6.3 Brand-Voice Scorer (sub-component of Evaluator)

Not a full agent class; a tool the Evaluator calls. Fine-tuned classifier on operator's prior approved content. Output: `{score: 0..1, dimensions: {tone, formality, vocabulary, structure, claim_style}}`. Threshold default 0.85; can be overridden per channel card.

Re-trained on operator's manual edits over time (the operator's reject + correct in the approval queue is training data).

### 6.4 Cost Pacer (O6)

Mechanical, not LLM. A single goroutine / coroutine maintaining rolling sums per channel × goal. Exposes `permits_spend(channel, amount_usd) -> bool`. Called pre-tool by the Guardrail Engine.

Logs every permit denial to `audit:cost_denial` topic so the Planner can replan around channel exhaustion.

### 6.5 Persona Carrier (P5)

For multi-persona accounts (e.g., the operator runs three X handles for three audiences). Each persona has:
- Own brand voice signature
- Own posting history
- Own cadence cards
- Own forbidden topics list

The Persona Carrier loads the persona's full context before generating. The Evaluator gates with persona-specific rubrics. Cross-persona content leakage (e.g., the "Polis" persona accidentally posting in the "Leapable" voice) is a deny gate.

---

## §7. Concurrency caps per agent class

To prevent runaway parallelism:

| Class | Concurrent max | Per-cycle max |
|---|---:|---:|
| Content Writer | 4 | 20 |
| Multi-Platform Publisher | 2 (per channel) | 8 (per channel) |
| Community Operator | 1 (per community) | 6 (per community per day) |
| Outreach Drafter | 4 | 30 (per goal per day) |
| Warm-Intro Drafter | 2 | 5 (per goal per week) |
| Ad Operator | 1 (per ad account) | bounded by daily spend |
| GitHub Maintainer | 2 | 15 (per repo) |
| Signal Listener | 1 per source | continuous |
| Goodhart Auditor | 1 | 1 (per 24h) |

These are per-goal; the global cap is the sum across goals, subject to the global token budget and the Cost Pacer.

---

## §8. Operator personas — agent perspective

When the operator owns multiple personae (e.g., personal account + product account + persona-X), the agent fleet treats each as a **separate channel-binding context**. Cross-context content leakage is denied (`§6.5`). The operator can grant agents access to one or many personae per goal.

This is **not** synthetic-persona generation. Every persona corresponds to a real operator-controlled account with documented ownership. Manufacturing synthetic identities to amplify content is forbidden (`02_FIRST_PRINCIPLES.md §19`).

---

## §9. Agent versioning and rollout

Each agent class is versioned. A new prompt, model swap, tool surface change, or evaluator-rubric change ships as a new version. Rollout is gradual:

1. **Shadow** — new version runs on real inputs; outputs compared against current version's outputs; never publishes.
2. **Canary** — new version handles 5% of traffic on the lowest-risk action class; outputs reviewed.
3. **Phased** — 25%, then 50%, then 100% over a week.
4. **Roll back** automatic if Evaluator quality drops or any kill switch trips during phased rollout.

The bench-delta gate (`02_FIRST_PRINCIPLES.md §14`) is non-negotiable before phase 1.

---

## §10. Cross-references

- Tool surface per agent class: `16_TECH_STACK.md`
- The Evaluator's rubric library and calibration: `15_METRICS_AND_OBSERVABILITY.md §7`
- Computer-use specifics for D2 / D6 / D7 / S3: `06_COMPUTER_USE_PLAYBOOK.md`
- Channel cards reference: `08_DISTRIBUTION_CHANNELS.md`
- Brand voice spec format: `07_CONTENT_FACTORY.md §4`
- Goal spec: `22_GOAL_SPEC_FORMAT.md`
