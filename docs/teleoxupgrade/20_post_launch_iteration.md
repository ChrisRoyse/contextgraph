# 20 — Post-Launch Iteration

**Authority over:** what to measure post-launch, when to revisit positioning, what to add next.

---

## 1. The first 14 days

### 1.1 What to watch (daily)

Vercel Analytics dashboard:

| Metric | What good looks like |
|---|---|
| Page views per route | `/` > `/research` > `/paper` > `/about` > `/contact` |
| Top referrers | Direct, search, occasional Twitter / Threads / LinkedIn |
| Bounce rate (homepage) | < 60% (research sites are higher than commerce) |
| Time on page (research) | > 60 seconds median (signal of actual reading) |
| Time on page (paper) | > 90 seconds median |
| Mobile vs desktop | Likely 60% desktop / 40% mobile for research traffic |
| Geographic distribution | US + EU heavy; if APAC suddenly spikes, investigate |

### 1.2 What to watch (weekly)

Vercel Speed Insights:

| Metric | Target |
|---|---|
| LCP p75 | < 2.5s |
| FID / INP p75 | < 200ms |
| CLS p75 | < 0.1 |

If any metric regresses, debug. The most likely cause is a slow third-party script (none should exist) or a bloated image.

### 1.3 Search Console

After 7–10 days, check:
- All 5 main routes are indexed
- No coverage errors
- New sitemap successfully read
- Click-through rate on impressions (will be near 0% initially; takes weeks for real ranking)

---

## 2. What outreach success looks like

The site is one part of an outreach pipeline (per `outreach/yann_lecun_outreach_strategy.md` and the email drafts). Success metrics for the site:

| Signal | What it suggests |
|---|---|
| A LeCun-orbit person (Cho, Olah, Xie, Rabbat) replies to an email | Outreach worked. Site convinced them to take the next step. |
| A research lab principal forwards the URL internally | Strong signal — site is sharable to peers |
| An arxiv-orbit Twitter account @-mentions or quotes the site | Organic signal — site is good enough to share |
| Organic search traffic for "JEPA verifiability" or "training data provenance" | Long-tail SEO is working |
| A request for private-preview access via the contact form | Site converted a cold reader into a research conversation |

---

## 3. When to revisit positioning

**Trigger to re-spec the homepage:**
- The principal's research direction shifts materially (e.g., next paper isn't JEPA-related)
- A frontier lab acquires the work (no positioning needed if so)
- The audience signal flips (e.g., enterprise buyers start emailing in a way that suggests commercial pivot)
- 6 months pass with no engagement (rethink everything)

**Trigger NOT to re-spec:**
- "I don't like the headline anymore"
- A friend suggested a different framing
- A competitor changed their site
- Vibes

The positioning is a strategic choice, not an aesthetic one. Defend it for at least 6 months unless one of the trigger conditions hits.

---

## 4. What to add next (in priority order)

### 4.1 Personal blog / writing surface

If/when the principal starts writing more, add `/notes` or revive `/blog` with a curated set of posts. Tone: same as the rest of the site — research-grade, no hype.

### 4.2 Research log / changelog

A `/log` page that lists what's shipped, when. Each entry one paragraph + date. Builds a sense of momentum and accumulating evidence.

Example entries:
- "2026-05-01: ME-JEPA paper draft completed; first round of private reviews sent to JEPA orbit."
- "2026-05-15: ClipCannon BSL 1.1 license adopted."

### 4.3 Talks / appearances

If the principal gives a talk or appears on a podcast, add a section to `/about` or a new `/talks` route listing them. Each entry: date, venue, title, link.

### 4.4 Citations of the work

If the ME-JEPA paper or ClipCannon gets cited in published research, add a `/cited-by` section to `/research`. Builds compounding credibility.

### 4.5 Reproducibility badge

If a reviewer publicly reproduces the paper's numbers, link to their writeup with their permission.

---

## 5. What NOT to add (anti-roadmap)

| Tempting to add | Why not |
|---|---|
| Newsletter signup | We have no newsletter; adding one is committing to a product |
| Comments / discussion | Unmoderated comments are a maintenance burden; moderated comments require a team we don't have |
| Live chat | Anti-research; signals SaaS |
| Pricing page | We have no pricing; the work isn't for sale yet |
| "Try the demo" interactive | Premature; the runtime is private |
| Customer logos | We have no customers; even if we did, this is research, not vendor |
| Awards / press kit | Premature; build the work first |
| Investor section | We are not raising; if we ever do, that's a separate spec |
| AI chatbot ("Ask Teleox AI") | Anti-research; the site is text |
| Animated background | Anti-research; visual restraint is the design |

---

## 6. Iteration cadence

**Quarterly review** (every 3 months):
- Review analytics: which pages get traffic, which don't
- Review outreach: which channels produced engagement
- Review copy: any phrase that aged poorly?
- Review positioning: still aligned with the work?

**Monthly review** (every month):
- Lighthouse scores still meeting targets?
- No 404s accumulating?
- Sitemap still accurate?
- Any new routes needed?

**Weekly review** (first 8 weeks only):
- Top entry pages
- Top referrers
- Any technical issues

---

## 7. Recovering from a positioning miss

If after 90 days the site clearly isn't landing with the target audience, the response is **not** to add more pages or "try harder" with the existing positioning. The response is to re-do the audience definition (return to `01_strategic_positioning.md` §1) and ask:

- Is the target audience real?
- Are they actually visiting?
- If they visit, do they bounce immediately?
- What signal would tell us the positioning is right?

If the audience is real but the positioning is off, rewrite the homepage. If the audience isn't real, rewrite the audience definition and start over.

---

## 8. Long-term: the / endpoint

In 12 months, the homepage should look very similar to today's spec, EVEN IF every other page has evolved. The hero ("Models can be opaque. Pipelines shouldn't be.") is meant to be durable across multiple research directions because it names a principle, not a project.

If the principle changes — if the work pivots away from pipeline verifiability — then the homepage changes too.

---

## 9. End-of-life criteria

The site should be decommissioned or radically restructured if:

- The principal stops doing research entirely
- The work becomes a product (then it's a product company website, not a research site)
- The principal joins a frontier lab (the work moves to that lab's surface)

Until one of those triggers, the site stays as designed and accumulates research artifacts over time.

---

## 10. Final note

This is a research-led site for a single principal. Its job is to get the right kind of attention from a small number of people. It does not need to be popular. It does not need to convert traffic. It does not need to grow. It needs to be **correct in its message and accurate in its content**.

If a year from now the site has 50 unique visitors per month, but 3 of those are research leadership at frontier AI labs who emailed to talk, the site is doing exactly what it is designed to do.
