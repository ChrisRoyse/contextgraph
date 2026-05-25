# Metrics and Falsification

What gets measured shapes what gets built. This document is the *contract* for what the platform measures, how, with what privacy guarantees, and — crucially — **what would falsify the entire project**. Following the same discipline as `docs/futurebuild/specs/TARGET-METRICS.md` for ContextGraph and `docs/futurebuild/specs/FSV-PROTOCOL.md` for the verification contract.

Per CLAUDE.md §0 — *every action must move a number on this sheet, or it is worthless.*

## 1. The four tiers (already established in `01` §2)

| Tier | Purpose | What moves the system |
|---|---|---|
| **Tier 1** | Civic outcomes — the only ship-gating numbers | Real-world flourishing |
| **Tier 2** | Anti-anti-objectives — what must NOT happen | Engagement, status, ads |
| **Tier 3** | Knowledge & love — deeper outcomes | Books read, debates attended, ties formed, romantic matches sustained |
| **Tier 4** | Doctrinal invariants — immovable fences | Architecture & governance |

A feature that does not move at least one Tier-1/2/3 number or preserve a Tier-4 invariant is **worthless** and must not ship.

## 2. The instrumented metrics — operational definitions

### 2.1 Tier 1 — Civic outcomes (the ship-gate)

| # | Metric | How measured | Source | Privacy |
|---|---|---|---|---|
| 1 | `serendipitous_event_rate_per_citizen_per_week` | Sum of attended Five-Format gatherings / weeks-active | `CF_GATHERING_OUTCOMES` | Per-citizen private; polis-aggregate published |
| 2 | `qualified_new_strong_tie_rate_per_citizen_per_year` | Self-attested 30-day signal "had off-platform contact with co-attendee X" rolled forward; survives 6mo follow-up | `polis_reflection_30d` + quarterly survey | Per-citizen private; aggregate published |
| 3 | `qualified_new_weak_tie_rate_per_citizen_per_year` | 30-day attestation "would meet again, even if hasn't yet" | `polis_reflection_30d` | Same |
| 4 | `loneliness_score_delta_vs_baseline` | UCLA-3-Item Loneliness Scale, administered quarterly, anonymized | quarterly survey | DP-aggregated only |
| 5 | `purpose_score_delta_vs_baseline` | Steger MLQ-Search 5-item subscale, administered quarterly | quarterly survey | DP-aggregated only |
| 6 | `eudaimonia_index_delta` | Custom 5-Likert per `01` §3 | quarterly peer-administered survey | DP-aggregated only |

**Baseline:** Phase 0 in each polis establishes a baseline via in-person sample survey of ~ 500 residents *before* the platform launches. Delta is computed against this within-polis baseline, not against other cities or other platforms.

**Ship-gate predicate:** Tier 1 #1, #2, #3, #4, #5, #6 must all clear their targets in *at least* 4 polises for *at least* 12 consecutive months *each*, while Tier 2 #10, #11, #12 remain at exactly their target values and all Tier 4 invariants hold.

### 2.2 Tier 2 — Anti-anti-objectives

| # | Metric | How measured | Trigger |
|---|---|---|---|
| 7 | `mean_session_length_minutes` | Server-side session timestamps; median, not mean (resistance to long outliers) | Alerts citizen at 5min; forces "take a break" at 10min |
| 8 | `sessions_per_user_per_day` | Authenticated session opens / day | Aggregate dashboard; SARB flag at p95 > 5 |
| 9 | `engagement_loop_addiction_score` | Stanford BEAR scale (Bergen Social Media Addiction Scale, modified for cooperative platforms), quarterly | Aggregate; rollback algorithm if score rises ≥ 0.5 between quarters |
| 10 | `status_score_dispersion` | Gini coefficient over any visible-to-others numeric in the codebase. **Target: exactly 0.** | CI check fails build if any new visible-to-others numeric is added without SARB amendment |
| 11 | `ad_revenue_per_user` | Total ad revenue / active citizens. **Target: exactly $0.** | Financial audit; non-zero triggers Trustee veto |
| 12 | `algorithmic_engagement_amplification` | Defined: `Corr(per-event score, P(citizen returns within 24h))` minus same correlation under random matching baseline. **Target: exactly 0** (algorithm does not differentially reward returning behavior). | Quarterly SARB audit |

