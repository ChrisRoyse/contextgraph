# Roadmap — From Zero to a Polis Network

Six phases over ~ 7 years. Slow on purpose. Compared to a venture-scale social product (Instagram: 0 → 100M users in 2.5 years), this is glacial. That is the point. Civic infrastructure is built on a generational timescale; rushing it is what produces enshittification later.

Phase gates are *empirical*, not date-based. We do not move to the next phase until the prior phase's evidence predicate is satisfied. If a phase fails its gate, we iterate or kill the project per `17` §2.

## Phase 0 — Foundation (Months 0–6)

**Objective:** specify, design, and instrument the first polis launch. No public-facing platform yet.

**Deliverables:**

- This doc set (`docs/serendipityengine/00`–`21`). ✓ in progress.
- Initial founder + 3-5 advisory board (technical, civic, legal, philanthropic).
- Selected first polis (a neighborhood-scale launch — likely Asheville NC, Pittsburgh PA, Lancaster PA, Bozeman MT, Boulder CO, Burlington VT, or a comparable mid-sized walkable American city with both intellectual density and lower cost-of-living — final selection by Phase 0 §3 criteria).
- Phase-0 baseline survey of ~ 500 residents of the first polis (UCLA-3, Steger MLQ, custom Eudaimonia, civic-engagement questions) administered in-person by paid interviewers — establishes the within-polis baseline against which Tier-1 #4-6 deltas are measured.
- Initial Partner Venue scouting (~ 50 potential venues in the launch neighborhoods).
- Founder pre-commitment trust agreement signed (`14` §9).
- Technical architecture (`15`) prototyped — extension of existing ContextGraph workspace; new crates skeletoned.
- Initial PoP credential infrastructure tested with ~ 25 friend-and-family attestation pilots.

**Phase 0 → Phase 1 gate:** all of the above completed and reviewed by advisory board. Baseline survey published. Technical prototype passes a 100-citizen synthetic-data simulation.

### First polis selection (§3 criteria for Phase 0)

The first polis should have:

- Population 50,000–250,000 (manageable scale; not NYC).
- Walkable urban core with ≥ 30 plausible Partner Venues within a 3-mile radius.
- High civic-engagement baseline (existing run clubs, library systems, religious communities, civic-arts scenes) to seed early Established cohort.
- Diverse demographics (avoid college-town monoculture).
- Founder or trusted ally personally embedded (cannot launch a polis from outside; needs in-person stewardship).

## Phase 1 — First polis MVP (Months 6–18)

**Objective:** prove the engine works in one neighborhood of one city, ~ 2,000 active citizens.

**Deliverables:**

- Public launch to ~ 100 invited Established-track citizens (drawn from existing civic-engagement communities).
- First 200 Symposia, ~ 80 Gymnasium sessions, ~ 40 Theater sessions, ~ 30 Agora sessions over Months 9-18.
- First Festival at Month 12 (~ 200 attendees).
- First chevruta cohort (~ 30 pairs reading 8-week works).
- Partner Venue program: 25 active partners by Month 9, 60 by Month 18.
- Mistake-loop training data feeding back into matcher (initially with a wide prior; tightens by Month 12).
- First 90-day baseline → 12-month follow-up survey to measure Tier-1 #4-6 deltas.

**Phase 1 → Phase 2 gate:** Tier-1 #1 ≥ 1.0 (events/citizen/week, median); Tier-1 #2 ≥ 1.5 (strong ties/citizen/year, projected from 12-month signal); Tier-1 #4 delta ≥ −0.5 (loneliness reduction); Tier-2 #10, #11, #12 each exactly target (architectural verification, not aspirational). If any of these fail, *iterate the algorithm and operations in Phase 1*, do not expand to Phase 2.

## Phase 2 — Full first polis (Months 18–30)

**Objective:** expand to full polis scale (~ 15,000–25,000 active citizens across 5-8 neighborhoods of the first city). Constitute the cooperative.

**Deliverables:**

- Citizen Assembly first convening (Month 24) — 1,000 sortitioned citizens; quarterly thereafter.
- SARB constituted (Month 24); first SARB-approved algorithm-weight changes ship.
- Cooperative incorporation completed; articles of incorporation enshrine Tier-4 invariants (`14` §4).
- Civic subscription rollout (Standard / Solidarity / Patron / Free); revenue stream activated.
- Civic Venue Program with public libraries + community centers in launch polis.
- First catalyst grant (~ $25k) to a neighborhood-entrepreneur cafe in a third-place desert.
- First polis-wide Festival ( Month 24) at ~ 1,500 attendees.
- Per-polis Tier-1 baseline → 24-month delta measured.

**Phase 2 → Phase 3 gate:** Tier-1 #1 ≥ 1.5, #2 ≥ 3, #4 delta ≥ −1.0, #5 delta ≥ +0.5, #6 delta ≥ +1.0 in the first polis, **sustained for ≥ 6 consecutive months**. Tier-2 architectural metrics intact. All 18 Tier-4 invariants pass quarterly audit. Cooperative constituted and functioning. If any gate condition fails, *deepen Phase 2 work and re-measure*, do not expand.

