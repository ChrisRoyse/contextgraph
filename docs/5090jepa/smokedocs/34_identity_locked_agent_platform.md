# 34 - Identity-Locked Agent Platform

## Goal

Combine the four building blocks (VoiceAgent real-time meeting bot from
ClipCannon, Constellation Guard for runtime identity, ME-JEPA for
domain-action prediction, Phase 12 privacy gate for licensed real
data) into a single deployable **enterprise agent platform** where
every agent is identity-locked, role-locked, and policy-locked by
**frozen geometric centroids** rather than fragile prompt rules.

The product: deploy a verifiable AI clone of any role (executive,
customer-success rep, internal helpdesk, expert reviewer) that:

- speaks in that person's or that role's voice (voice clone, doc 31);
- looks like them on video calls (Phoenix avatar);
- predicts and plans inside that role's domain (ME-JEPA);
- rejects any output that drifts off-identity or off-policy
  (Constellation Guard);
- carries cryptographic provenance per response (FSV bundle).

## Why Now

Enterprises are deploying internal AI assistants ad hoc. The two
universal failure modes are (a) **drift** (the assistant sounds wrong,
behaves wrong, contradicts policy) and (b) **non-defensibility** (you
cannot prove what it said or why). Both are solved by the
constellation pattern.

Consumer products (Replika, Character.AI) have shown the demand for
persona-locked agents. Nobody has shipped this for enterprise with
**verification at every output**.

## Architecture

```text
Per-tenant centroid registry
   │   ├── voice centroid (50-clip ref)
   │   ├── video centroid (FACS + identity)
   │   ├── text-style centroid (Shakespeare-LoRA-style; per-role tone)
   │   ├── domain centroid (ME-JEPA panel; e.g., support call playbook)
   │   └── policy centroid (forbidden topics, required disclaimers)
   │
   ▼
[ Role pack ]   role_taxonomy.<role>.toml: instruments, panels, centroids
   │
   ▼
[ Runtime ]   incoming request (Slack, Zoom, Teams, email)
   │
   ▼
[ Action selection ]   ME-JEPA plan over candidate responses
   │
   ▼
[ Generation ]   text + voice + (optional) video frame
   │
   ▼
[ Constellation Guard ]   cosine vs all centroids
   │   ├── reject if drift on voice → fall back to text-only
   │   ├── reject if drift on policy → typed refusal
   │   └── reject if drift on domain → escalate to human
   │
   ▼
[ Stream / response ]   with per-output provenance hash
```

Every step has provenance. Every rejection is logged with
per-modality cosines so an auditor can see exactly why.

## Three Productizations

### A. Executive Clone

The CEO/CTO/CMO role agent. Used for:

- attending Zoom/Teams meetings on the executive's behalf
  (VoiceAgent already supports this);
- responding to internal Slack queries in the executive's voice;
- drafting first-pass communications for review.

Centroid construction needs ~16 minutes of interview video (Phoenix
already operates at this scale per `docs2/alldata.md`). Voice + video
+ text-style + executive role-policy are all locked.

### B. Customer-Success Clone

A general-purpose enterprise CS agent. Voice/video are role-level
(not per-employee), but text-style + domain + policy are
deployment-specific. Talks to customers; routes hard cases to humans;
records every interaction with provenance for compliance.

### C. Internal Helpdesk / Expert Clone

An internal subject-matter-expert clone. Domain centroid is built
from the expert's documented work (Slack threads, Confluence,
Linear). Policy centroid is "company internal use only".
Constellation Guard refuses any output outside the domain, which is
the #1 customer-feedback request for internal AI assistants today.

## Tenant Provisioning Flow

1. Customer uploads reference assets per role:
   - voice: 50 audio clips of the target speaker;
   - video: 16 minutes of interview footage (optional);
   - text-style: 2,000+ words of representative writing;
   - domain corpus: 500+ documents/conversations/sessions;
   - policy doc: written policy + forbidden-topic list.
2. Platform builds centroids: voice, video, text-style, domain,
   policy. Hashed and frozen.
3. Customer reviews per-modality threshold calibration (cosine 5th
   percentile defaults; can tighten or loosen).
4. Customer connects integrations: Zoom, Teams, Slack, email, etc.
5. Soft-launch: agent runs in shadow mode (responds to none, logs all
   would-respond moments) for 7 days.
