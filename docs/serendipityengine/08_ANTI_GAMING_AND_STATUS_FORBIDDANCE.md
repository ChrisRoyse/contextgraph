# Anti-Gaming and the Architectural Forbiddance of Status

This document specifies the technical mechanisms that make it **impossible by construction** for any citizen to game the platform to gain status. It is the operational counterpart of Invariants I2, I3, I4, I7, I16, I17 from `03`. The goal is not "deter gaming"; it is "make gaming numerically meaningless."

Background: Jiang's central diagnosis in `4ehovUNrSrw`, `WoTOxE4DTFA`, `NpNCQ_gNqdE`, and `ra2v9HkaHz8` is that every great civilization eventually collapses because elites accumulate status, status concentrates, the bureaucracy and meritocracy that defended the polis becomes the polis's predator. We architect for this. The system must remain non-status-concentrating *forever*, not just "for the first year before VC investors arrive."

## 1. The threat model

| Threat | Description |
|--------|-------------|
| **T1: Status-seeker citizen** | A citizen optimizes their behavior to maximize visible rank, follower-equivalent, or social cachet on-platform. (Modern equivalent: LinkedIn power-user, Twitter influencer.) |
| **T2: Convener empire** | A particular Elder accumulates disproportionate convening power, becomes a gatekeeper, extracts side-deals (money, sex, votes). |
| **T3: Algorithmic cabal** | The engineering / governance staff secretly tunes weights to favor a faction, a city, a demographic. |
| **T4: Acquiring entity** | A future corporate buyer (Bending Spoons / Meta / private equity) acquires the platform and quietly adds growth-hack features. |
| **T5: Sybil attack** | A citizen or external actor creates many personas to manipulate matching, debates, or governance. |
| **T6: State capture** | A government / political party tries to use the platform's social graph for surveillance, recruitment, or coercion. |
| **T7: Status leak via DM / event** | A citizen publishes their "match list" on Twitter as a flex; an underground status game emerges off-platform. |
| **T8: Demographic ranking** | A user / journalist scrapes citizens' formats and infers a ranking by inverse approach. |
| **T9: Cooperative governance capture** | A faction wins enough sortition seats to undo Tier-4 invariants. |

Each is addressed below.

## 2. T1 Defense: there is no number to chase

### 2.1 Architectural absence of public metrics

**No number is ever displayed to any other citizen or to the citizen themselves about their own social standing.** Concretely:

- No follower count, like count, friend count, attendance count, "matched with X people," "you attended N events," "X% of your dinners produced a re-meet."
- No badge, blue checkmark, "Elder of the Quarter," "Top Conversationalist."
- No streak count, no daily-attendance bonus.
- No leaderboard of any kind, ever.

The citizen does see their **next event** and a **simple history** of the events they attended (date, format, names of attendees they consented to remember). They do not see any aggregate or ratio.

### 2.2 Architectural absence of asymmetric information

The matching algorithm does not publish per-citizen predictions to that citizen (or to anyone). A citizen cannot see "the algorithm thinks I'm a 9.2/10 dinner companion." That number is computed transiently, never stored in a queryable cross-citizen form, never exposed via any API.

### 2.3 No game theory to optimize

