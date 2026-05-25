# Business Model — Civic Subscription, Venue Revenue Share, Patronage, Cooperative

The business model is doctrine. Per Invariant **I5**: `ad_revenue_per_user_dollars = exactly 0`. Per Invariant **I13**: no third-party data sale. Per Invariant **I16**: no pay-to-play in matching. These three close out, by construction, the four most common social-platform revenue models (advertising, data brokerage, premium-priority-matching, lead-generation referrals).

What is left, and what is enough: a quadruple revenue stack — civic subscription, venue revenue share, patronage / philanthropy, and (year-3+) inter-polis cooperative services. The platform is run as a multi-stakeholder cooperative (`14`), not a profit-maximizing corporation; surplus is reinvested in the civic fund or returned as venue-partner discount.

## 1. The doctrine

The business model exists to **serve the mission, not the reverse**. We size the platform's operating cost to what the revenue stack can sustainably support; we do not expand the cost base to chase top-line growth. Modest growth, durable invariants, generational time horizon. (Compare Patagonia's 2022 trust restructuring: "Earth is now our only shareholder." Compare Mozilla Foundation's structure. Compare the John Lewis Partnership in the UK. Compare cooperative banks in Switzerland.)

The numbers below are illustrative for a 50,000-citizen polis after Year 2; actual per-city economics vary by region and stage. Detailed projection is in `18_ROADMAP_FROM_ZERO_TO_POLIS.md`.

## 2. Stream 1 — Civic Subscription (the largest stream)

### 2.1 Mechanics

Citizens pay a modest, sliding-scale annual **civic subscription** — comparable in spirit to a public library card, a YMCA membership, or a Belgian *culture-pass* — that gives them full access to the platform's matching, gatherings, Paideia layer, and Agora. Tiers:

| Tier | Annual cost (USD; localized) | Notes |
|---|---|---|
| **Standard** | $96 / year ($8/month) | The default — covers a citizen who attends ~ 8-16 events / year |
| **Solidarity** | $48 / year | For citizens whose income is below the city's living-wage threshold; no income proof required, citizen self-attests |
| **Patron** | $240 / year | For citizens who can afford to underwrite a Solidarity slot |
| **Free** | $0 | For citizens with attested financial hardship; covered by Patron + Civic Fund |

No exclusion. Inability to pay is **never** a barrier to citizenship.

### 2.2 What subscription is *not*

- **Not pay-to-play.** Subscription gives access; it does not buy match priority, faster invitations, or any algorithmic advantage. (Invariant I16.)
- **Not "premium content."** There is no premium content. Every Theater reading, every Agora primer, every gathering format is available to every tier.
- **Not freemium-with-paywall.** A citizen at the Free tier has the identical platform experience to a Patron.

### 2.3 Why it works economically

At Standard tier, $96/year × ~ 60% paying tier (40% Solidarity/Free, subsidized by 10% Patron) × 50,000 citizens = ~ $2.9M/year per polis civic subscription. This is the load-bearing stream. Compare: the Mozilla Foundation operates a global organization on ~ $500M/year; a polis-scale civic platform with strict cost discipline operates on a small fraction of that per city.

### 2.4 What the citizen sees

A simple annual or monthly recurring charge. One line item. No upsells, no premium ladders, no tracked-feature-usage. Annual renewal includes a one-page report of the platform's operations in the citizen's polis (Festival attendance, number of Agora hosted, Tier-1 metric movements — aggregate only, never per-citizen).

## 3. Stream 2 — Venue Revenue Share

### 3.1 Mechanics

Per `12` §3.2: Partner Venues pay a small per-attendee partnership fee (≤ 8%) on group-rate gatherings the platform sends them. The platform never touches food/drink money; the venue charges citizens directly at the gathering; the venue remits the partnership fee to the platform monthly.

### 3.2 Why this is structurally aligned

- The fee is a fraction of *marginal revenue* the venue would otherwise not have (shoulder-time slots).
- The fee scales with platform usefulness — more gatherings sent → more revenue, but also more value to the venue and more activity in the city.
- It cannot be extracted unilaterally — venue partners are cooperative members with governance seats (`14`), and the fee schedule is set by the cooperative as a whole, not by management.
- Capped at 8% by cooperative articles — increases require venue-partner-class approval.

### 3.3 Order of magnitude

A polis with 50,000 citizens, ~ 1.5 events/citizen/week × 50% gathering-at-a-venue rate × average $30 spend × 8% fee = ~ $4.7M/year per polis. This is the second load-bearing stream, comparable in size to civic subscription.

## 4. Stream 3 — Patronage and Philanthropy

### 4.1 Mechanics

- **Patron-tier civic subscriptions** as above.
- **Foundation grants** — the platform's civic mission (loneliness reduction, civic deliberation revival, third-place catalyst, cross-ideological tie formation) aligns directly with the missions of multiple major foundations (Rockefeller, Knight, Hewlett, Ford, MacArthur, individual donor-advised funds). Year 2+: dedicated grant-development cooperative role to pursue these.
- **Individual philanthropy** — the cooperative accepts named donations (a "Symposium Endowment" at a venue, a "Theater Series Endowment" for a season of readings) but with strict no-influence terms: donors receive a thank-you and a printed annual report; donors do not appear in any matching, debate, or governance role beyond their citizen membership.
- **Government civic infrastructure grants** — many cities, EU regions, and US states fund "civic engagement" / "loneliness" programs; the platform applies to these on the same footing as any non-profit.

