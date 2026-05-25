# Non-Negotiable Invariants (Tier-4 Doctrine)

These are the fences. They are not goals. They cannot be moved. Reaching any Tier-1/2/3 number by violating a Tier-4 invariant means **the system has not worked; it has cheated.** Identical asymmetric structure to `docs/futurebuild/specs/TARGET-METRICS.md §4` for ContextGraph.

Authority: this file outranks every other doc in `docs/serendipityengine/` on doctrinal matters. A feature PR that violates any invariant below must be rejected at the doc-review stage, before any code is written.

## 1. The 18 invariants

| # | Invariant | Required value | Why |
|---|-----------|----------------|-----|
| **I1** | `feed_exists` | **false** | A feed is a scroll surface. A scroll surface is an addiction. We do not have one. |
| **I2** | `follower_count_visible_to_anyone` | **false** | Status seed. See `08`. |
| **I3** | `like_count_visible_to_anyone` | **false** | Status seed. |
| **I4** | `public_profile_browsable` | **false** | Browsable profiles → swipe culture, status comparison, performative identity. |
| **I5** | `ad_revenue_per_user_dollars` | **exactly 0** | See `13_BUSINESS_MODEL.md`. Ad-funding forces engagement maximization. |
| **I6** | `algorithmic_engagement_amplification` | **exactly 0** | The algorithm optimizes serendipity, not engagement. |
| **I7** | `status_score_visible_to_others` | **exactly 0** (no such number computed or transmitted) | Bauchaud, Jiang on meritocracy, Athenian ostracism precedent. |
| **I8** | `infinite_scroll_anywhere_in_ui` | **false** | Dark pattern. |
| **I9** | `time_on_app_optimized_for` | **false** | Inverted KPI: we want time *off* app spent on platform-caused real-world activity. |
| **I10** | `cross_user_dim_compare_in_centroids` | **false** | TCT slot-identity (analogous to ContextGraph `flat_vector_concat_used`). User centroids exist per-slot; cross-slot dim compare is forbidden. |
| **I11** | `event_with_zero_irl_component_is_marketed` | **false** | Every primary engine output is an in-person calendar invite. Online-only formats exist only as preparation for IRL (`06`). |
| **I12** | `centralized_owner_can_unilaterally_change_invariants` | **false** | Governance under `14_GOVERNANCE_AND_SORTITION.md` requires citizen-assembly supermajority + sortitioned algorithmic-review-board approval. |
| **I13** | `user_data_sold_to_third_party` | **false** | Including aggregated/anonymized. |
| **I14** | `behavioral_data_used_to_target_advertising_off_platform` | **false** | We don't have advertising on-platform either; this closes the loophole. |
| **I15** | `inner_LLM_generates_messages_on_behalf_of_user` | **false** | No AI-ghostwritten DMs, no AI-paraphrased intros. Authenticity is non-negotiable. (Compare ContextGraph `feature = "llm"` compile guard.) |
| **I16** | `status_or_money_can_buy_priority_in_matching` | **false** | No pay-to-play, no "Premium" tier that gets better matches. (Optional convenience tier described in `13` may exist but must not change matching outcomes.) |
| **I17** | `match_explanations_published_per_user_publicly` | **false** | Why X was matched with Y is between X and Y. No "X gets matched 30 times a month, Y gets matched 3 times" leaderboards or rankings exposed anywhere. |
| **I18** | `every_algorithmic_weight_change_requires_sortitioned_review_log` | **true** | Per `14`. Algorithm transparency in mechanism; no silent reweights. |

## 2. Why these specific 18

Each maps to a documented failure mode of a prior platform:

- **I1, I8** → Instagram, TikTok, X. The feed is the addiction surface.
- **I2, I3, I4, I7, I16, I17** → all attempts (Snapchat in 2014, BeReal in 2022) to remove status leaks were partial and reversible. Only architectural absence works.
- **I5, I14** → Facebook, Google, every ad-funded social. Ads force the engagement loop.
- **I6, I9** → Twitter algorithmic timeline 2016+. Optimizing engagement turned a useful network into a rage machine (Ranking-for-Engagement working paper, ifo Institute).
- **I10** → ContextGraph doctrine: slot-identity preservation. Cross-slot dim-compare is mathematically meaningless and produces spurious matches.
- **I11** → Lunchclub failure mode (video calls displaced in-person; product died).
- **I12** → WeWork acquires Meetup → Bending Spoons acquires Meetup → product enshittification. Centralized ownership without sortitioned governance = capture.
- **I13** → Every data broker.
- **I15** → AI-mediated communication kills the signal that the introduction is between two human beings.
- **I18** → "Move fast and break things" applied to civic infrastructure breaks the civic infrastructure.

## 3. How invariants are enforced

Three layers of enforcement:

### 3.1 Architectural (compile-time / type-system / schema)

Wherever possible, the invariant is made impossible to violate by the type system or DB schema:

- No table exists named `user_follower_counts` or `user_likes_received`. A migration that adds one is rejected at PR review.
- The matching service has no API surface that accepts a "promote this user" parameter.
- The UI layer has no infinite-scroll component in the design system. (Compare ContextGraph's `compile_error!` guard for `feature = "llm"`.)

### 3.2 Procedural (CI / sortitioned review)

Before any PR merges:

- Lint pass scans for forbidden symbols (`follower_count`, `like_count`, `engagement_score`, `advertising_*`).
- An automated audit script (`scripts/ci/invariant_check.sh`) verifies each I1–I18 against the current codebase + database schema + UI inventory.
- Quarterly sortitioned algorithmic-review-board (`14 §3.2`) audits the running system for invariant drift and publishes a signed report.

### 3.3 Constitutional (governance)

The 18 invariants are written into the cooperative's articles of incorporation. Changing any of them requires:

1. A formal amendment proposal published 90 days in advance,
2. Approval by the sortitioned algorithmic-review-board (50-citizen panel, drawn by lot, rotating),
3. Approval by a city-wide citizen assembly (1,000-citizen sortitioned panel) in **every** active city,
4. A 12-month moratorium during which the amendment is debated in Agora-format sessions in every active city,
5. A final affirmative re-vote.

This makes invariant change theoretically possible (so the system is not literally constitutional) but practically very hard, which is the correct trade-off.

## 4. The "future-founder asks for an exception" protocol

When a future operator says "we just need a tiny follower count to drive growth," the answer is:

1. Cite this doc.
2. Open the amendment process. Wait 12 months. Defend the case in Agora.
3. If the proposal survives that, run it.

Most proposals will not survive 12 months of public debate. That is the design.

## 5. Cross-references

| Topic | Doc |
|---|---|
| Status-forbiddance technical detail | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |
| Governance mechanism | `14_GOVERNANCE_AND_SORTITION.md` |
| Business model (closes I5/I14) | `13_BUSINESS_MODEL.md` |
| Metrics that respect invariants | `16_METRICS_AND_FALSIFICATION.md` |
| Failure modes if invariants drift | `17_FAILURE_MODES_AND_BLINDSPOTS.md` |

---

**Last updated:** 2026-05-24. Any change requires the §3.3 amendment process.
