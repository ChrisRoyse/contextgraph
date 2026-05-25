# Failure Modes and Blindspots

Per CLAUDE.md §11 (forensic discipline) and the standard "ALL CODE IS GUILTY UNTIL PROVEN INNOCENT" stance. This is the project's red-team document. It enumerates the failure modes the system **will** encounter, the blindspots in the design, and the explicit countermeasures (or honest acknowledgments where the countermeasure is incomplete).

Jiang's lectures `NpNCQ_gNqdE` (decline), `JU_8fJjtGxA` (collapse), `K-_l9jBGo74` (death of civilization), `ajFXykT9Joo` (evil and power), and `ra2v9HkaHz8` (modernism) supply the historical case studies. Every great civilization eventually collapsed; we encode the most common collapse patterns and defend against them explicitly.

## 1. The catalog (FM-01 through FM-25)

Each failure mode: **what**, **how it manifests**, **leading indicator**, **countermeasure**, **residual risk**.

### Category A — Algorithmic / matcher failures

#### FM-01: Filter-bubble matching

- **What:** the matcher converges on tight in-cluster matches, producing high satisfaction but zero cross-cluster ties.
- **Manifests as:** Tier-1 #1-3 strong; Tier-3 #16 (cross-ideological) < 10%; M12 entropy across new ties drops.
- **Leading indicator:** rising clustering coefficient in the per-polis tie graph quarter-over-quarter.
- **Countermeasure:** MMR diversification (λ ≈ 0.6, `07` §5.2); bridging rerank (`07` §5.3); 15-25% sortitioned shuffle (`07` §5.4); Aleatory format for Newcomers (`06` §7); the ProjectedHomogeneity penalty (ε in `07` §2).
- **Residual risk:** moderate. The system has multiple defenses but the failure pattern is the gravitational pull of every recommendation system. Continuous SARB audit required.

#### FM-02: Algorithmic monoculture across embedders

