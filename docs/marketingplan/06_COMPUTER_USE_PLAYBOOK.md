# 06 — Computer-Use Playbook

> Synapse + Playwright + Browser-Use give the agent fleet the ability to operate any web platform a real human can — including the platforms with no API, with gated APIs, or with APIs so expensive they aren't economical at the operator's scale. This doc is the constraint sheet: how to do that without (a) burning the operator's accounts, (b) violating platform ToS, (c) generating brand-damaging "obvious AI" behavior, or (d) crossing into coordinated-inauthentic-behavior territory.

---

## §1. Why computer-use matters

In 2026, most of the channels that matter for the three products do not have economically viable agent-grade APIs:

| Channel | Real API? | Why computer-use anyway |
|---|---|---|
| X / Twitter | Yes, but $42k+/mo Enterprise tier for full posting + replying access | Operator-account-tier ops are economical via Synapse |
| LinkedIn | Posts/personal: very gated; Sales Nav: paid; full DM/connection: scrape-only | Computer-use is the only way to operate at the volume the engaged tier requires |
| Reddit | Yes, but rate limits + brand-detection make API agents look obvious | Computer-use matches human posting cadence better |
| Discord | Bots work for bot-tagged channels; community engagement as a human requires logged-in browser | Synapse logs into operator-owned account |
| Slack workspaces (operator joined) | App approval per workspace, often slow | Synapse operates as operator |
| TikTok | Creator API gated; comments/engagement gated; uploads via API are detection-flagged | Synapse uploads via the web client |
| Instagram | Graph API is permissioned and limited; commenting is gated | Synapse operates as operator |
| Hacker News | No real API for posting; one item per ~25 minutes from a logged-in account | Synapse logs in to operator's HN account |
| Product Hunt | API for listings; comments + maker engagement via browser | Synapse for engagement window |
| YouTube comments | Limited API; thread comments effectively browser-only at human cadence | Synapse for replies |
| GitHub | Excellent API (PyGithub, gh CLI) | API preferred; computer-use only for UI-only flows (sponsor signup, etc.) |
| Email + Calendar | Excellent API (Gmail, MS Graph) | API preferred |
| Ad platforms (Google, Meta, TikTok Ads, LinkedIn Ads, X Ads) | Yes via Markifact / Hyper MCP / Windsor | API preferred |
| CRM (HubSpot, Salesforce, Pipedrive, Klaviyo) | Yes via Markifact / Hyper MCP / Windsor | API preferred |

**Rule:** API where available and economical. Computer-use elsewhere. Never both at once on the same account (avoids fingerprint conflicts).

---

## §2. The three computer-use surfaces

### 2.1 Synapse (primary)

The operator-owned Synapse MCP server (`/home/cabdru/contextgraph/docs2/contextgraphspecs/synapse.md` etc.) gives the agent fleet:

- **Perception:** `observe`, `find`, `read_text` — read the screen and the UIA accessibility tree.
- **Action:** `act_click`, `act_type`, `act_press`, `act_aim`, `act_drag`, `act_scroll`, `act_pad`, `act_clipboard` — operate the OS at a human-shaped rate.
- **Reflexes:** `reflex_register` etc. — sub-frame reactive controllers for tasks that need <16ms latency (rarely needed for marketing; useful for capturing an ephemeral status drop or live-stream comment).
- **Safety:** `release_all`, operator panic hotkey (Ctrl+Alt+Shift+P) — the operator can stop everything in 50ms.

Synapse is the **default** for any platform that requires logging in as the operator on a real browser/desktop session.

### 2.2 Playwright (parallel scale)

For tasks that need parallel browser sessions across **many** scrap-class targets (e.g., influence network mapping, public LinkedIn profile data, public job listings), Playwright runs headless on a separate machine pool with rotating non-operator-account residential IPs.

**Important:** Playwright is for **read-only** parallel scraping of *public* surfaces. It is **not** used to operate operator accounts. If the action involves login as the operator, it goes through Synapse.

### 2.3 Browser-Use / OpenClaw-style adapters

For interactive flows that benefit from goal-driven LLM-in-the-loop browsing (e.g., "find the right contact-form on a partner's website and submit our partnership inquiry"), Browser-Use / OpenClaw provides natural-language browser automation. The agent describes the goal; the tool steers the browser.

These adapters wrap the same Tool Gateway surface so the rate-limit, brand-voice, and compliance gates still apply.

---

## §3. Operator-account doctrine

### 3.1 Account registry

Every operator-owned account the fleet may operate is registered in `accounts.yaml`:

```yaml
- account_id: x_chris_personal
  platform: x.com
  persona: chris (operator's primary)
  url: https://x.com/<handle>
  owner_documented: yes
  recovery_phone_owned_by: operator
  recovery_email_owned_by: operator
  agents_allowed:
    - D1  # Multi-Platform Publisher
    - D6  # Comment Triage
    - S2  # Signal Listener (read-only)
  rate_limits:
    posts_per_24h: 4
    replies_per_24h: 25
    likes_per_24h: 60
    follows_per_24h: 8
    dms_per_24h: 5
  brand_voice_spec_path: docs/brandvoices/chris.md
  ai_disclosure_required: true  # EU contacts only
```

