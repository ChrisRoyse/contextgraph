# 14 — Compliance and Ethics

> The hard floor. Every external action is gated by these rules. Violation is not a "warning" — it's a circuit breaker trip. This doc covers CAN-SPAM, GDPR, CCPA, CASL, the EU AI Act (Article 14 + Article 50, August 2026), platform ToS, ethical guardrails, and the operator's signed-off ethical floor for Tier-3 conduct.

---

## §1. The compliance floor (mechanically enforced)

Every outbound action through the Tool Gateway passes through the **ComplianceCheck** in the Guardrail Engine (`04_SYSTEM_ARCHITECTURE.md §8`). The check is deterministic, runs in O(1), and is **not LLM-mediated**. Tools that touch external surfaces are wrapped with compliance enforcement at the wrapper level, not at the agent prompt level. (`02_FIRST_PRINCIPLES.md §3` — trust the harness.)

The four primary regimes:

| Regime | Jurisdiction | When applies | Required actions |
|---|---|---|---|
| **CAN-SPAM** | United States | Any commercial email to US recipients | Truthful headers + subject; physical address; valid unsubscribe link honored within 10 business days; identify as ad when applicable |
| **GDPR + ePrivacy** | EU/EEA | Any data processing involving EU residents | Lawful basis (legitimate interest for B2B); privacy notice accessible; honor right-to-erasure, right-to-access; document legitimate interest balancing test |
| **CCPA** | California residents | Any data processing on CA residents | Honor opt-out requests; disclose data practices; "Do Not Sell My Personal Information" mechanism if sale occurs |
| **CASL** | Canada | Any commercial email/SMS to Canadian recipients | Implied or express consent; identification of sender; unsubscribe mechanism |

The fleet does **not** rely on the operator to remember these. The `send_email`, `send_sms`, `send_dm`, `paid_ad_targeting` tools are jurisdiction-aware wrappers.

---

## §2. The EU AI Act (Aug 2, 2026 — already in effect)

Regulation 2024/1689. Two articles directly govern the marketing fleet:

### 2.1 Article 50 — AI-content disclosure

> AI systems interacting with natural persons must disclose that the interaction is AI-generated unless the context makes it obvious. Article 50(4) specifically addresses AI-generated content intended to inform or influence — which describes AI-generated outbound marketing.

**Implementation:**
- Every outbound email to an EU recipient that was drafted or substantially modified by AI carries a disclosure line: *"Parts of this message were drafted with AI assistance."*
- Every public-facing post on operator's accounts that was drafted by AI and not edited substantially by the operator carries an AI-assistance footnote (channel-appropriate format).
- AI-generated chat agent interactions (e.g., comment-triage replies via D6) carry an in-thread disclosure on first reply.
- The disclosure is injected by the tool wrapper. The Content Writer cannot bypass it.

### 2.2 Article 14 — Human oversight

> For AI systems posing material risk, a natural person must be able to effectively oversee, interrupt, and override AI decisions.

**Implementation:**
- The autonomy slider (`02_FIRST_PRINCIPLES.md §2`) is the operator's effective oversight.
- Every external action class is configurable per goal between `AUTO` and `REQUIRE_APPROVAL`.
- Approval queue has documented audit trail: approver, approval timestamp, payload, decision.
- The operator's panic hotkey (kill switch) operates outside the agent loop and halts within 50ms.

This isn't a checkbox. The operator's role as overseer must be **documentable** — per-campaign approvals are logged in `CF_AUDIT_LOG` with operator ID, timestamp, decision. Auto-rejected items also log.

### 2.3 The "deployer" obligation

Article 50 places obligations on the **deployer** (the company sending the messages), not the model provider. The operator (us) is the deployer. The compliance burden is ours, not Anthropic's.

---

## §3. Cold outbound — what's legal, what's not

The most-litigated marketing surface. Per the 2026 cold-email compliance literature:

### 3.1 What's legal

- **B2B email under CAN-SPAM** with valid physical address, working unsubscribe link, truthful sender, accurate subject. **No prior consent required** in the US.
- **B2B email under GDPR via legitimate interest** with documented balancing test (Article 6(1)(f)).
- **Personalization via public information** (LinkedIn job title, public company news, public conference attendance).
- **Follow-ups** within established conversation threads.
- **AI-drafted email content** — the authoring tool doesn't affect compliance. Disclosure required only under EU AI Act if delivered to EU recipient.
- **CRM enrichment** from public sources.

### 3.2 What's not legal

- **B2C email outbound without explicit opt-in** in EU (GDPR), Canada (CASL), Australia (Spam Act).
- **Mass automated outreach without unsubscribe** — CAN-SPAM violation in US.
- **Scraping personal data behind logged-in walls** (LinkedIn member info beyond your network, Twitter DMs, private Slack).
- **Inferring protected attributes** (orientation, political, religious, health, race) and storing/using them.
- **Buying email lists** that don't include verified consent for the seller-to-purchaser handoff.
- **AI-personalized B2B email to EU contacts without disclosure** (EU AI Act, Aug 2026).
- **Automated decisions with legal or significant effects on individuals** without human review (GDPR Art. 22).

