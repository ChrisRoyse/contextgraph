# 10 — Influence Network Mapping (the T3 Elite Tier)

> Steve Hutto's framing: *"Every ultra-rich has their own book. Every media bubble is very closed-door. So you have to figure that out. Who, what is my brother or my brother-in-law or sister-in-law, or my partner or this or that, who influences that person?"* This doc operationalizes elite-tier reach as a graph problem: map the network, find the shortest warm-intro path, and execute through operator-mediated handoffs — never agent-direct outreach.

---

## §1. Why Tier 3 is structurally different

T1 channels are open broadcast. T2 channels are open community. T3 channels are **closed doors**:

- Private dinners, off-the-record retreats, salon-style events
- Group chats (Signal, WhatsApp, iMessage threads, private Slacks)
- Curated invite-only conferences (TED, All-In summit, intellectual retreats, founder-only events)
- Investor portfolio mailing lists
- Board-level decision conversations

No amount of agent automation reaches behind these doors. The doors open only via **trusted human introduction**. The fleet's job is not to walk through the doors. It is to **make sure the operator never misses a viable bridge** to a door worth walking through.

---

## §2. The graph

A directed graph where:

- **Nodes** = people (and organizations as a coarse aggregate type)
- **Edges** = documented relationships, typed by relationship class

Relationship classes, ordered by edge strength:

| Class | Edge weight | Example |
|---|---|---|
| Co-founded | 1.00 | "X and Y co-founded Acme 2019-2022" |
| Direct manager–report | 0.95 | "Y reported to X at Anthropic" |
| Investor in your company | 0.90 | "X is a Series-A investor in operator's prior co" |
| Multi-year close colleague | 0.85 | "X and Y worked closely at OpenAI 2020-2024" |
| Board member of operator's company | 0.85 | |
| Board overlap (sit on a shared board) | 0.75 | |
| Repeated public co-appearance | 0.65 | "X and Y did 3 podcasts together in past 12 months" |
| Mutual close LinkedIn connection density >5 | 0.55 | |
| Shared significant past employer | 0.45 | |
| Shared alma mater, same cohort year | 0.45 | |
| Same advisor / mentor | 0.40 | |
| Shared portfolio-company member of investor | 0.35 | |
| Shared major industry conference attendance | 0.25 | |
| 2nd-degree LinkedIn connection | 0.15 | |
| Co-attended same generic event | 0.10 | |

A "warm intro path" is a chain of edges from operator → target. The strength of the path is the **minimum** edge weight on the chain (the chain is only as strong as its weakest link). The fleet prefers paths of length ≤ 3 with weakest edge ≥ 0.45.

---

## §3. The Influence Network Mapper (S3)

The agent class that owns the graph. From `05_AGENT_FLEET_DESIGN.md §6.1`.

### 3.1 Inputs

- **Operator's network** — LinkedIn export, Gmail contacts, Cal events, prior podcast guest list, prior investor list, board affiliations.
- **Target list** — Tier-3 contacts the operator wants to reach (operator-curated; the fleet may suggest based on signals but operator confirms).
- **Public graph signal** — Crunchbase, GitHub orgs, board records (where public), conference attendance lists (public), shared-publication co-authorships.

The Mapper **does not scrape private data**. It uses what is publicly indexed plus what the operator has shared from their own export. It never infers protected attributes (orientation, political affiliation, health, religious).

### 3.2 Process

1. Ingest operator-side network. Tag every node with attributes the operator has explicitly added (relationship class, last contact, last meaningful exchange, notes).
2. For each Tier-3 target, run BFS to find paths from operator-side nodes.
3. Score each path by `min(edge_weights)`.
4. Pick top 3 viable paths (path length ≤ 3, min-edge ≥ 0.45).
5. For each viable bridge, produce a one-paragraph "why this person could intro" — drawn from documented relationship history, recent public signals (shared interests, projects, conversations).
6. Score the bridge's **availability**: when did the operator last meaningfully interact with them? A 2-year-cold bridge is a different ask than a last-quarter bridge.
7. Output: per-target, ranked viable paths, each with bridge + intro reasoning + last-touch staleness.

### 3.3 Outputs

A markdown ranking, queued for operator review:

```markdown
## Target: Alice Kowalski (CEO, frontier AI lab)
**Why this target matters:** Decision-maker on enterprise AI tooling spend; her endorsement
in the operator's stack would unlock a procurement conversation at her company and signal-boost
to peers.

### Path 1 (strength 0.85, length 2)
Operator → Bob Lee (co-investor in operator's prior co, last contact: 6w ago) → Alice
**Bridge:** Bob Lee.
**Why Bob → Alice:** Bob led the Series B of Alice's previous startup (2018-2020); they
remain close per joint podcast appearance Q4 2025.
**Suggested intro reason:** Alice was quoted in NeurIPS 2025 panel as concerned about
"verifying AI-generated code in regulated industries" — the operator's product is precisely
this problem domain.
**Operator action:** Draft request to Bob (operator-approved, then sent).

### Path 2 (strength 0.55, length 3) [backup]
Operator → Carol Mendes (alumni) → Dave Singh (Carol's ex-coworker, current AT Anthropic)
→ Alice
**Bridge sequence:** Carol → Dave → Alice.
**Why this path:** Three-hop, weakest edge 0.55. Use only if Path 1 fails or as parallel
non-overlapping ask.

### Path 3 (strength 0.45, length 3) [low priority]
[...]
```

---

## §4. The Warm-Intro Drafter (D5)

Once a path is approved, the Warm-Intro Drafter produces the **forwardable paragraph** — a paragraph in the bridge's voice that the bridge can copy-paste-with-light-edit and forward.

### 4.1 The structure

```
[Bridge's name] forwards the operator's request to [Target]:

Hi [Target's first name],

[One sentence connecting on a recent public signal — what the target has been
talking about / building / thinking about. Specific, not generic.]

I want to introduce you to [Operator's name]. [One sentence on operator's
relevance to the target's recent signal — what they've built, the proof point,
the named users.]

If it's worth 20 minutes, [operator's name] would love to share what they've
found. If not, no worries — wanted to make sure the introduction was on your
radar.

[Bridge's existing sign-off pattern.]
```

The Drafter pulls the bridge's prior writing samples (publicly available emails, intros they've made before, podcast intros) to match voice. The operator reviews and edits before sending.

### 4.2 Hard rules

- **The Drafter never sends.** It produces text the bridge sends.
- **The bridge must voluntarily forward.** If the bridge declines, the path is dead — do not retry through the same bridge.
- **No mass templates.** Every intro request is custom to the target and bridge.
- **No fabricated relationship history.** If the operator and bridge haven't talked in 2 years, the request acknowledges that.
- **EU AI Act disclosure not required for warm intros** drafted by AI when the bridge edits before sending (the human is the author of the final text). However, the operator should disclose to the bridge that the draft was AI-assisted on first use, as a professional courtesy.

---

## §5. The Hormozi Lead Getter framing

Hormozi: *"You do not 'do' affiliates or 'do' customer referrals... they come from warm outreach, cold reach outs, posting content, and running paid ads. And once you get them, they do it for you."*

The Influence Network Mapper is the same idea applied to the Elite tier:

- The fleet does not **do** elite-tier endorsements.
- It identifies the **bridges** who, once cultivated, will produce elite-tier reach.
- The bridges become T3 lead-getters: the people who, once warm to the operator, refer the operator into more T3 contexts.

This is the Hormozi-style compounding move applied to Tier 3. Treat bridges like high-value customers: deliver value to them, stay top-of-mind without spam, surface relevant signals.

---

## §6. The Pre-Call Briefer

Before any operator–target conversation, the Pre-Call Briefer (a sub-agent of the Mapper) produces a one-page brief:

```markdown
## Pre-Call Brief: Alice Kowalski

### Conversation context
- Bridge: Bob Lee
- Bridge's framing of the intro: "[verbatim from Bob's forwarded note]"
- Scheduled: 2026-06-10 14:00 PT, Zoom
- Operator's stated objective: explore enterprise pilot for Leapable

### Last 90 days of public signal on Alice
- Mentioned "AI verification" 4 times across podcast / X / NeurIPS
- Posted X thread on "AI code review is broken" on 2026-04-12 (1.2k likes)
- Spoke at AI Infra Summit 2026, mentioned current frustration with hallucinated patches in CI
- Recent reading list (book club tweet): "Designing Machine Learning Systems"

### What she cares about right now (synthesized)
1. AI verification under regulatory pressure
2. Composability of AI agent tooling
3. Operator burnout on engineering teams

### Operator's strongest talking points (mapped to her interests)
- Leapable's deterministic verification = her #1 (regulatory)
- MCP-native interface = her #2 (composability)
- Reduces senior engineer code-review hours = her #3 (burnout)

### What NOT to bring up
- Pricing on first call (Bob's note: "she'll find that off-putting")
- Competitor comparisons until she asks
- Sales-oriented language; she's a builder
```