No account = no agent operations. The Guardrail Engine rejects tool calls referencing unregistered account IDs.

### 3.2 No multi-account amplification

The fleet never spins up additional accounts to amplify the operator's content. No engagement farming. No upvote rings. No follower-purchasing. Synthetic-persona manufacturing is forbidden (`02_FIRST_PRINCIPLES.md §19`).

If the operator legitimately runs multiple personas (e.g., a personal account + a product account + a niche-persona account), each is registered separately with its own brand voice and the Persona Carrier (P5) enforces non-leakage.

### 3.3 No credential sharing

Each agent receives **per-account scoped tokens** with the smallest possible permission set. The Operator Agent rotates tokens monthly. Synapse sessions on the operator's desktop authenticate via OS-level keychain access, not by storing passwords in agent prompts.

---

## §4. Rate limiting — the 30%-below rule

Each platform has a published or empirically-known rate-limit threshold above which bot detection fires. The fleet sets internal rate caps at **30% below** that threshold by default. The caps live in `channel_cards/<platform>.yaml`:

```yaml
platform: x.com
detection_thresholds_published:
  posts_per_24h: 100  # well-known soft cap before review
  replies_per_24h: 100
  follows_per_24h: 50
  likes_per_24h: ~200
  dms_per_24h: 50  # to non-followers; followers higher
internal_rate_caps:
  posts_per_24h: 4    # operator preference; well below detection
  replies_per_24h: 25
  follows_per_24h: 8
  likes_per_24h: 60
  dms_per_24h: 5
posting_cadence_distribution: stochastic_log_normal_mean_4h_stddev_2.5h
detection_signals_to_avoid:
  - perfectly_periodic_posting
  - identical_phrasing_across_replies
  - rapid_fire_likes_to_new_accounts
  - all_actions_from_same_IP_block
  - DM_template_within_10_seconds_of_follow
```

The `posting_cadence_distribution` is the key non-obvious gate: real humans post at irregular intervals (log-normal-ish around their habit). The fleet samples from a distribution rather than firing on a fixed schedule.

### 4.1 Stochastic timing

Every external action that has a delay component (post, reply, follow, like) is enqueued with a **sampled delay** from the channel's distribution. The action does not execute at the planner's target time; it executes between [target − jitter] and [target + jitter] where jitter is drawn from the distribution. This makes the fleet indistinguishable from a real but disciplined human operator at the macro level.

### 4.2 Token-bucket implementation

Per channel × account, a token bucket refills at the internal-cap rate. The Guardrail Engine's `RateLimitCheck` decrements the bucket on each call. Over-empty = `RATE_LIMIT_DENY`. The Planner sees the denial and reschedules; it does not retry-loop. Repeated denials trip the `Fuse` circuit breaker.

---

## §5. The Hormozi Volume question

Alex Hormozi's content advice: "We put out 450 pieces of content per week." "One reel a day is not enough."

The fleet **can** sustain that volume, but **must not** sustain it on a single channel × account beyond the rate cap. The reconciliation:

| Pattern | Volume strategy |
|---|---|
| Hormozi-volume on **owned surfaces** (operator's website blog, operator's newsletter, operator's YouTube) | Yes — no third-party rate limit |
| Hormozi-volume on **operator's** social handles | No — respect platform rate caps |
| Hormozi-volume **across operator's surfaces by repurposing one core piece into N formats** | Yes — this is the actual move |

The Content Factory (`07_CONTENT_FACTORY.md`) is the implementation. One long-form anchor → 8 short-form derivatives → 4 platform-native format adaptations → channel-paced publishing per the rate cards.

**Net effect:** 1 long-form piece × 12 formats × 8 platforms = 96 publishings per week, every one of which sits inside its channel's rate cap and looks like normal cadence on that channel. This matches Hormozi-volume on **aggregate reach** without violating any single platform's threshold.

---

## §6. Anti-detection content patterns

Beyond rate limits, modern platform detection looks at content patterns:

| Detection signal | Anti-pattern (fleet must not do) | Mitigation |
|---|---|---|
| Repeated phrasing across replies | "Thanks for the comment! Great point!" repeated | Comment Triage uses ≥3 variant templates + LLM rephrasing on every reply |
| All replies start with the same word | "Yes" / "Yes" / "Yes" | Lint check denies same-first-word streak >2 |
| Mass-engaging only on follower-of-follower accounts | Suspicious engagement graph | Mix engagement: peers, larger accounts, smaller accounts, randoms |
| Replies under 8 seconds after a post | Inhuman speed | Min delay 30s on replies; sampled from distribution |
| Identical link UTMs across all posts | Trackable as a single campaign | Per-channel UTM variation |
| AI-detection signatures in copy (em dashes, "delve", "tapestry", "navigate the complexities") | Modern LLM-written prose markers | Brand-voice evaluator scores AI-likelihood; rewrite above threshold |
| Engagement only during one timezone band | Reveals automation centralization | Stochastic distribution across operator's actual active hours |

The Brand Voice Scorer (`05_AGENT_FLEET_DESIGN.md §6.3`) is the primary defense. It is fine-tuned on the operator's approved content. Content that scores too LLM-shaped is rewritten before publish.

---

## §7. EU AI Act Article 50 disclosure (August 2026)

For content **created or substantially modified by AI** delivered to EU recipients, disclosure is required. The fleet handles this mechanically:

```
crates/marketing-plan/templates/eu_disclosure.md:
> Parts of this message were drafted with AI assistance.
```

**Implementation:**
- Every outbound email tool has a wrapper `send_email_eu(...)` that injects the disclosure if the recipient's resolved jurisdiction is EU/EEA.
- Every published post that goes to an account with >5% EU follower share carries the disclosure as the last line.
- "Substantially modified" is the trigger; pure operator-written content with light LLM polish does **not** need disclosure.

The disclosure is a tool-side wrapper, not a prompt instruction. (`02_FIRST_PRINCIPLES.md §3` — trust the harness.)

---

## §8. Per-platform playbooks

The full per-platform behavioral specs live in `channel_cards/<platform>.md`. Summary:

### 8.1 X / Twitter

- Operator account login via Synapse on operator desktop.
- 4 posts / 24h cap; stochastic distribution mean ~6h apart.
- Threads: max 1 / 24h for substantive multi-tweet threads.
- Replies to mentions: 25 / 24h; min 30s after notification; LLM-judged for substantive vs ack-only.
- Likes: 60 / 24h; varied targets (not just top-of-feed).
- DMs: 5 / 24h to non-followers, with `REQUIRE_APPROVAL` always; followers higher cap but still gated.
- Profile changes: `REQUIRE_APPROVAL` always.

### 8.2 LinkedIn

- Operator account.
- 1 post / 24h on personal; 2 / 24h on company page.
- Comments on others' posts: 20 / 24h; substantive comments only.
- Connection requests: 8 / 24h; only to relevance-scored prospects; `REQUIRE_APPROVAL` for net-new connections.
- DMs (post-connection): 5 / 24h; `REQUIRE_APPROVAL` for first message; `LOG` for follow-ups in established thread.

### 8.3 Reddit

- Operator account with karma > 500 minimum before any product mention.
- 4 substantive comments / 24h across subreddits.
- 1 post / 72h max; subreddit-specific rules in each card.
- No product mention until karma + reputation threshold in target subreddit (Joe Karlsson DevRel rule: earn the right to post).
- Cross-post detection: never post identical content across subreddits.

### 8.4 Hacker News

- Operator account.
- Comments: 6 / 24h; substantive only; no boilerplate.
- Show HN: 1 / 90 days maximum; coordinated launch (see PH launch playbook).
- Upvote rings: never. Even between operator's personal account and product launches.

### 8.5 Discord

- Operator account in each registered server.
- Replies: server-specific; default 10 / 24h per server.
- DMs: `REQUIRE_APPROVAL` always.
- Read-only listening: unrestricted (for Signal Listener S2).

### 8.6 TikTok

- Operator account.
- 3 uploads / 24h max (Polis especially).
- Comments: 20 / 24h.
- No bot likes / no engagement pods.

### 8.7 Email (any jurisdiction)

- API preferred (Gmail, MS Graph).
- CAN-SPAM block on every commercial email: physical address, unsubscribe link, accurate from.
- GDPR legitimate-interest documentation per recipient region.
- EU AI Act disclosure if AI-generated/AI-modified.
- Cold outbound: 30 / 24h sustained max; first message `REQUIRE_APPROVAL` per net-new recipient.

---

## §9. Auditability

Every computer-use session is recorded:

- Full DOM/UIA snapshots at decision points
- Screen captures on operator-requested audit only (privacy default off; on-demand)
- Action log: timestamp, tool, payload, latency, outcome, idempotency key
- Tracing: LangSmith / OpenTelemetry span per action

Operator can run `marketing audit --account=x_chris_personal --since=24h` and see every action with replay capability.

---

## §10. Recovery from account suspension

Despite all the rate-limiting, platforms occasionally false-positive. The recovery playbook:

1. **Stop all activity on the affected platform across all goals immediately** (kill switch).
2. File the platform's standard appeal — Operator Agent drafts; operator sends.
3. Reflector files a structured post-mortem: what tool sequence preceded suspension?
4. Update the channel card: lower the rate cap; widen the cadence distribution; add the detected signal to the avoid list.
5. Resume only after platform reinstates **and** the channel card update has been operator-approved.

Suspension is treated as a critical learning event, not a routine. Repeat suspensions on the same platform → permanent deny of that platform until operator-level architectural review.

---

## §11. Cross-references

- Synapse contract (Action/Perception/Reflex): see ContextGraph Synapse systemspec docs.
- Channel cards: `channel_cards/*.yaml` (per-platform operational state)
- Brand Voice Scorer: `05_AGENT_FLEET_DESIGN.md §6.3`
- Compliance details: `14_COMPLIANCE_AND_ETHICS.md`
- Auditability and operator dashboards: `19_OPERATIONS_AND_CADENCE.md`
- Account registry schema: `16_TECH_STACK.md §6`
