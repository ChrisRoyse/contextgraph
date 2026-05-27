# 18 — Case Studies

> Teardowns of the modern growth-engine winners. Each case study is read for **what to copy** and **what to skip**. The goal is not imitation; it's understanding which mechanisms transfer and which were one-time anomalies. Cursor / Linear / Vercel for dev tools. BeReal / Bluesky for consumer social. Stripe for distribution. Plus a Hormozi posted-and-ghosted vs mega-launch case.

---

## §1. Cursor — $0 to $2B ARR in 30 months

**Pattern:** zero marketing spend; obsessive product focus; founder voice on X; bottom-up adoption; enterprise upsell emerges from organic developer use.

### 1.1 The numbers (verified)

- Co-founded 2022 by Michael Truell, Sualeh Asif, Arvid Lunnemark, Aman Sanger (all MIT).
- Spent 2022 building CAD AI tools; pivoted to AI code editor early 2023.
- Shipped MVP alongside GPT-4 in March 2023.
- $1M ARR by end of 2023.
- $100M ARR by end of 2024 (the fastest $1M-to-$100M ramp in SaaS history).
- $1B ARR Nov 2025 ($29B valuation Series D).
- $2B ARR run rate Q1 2026.
- ~1.5M paid developers (per public disclosures).
- Half of Fortune 500 using it.

### 1.2 What they did — and what to copy

| What | Notes | Copy for our stack? |
|---|---|---|
| Founder Twitter as catalyst | Aman Sanger's posts about AI coding caught Notion's Ivan Zhao asking to test it; that's the inflection moment | **Yes** — operator's X is the catalyst. Don't delegate the voice. |
| Build a real product first | 1 year in CAD wilderness before pivoting. Five weeks of dogfooding before launch | **Yes** — product is the marketing |
| Posted-and-ghosted launch | "MVP alongside GPT-4 launch" — no press, no PH, no fanfare | **Yes** for Phase A; switch to mega-launch later (`17_LAUNCH_PLAYBOOK_90DAY.md`) |
| Free tier with generous limits | 2,000 free completions/month at launch | **Yes** — see `11_PRODUCT_PLAYBOOKS.md §1.2` (Grand Slam Offer) |
| Founders use their own product as primary daily tool | "We had an overriding fear of focusing on anything other than making the product better" | **Yes** — operator must dogfood all three products |
| Customer expansion as the marketing channel | OpenAI / Midjourney / Perplexity / Shopify developers brought it in; IT noticed the spend; enterprise conversations started | **Yes** — the bottom-up motion is doctrine for Leapable and Synapse |
| Slow hiring | One person every 2-3 months at $100M ARR | **N/A** for our 1-operator start |
| Avoided networking events, conferences, speaking engagements | First year of explosive growth | **Partial** — operator should attend strategically chosen Tier-2 / Tier-3 events |
| No PR firm, no marketing agency, no content team for the first year | | **Partial** — we replace this with the agent fleet; the founder voice still doesn't delegate |
| Quote-tweet user wins; never claim them | Vibe-coding meme, 8-year-old Cursor demo, voice-coding clips — Cursor amplified, never orchestrated | **Yes** — Linear pattern + Cursor pattern combined |

### 1.3 What NOT to copy

- The pre-pivot 1 year in CAD wilderness. Don't replicate the meandering; we have product-market fit hypotheses to test deliberately.
- The "no marketing budget at all" framing — we have an agent fleet that produces marketing at orders of magnitude less cost than human teams, so $25k/mo per goal isn't "marketing spend" in the same sense.
- The "delay all networking" rule for Polis — Polis fundamentally requires the operator to physically attend events.

### 1.4 The Cursor lesson, encoded

**Doctrine takeaway:** *Build an exceptional product; let the operator's voice carry it; amplify users; bottom-up to enterprise; resist marketing fashion until the product is undeniable.*

---

## §2. Linear — building in public + customer amplification engine

**Pattern:** Twitter as a live public changelog; quote-tweet engine that turns customers into the marketing department.

### 2.1 What they did