The brief is operator-only; never goes to the target. It's the operator's prep, not a script.

---

## §7. Follow-Up Tracker

A passive monitor. For every Tier-3 contact who's been in conversation with the operator:

| Trigger | Suggestion to operator |
|---|---|
| 90 days since last meaningful exchange | "Re-engage on a relevant signal" + the most recent relevant signal from the target's public surface |
| Target ships a launch / event / paper | "Send congratulations + one line of specific value-add" |
| Target moves company / role | "Acknowledge + recalibrate ask" |
| Target appears at an upcoming event the operator could attend | "Consider attending; expected attendees include X, Y, Z" |

The Tracker **never sends**. Operator-mediated.

---

## §8. Bridge cultivation

The single highest-ROI move on Tier 3 is **investing in bridges** — the people in the operator's network who could become repeat-introducers. Not transactional ask-once-and-burn-them; long-term-value-add.

Per-bridge investment cadence:

| Bridge tier | Touch frequency | Value-add per touch |
|---|---|---|
| Active high-leverage bridge (>3 successful intros in 12mo) | every ~30 days | personalized signal: "saw your portfolio company X is hiring for Y; I have someone good"; or a relevant introduction back to them |
| Active bridge (1-2 successful intros) | every ~60 days | thoughtful comment on their public work; relevant signal forward |
| Latent bridge (no asks made yet) | every ~90 days | low-touch — a "this made me think of you" share |
| Cold bridge (>12 months silent) | re-engage with one specific signal | one well-thought-out ask is worth more than five generic check-ins |

The Follow-Up Tracker manages the cadence; the operator does the touches; the Drafter helps with copy when requested.

---

## §9. Permission classes and hard rules

Every action in this doc is `REQUIRE_APPROVAL` by default with **no operator-configurable bypass**:

- The Mapper proposes; the operator confirms target list and approves bridge paths.
- The Drafter writes; the operator reviews and either sends as-is or edits.
- The Tracker monitors; the operator decides each suggested touch.
- The Briefer prepares; the brief is operator-eyes-only.

Hard rules (`02_FIRST_PRINCIPLES.md §19`):
- **Never** send automated outbound to a Tier-3 contact.
- **Never** scrape personal data beyond what's publicly indexed.
- **Never** infer protected attributes.
- **Never** store dossiers on health, family, politics, religion.
- **Never** assume a relationship the operator hasn't confirmed.
- **Never** use Tier-3 names in T1/T2 marketing copy without explicit, written permission.
- **Never** drop a Tier-3 name in a sales DM or fundraise to imply endorsement that isn't there.

A single violation of this section is a **brand-level kill switch event**. The Risk doc treats it as Tier 1 severity.

---

## §10. The endorsement asymmetry

Hormozi: *"The last skill you ever need to learn is how to get other people to do everything you need for you."*

Tier-3 endorsement is the highest-leverage form of this. A single named adoption by a recognized expert ("I use Leapable", "I trust Synapse for X", "I attended a Polis salon and here's why it works") moves a market segment.

**But**: a Tier-3 endorsement that's clearly transactional ("paid promotion") is **negative leverage**. The endorser loses credibility; the operator's brand inherits the suspicion.

Doctrine:
- **Never pay for a Tier-3 endorsement.** Hire them as an advisor (with real equity, real role) or get an unpaid genuine endorsement, but not influencer-style sponsored post.
- **Never embellish what the Tier-3 person said.** "Used it twice" ≠ "endorses it."
- **Always honor a request to be removed.** Tier-3 contacts who later reverse their endorsement get silent removal, no public response.

---

## §11. Cross-references

- Tier model: `03_TIER_MODEL_AUDIENCE.md`
- Mapper agent spec: `05_AGENT_FLEET_DESIGN.md §6.1`
- Drafter agent spec: `05_AGENT_FLEET_DESIGN.md §1.4` (D5)
- Tier-3 doctrine: `02_FIRST_PRINCIPLES.md §19`
- Per-product elite-tier playbooks: `11_PRODUCT_PLAYBOOKS.md`
- Closed-bubble channels for elite content consumption: `08_DISTRIBUTION_CHANNELS.md §3.6`
