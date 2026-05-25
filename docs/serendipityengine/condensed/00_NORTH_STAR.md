# Polis — North Star (RE-READ EVERY COMPACTION)

> **The codename for the system being built is `Polis`.** Build is DONE when `08_DONE_PREDICATE.md` evaluates `TRUE` on every line. At that moment: all software shipped, all assets generated, single-polis pilot live in prod, first 100 invited citizens can onboard end-to-end.

## 1. Read order on every compaction

1. `00_NORTH_STAR.md` ← you are here
2. `01_INVARIANTS.md` — guardrails (Tier-4 fences, mechanical CI check)
3. `08_DONE_PREDICATE.md` — the stopping point (mechanical)
4. `09_BUILD_PROTOCOL.md` — how to ship (codex protocol + asset gen + FSV)
5. Load others as needed: `02` glossary, `03` substrate, `04` formats+lifecycle, `05` algorithm, `06` gov+business, `07` tech+assets, `10` kill-switches+sources

## 2. What Polis is

Anti-engagement, IRL-first, civic-cooperative platform.
Five Greek-inherited gathering formats (`04`): Symposium, Agora, Gymnasium, Theater, Festival (+ Aleatory Newcomer).
Multi-embedder Citizen Constellation matcher (`03`, `05`) with frozen-target TCT guard.
Sortitioned cooperative governance (`06`).
Generates ≥ 1.5 serendipitous in-person events / citizen / week. Produces real-world friendship, romantic love, wisdom, civic deliberation. Architecturally forbids status-games, ads, feeds, engagement-amplification.

## 3. Codex role

**Codex is the build agent.** Codex:

- Implements every Rust crate, every MCP tool, every UI screen, every deployment script.
- **Generates every visual asset** via the image-generation tools available to codex: logo, brand palette, typography spec, UI illustrations, icon set, marketing landing page, in-product diagrams, festival posters, the wooden-token-3D-render for Agora rendered as SVG. Codex never blocks on a human designer.
- Runs FSV (`docs/futurebuild/specs/FSV-PROTOCOL.md` pattern) on every primitive; reads source-of-truth back.
- Owns the build until `08_DONE_PREDICATE.md` passes. Civic-outcome metrics (post-launch Tier-1/2/3) are *not* codex's responsibility — those are measured after live launch.

**Codex must not** add features beyond spec, add status surfaces (see `01`), add engagement-maximizing UI patterns, add advertising hooks, add data-broker APIs, ship without FSV evidence.

## 4. What this is NOT (forbidden surfaces — full list in `01`)

| Forbidden | Invariant |
|---|---|
| Feed | I1 |
| Follower / friend / connection count visible to anyone | I2 |
| Likes / reactions visible to anyone | I3 |
| Public browsable profile | I4 |
| Advertising revenue | I5 |
| Engagement amplification | I6 |
| Status score (any visible-to-others numeric ranking) | I7 |
| Infinite scroll | I8 |
| Time-on-app optimization | I9 |
| Cross-slot dim-compare of Constellation centroids | I10 |
| Zero-IRL-component event marketed as primary product | I11 |
| Unilateral owner change of Tier-4 | I12 |
| Third-party data sale | I13 |
| Off-platform behavioral ad targeting | I14 |
| Inner-LLM-generated messages on behalf of user | I15 |
| Pay-to-play matching priority | I16 |
| Per-user match-explanation published | I17 |
| Silent algorithm weight changes | I18 |

## 5. Doctrinal substrate

`Polis` reuses the engineering doctrine of `CLAUDE.md` + `docs2/AICodingAgentSuperPrompt.md` + `docs2/hardeningprompt.md` (FSV, simplification ≤500 lines/≤30 lines/cyclomatic ≤10, audit-log, fail-closed). All universal rules apply identically. Project-specific delta is in this `condensed/` directory.

## 6. The build-done sentence (mechanical)

```
BUILD_DONE ⇔
  ∀ check ∈ 08_DONE_PREDICATE.md : check.passes
  ∧ ∀ invariant ∈ 01_INVARIANTS.md : invariant.audited_pass
  ∧ ∀ axis ∈ docs2/hardeningprompt.md (14 axes) : axis.checklist_clean
  ∧ pilot_polis.deployment.state == "live, ready, can onboard 100 citizens"
```

## 7. The civic-success sentence (POST-build; not codex's gate)

```
POLIS_WORKS ⇔  (measured after launch by ops team, not codex)
  serendipitous_events_per_citizen_per_week ≥ 1.5 (median, polis A)
  ∧ qualified_new_strong_ties_per_citizen_per_year ≥ 3
  ∧ UCLA-3 loneliness delta ≤ −1.0
  ∧ Steger MLQ purpose delta ≥ +0.5
  ∧ eudaimonia index delta ≥ +1.0
  STABLE × 12 consecutive months
  AND every I1–I18 holds
  AND no F1–F7 kill-switch (`10`) has fired
```

Codex's job is the first sentence. The cooperative's job, after launch, is the second.

## 8. Universal doctrine deltas (project-specific)

| Where doctrine and project disagree | Use |
|---|---|
| Protocol detail (FSV, claim discipline, RCA) | Doctrine wins (`docs2/AICodingAgentSuperPrompt.md`) |
| Tier-4 invariants, embedder slate, format spec | This `condensed/` directory wins |
| File-length, function-length, simplification | Doctrine universal limits (≤500/≤30/cyc≤10) |
| Memory persistence across sessions | GitHub Issues (doctrine §3); no `./memory/*.md` |
| Build/test/lint commands | Project (`07_TECH_AND_ASSETS.md`) |

---

**Last updated:** 2026-05-24. Update only on doctrinal change.
