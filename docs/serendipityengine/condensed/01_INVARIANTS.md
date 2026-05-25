# Tier-4 Invariants — Hard Fences (CI-checked, immovable)

Authority: outranks every other doc in `condensed/` on doctrinal matters. PR violating any of I1–I22 must be rejected at review *before* code is written. Amendment process: §3.

## 1. The 22 invariants

| # | Predicate | Required | Enforced where |
|---|---|---|---|
| **I1** | `feed_exists` | `false` | UI inventory + lint forbidden symbols (`feed`, `timeline`, `for_you`) |
| **I2** | `follower_count_visible_to_anyone` | `false` | DB schema scan; UI inventory |
| **I3** | `like_count_visible_to_anyone` | `false` | Same |
| **I4** | `public_profile_browsable` | `false` | UI inventory; route lint |
| **I5** | `ad_revenue_per_user_dollars` | `0` | Financial audit |
| **I6** | `algorithmic_engagement_amplification` | `0` | SARB quarterly audit (`Corr(score, P(return 24h)) − random_baseline = 0`) |
| **I7** | `status_score_visible_to_others` | `0` (no such number computed/transmitted) | Code scan: forbidden symbols (`reputation`, `karma`, `score`, `rank`, `level`, `badge`, `tier_*` UNLESS subscription tier) |
| **I8** | `infinite_scroll_anywhere_in_ui` | `false` | UI inventory; no `IntersectionObserver` paginated feed component |
| **I9** | `time_on_app_optimized_for` | `false` | Inverted KPI: session length capped 5min soft / 10min hard; medianized; alert at p95 > 8min |
| **I10** | `cross_slot_dim_compare_in_centroids` | `false` | Type system: `Constellation` is `Vec<(Slot, Embedding)>`; no `.flatten()` method exposed; lint scans for forbidden ops |
| **I11** | `event_with_zero_IRL_component_marketed_as_primary` | `false` | Every primary engine output is a calendar invite with venue + time |
| **I12** | `centralized_owner_can_unilaterally_change_tier4` | `false` | Cooperative articles (`06` §4); amendment process below |
| **I13** | `user_data_sold_to_third_party` | `false` (including aggregated/anonymized) | No outbound data API to third parties |
| **I14** | `behavioral_data_targets_off_platform_ads` | `false` | No outbound tracking pixel; no analytics SDK |
| **I15** | `inner_LLM_generates_messages_on_behalf_of_user` | `false` | Compile-guard analogous to ContextGraph `compile_error!` on `feature = "llm"`; no LLM-mediated DM endpoint |
| **I16** | `status_or_money_can_buy_priority_in_matching` | `false` | Matcher accepts no priority parameter; subscription tier is access-gate-only |
| **I17** | `match_explanation_published_per_user_publicly` | `false` | Per-citizen predictions never persisted in cross-citizen-queryable form |
| **I18** | `algorithm_weight_change_lacks_sortitioned_review_log` | `false` (every change requires SARB log) | All weight writes go through `polis-governance::sarb::approve_weight_change` |
| **I19** | `new_embedder_slot_added_without_distinctness_gate` | `false` | Inter-embedder distinctness gate (`03` §5): pairwise correlation ≤ 0.6 on calibration set, else reject slot |
| **I20** | `release_ships_without_FSV_evidence` | `false` | CI: every release requires SoT readback JSON + audit-log row |
| **I21** | `release_ships_without_14_axis_hardening_pass` | `false` | CI: per-PR hardening checklist (`docs2/hardeningprompt.md`) |
| **I22** | `data_breach_response_>_72h` | `false` | Incident-response runbook; auto-page; signed disclosure within 72h per GDPR Art 33 |

## 2. Enforcement (3 layers)

| Layer | Mechanism | Cadence |
|---|---|---|
| **Architectural** | Compile guards, type system, schema migration review | Every PR |
| **Procedural** | `scripts/ci/invariant_check.sh` (auto), sortitioned SARB audit | Per-PR / Quarterly |
| **Constitutional** | Cooperative articles + asset-lock + trust (`06` §4) | Permanent |

CI script `scripts/ci/invariant_check.sh` MUST verify all of:
- I1, I2, I3, I4, I7, I8 → forbidden symbol scan (allow-list of legitimate occurrences in this file only)
- I10 → no `.flatten()` or analogous concat on `Constellation` type
- I15 → `feature = "llm"` rejected at workspace build
- I19 → distinctness-gate runs and passes for current embedder slate
- I20, I21 → release tag refuses if FSV evidence missing

## 3. Amendment process (any Tier-4 change)

```
amend(I_n) ⇒
  publish_proposal(I_n, reasoning, 90d_lead)
  ∧ SARB.approve(I_n) with ≥60% supermajority of 50-citizen panel
  ∧ ∀ polis ∈ active_polises : CitizenAssembly(polis).approve(I_n) ≥60%
  ∧ moratorium(I_n, 12 months) with Agora debate in every polis
  ∧ revote_after_moratorium with same thresholds
  ∧ ¬TrusteeCouncil.veto(I_n)  (or 75% Citizen-Assembly override of veto)
```

Designed to be *theoretically* possible but practically very hard (≥ 12 months end-to-end).

## 4. Path discipline (behavioral; in-line with CLAUDE.md §7)

Never edit without operator confirmation:
- `.env`, `.env.*`
- `*.pem`, `*.key`, `**/credentials.json`
- Anything under `swebench/**`, `tests/oracle_**/**`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`
- The cooperative articles of incorporation in `governance/articles/*`

## 5. Forbidden git operations

`push --force`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase` on shared branches, `--no-verify`, `--no-gpg-sign`.

## 6. Codex-specific addenda

Codex must:
- Never disable an invariant check to make a test pass.
- Never paraphrase an invariant in code comments (paraphrase loses meaning per `compressionprompt.md` §6.1).
- Never add a "TODO: enforce later" placeholder for an invariant. Either implement or block on the issue.
- File a GitHub issue immediately on observing any drift toward invariant violation, even if not the active task.

---

**Last updated:** 2026-05-24. Changes require §3 amendment process.