## Phase 3 — Second + Third Polises (Months 30–48)

**Objective:** prove the engine generalizes by launching in 2 additional cities; demonstrate that local sortitioned governance functions across multiple polises.

**Deliverables:**

- Polis 2 launch (Months 30-36): a city deliberately different from Polis 1 (e.g., if Polis 1 is mid-sized walkable secular Western, Polis 2 should test a different cultural profile — perhaps a small religiously-active community, or a Latin American mid-sized city, or a working-class neighborhood in a major metro). Selection by sortitioned expansion-decision panel.
- Polis 3 launch (Months 36-42): a third profile, increasing diversity.
- Cross-polis affiliate billing + central cooperative services activated.
- First cross-polis Agora pilot (Month 42; experimental).
- Tier-1 + Tier-3 metrics tracked per-polis with publicly-published quarterly reports.

**Phase 3 → Phase 4 gate:** Polis 2 and Polis 3 each hit Phase-2-gate Tier-1 numbers within their first 18 months of operation; their Tier-1 #16 (cross-ideological) and #17 (cross-generational) lift meaningfully; cross-polis governance functions. If a polis fails its gate, the central cooperative pauses new expansion, conducts root-cause analysis, possibly retires or restructures that polis.

## Phase 4 — Polis Network bootstrap (Months 48–60)

**Objective:** grow to 8-12 polises across diverse contexts; refine the central cooperative; establish cultural infrastructure of the Polis Network.

**Deliverables:**

- 5-9 additional polis launches (mix of US, Canada, Western Europe, perhaps one in Asia or Latin America — sortitioned decision by Year-4 cooperative).
- Polis Network protocol formalized: shared technology infrastructure, shared Paideia curatorial council (rotating across polises), inter-polis Festival exchanges.
- First international Polis Network gathering (Year 5): 20 sortitioned citizens from each polis meet for a week at a rotating host polis; share lessons; publish findings.
- Civic-Fund-supported research grants commissioned (~ 4 academic studies on the platform's outcomes).
- Founder transitions to Trustee-class only; full operational handoff to cooperative management + SARB + Citizen Assemblies (`14` §9 schedule).

**Phase 4 → Phase 5 gate:** ≥ 8 polises operating with each meeting Tier-1 gate sustainably; cooperative governance functions across polises; founder operational handoff complete and stable.

## Phase 5 — Maturation (Years 5–7)

**Objective:** stabilize as a multi-polis cooperative civic-infrastructure project; refine; deepen.

**Deliverables:**

- Year-5 retrospective: published, public, with full Tier-1/2/3 data per polis; named what we got right; named what we got wrong.
- Year-2+ scope items activated:
  - Family-format gatherings (`09` §8).
  - Cross-generational chevruta.
  - Storgē-layer work (`10` §5).
- Open-source contributions back to ContextGraph and ClipCannon (the platform is itself an FSV/hardening case study for those projects).
- Possible launch of "Polis Network of Networks" coordination (peer cooperation with adjacent civic-cooperative projects: Mondragon, John Lewis Partnership, Patagonia Trust, etc.).

**Phase 5 → ongoing:** there is no "Phase 6 = world domination" gate. The platform aims to be a *stable, lasting* piece of civic infrastructure — like a public library system — that operates indefinitely under sortitioned cooperative governance, growing slowly as new polises naturally request to join.

## A note on growth rate

A modern social product at venture pace would expect 100x growth per year. This roadmap projects ~ 4x growth per year (in number of polises) through Phase 4, then sub-linear thereafter.

This is *intentionally* slow because:

- Each polis requires deep Partner Venue work, sortitioned governance bootstrap, and community trust-building that cannot be parallelized at scale.
- Faster growth would push us into ad-revenue or growth-hack territory (forbidden by Invariants I5 / I9).
- The platform's value compounds over decades; rushing the early years undermines the late ones.
- We are not optimizing for shareholder return; we are optimizing for civic flourishing on a generational timescale.

## Estimated active-citizen counts by phase

| End of phase | Active citizens (total network) |
|---|---|
| Phase 0 | ~ 0 (no public launch) |
| Phase 1 | ~ 2,000 |
| Phase 2 | ~ 20,000 |
| Phase 3 | ~ 50,000 |
| Phase 4 | ~ 200,000 |
| Phase 5 | ~ 500,000 |

Versus Instagram's first 18 months (~ 30M). The whole point is to be different.

## Cross-references

| Topic | Doc |
|---|---|
| Tier-1/2/3/4 metrics gate-gating each phase | `16_METRICS_AND_FALSIFICATION.md` |
| Governance transition schedule (Trustee handoff) | `14_GOVERNANCE_AND_SORTITION.md` §9 |
| Business model revenue trajectory | `13_BUSINESS_MODEL.md` |
| Third-place revival per-polis playbook | `12_THE_THIRD_PLACE_REVIVAL.md` |
| Falsification predicates that can halt phases | `16_METRICS_AND_FALSIFICATION.md` §4 + `17` §2 |
| Technical architecture buildout estimate | `15_TECHNICAL_ARCHITECTURE.md` §11 |

---

**Last updated:** 2026-05-24.