6. Live: agent responds; every response has per-output provenance;
   weekly drift report shows rejection rates.

## Role Pack Schema (Extension Of Domain Pack)

```toml
[role.executive_clone_alice]
extends_domain = "general_executive_v1"
voice_centroid_set    = "voice/alice_v1"
video_centroid_set    = "video/alice_v1"
text_style_set        = "text_style/alice_v1"
domain_centroid_set   = "domain/exec_calendar_v1"
policy_centroid_set   = "policy/acme_corp_v1"

acceptance = "all"     # all five must pass
fallback   = "text_only_with_audit_log"

[role.executive_clone_alice.thresholds]
voice = 0.92
video = 0.88
text_style = 0.85
domain = 0.70
policy = 0.95          # strictest on policy
```

Single config file fully specifies the agent. No code change to add
a new role.

## Pricing

| Tier | Audience | Pricing | Capabilities |
| --- | --- | --- | --- |
| **Team** | Single role, ≤10 seats | $100/seat/mo | One role pack; voice + text; web UI; basic dashboard |
| **Department** | Multi-role, ≤100 seats | $200/seat/mo | All modalities; up to 10 role packs; per-role drift dashboard |
| **Enterprise** | Org-wide | $500K+/yr | Custom roles; on-prem; Phase 12 privacy gate; integration with internal IdP |

The $/seat/mo Team tier is the entry point for mid-market; Enterprise
is where the deal sizes scale.

## Trust + Safety + Privacy

- **Identity consent**: source person must attest to clone use;
  written record + voice-attestation step.
- **Policy enforcement first, drift second**: policy-centroid threshold
  always tightest; an off-policy output is rejected before even
  computing other modality costs.
- **Phase 12 privacy gate** (doc 25) for customer corpora that include
  individual subjects: pseudonymization, deletion propagation, DP
  optional.
- **Audit immutability**: every agent response → audit log entry →
  cryptographic chain → public timestamp. Customer cannot retroactively
  delete an agent action they wish to disavow without leaving a
  tombstone.

## Hard Engineering Items (Pre-Launch)

- Multi-modal centroid registry (cross doc 28, doc 31);
- Role pack schema and validator;
- Multi-channel routing (Slack/Teams/Zoom/email integrations);
- Shadow-mode operation;
- Per-modality drift dashboard;
- Phase 12 privacy gate productized for tenancy;
- Single-sign-on (SAML/OIDC) for enterprise;
- Real-time latency budget (Voice + Constellation Guard streaming);
- Compliance overlay (doc 33 dependency; ELA bundling option).

## Why This Beats Prompt-Engineered Agents

Today's enterprise agents are LLM + system prompt + retrieval +
guardrail rules. Failure modes:

- **Prompt injection**: Constellation Guard is geometric, not text;
  cannot be jailbroken with a clever string.
- **Drift across deployments**: prompt-tuned behavior degrades when
  base model updates; centroid-tuned behavior is invariant to base
  model swaps as long as embedders are frozen.
- **No per-output verification**: prompt-engineered agents can output
  anything within token-prediction constraints; centroid-locked agents
  are bounded geometrically per output.
- **No defensible audit story**: "the LLM said it" is not a regulator
  answer; "this output passed all five centroid thresholds at these
  cosines" is.

This is the second category of the EU AI Act compliance product
positioning (doc 33), but for internal-AI deployment rather than
external regulatory artifact.

## Coordination With Other Docs

- Doc 28 (Constellation Guard): mandatory infrastructure.
- Doc 31 (voice clone): provides voice-centroid construction +
  generation primitives.
- Doc 33 (EU AI Act): customers in regulated industries bundle this
  with the compliance product.
- Doc 25 (Phase 12): real subject-level data activates the privacy
  gate.
- Doc 35 (open-core hosted): enterprise tier of agent platform =
  Sovereign-grade open-core.
- Doc 37 (domain pack registry): role packs share registry with
  domain packs.

## Acceptance

Identity-locked agent platform is **launch-ready** when:

- one paying Team-tier customer has deployed an executive clone with
  all 5 centroid thresholds active;
- shadow-mode runs successfully for 7 consecutive days with no drift
  alerts;
- audit chain anchors to public timestamp service;
- prompt-injection pen test fails to produce off-policy output (this
  is the proof point against prompt-engineered alternatives);
- per-output provenance is downloadable and verifiable on a
  customer-side replay.