### 3.3 Documented legitimate interest balancing test (GDPR Art. 6(1)(f))

For B2B outbound to EU contacts, the operator maintains a written balancing test per campaign:

```yaml
campaign_id: leapable_enterprise_q3_2026
target_population:
  - role: VP Engineering / Director of AI Platform / Head of Developer Tools
  - employer: any company with ≥50 engineers and AI agent deployment
purpose: introduce Leapable for AI patch verification
data_source: public LinkedIn, Crunchbase, conference attendance lists, public AI-discussions
data_minimization: only role, employer, public-statement context; no protected attributes
balancing_test:
  legitimate_interest: yes — B2B product introduction relevant to recipient's stated role
  recipient_expectation: yes — recipients in this role are routinely contacted about AI tooling
  recipient_rights: full opt-out per email + unsubscribe portal; right-to-erasure honored within 30 days
  data_minimization_check: yes — we collect only what's needed for personalization
override_consent_required: no — this is legitimate interest, not consent
balancing_test_signed_by: operator
signed_at: 2026-05-26
```

The operator signs the balancing test per campaign. The fleet enforces by refusing to launch a campaign without a signed test.

---

## §4. Platform ToS

Every platform has a ToS. The fleet operates **only** within ToS. ToS violations are an immediate kill switch.

| Platform | ToS-binding rules the fleet honors |
|---|---|
| X / Twitter | No automation in violation of platform automation rules; no coordinated inauthentic behavior; no spam; rate-limits respected; computer-use as logged-in user on operator-owned account is allowed |
| LinkedIn | No automated scraping of member info beyond public; no automated connection requests outside platform tools; computer-use as logged-in user on operator-owned account is the gray-area allowed pattern; no automated profile data exports |
| Reddit | API-based bots permitted with clear bot disclosure; computer-use as a human-shaped logged-in user permitted; coordinated upvote/downvote rings prohibited |
| Discord | Bots in bot-tagged channels permitted; impersonation prohibited; mass-DM prohibited |
| Hacker News | One account per person; no upvote rings; no posting from anonymous accounts on behalf of brand |
| TikTok | No automated uploading via scraped API; computer-use on operator account permitted with rate-limit respect; no fake accounts |
| Instagram / Meta | No automation against ToS; ad platform separate from organic |
| YouTube | Channel-owner automation allowed within YouTube Studio limits; no coordinated comment-manipulation |
| GitHub | API-based automation permitted with token; webhook automation permitted; no spam in PRs/issues |

The Channel Cards (`08_DISTRIBUTION_CHANNELS.md §5`) carry the per-platform ToS-derived rate caps. The fleet operates **30% below** the bot-detection thresholds (`06_COMPUTER_USE_PLAYBOOK.md §4`).

---

## §5. Ethical floor (operator's signed commitments)

Beyond legal compliance, the operator commits to the following ethical floor. These are non-negotiable doctrine.

### 5.1 No deception

- Never claim AI-generated content was human-written when it wasn't (and vice versa where material).
- Never fabricate testimonials, endorsements, reviews, or quotes.
- Never paint a competitor's product inaccurately. Comparison content is rigorously fact-checked.
- Never use Tier-3 names in marketing without their explicit, written permission.
- Never invent or imply relationships, partnerships, or endorsements that don't exist.

### 5.2 No coordinated inauthentic behavior

- Never operate accounts not owned by the operator.
- Never use synthetic personae to amplify the operator's content.
- Never coordinate engagement across accounts to game algorithms.
- Never purchase followers, likes, comments, reviews, upvotes.
- Never use bots to inflate metrics.

### 5.3 No abuse of personal data

- Never collect protected attributes (orientation, political, health, religion, race).
- Never infer protected attributes from public signals.
- Never store dossiers that include such inferences.
- Never use scraped personal data outside legitimate-interest documented use cases.
- Honor every opt-out + right-to-erasure request within 7 days.

### 5.4 No predatory tactics

- Never target vulnerable populations (minors, recently bereaved, those in financial distress, those in active addiction recovery) for paid acquisition.
- Never use dark patterns in funnel UI (forced continuity, hidden costs, "confirm-shaming" decline buttons).
- Never use scarcity / urgency that isn't real ("only 3 left" when there's unlimited).
- Never use confusing pricing or hidden charges.

### 5.5 No platform manipulation

- Never participate in upvote rings, follower rings, retweet rings.
- Never gamify community moderation.
- Never use multiple operator accounts to vote on a single piece of content.
- Never use AI to mass-evade content moderation.

---