- Twitter functioned as a live, public changelog rather than traditional marketing.
- Showed (not told) velocity through near-daily ship updates.
- Hyper-authentic — direct feed from the development process; not polished campaign.
- Quote-tweet engine: their feed is mostly Quote Tweets of customer praise + replies celebrating customers, not self-promotion.
- Slack community as Tier-2 engagement surface.

### 2.2 What to copy

| What | Copy for our stack? |
|---|---|
| Live public changelog | **Yes** — D1 Multi-Platform Publisher posts the build-log archetype 1×/week; operator's X carries the founder-thread version of major ships |
| Quote-tweet customer praise as the main feed | **Yes** — D6 Comment Triage drafts QT replies; operator's account is mostly amplifying users, not pushing product |
| Slack/Discord community of users | **Yes** — Discord is the Tier-2 home base for CG and SY |
| Tweet-as-changelog discipline | **Yes** — agents handle posting; operator handles the founder-thread style ship-announcements |

### 2.3 The Linear lesson, encoded

**Doctrine takeaway:** *Make the customers the marketing channel. Be obsessively transparent about velocity. Show shipping; don't claim it.*

---

## §3. Vercel — distribution through Next.js

**Pattern:** Massive open-source download number (Next.js 200M downloads/week) → enterprise relationships → AI infrastructure distribution.

### 3.1 What they did

- Founded ZEIT in 2015 (a decade-long arc to $9.3B valuation by 2025).
- Made Next.js the most-downloaded React framework; gave it away free.
- Enterprise relationships (OpenAI, Anthropic, PayPal, Nike, Walmart) came from Next.js adoption.
- v0 (AI UI generator) launched into the existing Next.js distribution.
- WPP partnership: every WPP client uses Next.js + Vercel as default — enterprise distribution via agency partnership.

### 3.2 What to copy

| What | Copy for our stack? |
|---|---|
| Open-source the primitive; commercialize the integration | **Yes** — Leapable's rule libraries open-source; Pro tier monetizes higher-volume verification + private rules. Synapse open-source core; Pro tier monetizes ViGEm + team features. |
| Become the default in adjacent stacks | **Yes** — Leapable as default verifier in AI agent frameworks; Synapse as default computer-use backend |
| 10+ year arc | **Yes** — operator's mindset is multi-year compounding, not 90-day spike |
| Distribution partnerships at the platform level | **Yes** — `12_DEVELOPER_MARKETING.md §6` (Integrations Guild) |

### 3.3 The Vercel lesson, encoded

**Doctrine takeaway:** *Massive free distribution + enterprise upsell. The free thing must be genuinely best-in-class; the paid thing must integrate seamlessly with the free thing.*

---

## §4. BeReal — campus ambassadors + TikTok virality

**Pattern:** Single-country seed (France colleges) → US campus ambassador program → TikTok-driven viral hits → mainstream.

### 4.1 The numbers

- Founded Dec 2019.
- a16z + Accel $30M round Mar 2021 (mostly off French traction).
- March 2022: top 10 in App Store.
- April 2022: 2.9M DAU.
- July 2022: 5M DAU; #1 in App Store.
- 41M MAU by late 2026.

### 4.2 What they did

- Slow burn in France (2.5 years before US break-out).
- Campus ambassadors (ex-Bumble exec leading US): $30/successful referral, $50/install+review at Brown, Harvard, Rice, UT Austin, Georgetown.
- TikTok-driven virality: 660M #bereal views; 9 official BeReal TikToks crossed 1M views in May-Jul 2022.
- David Aliages: organic 2M-view TikTok demo of the app → BeReal hired him to lead social.
- Creative constraint (1 push notification per day) → product itself is shareable.
- No algorithm, no follower counts, no explore feed — anti-engagement-farming by design.

### 4.3 What to copy for Polis

| What | Copy for Polis? |
|---|---|
| Single-country seed (could be single-city for Polis) | **Yes** — Cambridge/Somerville as seed |
| Campus ambassador economics ($30 referral + $50 install) | **Yes — direct copy** |
| TikTok-driven viral hits | **Yes** — 3-5 vertical / day, hunting for the David-Aliages-equivalent breakout |
| Anti-engagement-farming as a differentiator | **Yes** — Polis doctrine alignment |
| Creative constraint baked into product | **Yes** — Polis events are time-and-place-bound; no infinite scroll |
| Slow-burn willingness | **Yes** — Polis is years, not months |
| Friend-graph required for value | **Yes** — Polis events get richer with more friends |

