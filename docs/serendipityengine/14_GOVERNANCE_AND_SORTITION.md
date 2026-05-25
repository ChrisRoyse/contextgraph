# Governance and Sortition

The platform is governed not by a CEO, not by a board of directors elected by capital, not by the most-attended-citizen, and not by a DAO with token voting. It is governed by **sortition** — selection by lot from the eligible population, with stratified rotation — modeled on the classical Athenian *klerōtērion* and the modern Belgian G1000 / Ostbelgien Model / Irish Citizens' Assembly experiments.

This is one of the most carefully argued, least common, and most robust pieces of the entire design. It directly defends Invariants I12 (no unilateral invariant change), I18 (algorithmic-weight-change requires sortitioned-log), and serves as the primary defense against threats T2 (convener empire), T3 (algorithmic cabal), T4 (acquiring entity), and T9 (governance capture) in `08`.

Jiang on why this is correct: meritocracy, gerontocracy, and bureaucracy all converge on capture (`NpNCQ_gNqdE`, `4ehovUNrSrw`, `ra2v9HkaHz8`). The Athenians knew this; they selected magistrates by lot, not by election, for exactly this reason. The Belgian G1000 (2011), the Irish Citizens' Assembly (2016-present, including the constitutional referenda on abortion and marriage equality), the French Convention Citoyenne pour le Climat (2019-20), and the German Federal Citizens' Council on Climate (2023) have all empirically replicated Athens' insight at modern scale.

## 1. The five governance bodies

### 1.1 The Citizen Assembly (per polis)

- **Composition:** 1,000 citizens, stratified sortition (age, gender, neighborhood, length-of-residency, format-preference cluster), drawn afresh every 12 months. Service is voluntary but warmly encouraged.
- **Sortition pool:** every Established+ citizen who has not served in the last 24 months, weighted to ensure representation of every neighborhood and demographic band.
- **Compensation:** time-replacement stipend (sized to cover lost wages for citizens who attend; modest for high-income citizens) + child-care + transit.
- **Meets:** quarterly (4 weekend sessions per year, ~ 1.5 days each).
- **Mandate:**
  - Approve the polis's annual civic budget allocation.
  - Approve any change to local Partner Venue program.
  - Receive aggregate Tier-1 metric reports and direct the local polis cooperative on responses.
  - Adjudicate disputes between the local polis and the central cooperative.
  - Vote on Tier-4 invariant amendments (in concert with all other polis assemblies — see §6).
  - Commission Agora sessions on city-scale propositions.
- **Decision rule:** simple majority for most decisions; 60% supermajority for Tier-4-relevant decisions; 75% for any decision overriding sortitioned-review-board recommendation.

This is the platform's parliament-of-everyone.

### 1.2 The Sortitioned Algorithmic Review Board (SARB) (central, cross-polis)

Per `08` §5.3:

- **Composition:** 50 citizens, stratified across all polises, 6-month rotating terms (25 rotated each 3 months).
- **Sortition pool:** Established+ citizens, with weighting to ensure technical-literacy representation (~ 20% engineers/data scientists/legal scholars; ~ 80% other) — *not* to give the technical members more power, but to ensure technical questions get translated.
- **Compensation:** modest honorarium per session + travel + child-care.
- **Meets:** monthly, hybrid in-person/video.
- **Mandate:**
  - Approve every change to the matching algorithm weights, the τ_m guard thresholds, the Constellation specification, or the bridging-rerank parameters.
  - Approve the proposition-drafting-panel slate for each polis (rotation, conflict checks).
  - Receive quarterly fairness reports (per-cell aggregate Tier-1 outcomes by demographic / neighborhood).
  - Commission independent audits.
  - Halt (by 60% supermajority) any algorithm change pending citizen-assembly review.
- **Decision rule:** simple majority for routine; 60% supermajority for any change affecting Tier-1/2/4 metrics or invariants.
- **Information access:** aggregate distributions only, with differential-privacy on any released numeric. No per-citizen data.

This is the platform's *algorithmic supreme court*.

### 1.3 The Proposition-Drafting Panel (per polis)

Per `11` §2.1:

- **Composition:** 3 Elders (per `04` §4) per polis, rotating 3-month terms.
- **Sortition pool:** Elders only.
- **Mandate:**
  - Select 1-2 propositions per week from the citizen-submitted queue for the polis's Agora schedule.
  - Commission steelman primers from citizen-Elders.
  - Adjudicate "third-rail" zone rulings on contested propositions, subject to SARB and Citizen Assembly oversight.