- **What:** the 13 embedders' rankings collapse to near-identity (the inter-embedder distinctness ContextGraph #709 is designed to prevent breaks down on the citizen-domain data).
- **Manifests as:** per-modality cosine of independent embedders rises above 0.85 systematically.
- **Leading indicator:** inter-embedder distinctness gate (ported from ContextGraph #715) trips.
- **Countermeasure:** continuous distinctness audit; if breached, retire affected slot until rebuilt; algorithm degrades gracefully to N-1 slots (per `05` §7 #3).
- **Residual risk:** low.

#### FM-03: Goodhart on a Tier-1 metric

- **What:** the algorithm finds a hack that lifts a Tier-1 number without producing the underlying outcome — e.g., gameplaying the 30-day attestation by sending reminder nudges that elicit positive responses.
- **Manifests as:** Tier-1 #2 (strong ties) lifts but Tier-1 #4 (loneliness) does not; or Tier-3 #16 lifts but cross-ideological friendships don't last 6 months.
- **Leading indicator:** divergence between attestation-based metrics and survey-based metrics quarter-over-quarter.
- **Countermeasure:** the Tier-1 #4-6 survey metrics are *peer-administered, in-person, quarterly* (`16` §3.3) — much harder to Goodhart than self-attestations. Per-metric SARB audit on divergence.
- **Residual risk:** moderate. Goodhart's law is the recommendation system's terminal disease.

#### FM-04: Algorithm produces sterile monoculture of well-matched friends

- **What:** the matcher succeeds at Tier-1 metrics 1-3 but the matched ties themselves form a closed sub-culture; no exposure to people unlike the citizen.
- **Manifests as:** Tier-3 #16 + #17 (cross-ideological / cross-generational) flat or declining over time.
- **Leading indicator:** Aleatory-format reflection ratings rise over time (citizens prefer aleatory over algorithmic — a signal the algorithm is too narrow).
- **Countermeasure:** Aleatory format (`06` §7); bridging rerank weight (β, ε in `07` §2) increases automatically when #16/#17 lag; quarterly SARB-commissioned monoculture audit.
- **Residual risk:** moderate.

#### FM-05: Cold-start failure for newcomers

- **What:** new citizens get poor matches in their first 30 days, churn out before forming any tie.
- **Manifests as:** Newcomer 90-day retention < 50%; first-event reflection score < 3.0.
- **Leading indicator:** Newcomer cohort attendance trajectory drops below veteran cohort at week 3.
- **Countermeasure:** doubled fairness-floor (`07` §7) for Newcomers; mandatory Welcoming-Citizen intake (`04` §3.2); first-format limited to lower-stakes Symposium / Gymnasium / Theater (`04` §3.3); 30-day check-in outreach (`04` §3.4).
- **Residual risk:** moderate. Cold-start is the killer for every social product.

### Category B — Status / gaming failures

#### FM-06: Convener empire

- **What:** a charismatic Established+ citizen accumulates de-facto status by being a frequent convener; extracts side benefits.
- **Manifests as:** one citizen convening > 20% of polis Symposia; attendee complaints of preferential treatment; off-platform gossip.
- **Leading indicator:** convener-frequency Gini > 0.6 in a polis.
- **Countermeasure:** per-citizen convening caps (`08` §4); convener-feedback sortitioned audit; rotation requirements; service-credit-only compensation.
- **Residual risk:** low (architectural) but the *cultural* risk of convener-celebrity remains.

#### FM-07: Off-platform status game emerges

- **What:** citizens flex on Twitter/Instagram about platform attendance (which Festival they got into, which Agora they spoke at), creating an external status hierarchy.
- **Manifests as:** social-media chatter; perception of "in-group" citizens.
- **Leading indicator:** mainstream press coverage that profiles individual "elite" citizens.
- **Countermeasure:** no shareable artifacts (`08` §8); cultural seeding via Citizenship Reading (`04` §3.5); aesthetic understatement of brand; long-arc social norms.
- **Residual risk:** high. We cannot prevent off-platform behavior; we can only refuse to fuel it.

#### FM-08: Sybil attack on governance

- **What:** an outside actor creates many fake personhood credentials to bias sortition draws or stuff Agora panels.
- **Manifests as:** anomalous demographic distribution in sortition; statistically unlikely position alignment in Agora.
- **Leading indicator:** spike in PoP credentials from a single attestation source or KYC provider.
- **Countermeasure:** in-person attestation preferred (`08` §3); Worldcoin-tier biometric backup; cross-attestation network anomaly detection.
- **Residual risk:** moderate as biometric PoP matures; high if we rely solely on KYC.

#### FM-09: Cooperative governance capture

- **What:** a faction (political, religious, ideological) wins enough SARB or Citizen Assembly seats via biased turnout or hidden coordination, attempts to bend doctrine.
- **Manifests as:** voting blocks aligned on Tier-4-adjacent issues; targeted recruitment of citizens to attend Agora on a wedge issue.
- **Leading indicator:** abnormal turnout pattern in Citizen Assembly; sortition-decline rate spikes among one demographic.
- **Countermeasure:** stratified sortition (`14` §3); 12-month moratorium on Tier-4 amendments (`03` §3.3 + `14` §6); Trustee-class veto on Tier-4 violations.
- **Residual risk:** moderate. Mitigations are robust but no governance system is fully capture-proof.

### Category C — Business / financial failures

#### FM-10: Civic subscription churn

- **What:** subscription revenue falls below operating cost; civic fund drains; layoffs; service degradation; doom loop.
- **Manifests as:** subscription churn > 30%/year sustained.
- **Leading indicator:** Tier-1 #1 dropping in cohort prior to subscription renewal.
- **Countermeasure:** subscription pricing tied to demonstrable value (Tier-1 #1 in citizen's polis); Solidarity tier for affordability; civic fund as backstop; honest acknowledgment in `13` that revenue follows mission success.
- **Residual risk:** moderate.

#### FM-11: Venue-partner ecosystem collapse

- **What:** Partner Venues exit en masse (economic downturn, chain consolidation, post-COVID-style restaurant attrition).
- **Manifests as:** ratio of citizens to active Partner Venues > 2,000:1 in a polis (the threshold below which gatherings become hard to host).
- **Leading indicator:** quarterly Partner Venue retention < 90%.
- **Countermeasure:** Civic Venue Program (`12` §4); catalyst grants (`12` §5); cooperative-owned third-place pilots; venue-partner cooperative governance to keep partnerships aligned.
- **Residual risk:** moderate; depends heavily on macroeconomic conditions for small-business retail.

#### FM-12: Acquisition pressure

- **What:** a corporate acquirer offers a large sum; founder, board, or worker-members are tempted.
- **Manifests as:** acquisition inquiries; press leaks; pressure inside the cooperative.
- **Leading indicator:** unsolicited M&A advisor contact.
- **Countermeasure:** asset-lock + cooperative articles + Trustee-class trust + Tier-4 invariant entrenchment (`14` §4); founder pre-commitment trust agreement on day 1 (`14` §9).
- **Residual risk:** low (legally) but cultural / morale risk remains.

### Category D — Mission / civic-outcome failures

#### FM-13: Platform produces shallow but not deep ties

- **What:** weak-tie metric (Tier-1 #3) hits but strong-tie metric (#2) and loneliness metric (#4) don't move.
- **Manifests as:** people attend events but report no closer friendships, no reduced loneliness.
- **Leading indicator:** divergence between #3 (attestation-based) and #2 (survey-based).
- **Countermeasure:** Chevruta layer (`09` §4.2); Symposium repeat-cohort design; long-arc patience (deep tie formation is a 12-24 month process).
- **Residual risk:** moderate; this is the deepest hypothesis under test.

#### FM-14: Algorithmic monoculture by city

- **What:** the platform succeeds in one type of city (urban, dense, educated, secular) but fails in others (suburban, sparse, religious, working-class).
- **Manifests as:** Tier-1 #1 spread across polises far above 1.5x; some polises chronically lag.
- **Leading indicator:** post-launch first-6-month Tier-1 trajectory diverges by polis demographic.
- **Countermeasure:** per-polis algorithmic calibration; explicit attention to non-urban / non-secular / non-educated polis launches as Year 2-3 priorities; sortitioned local adaptation of formats (e.g., Symposium = "potluck dinner" in some regions; Agora = "town hall" in others).
- **Residual risk:** moderate; documented in the roadmap (`18`).

#### FM-15: The platform becomes a "cult of itself"

- **What:** citizens spend more time talking about the platform than actually living the platform's intended outcomes; the platform becomes a totalizing identity.
- **Manifests as:** community memes, jargon, in-group identity markers; citizens framing their entire social life through the platform.
- **Leading indicator:** social-media chatter density about the platform > some threshold; Eudaimonia question 4 (civic value to non-platform neighbors) lags.
- **Countermeasure:** deliberate aesthetic understatement; explicit doctrine against in-group identity; the Aleatory format's exposure to outside-cluster citizens; cultural seeding in Citizenship Reading that the platform is a *means* to flourishing, not flourishing itself.
- **Residual risk:** moderate.

### Category E — Privacy / security failures

#### FM-16: Data breach exposing Citizen Constellations

- **What:** a breach exposes per-citizen Constellations.
- **Manifests as:** intrusion detection alert; press disclosure; citizen complaints.
- **Leading indicator:** anomalous data-access patterns in audit log.
- **Countermeasure:** per-citizen encrypted-at-rest with PoP-derived keys (`15` §5.1); ephemeral key access in matching service; HSM key management; two-person-rule for privileged data access; annual independent penetration testing.
- **Residual risk:** moderate. No security system is perfect; the architecture is designed so a breach exposes ciphertext, not plaintext.

#### FM-17: Inadvertent de-anonymization via aggregates

- **What:** a clever attacker re-identifies individual citizens from "anonymized" aggregate releases.
- **Manifests as:** academic paper or journalist exposé that names individuals from aggregate data.
- **Leading indicator:** queries to the public-dashboard API with unusual cell-size targeting.
- **Countermeasure:** ε-DP on all aggregates with minimum cell size of 30 (`16` §3.1); SARB review of any released dataset; explicit re-identification adversary analysis on schema changes.
- **Residual risk:** moderate.

#### FM-18: Government surveillance subpoena pressure

- **What:** state actor pressures the platform for citizen social-graph data, dissident identification, attendance records at controversial Agora.
- **Manifests as:** subpoenas; National Security Letters in the U.S.; equivalent in other jurisdictions; pressure from local police on Partner Venues.
- **Leading indicator:** legal counsel receives requests with broad scope.
- **Countermeasure:** minimal-data-retention policy (`08` §7); per-citizen encryption (no platform-side decryption of Constellation possible); public transparency report (number of requests received, complied with, rejected); legal pre-commitment to fight overbroad requests; pre-staged jurisdictional flexibility (cooperative incorporated in privacy-favorable jurisdiction).
- **Residual risk:** moderate. Cannot fully resist a determined state actor; minimum-data + ciphertext-only architecture limits damage.

### Category F — Cultural / civilizational failures

#### FM-19: Off-platform behavioral norms break (harassment, predation)

- **What:** a citizen behaves badly at a gathering — harassment, predation, violence.
- **Manifests as:** citizen reports; venue staff reports; in extreme cases police involvement.
- **Leading indicator:** convener flags during gathering; reports in 24h reflection.
- **Countermeasure:** Venue Code of Conduct (`12` §6); convener authority to remove attendee; sortitioned-review-board hearing; tiered penalty (first violation = 90-day pause; second = lifetime ban); explicit cooperation with law enforcement on criminal conduct.
- **Residual risk:** moderate; no system can fully prevent individual bad behavior.

#### FM-20: Cross-cultural failure in international expansion

- **What:** the Greek/Western intellectual framework (Plato, Dante, Aristotle, Putnam, Granovetter) does not translate cleanly to non-Western polises.
- **Manifests as:** Tier-1 #1 in (say) a Tokyo or São Paulo polis lags despite similar engineering effort.
- **Leading indicator:** intake-interview qualitative feedback; format-attendance variance across format types.
- **Countermeasure:** local sortitioned curatorial council (`09` §3.2) adapts the Core Canon; format definitions are robust under local interpretation; local Citizen Assembly approves local adaptations; cross-cultural advisory in Year-2+ expansion (`18`).
- **Residual risk:** high. The deepest unknown in the design.

#### FM-21: Modernity-resistance failure

- **What:** the platform requires more deliberate, slow, in-person engagement than modern citizens are willing to give; we cannot bootstrap initial attendance.
- **Manifests as:** Phase-0 launches stall; conversion rate from "join" to "attended-1-event" < 50%.
- **Leading indicator:** Phase-0 sample-cohort attendance < 30% within first 14 days.
- **Countermeasure:** deliberate slow-and-deep onboarding accepted as design (`04`); Phase-0 launches partner with existing high-engagement local communities (e.g., run clubs, library reading groups) to seed; honest acknowledgment that the platform is not for every citizen.
- **Residual risk:** moderate. The platform's price of admission is real.

### Category G — Trajectory failures

#### FM-22: The Jiang collapse pattern: elite overproduction

- **What:** as the cooperative matures, a layer of professional cooperative staff becomes a class apart from ordinary citizens; gradually accumulates power; sortition becomes ceremonial.
- **Manifests as:** staff turnover declines; staff salaries drift up; SARB recommendations are softened in practice.
- **Leading indicator:** ratio of staff-to-sortitioned-decisions trends upward over years.
- **Countermeasure:** mandatory cooperative wage ratio (max 4:1 top-to-bottom per cooperative bylaws); staff rotation through sortitioned roles; SARB-commissioned annual staff-power audit; founder Trustee class explicitly named not to be staff.
- **Residual risk:** moderate over decade-plus time horizon. Jiang's collapse pattern is the hardest to resist over very long arcs.

#### FM-23: Mission drift via incremental compromise

- **What:** in response to growth pressures, the cooperative quietly adds features (e.g., a "convenience" notification system that turns into a feed; a "premium discovery" tier that turns into pay-to-play) that incrementally violate doctrine.
- **Manifests as:** Tier-2 #7 (session length) rising over years; Tier-4 invariant audits requiring "interpretation."
- **Leading indicator:** doctrine-amendment proposals more frequent year-over-year.
- **Countermeasure:** Tier-4 amendment process bar (`03` §3.3); quarterly invariant audit by SARB; published mission-drift annual report.
- **Residual risk:** moderate. Eternal vigilance is the price.

#### FM-24: Cooperative-founder fork

- **What:** the founder, or a successor, fundamentally disagrees with the cooperative's direction; tries to fork the open-source code and run a competing platform with weakened doctrine.
- **Manifests as:** code fork; competing platform announcement.
- **Leading indicator:** intra-cooperative governance disputes escalate.
- **Countermeasure:** open-source license with CommonsClause-equivalent supplementary terms preventing proprietary re-launch against Tier-4 invariants (`15` §8); brand and citizen-network effects favor the cooperative; cultural cohesion via shared paideia.
- **Residual risk:** moderate.

#### FM-25: AI-substitution collapse

- **What:** as LLMs / AI agents become ubiquitous, citizens substitute AI conversation for human conversation; the platform's value proposition (in-person human contact) is undermined by sloth.
- **Manifests as:** Tier-1 #2, #5 (strong-ties, purpose) decline even as platform usage holds.
- **Leading indicator:** quarterly survey question "how often do you have meaningful conversation with an AI?" rises across cohorts.
- **Countermeasure:** Invariant I15 (no platform-internal LLM mediation); explicit cultural seeding via Theater readings on Marcus Aurelius, Pascal, Wendell Berry on attention and presence; in-person ritual emphasis; longer-arc civilizational case.
- **Residual risk:** high. This is a civilization-scale headwind, not just a platform-scale one.

## 2. The honest meta-failure

If 30%+ of these failure modes materialize simultaneously, the platform has failed. We say so explicitly. We will close it down, publish the post-mortem, contribute the substrate to other civic-purpose projects, and ensure that the next iteration learns. (Per Popper's discipline + the FSV protocol.)

The platform is not so important that it should be propped up against the evidence. It is a *bet* on a particular theory of how civilization can be rebuilt. If the theory is wrong, the evidence will say so, and we will obey the evidence.

## 3. Cross-references

| Topic | Doc |
|---|---|
| Falsification predicates that fire on these failures | `16_METRICS_AND_FALSIFICATION.md` §4 |
| Tier-4 invariants under threat | `03_NON_NEGOTIABLE_INVARIANTS.md` |
| Anti-gaming threat model | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` §1 |
| Algorithm adversarial corpus | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §9 |
| Roadmap with phase-gated kill-switches | `18_ROADMAP_FROM_ZERO_TO_POLIS.md` |
| Jiang's collapse patterns | `20_JIANG_DOCTRINE_INTEGRATION.md` |

---

**Last updated:** 2026-05-24.