### 4.4 The BeReal lesson, encoded

**Doctrine takeaway for Polis:** *Density before breadth. Campus before geography. Friend-cluster before user-acquisition. Anti-algorithm as feature, not bug. TikTok creates the mainstream pull; organic creators are the inflection moment.*

---

## §5. Bluesky — federation as cold-start

**Pattern:** Invite-only gates → opened public at the right cultural moment → 14.5M users.

### 5.1 The numbers

- Bluesky 14.5M users as of late 2025.
- 1.5M signups in the week after opening to public (Feb 2024) — 45% growth in one week.
- Compared: Threads 275M MAU, Mastodon 9M.

### 5.2 What they did

- Invite-only beta period (built density before opening floodgates).
- Opened gates when X had a credibility crisis — timing matters.
- AT Protocol federation: lets users pick their own algorithms.
- Community moderation (moderation as feature).

### 5.3 What to copy

| What | Copy for our stack? |
|---|---|
| Invite-driven density | **Polis — yes** (campus invite codes per ambassador in early phase) |
| Open-gates timing | **All products** — coordinate mega-launch with competitor crises or industry events when possible |
| User choice in moderation / algorithm | **Polis — partial** (Polis values the lack-of-algorithm as the feature) |

### 5.4 The Bluesky lesson, encoded

**Doctrine takeaway:** *Density before openness. Open the gates when the cultural moment is right. Federation/choice is a feature when the competitor's centralization fails.*

---

## §6. Stripe — distribution through ecosystems

**Pattern:** Default payment partner in dev ecosystems (Shopify, etc.) → enterprise sales emerged after the developer adoption.

### 6.1 What they did

- Didn't build a traditional enterprise sales team from the start.
- Became default payment partner for Shopify and other rising platforms.
- Grew exponentially via ecosystem partnerships, not linear via sales force.
- "In the Stripe era, distribution created trust" — opposite of "trust creates distribution" of the AI era.

### 6.2 What to copy

| What | Copy for our stack? |
|---|---|
| Default-partner pattern (become embedded in growing ecosystems) | **Yes** — Leapable as default verifier in AI agent frameworks; Synapse as default computer-use; Polis as default gathering primitive for intellectual creator newsletters |
| Avoid premature enterprise sales | **Yes** — bottom-up adoption first; enterprise emerges |
| Ecosystem mode beats sales mode | **Yes** — Influence Network Mapper targets ecosystem partners, not individual buyers |

### 6.3 The Stripe lesson, encoded

**Doctrine takeaway:** *Distribute through growing ecosystems. Become the default. Let enterprise sales emerge from developer adoption, not the reverse.*

---

## §7. Hormozi — posted-and-ghosted vs mega-launch

**Pattern:** Same operator; two different launch patterns for two different goals.

### 7.1 `$100M Offers` (2021, posted-and-ghosted)

- No fanfare, no launch, no PR.
- 99¢ introductory Kindle price; free 90-minute course included with no opt-in.
- High value-to-price discrepancy.
- Grew via word of mouth.
- 26,000 five-star Amazon reviews 4 years later, still #1 in category.

### 7.2 `$100M Money Models` (2025, mega-launch)

- $4M spent in advertising.
- $7M in book sales pre-event.
- Coordinated live event (free to attendees because the offer + ads + money model wrapped together broke even).
- Multi-channel: ads + email + affiliates + warm outreach + content + cold outreach + PR + paid podcasts.
- Designed to demonstrate the concept by being the concept.

### 7.3 What to copy