## §6. Polis-specific ethical commitments

Polis is a consumer social product with in-person attendance. Additional commitments:

- **Attendee consent on photo / quote / recording** — opt-in default, not opt-out.
- **No undisclosed influencer events** — if a host is compensated, attendees know.
- **No targeting by inferred political / religious / sexual identity.**
- **Active safety culture at events** — anti-harassment policy, named contact for incidents, clear consequences.
- **Privacy of attendee contact data** — Polis does not sell or share attendee lists.
- **Right to leave**: attendees can request all their event data deleted; honored within 7 days, no questions asked.

---

## §7. The audit trail

Every external action is logged in `CF_AUDIT_LOG` with sufficient information that:

- A regulator could be answered with documentary evidence within 24 hours
- An attendee / lead / contact could be answered "what data do you have on me" within 7 days
- A platform-suspension review could be answered with a full history of every action on the affected account

Schema:

```yaml
audit_id: uuidv7
goal_id: leapable-50k-q3-2026
cycle_id: 2026-05-26T14:00:00Z
node_id: experiment-ph-launch-001
agent_class: D1
tool: x_post
account_id: x_chris_personal
timestamp: 2026-05-26T14:18:32Z
payload_hash: sha256:...
payload_content: "[…]"
guardrail_decision: allow
brand_voice_score: 0.91
compliance_checks_passed: [host_allowed, rate_limit_ok, brand_voice_ok, eu_disclosure_na_for_us_account]
operator_approval_required: false
operator_approval_log: na
outcome: ok
external_response: "{platform-response}"
cost_usd: 0.0
cost_tokens: 1240
side_effects: [post_published]
related_audit_ids: [...]
```

Audit log is append-only. Operator can query via dashboard: "show me every action on this account in the last 7 days," "show me every email to this contact ever," "show me every paid ad spend on this campaign."

---

## §8. The Right to Erasure (GDPR Art. 17 + CCPA)

Implementation:
- An EU recipient's right-to-erasure request triggers:
  - Delete contact from CRM
  - Delete from CF_CONTACTS in the marketing graph
  - Delete from any campaign suppression list (so they're not re-acquired by mistake)
  - Add to permanent "do not contact" list (preventing future re-import)
  - Confirm completion to requester within 30 days
- A CCPA "do not sell my personal info" request triggers similar deletions plus a Sell-Indicator flag (we don't sell info, but track the request).
- Both go through the **GDPR Tool Endpoint** wrapper, which is operator-monitored.

The fleet does **not** auto-delete on receiving an inbound email mentioning "remove me" — it raises an approval-queue item with the request quote, draft response, and proposed action. Operator confirms.

---

## §9. Operator's escalation tree

When the fleet detects something compliance-adjacent:

| Detection | Routing |
|---|---|
| Brand-voice violation post-publish | Reflector + Operator notification + content takedown if severe |
| Rate-limit warning from platform | Operator notification + channel-card update + rate-cap reduction |
| ToS warning from platform | **Operator immediate notification** + activity pause on affected platform + Reflector post-mortem |
| Account suspension | Kill switch on platform + operator-led appeal + post-mortem |
| Regulator inquiry (GDPR, CCPA, etc.) | **Operator immediate notification** + legal escalation + every relevant operation paused |
| Subpoena or law enforcement contact | **Operator immediate notification** + legal counsel + no agent activity related to the matter until counsel reviews |
| Subject access request | Approval queue with proposed response + operator review |
| Right-to-erasure request | Approval queue with proposed deletions + operator review |

The first three are routine. The bottom four are escalations that pull in counsel.

---

## §10. The "no" list (forever)

Specific tactics the fleet **never** uses, regardless of operator approval:

- Mass cold DM via automation
- Multi-account upvote / engagement rings
- Synthetic personae
- Buying engagement signals (followers, likes, reviews, upvotes)
- Dark patterns
- Predatory targeting
- Scraping behind logged-in walls
- Inferring protected attributes
- Using a real person's identity without consent

These cannot be enabled in the goal spec. They are not subject to autonomy slider override. The Guardrail Engine refuses tool calls implementing them, with a clear error code.

---

## §11. Cross-references

- Guardrail engine: `04_SYSTEM_ARCHITECTURE.md §8`
- Computer-use ToS + rate caps: `06_COMPUTER_USE_PLAYBOOK.md §4`
- Tier-3 ethics (closed-bubble conduct): `10_INFLUENCE_NETWORK_MAPPING.md §9`
- Polis-specific event ethics: `13_CONSUMER_SOCIAL_LAUNCH.md §10`
- Audit log schema: `15_METRICS_AND_OBSERVABILITY.md §8`
- Kill switches and circuit breakers: `20_RISK_AND_FAILURE_MODES.md`
- Operator approval queue UX: `19_OPERATIONS_AND_CADENCE.md §3`
