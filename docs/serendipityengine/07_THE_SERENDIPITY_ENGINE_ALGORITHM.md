# The Serendipity Engine Algorithm

The matching engine is the heart of the system. This document specifies what it optimizes for, what guardrails constrain it, and how it learns. Discipline imitates `docs/futurebuild/specs/FSV-PROTOCOL.md` and the TCT predictor architecture of `docs/PAPER.md` §5.

## 1. The problem formally

**Given:**

- A set of citizens `C = {c_1, ..., c_N}` in a polis, each with a Constellation `Φ(c_i)` per `05`.
- A weekly capacity for each of the Five Formats (`06`) limited by available venues, conveners, and citizen preferences.
- A history of past events with attendee tuples and post-event reflection signals.

**Find:** an assignment of citizens to gatherings each week that maximizes expected Tier-1 outcomes (`01` §2) over a 12-month horizon, subject to the Tier-4 invariants (`03`) and the constraint that every event is one of the Five Formats.

## 2. The objective function

Borrowing Schmidhuber's compression-progress framing for "interesting," the per-citizen-per-event utility is:

```
U(c, g) = α · ExpectedTieFormation(c, g)         # tie-forming value
        + β · ExpectedCompressionProgress(c, g)  # learning / "interesting" value
        + γ · ExpectedJoy(c, g)                  # affective / fun value
        + δ · CivicValue(c, g)                   # benefit-to-others-not-c
        − ε · ProjectedHomogeneity(g)            # anti-bubble penalty
        − ζ · ScheduleStrain(c, g)               # anti-burnout penalty
```

where:

- `ExpectedTieFormation`: regressor trained on (Constellation pair compatibility, format, prior co-attendance, post-event "would re-meet" signal) → P(re-meet within 30 days), calibrated via split-conformal predictor as in ContextGraph (`crates/context-graph-solver`).
- `ExpectedCompressionProgress`: per-citizen, modeled as the divergence between the citizen's pre-event Constellation and the post-event update direction. Operationalized: cosine distance between the citizen's M1 centroid and the *median* M1 centroid of co-attendees, capped — too close = redundant, too far = alienating; sweet spot ≈ 0.25-0.45 cosine.
- `ExpectedJoy`: regressor on (rhythm match M7, somatic-band match M10, vocabulary overlap M5) → P(citizen rates event ≥ 4 on "was worth your time").
- `CivicValue`: weight for events that pair a Newcomer with Established citizens (newcomers absorb city-knowledge); weight for events that include a sortitioned welcoming role; weight for Aleatory format.
- `ProjectedHomogeneity`: bridging-based-rerank penalty (per Ovadya / Belfer 2022); raised aggressively when M9 topic-portfolio overlap across attendees > 0.75 or M12 ideological-clustering > 0.7.
- `ScheduleStrain`: capacity penalty if citizen has already attended ≥ 3 events that week; ramps super-linearly.

**Default weights (Phase 0 calibration):** α=0.30, β=0.25, γ=0.20, δ=0.10, ε=0.10, ζ=0.05. All weights are version-controlled, every change requires sortitioned algorithmic-review-board log (`14 §3.2`), and the current values are published openly per Invariant I18.

## 3. The Constellation Guard (the `G_τ` gate)

Before any candidate gathering is scored by §2, it must pass the Constellation Guard:

```
G_τ(g) = 1 [ ∀ c ∈ attendees(g) :
             ∀ m ∈ M_required(format(g)) :
               cos(φ_m(c), c_m^centroid(g)) ≥ τ_m(format(g)) ]
```

In English: for each format, certain modalities (M13 hard constraints **always**; M7 rhythm and M10 somatic-band **for Gymnasium**; M5 vocabulary and M11 aesthetic **for Symposium**; M4 narrative-arc and M12 ideological-diversity **for Agora**; M9 topic and M1 semantic **for Theater**) must clear a same-quality-band floor `τ_m` computed on a disjoint per-format calibration set, **never tuned on the metric we evaluate** (per `docs/PAPER.md` §5.4 discipline).