| What | Copy for our stack? |
|---|---|
| Posted-and-ghosted for Phase A | **Yes** — `17_LAUNCH_PLAYBOOK_90DAY.md §1` |
| Mega-launch for Phase C | **Yes** — same doc |
| The Core Four (warm outreach + content + cold outreach + paid ads) all hitting at launch | **Yes** — launch day playbook in `17_LAUNCH_PLAYBOOK_90DAY.md §7` |
| The Lead Getters reinforcing the Core Four (affiliates, customer referrals, employees) | **Yes** — `08_DISTRIBUTION_CHANNELS.md §2` |
| "Demonstrate the concept by being the concept" | **Yes** — Leapable's marketing must itself be verified; Synapse's marketing must itself be agent-executed; Polis's marketing must itself be physical-event-grounded |
| Free book + free course as massive-value lead magnet | **Yes** — `07_CONTENT_FACTORY.md` lead magnet pattern; `11_PRODUCT_PLAYBOOKS.md` per-product lead magnets |

### 7.4 The Hormozi lesson, encoded

**Doctrine takeaway:** *Both patterns work. Use posted-and-ghosted to validate; use mega-launch to scale. Massive-value lead magnets pull the audience to you. The marketing for X is itself an example of X.*

---

## §8. Common patterns across all wins

Synthesizing across Cursor / Linear / Vercel / BeReal / Bluesky / Stripe / Hormozi:

1. **Exceptional product is the marketing.** No amount of fleet velocity rescues a mediocre product.
2. **Founder voice is the catalyst.** It can be small in volume (Cursor) or substantial (Hormozi) but it's never delegated.
3. **Free or generous tier as the wedge.** Cursor free tier, Next.js free, BeReal free, Bluesky free, Hormozi 99¢ book with free course.
4. **Density before breadth.** Single channel, single city, single ecosystem — then expand.
5. **Customers as the marketing channel.** Quote-tweet customers, amplify user wins, build them into ambassadors.
6. **Slow burn → inflection moment.** Each took years. Patience is doctrine.
7. **Anti-fashion stance.** Refused obvious-correct moves (Cursor avoided conferences; BeReal removed algorithms; Linear refused TV-ad-style marketing).
8. **Timing the mega-moment.** When the inflection is reached, double-down with coordinated launch.

These eight elements are the case-study-extracted doctrine. The fleet's 90-day playbook is engineered around them.

---

## §9. Anti-patterns from failed cases

Counter-examples (companies / products that had brief spikes and then died):

- **One-platform dependency** — Vine, Clubhouse, Vero. Built on a single platform's algorithm; when the platform changed, the product collapsed. **Lesson:** multi-channel from day 1.
- **Mass-DM cold outreach with no human gate** — name redacted: brief growth followed by domain-reputation collapse, account bans, brand-damaged-irreversibly. **Lesson:** human-in-the-loop on outbound, always.
- **Influencer-marketing-only consumer launches** — name redacted: $5M spent on influencer campaign, 1M downloads, 28-day retention 3%, dead in 6 months. **Lesson:** retention floor before scaling.
- **Posted-and-ghosted with a mediocre product** — most indie launches. **Lesson:** the product must carry the load; marketing reveals it but doesn't replace it.
- **Mega-launch with no Phase A/B** — high-spend launches that died because the product hadn't been validated. **Lesson:** the launch isn't the start; it's the result of the prior 60 days of compounding.

---

## §10. The case-study summary one-liner per product

| Product | Closest reference pattern | One-line strategy |
|---|---|---|
| Leapable | Cursor + Linear + Vercel | Build an undeniable product; founder X-voice catalyst; amplify users; bottom-up to enterprise; default-partner in AI agent frameworks |
| Synapse | Cursor + Stripe | Free open-source core; become the default computer-use backend; hackathons + integrations guild |
| Polis | BeReal + Bluesky | Single-city seed with campus ambassadors; TikTok virality; anti-engagement-farming as the feature; density before breadth |

---

## §11. Cross-references

- Per-product launch playbooks: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Per-product GTM strategies: `11_PRODUCT_PLAYBOOKS.md`
- DevRel flywheel (Cursor/Linear/Vercel application): `12_DEVELOPER_MARKETING.md`
- Polis-specific (BeReal/Bluesky application): `13_CONSUMER_SOCIAL_LAUNCH.md`
- Channel mix: `08_DISTRIBUTION_CHANNELS.md`
- Doctrine extracted from these cases: `02_FIRST_PRINCIPLES.md`