### 1.4 The Welcoming Cohort (per polis)

Per `04` §3.2:

- **Composition:** ~ 20-50 citizens per polis (size scales with polis), rotating 6-month terms.
- **Sortition pool:** Established+ citizens with positive prior welcoming-volunteer indication.
- **Mandate:**
  - Conduct intake interviews for new citizens.
  - Periodically refresh in light training (~ 4 hours / quarter) on intake protocol and active listening.
- **Compensation:** service credit (free venue meal) per interview; small honorarium per quarter.

### 1.5 The Trustee Council (central, founder + named long-term stewards)

- **Composition:** the founder + 4-7 named trustees (typically: a constitutional-law scholar, a civic-non-profit veteran, a steward of an existing cooperative organization, an elder of one of the early polises, a representative of the worker-class cooperative members) — terms vary; named in the cooperative's articles.
- **Mandate:**
  - Hold the asset-lock and articles-of-incorporation; serve as guarantors of the Tier-4 invariants against capture.
  - Sign off on **founder succession** decisions.
  - Enjoy *veto-only* power over decisions that would violate Tier-4 invariants. **Cannot** initiate, direct, or override on routine governance.
  - Are themselves subject to recall by 75% Citizen Assembly vote in the polis they live in (i.e., they can be deposed if they overreach).

The Trustee Council is the *constitutional* layer — last line of defense against capture, not the day-to-day driver.

## 2. The four membership classes of the cooperative

Per `13`:

| Class | Who | Voting | Surplus share |
|---|---|---|---|
| **Citizen members** | Every active citizen | Via Citizen Assembly sortition | None directly; surplus reinvested in civic fund + venue program |
| **Worker members** | Employees of the cooperative (engineers, polis staff) | 25% of board seats + worker-council | Modest patronage dividends per cooperative norm |
| **Venue-partner members** | Partner venues (`12` §3.4) | 15% of board seats | Partnership-fee credit returns |
| **Trustee-class** | Founder + named trustees | Veto-only (§1.5) | None (salary as employees only) |

The board (~ 12 seats) is composed: 5 Citizen-Assembly-elected, 3 Worker-elected, 2 Venue-elected, 2 Trustees. No single class can dominate; major changes require cross-class consent.

## 3. Sortition methodology (the technical detail)

**Stratified sortition** (per Helene Landemore's *Open Democracy*, 2020; David Van Reybrouck's *Against Elections*, 2016):

1. Define stratifying dimensions (age, neighborhood, length-of-residency, gender, format-preference cluster, M9 topic-portfolio cluster).
2. Compute polis population proportions across the dimensions.
3. Draw lots within each stratum proportional to population, ensuring representation in expectation.
4. Reject draws where the sortitioned panel falls outside ± 5 percentage points of population on any dimension; redraw.
5. Publish the resulting panel composition (aggregate, not names) for auditability.
6. Individuals notified, invited; serve voluntarily (~ 60-80% acceptance rate typical in modern sortition experiments — replenish from a backup pool to fill declined slots, maintaining stratification).

Implementation: standard verifiable-random-function (VRF) — the same primitive used in modern blockchain consensus. The draw is auditable by any citizen via the sortition-receipt log.

## 4. The asset-lock and the cooperative articles

The cooperative's articles of incorporation:

- **Asset-lock:** the cooperative's assets (the platform's IP, citizen Constellation substrate, venue partnerships, civic fund, brand) cannot be sold or transferred to any entity that does not preserve the Tier-4 invariants. Modeled on UK Community Interest Company asset-locks and Patagonia's 2022 trust.
- **No equity issuance** to outside investors. No conversion to a corporation that would have shareholders. Capital is raised only via PRI loans or patient philanthropic equity (per `13` §7).
- **Invariant entrenchment:** the Tier-4 invariants (`03`) are written into the articles as *trust-protected purposes*. Even unanimous cooperative vote cannot remove them without the Trustee Council's separate concurrence — and the Trustee Council itself is bound by the articles.
- **Dissolution:** if the cooperative ever dissolves, assets transfer to a successor cooperative that re-affirms the Tier-4 invariants. No member receives any liquidation proceeds.