Any candidate gathering that fails the guard is rejected at the gate, regardless of `U` score. This prevents the matcher from gaming the objective by selecting "high-U" gatherings that catastrophically miss a hard constraint (e.g., scheduling a 6am run for a citizen who's stated unavailable before 9am).

## 4. The weak-tie-first principle

Per Granovetter — and per the explicit Tier-1 metric #3 (`01` §2) — the matcher **biases against putting strong ties in the same gathering** when other options exist. Two close friends sharing a Symposium is not serendipity; it's a dinner they could have had on their own. The matcher's mental model:

- **Strong tie (defined: ≥ 3 prior co-attendances + sustained out-of-platform contact):** at most 1 strong-tie pair per gathering (zero preferred for Newcomers).
- **Recent weak tie (1-2 prior co-attendances):** at most 2 such pairs; no triangle of 3 who've all met.
- **Stranger pair:** the dominant connective tissue of every gathering.

This is the inverse of Facebook ("ranked feed shows you what your friends are doing"); the platform is engineered to expose you to people you would not have met otherwise.

## 5. Selection mechanism — MMR + sortition + bridging rerank

Each week, the matcher generates candidate gatherings by:

1. **Coarse:** k-nearest-neighbor over per-modality Constellation cosine (using the same HNSW infrastructure as ContextGraph), filtered by hard constraints (M13), produces ~100 candidate attendees per slot.
2. **Diversification:** **Maximal Marginal Relevance (MMR)** rerank (existing in `crates/context-graph-solver`) with λ ≈ 0.6 — half relevance to seed, half spread across the candidate pool. This prevents filter-bubble selection.
3. **Bridging rerank:** apply Ovadya-style bridging weights — candidates that bridge between previously-disconnected sub-clusters in the M9/M12 graph receive a multiplicative boost.
4. **Sortitioned shuffle:** for each gathering, a fraction of slots (~15% for Symposium, ~25% for Agora, 100% for Aleatory) are filled by **uniform-random draw from the eligible pool**, not by algorithmic ranking. This is the structural counter-pressure to algorithmic monoculture. The fraction is published and audited.
5. **G_τ filter:** §3.
6. **Score and emit:** §2 ranks the surviving candidate gatherings; the top-N per week are emitted as invitations, subject to per-citizen weekly budget caps.

## 6. The mistake loop — how the matcher learns

Exactly the ContextGraph #406 / #523 architecture, applied to social matching:

- **Inputs (training signals):**
  1. 24h post-event reflection (per `04` §3.4) — primary.
  2. 7-day re-attendance opt-in.
  3. **30-day tie-strength signal** — the citizen self-reports whether they've had real out-of-platform contact (coffee, message exchange not on platform, etc.) with at least one new co-attendee.
  4. Quarterly eudaimonia survey (`01` §3).
- **Online head (per-citizen weight nudges):** a small per-citizen sub-model updates weight profiles (which of α, β, γ, δ matter most for *this* citizen) under EWC++ Fisher protection (per ContextGraph #406), so updates do not catastrophically forget the citizen's stated preferences.
- **Frozen targets:** the Constellation centroids themselves do **not** move. (Per `05` §3 — frozen-target principle from TCT.)
- **Replay buffer:** events where the matcher predicted a 0.8 re-meet rate and got 0.1 become high-weight mistake-replay samples for the global ranker.
- **Auditable:** every weight update logs to `CF_MATCHING_WEIGHT_HISTORY` with operator (always "system" + sortitioned-review-board approval ID), reason, and pre/post values. Compare ContextGraph `lifecycle-provenance` agent.

## 7. The capacity-allocation problem

Each city has finite weekly capacity per format (venues, conveners, citizen energy). Allocation across citizens is a **stable-matching / optimal-transport** problem:

- Each citizen has a weekly demand vector (which formats they want, how much).
- Each format has a weekly supply (gatherings × seats).
- Match via a modified **Gale-Shapley** with capacity, breaking ties via the §2 utility — ensuring every citizen gets matched to at least 1 high-utility gathering per week before any citizen gets a 2nd, before any gets a 3rd, etc. This is a fairness floor (Rawlsian, not utilitarian).

For Newcomers, the floor is doubled (a Newcomer who hasn't had an event in 14 days is force-promoted to the next available gathering, even at a modest utility cost — onboarding fragility is acute).

## 8. What the algorithm is forbidden from doing

(All enforced by I-series invariants in `03`.)

- It is **forbidden from using "engagement"** as a signal — time-in-app, sessions, notifications-opened. These are all anti-objectives.
- It is **forbidden from amplifying high-output citizens** (the citizen who attends 20 events/month does not get prioritized — `δ` even penalizes excess attendance via ζ).
- It is **forbidden from optimizing for "growth"** — invite acceptance rate, virality, etc. The platform's growth is governed by `18`, not the algorithm.
- It is **forbidden from running A/B tests** on attendees without sortitioned-review-board approval and a pre-registered, falsifiable hypothesis (per `16`). No silent experimentation on human social fabric.
- It is **forbidden from incorporating any social-graph-position metric** that would constitute a centrality measure visible to others or that would advantage central nodes in selection.

## 9. Adversarial cases (the algorithm gets tested against these)

Per CLAUDE.md §10 ("Assume the task is wrong"), the algorithm is continuously adversarially tested. The adversarial corpus (analogous to ContextGraph's TASK-PY-G-060 30-fixture set):

| # | Adversarial pattern | How matcher must fail-closed |
|---|---|---|
| A1 | Citizen Sybils — multiple personhood accounts | Proof-of-personhood at onboarding (`08` §3); no signal accepted from un-verified |
| A2 | Convener cliques — same group always co-attending | Strong-tie cap from §4 + sortitioned shuffle from §5.4 |
| A3 | Topic monoculture — all matches in one topic | MMR + bridging rerank + ε penalty from §2 |
| A4 | Romance prospecting at non-romance formats | Hard constraint M13 + Symposium dress-code-of-conduct enforced behaviorally; reports → 90-day pause |
| A5 | Newcomer ghosting — citizen attends 1 event then vanishes | Outreach offer at §4.4 30-day check; no shaming |
| A6 | Convener overuse — one Elder hosts 80% of Symposia | Convener slot rotation, max 2/month per Elder |
| A7 | Citizen tries to game by stuffing intake interview with high-status terms | DDA's multi-embedder defense from `05` §7 #2; bridging rerank reduces homophily of high-status terms |
| A8 | A city's matching becomes dominated by a single sub-clique | Quarterly bridging-coverage audit by sortitioned review board; algorithm reweight if homogeneity drifts |

## 10. Performance & infrastructure

- The matching service runs on the same RocksDB + 13-embedder substrate as ContextGraph, sharing CFs:
  - `CF_CITIZEN_CONSTELLATION` (per `05`)
  - `CF_CITIZEN_PROVENANCE`
  - `CF_GATHERING_CANDIDATES`
  - `CF_GATHERING_OUTCOMES`
  - `CF_MATCHING_WEIGHT_HISTORY`
- Weekly matching computation: O(N × log N) for HNSW recall × number of candidate gatherings; for a 50,000-citizen polis the full weekly compute completes in < 30 minutes on aiwonder-class hardware (verified target).
- Real-time path: citizen RSVP / drop-out triggers an incremental re-match for the affected gathering only — < 5s p95.

## 11. Cross-references

| Topic | Doc |
|---|---|
| The Constellation it consumes | `05_THE_CONSTELLATION_DDA_LAYER.md` |
| The Five Formats it produces invitations to | `06_THE_FIVE_GATHERING_FORMATS.md` |
| Anti-gaming mechanisms | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |
| Metrics the algorithm is judged by | `16_METRICS_AND_FALSIFICATION.md` |
| Adversarial cases & failure modes | `17_FAILURE_MODES_AND_BLINDSPOTS.md` |
| Technical infrastructure | `15_TECHNICAL_ARCHITECTURE.md` |

---

**Last updated:** 2026-05-24.