### 2.3 Tier 3 — Knowledge & love

| # | Metric | How measured | Frequency |
|---|---|---|---|
| 13 | `books_completed_per_citizen_per_year` | Optional in-app attestation (private; only aggregate stats reported) | Annual rolling |
| 14 | `weekly_debate_or_symposium_attendance_rate` | Attendance / weeks-active among eligible citizens | Weekly aggregate |
| 15 | `serendipitous_romantic_match_rate` | 6-month follow-up self-attestation "I am partnered with someone I met at a gathering" | Quarterly |
| 16 | `cross-ideological-tie_rate` | New strong ties where M12 ideological cosine < 0.3 / all new strong ties | Quarterly aggregate |
| 17 | `cross-generational-tie_rate` | New strong ties where citizen-ages differ by ≥ 15 years / all new strong ties | Quarterly aggregate |

### 2.4 Tier 4 — Doctrinal invariants (binary)

Per `03`. Each I1-I18 is binary `pass/fail`. Failure of any is failure of the platform itself.

CI / audit pipeline:

- Source-code lint: forbidden symbol scan (`follower_count`, `engagement_score`, etc.).
- Schema audit: forbidden column scan.
- UI inventory audit: no infinite-scroll component, no public-profile-browse component.
- Aggregate financial audit: `ad_revenue == 0`, `third_party_data_sale == 0`.
- Sortitioned-review-board independent verification quarterly + signed report published.

## 3. Privacy architecture for measurement

### 3.1 Differential privacy on aggregates

Every public aggregate is computed with ε ≤ 1.0 DP and a minimum cell size of 30. Per-cell aggregates intended for SARB use ε ≤ 3.0 with stronger downstream-disclosure controls.

### 3.2 No per-citizen leaderboards anywhere