This is the legal architecture that makes the platform *practically* capture-resistant. The articles + asset-lock + trust make it economically irrational for an acquirer to acquire (there's nothing to acquire) and contractually impossible for governance to sell (sale is structurally forbidden).

## 5. The decision-making hierarchy

Routine matters → cooperative staff. Tactical decisions → polis director + worker council. Strategic / cross-polis → board + SARB. Civic / value-laden → Citizen Assembly. Tier-4-touching → Citizen Assembly + SARB + Trustee Council per §6 amendment process.

## 6. The Tier-4 amendment process (the constitutional bar)

Already specified in `03` §3.3, repeated here for completeness:

1. **Proposal published** with full reasoning, 90 days in advance.
2. **SARB approval** — 60% supermajority of the 50-citizen panel.
3. **Citizen Assembly approval** — 60% supermajority in *every* active polis.
4. **12-month moratorium** during which the amendment is debated in Agora-format sessions in every polis.
5. **Final re-vote** — same supermajority thresholds.
6. **Trustee Council concurrence** — non-objection; a Trustee veto requires a 75% Citizen-Assembly override to overturn.

This makes Tier-4 amendment theoretically possible but practically very, very hard — about as hard as amending a written constitution. That is the correct trade-off.

## 7. Why this works (the empirical record)

| Empirical case | Mechanism | Outcome |
|---|---|---|
| Athens 5th-4th c. BCE | Sortition for magistracies, juries, council | Most creative society in human history per Jiang `4ehovUNrSrw` |
| Renaissance Venice / Florence | Sortition for top offices | Stable republics for centuries amid Italian chaos |
| Belgian G1000 (2011) | Stratified sortition, 704 citizens | Produced credible federal-reform recommendations after political deadlock |
| Irish Citizens' Assemblies (2016-) | 99 sortitioned citizens | Drove abortion + marriage referendum reforms |
| Ostbelgien Model (2019-) | Permanent sortitioned citizen council | First standing sortitioned legislative body in modern Europe |
| French Convention Citoyenne (2019-20) | 150 sortitioned citizens | Produced 149 climate-policy recommendations |
| Wikipedia (2001-) | Distributed editing, sortition-like dispute resolution | Largest collaborative knowledge project in history |

Sortition has been continuously validated as the most capture-resistant scalable governance mechanism for civic-purpose institutions. We borrow it deliberately.

## 8. What we are not

- **Not a DAO.** Token-weighted voting is plutocratic. Our governance weights every citizen equally, by lot.
- **Not a co-op of conveners** (which would concentrate power in the most active citizens). Sortition draws from the *entire* citizenry.
- **Not direct democracy.** Direct democracy at scale is captured by intensity (the loudest minority wins). Sortition with random sampling avoids this.
- **Not technocracy.** SARB has technical members but is dominated by ordinary citizens, with the technical members in a translation role.
- **Not founder-CEO governance.** The founder is a trustee with veto-only power on invariant violations, not an executive with operational control. The platform is built to outlive any individual.

## 9. The transition from founder-led to fully-sortitioned

In Phase 0 (`18` §3), the platform is founder-led with informal advisory. As the first polis matures, governance progressively transitions:

- **Year 1:** founder + 3-person advisory board; founder veto on everything; minimal citizen input.
- **Year 2:** first Citizen Assembly convened (after the first polis reaches 5,000 active citizens); SARB constituted; founder retains veto.
- **Year 3:** full cooperative incorporation; board of 12; founder transitions to Trustee-class with veto-only role; SARB has binding authority over algorithm changes.
- **Year 5+:** founder may step back from any operational role; Trustee-class includes named successor candidates; full sortitioned governance.

This is documented as a binding sequence in the founder's pre-commitment trust agreement on day 1, so the founder cannot quietly retain authority past Year 3.

## 10. Cross-references

| Topic | Doc |
|---|---|
| Tier-4 invariants enforced by governance | `03_NON_NEGOTIABLE_INVARIANTS.md` |
| Anti-gaming including governance-capture threat | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |
| Agora protocol (Citizen-Assembly's deliberation source) | `11_THE_DEBATE_LAYER_AGORA_2_0.md` |
| Business model + cooperative structure | `13_BUSINESS_MODEL.md` |
| Roadmap including governance transition | `18_ROADMAP_FROM_ZERO_TO_POLIS.md` |
| Failure mode: governance capture | `17_FAILURE_MODES_AND_BLINDSPOTS.md` |

---

**Last updated:** 2026-05-24.
