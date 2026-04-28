---
namespace: research
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T16:10:00Z
status: active
tags: conference-strategy, email-templates, outreach, primitive-first
---

# Conference Email + DM Templates

**Scope-guard rules (non-negotiable)**:
- Every Case 3 mention carries `within-WavLM-family, N=10 held-out English sentences, one speaker, single session, EXP-1 pending` inline
- Every "up to 91" mention carries `constructive upper bound, EXP-2 pending`
- Every Shumailov reference frames as `structurally outside the recursion regime — scope, not refutation`
- Every Case 1 / Shakespeare LoRA reference carries `MEASURED 2026-04-17` + HF URL
- No prohibited phrases: SOTA / beats / refutes / cross-encoder verified / commercial TAM / jailbreak-proof / mathematically unable
- Every cold email signed from `chrisroyseai@gmail.com` (existing Day-0 discipline)
- Every email carries ResearchGate URL + DOI

**Framing doctrine** (`memory/context/mission-brief.md`):
- Primitive-first: we are presenting a primitive to be critiqued, not a product to be purchased
- Collaborate, never sell (Forbes Brustein #11 alignment)
- 15-minute coffee ask — not dinner, not multi-hour meeting
- "No expectation either way" closure removes pressure

---

## TEMPLATE 1 — Cold DM / Email to a frontier-lab researcher (Track B)

**When to use**: LinkedIn DM or academic email to Dohmatob-class / Khattab-class / Perez-class researcher, 4–8 weeks pre-conference.

**Subject line options**:
- Format A (paper-specific): `Re §3.3 vs [their specific paper] — [CONFERENCE] coffee?`
- Format B (primitive-class): `N=13 embedder panel generalizing [their concept] — [CONFERENCE]?`

**Body**:

```
Hi [First Name],

I've been reading [specific recent paper / post / talk — 1 concrete reference]. Your
framing of [specific thing they said] was what made me re-examine [specific §X.Y of
my manuscript].

I published a paper this month on a data-layer primitive (ResearchGate 403916407, DOI
10.13140/RG.2.2.24370.57288): project a fixed corpus of n inputs through N frozen
approximately-independent embedders → up to n·(N+C(N,2)) structured clean labeled
supervisory signals. §0.2 is the counting identity; constructive upper bound, tightness
pending an EXP-2 pairwise-MI audit on the 13-embedder panel.

[If applicable]: §3.3 argues this primitive is structurally outside the Shumailov-form
recursion regime — a scope argument, not a refutation. [Their prior theorems] still bind
exactly where they said they bind.

I'll be at [CONFERENCE] [dates]. If 15 minutes over coffee makes sense for you, I'd
value the critique. No expectation either way — if your schedule's locked, I'll look
for you around the venue.

Also: reproducibility anchor — Shakespeare style-transfer LoRA on Gemma-4, MEASURED
2026-04-17, SFT loss 1.12 (98.9% tok-acc), DPO loss 0.47 (98.0% rewards-acc). Weights
and recipe public: huggingface.co/cabdru/shakespeare-lora-gemma4.

— Chris Royse
Teleox.ai / Leapable.ai — methodology contribution from commercial engineering practice
Paper: researchgate.net/publication/403916407
```

**Customization slots**:
- `[specific recent paper / post / talk]` — must be real and Playwright-verified
- `§X.Y of my manuscript` — must be actually relevant to their work
- `[If applicable]` — remove the Shumailov paragraph for non-collapse-theorists

---

## TEMPLATE 2 — Cold LinkedIn message to a sovereign-capital / VC principal (Track A)

**When to use**: LinkedIn InMail to PIF / MGX / Mubadala / Andreessen / Lux / Khosla principal, 4–8 weeks pre-conference.

**Subject / opening line (first 120 chars visible in notification)**:
```
Teleox primitive — 3-min read + [CONFERENCE] hallway?
```

**Body**:

```
Hi [First Name],

I noticed [specific recent thing they announced / said / invested in — 1 concrete line].

Short version: I've been building a primitive in the AI data layer that I think matters
for the sovereign-compute thesis your firm has been articulating. Not a product pitch —
I'm asking for a critique.

The primitive: project a fixed corpus through N frozen embedders → up to n·(N+C(N,2))
structured supervisory signals (counting identity, constructive upper bound; EXP-2
pairwise-MI audit pending). This moves the data-wall argument sideways — the limit
isn't "how much data is there," it's "how many orthogonal views can I compute over
the data I have."

Three case studies are MEASURED on my own compute (not projections), with full
scope-tag disclosures:
- Case 1: Shakespeare LoRA on Gemma-4 (HF public; 2026-04-17)
- Case 2: deterministic CEO-avatar / meeting-bot stack (operational)
- Case 3: voice cloning — 0.961 WavLM SECS, within-WavLM-family, N=10 held-out English
  sentences, one speaker, single session, ECAPA-TDNN triangulation pending

I'll be at [CONFERENCE] [dates]. If 15 minutes makes sense, I'd value the exchange.
Paper: researchgate.net/publication/403916407 (DOI 10.13140/RG.2.2.24370.57288).

No expectation either way.

— Chris Royse, Teleox.ai
```

**Customization slots**:
- `[specific recent thing]` — must be Playwright-verified (their LinkedIn post / announcement / portfolio add)
- Shorten Case 2 / Case 3 lines if LinkedIn InMail character limits require (InMail limit is 2000 chars for InMail credit; 300 chars for free)

---

## TEMPLATE 3 — Second touch / warm referral introduction

**When to use**: A mutual connection is willing to make a 2-line intro. Draft this FOR them so they can copy-paste.

**Draft for the connector to send**:

```
[Target first name], I'd like you to meet Chris Royse. He's been working on a
data-layer primitive (DDA — derived data abundance) that I think is up your alley
given your [specific interest the connector knows about the target].

Chris — [Target first name] leads [role] at [org]. Worth a 15-min exchange at
[CONFERENCE] or over Zoom if that doesn't work out.

Let me leave you two to it.
```

**Follow-on email from Chris to target (within 24 hours of connector's intro)**:

```
[Target first name] — thanks for the intro [Connector's first name].

Short on my side: [one-line primitive framing]. Paper: researchgate.net/publication/403916407.

I'll be at [CONFERENCE] [dates]. Here are three coffee slots that work on my end —
feel free to grab any or suggest your own:
- [Day 1, time]
- [Day 2, time]
- [Day 3, time]

If a pre-conference Zoom is easier, schedule.teleox.ai has open 20-min and 45-min
slots. No expectation either way.

— Chris
```

---

## TEMPLATE 4 — Response when a target replies positively pre-conference

**When to use**: An A-tier target replied "yes let's meet." Within 2 hours, send this.

```
[First name] — appreciate you making time.

Confirming [day, date] at [time] at [specific place — e.g., "Beverly Hilton lobby
Starbucks" for Milken; "Ritz-Carlton Riyadh business-center coffee" for FII].

I'll have 15 min blocked. Happy to run longer if we're mid-thread when the clock
hits zero.

One-line primer so we don't spend the first 5 min on setup: my §0.2 counting
identity says a fixed corpus of n inputs projected through N embedders yields up
to n·(N+C(N,2)) signals. The question I most want your read on is §[their-
interest-adjacent section — 3.3 for collapse theorists, 3.4 for retrieval people,
5.4 for alignment researchers].

Paper: researchgate.net/publication/403916407.

— Chris
```

---

## TEMPLATE 5 — Soft-no / "I'm not sure I'll be there" reply handling

**When to use**: Target replied but is non-committal ("not sure I'll attend", "my schedule's crazy").

```
[First name] — no problem at all. Two paths that don't require the conference:

(a) 20-min Zoom whenever the next 6 weeks have a slot: schedule.teleox.ai or
   feel free to suggest a time directly.

(b) Just the PDF, asynchronous — if a 15-min read produces a critique, I'll take
   that as the most efficient exchange possible. Attached.

Appreciate the response either way.

— Chris
```

---

## TEMPLATE 6 — Walk-up "do-not-be-creepy" opener for an unplanned Tier-A encounter at the venue

**When to use**: You physically encounter a target you wanted to meet — they haven't replied to DMs, but you're standing 3 feet apart in a hallway.

**Forbes Brustein #9 rule**: Don't scan badges. **Forbes Brustein #3 rule**: Open on something they did, not on your own credential.

**Opener**:

> *"[First name] — that [specific question / comment / panel moment] earlier was the thing that made me want to say hello. I'm Chris Royse. My recent paper argues [15-second primitive framing, §0.2 counting identity]. Is there a time this week I could send you a 5-min read and a single specific question?"*

**Variant if they were on stage**:

> *"Really appreciated your point on [specific thing]. I'm Chris Royse — I published a paper recently arguing [one-sentence primitive framing]. Would you be open to a 5-min read + one question over email?"*

**End of opener**: wait for their answer. Do NOT pitch further. If they say yes → hand over QR-code card. If they say "email me" → follow up within 24 hours with Template 1 shortened to 150 words.

---

## TEMPLATE 7 — Post-conference same-day follow-up (within 24h of meeting)

**When to use**: You had the meeting. Now you need to compound.

```
[First name] — good to meet you at [place, day].

Three concrete follow-ups based on our conversation:

1. [Specific thing you said you'd send]. Attached / linked here: [URL].
2. The single question I most wanted your read on: [one crisp specific question
   derived from the conversation — not a generic "what do you think?"]
3. If pairwise-MI audit results interest you when they land (EXP-2, ~4-6 weeks
   out), I'll send a 2-page summary then. Let me know if you'd rather opt out.

Paper: researchgate.net/publication/403916407 if you want to re-read any
section after our conversation.

— Chris
```

**Forbes Abouelenein #7**: *"Follow up fast: Send a photo and your contact information immediately after meeting someone."* The photo (if one was taken) goes in a follow-up X DM or LinkedIn message separately — keep the email substantive.

---

## TEMPLATE 8 — Post-conference 14-day breakup (if no post-meeting response)

**When to use**: You met at the conference, you followed up per Template 7, 14 days of silence.

```
[First name] —

Circling back once. The EXP-2 pairwise-MI audit is now [update on status], and
I wanted to flag it since our conversation touched on [specific area].

If the timing's wrong, no problem — I'll stop bumping this thread. If you'd like
to re-engage later (post-NeurIPS / next quarter / whenever), my Calendly is at
schedule.teleox.ai.

— Chris
```

**Rule**: if no reply after this one — archive the thread. Don't send a third touch from the conference arc. Day-14 breakup is the final touch per standard `agent-00-orchestrator--pre-send-checklist.md` doctrine.

---

## CONFERENCE-SPECIFIC OPENING-LINE TUNING

Each conference audience has different tonal defaults. Adjust Templates 1 + 2 opening lines:

| Conference | Audience tuning | Opening line tonality |
|---|---|---|
| **FII 10 Riyadh** | Sovereign capital — respect hierarchy, formality first | "I noticed [their firm]'s recent [move]. I'd value [first-name honorific]'s read on a primitive I've been building..." |
| **FII PRIORITY Europe Rome** | European strategic-autonomy — sovereignty-of-training-signal framing | "Your firm's Europe-capital-sovereignty thesis maps directly onto a primitive I'm working on — sovereignty of the AI data layer..." |
| **Milken Global LA** | US strategic capital + AI GPs — pragmatic, metric-literate | "Two metrics you might care about: [Case 1 MEASURED numbers] + [Case 3 MEASURED number with scope tag]. The paper behind them..." |
| **NeurIPS Atlanta** | ML research community — technical rigor + epistemic humility | "§0.2 counting identity with pairwise-MI tightness open (EXP-2 pending) — I'd value your read on..." |
| **ICML Seoul** | Mix of research + applied teams — technical-but-application-minded | "Data-layer primitive with reproducibility anchor on Gemma-4 (HF public); the §5.4 verifiability predicate is what I most want..." |
| **SIGIR Melbourne** | Retrieval / IR community — Khattab-adjacent late-interaction frame | "§3.4 argues pool-relative novelty beyond late-interaction on a fixed panel. Specifically curious whether your recent [their work] changes the story..." |
| **LEAP Riyadh** | Scale-fest, noisy floor — we don't target this with emails; see Execution SOP walk-up rules | N/A — don't email-warm LEAP; go walk-up if attending |

---

## Pre-send checklist per email/DM (enforce every time)

- [ ] Recipient name verified via Playwright (not LLM-guessed)
- [ ] Recipient email / DM handle verified via Playwright (not LLM-guessed)
- [ ] `[specific recent thing]` hook is a real thing they did (not generic)
- [ ] `§X.Y` reference is actually relevant to their work
- [ ] Case 3 scope tag present if Case 3 mentioned
- [ ] "Up to 91" hedge paired with "constructive upper bound" + "EXP-2 pending" if §0.2 mentioned
- [ ] Shumailov framed as "structurally outside" / "scope argument" if mentioned
- [ ] No prohibited phrase (SOTA / beats / refutes / cross-encoder verified / commercial TAM)
- [ ] Paper URL + DOI present
- [ ] HF LoRA URL present if Case 1 mentioned
- [ ] "No expectation either way" closure present
- [ ] Single specific ask (15-min coffee / Zoom / read question) — not vague
- [ ] Signature: Chris Royse / Teleox.ai / Leapable.ai
- [ ] Send from chrisroyseai@gmail.com (Track B researcher) OR LinkedIn Chris Royse (Track A principal)

---

## Cross-reference

- Go/no-go per conference: `01_CONFERENCE_PRIORITY_MATRIX.md`
- Warm-up timeline: `02_CONFERENCE_WARMUP_TIMELINE.md`
- On-site execution: `04_CONFERENCE_EXECUTION_SOP.md`
- Standing email drafts (Days 0-5): `../07_outreach_artefacts/11_email_drafts.md`
- Pre-send checklist (standing): `../../memory/blockers/agent-00-orchestrator--pre-send-checklist.md`