Per Invariant I17 + `08` §2: no per-citizen metric is ever visible to another citizen, to a journalist, or to a researcher (except the citizen's own data, available via `polis_citizen_audit_export`).

### 3.3 Survey administration

- UCLA-3, Steger MLQ, BEAR, and eudaimonia surveys are administered **in person** by sortitioned peer citizens (rotating role) on a private tablet, with the responder's responses entered directly into the secure surveying app — no platform-staff access.
- Sample-based, not census — a stratified sample of ~ 5-10% of citizens per polis per quarter is sufficient for ε-DP-aggregated statistics with the required power.

### 3.4 No A/B testing on citizens without consent

Per `07` §8: any experimental change to the matching algorithm that would differentially affect Tier-1 outcomes requires:

1. Pre-registered hypothesis with falsification criterion.
2. SARB approval.
3. Citizen-Assembly approval if Tier-1 affected.
4. Informed-consent opt-in from each participating citizen.

This is the standard for medical clinical trials. It is the correct standard for civic-infrastructure experimentation. Silicon Valley "growth A/B tests" are forbidden.

## 4. The falsification predicates — what would kill the project

The mark of a serious civic-engineering project is that it specifies, in advance, what would *falsify* it. Per Popper / per the FSV discipline.

| # | Falsification predicate | Consequence if true |
|---|---|---|
| **F1** | After 24 months in ≥ 3 polises, Tier-1 metrics #1, #2, #4 fail to move ≥ 50% of target | Halt new-polis launches; sortitioned root-cause analysis; possibly retire the platform |
| **F2** | A polis matches Tier-1 numerically but Tier-2 (#10/#11/#12) drifts non-zero | Architecture has failed; rollback to last clean state; commission audit |
| **F3** | A polis grows to scale but its `cross-ideological-tie_rate` (Tier-3 #16) is < 5% | Bridging-rerank not working; algorithm reweight; possible doctrinal review |
| **F4** | Independent academic study finds the platform produces *higher* loneliness than non-platform-using cohort in matched cities | Re-think the whole product (this is the strongest possible falsifier) |
| **F5** | A Tier-4 invariant is found to be silently violated (e.g., engineering audit finds a quietly-shipped follower-count feature) | Immediate rollback + sortitioned-review-board investigation + Trustee-class hearing |
| **F6** | The cooperative governance fails to constitute a SARB after 24 months | Civic-engagement model has failed; revisit governance design |
| **F7** | Average Eudaimonia delta is negative across 4+ polises after 36 months | The whole product theory may be wrong; major review |

These are not "stretch goals." They are explicit kill-switch criteria. The platform is built to accept being wrong if it is wrong.

## 5. The independent-research collaboration program

The platform actively *invites* external academic study (per `13` §4). Three pre-approved collaboration modes:

- **Public-aggregate replication:** any researcher can replicate our published Tier-1 aggregates from our DP-protected public datasets.
- **In-person opt-in cohort studies:** academic teams may, with each citizen's individual opt-in consent, conduct deeper longitudinal studies (mental health, civic-engagement, social-network analysis) on consenting subsamples. No platform involvement in the study design; we provide only the introductions to consenting citizens.
- **Code + algorithm audit:** the matching algorithm code is open-source; any researcher may run their own bias / fairness audit. Findings published independently.

This is the discipline of an open-source civic infrastructure that wants to be wrong if it is wrong, and right if it is right, on the strongest possible evidence.

## 6. The "movement of metrics" cadence

| Frequency | What is computed | Who sees it |
|---|---|---|
| Real-time | Per-polis attendance, RSVP, current matchmaking queue depth | Polis operations staff (anonymized) |
| Daily | Tier-2 #7, #8 aggregates (session length, sessions/day) | SARB dashboard |
| Weekly | Tier-1 #1, Tier-3 #14, Tier-2 #10 (audit) | SARB + polis director |
| Monthly | Tier-1 #2, #3 aggregates; Tier-3 #16, #17; algorithm-weight-change log | SARB |
| Quarterly | Tier-1 #4, #5, #6 (survey-based); Tier-3 #13, #15 (survey); Eudaimonia | Citizen Assembly + SARB + public |
| Annually | Full retrospective report per polis; ship-gate evaluation | Public (DP-aggregated) |

## 7. The "this is the goal" sentence

Per CLAUDE.md §0 discipline (one-line predicate):

```
SERENDIPITY_ENGINE_WORKS ⇔
  Tier-1 metrics #1–#6 each ≥ target
  STABLE × 12 consecutive months
  in ≥ 4 active polises
  AND Tier-2 metric #10 == 0, #11 == 0, #12 == 0
  AND every Tier-4 invariant I1–I18 holds
  AND no falsification predicate F1–F7 has fired in the last 24 months
```

When this evaluates `TRUE` → the platform has worked. Per `01` §6, the platform begins polis-network expansion under sortition-governed protocol. Nothing else is the goal.

## 8. Cross-references

| Topic | Doc |
|---|---|
| Tier-1/2/3/4 metric definitions (full) | `01_VISION_AND_NORTH_STAR.md` |
| Invariant list | `03_NON_NEGOTIABLE_INVARIANTS.md` |
| Algorithm & A/B-testing constraints | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §8 |
| Anti-gaming privacy mechanisms | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |
| Privacy architecture | `15_TECHNICAL_ARCHITECTURE.md` §5 |
| Failure modes if metrics drift | `17_FAILURE_MODES_AND_BLINDSPOTS.md` |
| Verification protocol pattern | `docs/futurebuild/specs/FSV-PROTOCOL.md` (project root) |

---

**Last updated:** 2026-05-24.