### 4.2 Constraints

- No corporate sponsorships of any gathering. (Invariant I5 extended.)
- No named-rights for any platform feature ("the Acme Symposium" — forbidden).
- No quid-pro-quo with donors — donations are gifts to the cooperative, not investments with returns.
- Donors do not appear in any way in citizen-facing surfaces (no donor wall, no donor newsletter to general citizens, no "sponsored Agora").

### 4.3 Order of magnitude

For a polis at scale, Patron-tier covers ~ 5-10% of operating budget; institutional philanthropy is a long-arc accelerant rather than a load-bearing stream. Total ~ $500K-$2M/year per polis as the platform matures.

## 5. Stream 4 — Civic Fund (cross-polis cooperative services)

### 5.1 Mechanics

Year-3+: as the Polis Network expands across cities, the central cooperative provides shared services to local cooperatives — common technology infrastructure (the matching engine, the Citizen Constellation substrate, the Agora protocol), shared Paideia curatorial council, shared sortitioned algorithmic-review-board, shared legal / audit / compliance.

Local polis cooperatives pay a small per-citizen affiliation fee to the central cooperative (~ $4-8/citizen/year) covering these shared services. This is sized to break-even on shared services, not as a profit stream.

### 5.2 The civic fund

A reserve fund accumulated from surplus across all streams. Used for:

- Catalyst grants in third-place deserts (`12` §5).
- Solidarity-tier and Free-tier subsidies.
- New-city launch costs (the first 18 months of a new polis run negative; civic fund covers the gap, repaid as the polis reaches break-even).
- Open-source maintenance of the platform's stack (returned to ContextGraph, ClipCannon, and the broader research community per `15`).
- Research grants to academic studies of the platform's civic outcomes (independent, no influence on findings).

## 6. The full operating budget at scale (illustrative)

For a single mature 50,000-citizen polis, ongoing operating cost per year:

| Cost category | Annual cost (USD) |
|---|---|
| Engineering / platform maintenance (allocated share of central cooperative) | ~ $400K |
| Local polis staff (3-5 FTE: local director, community-development lead, two sortition / convener support coordinators, half-time legal/finance) | ~ $600K |
| Welcoming-citizen / sortition stipends (lightly compensated rotating roles, service-credit + small honorarium) | ~ $300K |
| Catalyst grants + civic-fund local allocation | ~ $300K |
| Venue partnership program staff + outreach + reimbursements | ~ $400K |
| Legal / audit / compliance / cooperative-governance support | ~ $200K |
| Festival / public-program direct costs | ~ $300K |
| **Total** | **~ $2.5M** |

Revenue at scale (civic + venue + patronage + central affiliation): ~ $7M. Surplus: ~ $4-5M, reinvested per `14 §4` cooperative articles.

This is healthy for a civic-infrastructure cooperative. It is not "venture returns." Anyone seeking venture returns should not invest in this platform.

## 7. Funding the build-out (the cold-start problem)

The platform requires upfront capital before any city reaches break-even. This is funded by:

- **Founder seed** — the founder's own commitment as patient capital, accepting a long-arc return (the cooperative cannot pay out equity; the founder receives a salary as a trustee-class member, not a return on capital).
- **Mission-aligned PRI / program-related investments** from foundations (low-interest, civic-purpose loans).
- **Patient philanthropic equity** — donor-advised funds or family offices willing to contribute capital structured as a forgivable loan repaid only if the cooperative consistently generates surplus.

We **do not** take traditional venture capital. Standard VC term sheets are incompatible with the cooperative + asset-lock + Tier-4 invariants. Attempting it would require violating Invariant **I12** within 18 months as VCs demand growth-rate and exit-path commitments incompatible with the doctrine.

## 8. The honest case to a prospective citizen

> *Why does this cost $96/year?* — Because anything you depend on long-term should be paid for by you or by people who love you, not by advertisers whose interests are opposed to yours. Public libraries cost something. Symphony halls cost something. The civic infrastructure of a polis cost something in classical Athens (the *liturgia*). $96/year is less than 9 hours of minimum wage, less than a streaming-service subscription, less than two restaurant dinners. If you genuinely cannot pay, you join Solidarity or Free and your full membership is exactly the same.

## 9. The honest case to a prospective foundation

> *Why fund a city's civic platform when you could fund traditional non-profits?* — Because the mechanisms that produce civic flourishing — Granovetter weak ties, Oldenburg third places, Ovadya bridging, Putnam social capital — are exhaustively documented but currently delivered nowhere at scale. The serendipity engine is the first deliverable infrastructure for these mechanisms. A modest grant funds the launch of a polis; subscription + venue revenue funds steady-state; the long-term flourishing belongs to the city.

## 10. Cross-references

| Topic | Doc |
|---|---|
| Tier-4 invariants closing other revenue models | `03_NON_NEGOTIABLE_INVARIANTS.md` |
| Venue partnership economics | `12_THE_THIRD_PLACE_REVIVAL.md` §3 |
| Cooperative governance structure | `14_GOVERNANCE_AND_SORTITION.md` |
| Roadmap with per-stage revenue trajectory | `18_ROADMAP_FROM_ZERO_TO_POLIS.md` |
| Anti-gaming including no pay-to-play | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |

---

**Last updated:** 2026-05-24.