T1 only succeeds if a citizen knows what to do to improve their visible rank. We display no visible rank. There is nothing to optimize. (Compare ContextGraph's `flat_vector_concat_used = false` doctrine: the forbidden surface simply does not exist.)

## 3. Proof of Personhood (defeats T5)

Per `04` §3.1: every citizen must hold a unique-personhood credential. Three options:

1. **In-person attestation by an existing Established+ citizen** (preferred). The attesting citizen verifies the new citizen is a single human; both sign a credential together; the attestation is recorded in `CF_CITIZEN_ATTESTATIONS` with explicit operator attribution. Attestation does not give the attester any visible privilege.
2. **Worldcoin Orb or equivalent biometric** (where available). Iris scan generates a zero-knowledge unique-personhood proof per `2026/333` (Choudhuri et al.). Iris data is never transmitted to or stored by the platform; only the ZK proof is held.
3. **Government-ID + selfie + liveness** (fallback). Document is verified by a third-party KYC service (e.g., Persona) and then **immediately deleted**; only a one-way unique-personhood hash is retained.

A citizen with multiple personhood-credentials linked to the same biometric/ID has their accounts merged into one; the duplicates are deactivated. Sybil networks are prevented at credential time.

## 4. T2 Defense: convener power is sortitioned and rotated

- **Convener slots are rotating sortitioned roles.** No citizen may convene more than 2 Symposia per month, more than 1 Agora per quarter, more than 4 Gymnasium sessions per quarter.
- **Convening is mildly compensated in service-credit only** (free venue meal, ride credit) — never in money, never in any transferrable asset.
- **Convening visibility is local-only**: a citizen sees who convened the gatherings they attended; nobody sees a global "top conveners" list.
- **Side-payments and gifts to conveners by attendees are explicitly forbidden** in the citizen pledge; reports trigger a sortitioned-review-board investigation.
- **Convener feedback loops are private**: a sortitioned advisor reviews convener performance quarterly, anonymously, with no public output.

## 5. T3 Defense: algorithmic transparency + sortitioned review

### 5.1 Mechanism transparency

The matching algorithm's full code, the weights (α, β, γ, δ, ε, ζ from `07` §2), the bridging-rerank parameters, the τ_m thresholds, and every weight change history are **published**, signed, and timestamped. The matcher binary is reproducible-build verified.

### 5.2 Per-citizen prediction opacity

Per-citizen predictions are not published (preserving citizen privacy and preventing T7/T8). But aggregate distributions (e.g., "% of Newcomers who got matched within 14 days," "Gini of attendance frequency across active citizens") are published quarterly.

### 5.3 Sortitioned Algorithmic Review Board (SARB)

A 50-citizen panel, drawn by stratified lot (age, neighborhood, format-preference), rotating 25 members every 6 months, that:

- Reviews and signs off on every algorithmic weight change before it ships.
- Receives quarterly fairness reports (per-cell aggregate Tier-1 outcomes broken down by demographic, neighborhood, newcomer/established).
- May commission audits.
- May, by 60% supermajority, **halt** any algorithm change pending citizen-assembly review.

The SARB cannot read individual citizen Constellations or predictions; they see only aggregate distributions, with formal differential-privacy guarantees on any released numeric.

## 6. T4 Defense: cooperative ownership + constitutional invariants

Per `13` and `14`: the operating entity is a **multi-stakeholder cooperative** (citizen-members, worker-members, venue-partner-members, founder-trustee). The Tier-4 invariants are written into the articles of incorporation. Acquisition by a profit-seeking entity is **forbidden** by the articles. Asset-lock provisions (modeled on UK Community Interest Companies and US "Steward-Ownership" structures, e.g., Patagonia 2022 trust) make the cooperative's assets non-transferable to anyone who would violate the invariants. The founder pre-commits, on day 1, in a binding trust agreement, that they will not approve sale or invariant-violating amendment for the lifetime of the cooperative.

## 7. T6 Defense: minimal data, encrypted at rest, no aggregate-disclosure pipeline

- Citizen Constellations are encrypted with per-citizen keys derived from their PoP credential (per `05` §6); the platform operator cannot decrypt them without the citizen's active key.
- Event-attendance records are stored as opaque IDs server-side; the mapping to citizen names is held in a separate, citizen-decryptable store.
- The system has **no API to dump the social graph** to any government or external party. Subpoenas for individual citizen records are honored (rule-of-law) but produce only that citizen's records, not their neighbors'.
- Aggregate analytics are differential-privacy-protected; no per-cell aggregate of < 30 citizens is releasable.
- Per `03 I13` and `I14`, no third-party data sale is permitted — including for research collaborations unless the citizens individually opted in (à la Apple's Privacy-Preserving Federated Learning).

## 8. T7 Defense: structural disincentive to flex

- The citizen has no graphic, screenshotable artifact to share. (No "best of 2026" generated card, no calendar widget, no profile.)
- The platform's name/aesthetic is deliberately understated (no glossy, share-worthy branding).
- In-event behaviors enforce phone-down policies (`06` §2 — Symposium basket).
- Cultural norm reinforcement: every Citizenship Reading (`04` §3.5) includes discussion of the Athenian *eknomōs* (the "what is said here stays here" convention) and modern equivalents (AA's Twelfth Tradition: "ever to place principles before personalities").
- We can never *prevent* a citizen from posting "I had a great dinner!" — and that's fine. What we prevent is the platform producing the *raw material* for status games.

## 9. T8 Defense: rate-limiting + differential privacy on any cross-citizen query

- The platform exposes no API that lets one citizen learn whether another citizen attended a given event. Even friends-in-the-platform can only know they were co-attendees of an event they were also at.
- All public dashboards are aggregated to ≥ 30 citizens with ε-DP.
- Scraping attempts are detected (high-rate-of-request patterns, headless-browser fingerprints) and rate-limited / banned per terms.

## 10. T9 Defense: amendment supermajority + 12-month moratorium

Per `03` §3.3: changing any Tier-4 invariant requires SARB approval + citizen-assembly approval in *every* active city + a 12-month moratorium of public Agora-debate + a final re-vote. This makes capture-via-governance extraordinarily slow and visible, and gives the surrounding civic society time to organize against it.

In addition: the cooperative's articles of incorporation make the Tier-4 invariants a *trust-protected purpose*. Even if the cooperative votes unanimously to violate them, the trust may sue the cooperative for breach and dissolve it, with assets transferred to a successor cooperative that re-affirms the invariants. (Modeled on Patagonia's "earth is the only shareholder" trust.)

## 11. The cultural counter-defense

Architecture is necessary but not sufficient. Status seeking is human nature; if we suppress it on-platform, it will displace to *off*-platform vectors (e.g., a citizen flexes on Twitter that they got into a hard-to-attend Festival). Our counter-defenses:

- **Cultural seeding** — onboarding, Citizenship Reading, recurring Theater on texts about humility (Aurelius *Meditations*, Augustine *Confessions*, Tao Te Ching).
- **Sortition** — anti-elite by design; cycles roles through ordinary citizens; makes "Elder" a rotating responsibility, not a permanent identity.
- **Public ritual of departure** — when an Elder rotates out of a role, they are publicly thanked, then return to ordinary citizenship; visible "passing the torch" makes role-clinging cultural deviance.
- **Festivals** (`06` §6) — large gatherings with no commercial logos, no MC celebrities, no VIP areas. Equality is the aesthetic.

## 12. What we explicitly accept

- **Some citizens will leave** because there is no status to gain. That is correct; the platform is not for them, and is honest about it.
- **Growth will be slower than Instagram's.** That is the design.
- **A few citizens will try to game anyway** — by being charming conveners who hope to extract favors, by status-signaling in their bios, by gossiping post-events. Architecture cannot prevent micro-failures; it prevents systemic, compounding, multi-million-citizen status concentrations.

## 13. Cross-references

| Topic | Doc |
|---|---|
| The Tier-4 invariants that this implements | `03_NON_NEGOTIABLE_INVARIANTS.md` |
| The Constellation that anchors un-gameable matching | `05_THE_CONSTELLATION_DDA_LAYER.md` |
| The matching algorithm that respects these | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` |
| Governance that enforces all of this | `14_GOVERNANCE_AND_SORTITION.md` |
| Business model that closes the ad-funded gateway | `13_BUSINESS_MODEL.md` |
| Failure modes if defenses lapse | `17_FAILURE_MODES_AND_BLINDSPOTS.md` |

---

**Last updated:** 2026-05-24.
