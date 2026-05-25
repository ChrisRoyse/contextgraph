# Greek Translation Table — Classical Institutions → System Primitives

The entire design is a deliberate, careful translation of classical Athenian civic institutions (5th-4th century BCE) into digital-civic infrastructure for the 21st century. Jiang's repeated thesis (`4ehovUNrSrw`, `WoTOxE4DTFA`, `iTuRB48Of9Y`, `l6AroD58jkE`, `z0AwFin83lo`, `LWlTtdNww_k`) is that the Athenian polis is the empirically validated maximum-creativity civilization — the operating system of human flourishing. We do not romanticize ancient Athens (slavery, exclusion of women and foreigners, periodic violence — all real and disqualifying as a wholesale model). What we borrow is its **institutional geometry**, applied to a fully-inclusive 21st-century citizen body.

This table is the explicit mapping. Every system primitive should be traceable back to a Greek institutional ancestor; every Greek institution that mattered should have a 21st-century equivalent in this design.

## 1. The institutional translation table

| Greek institution | What it was | Our system primitive | Spec |
|---|---|---|---|
| **πόλις** *polis* | The city as the unit of citizenship — small enough for face-to-face, large enough for true diversity | The polis (one per city); city-anchored credentialing | `04`, `12`, `18` |
| **κοινωνία** *koinōnia* | Fellowship under shared inquiry — the *soul* of civic life | The platform's operating word; what it produces | `02` §4, all gathering formats |
| **ἀγορά** *agora* | Public square for free reasoned debate among equals | Agora format (5 gathering formats) | `06` §3, `11` |
| **συμπόσιον** *symposium* | Small evening gathering combining philosophy, friendship, eros | Symposium format | `06` §2 |
| **γυμνάσιον** *gymnasion* | Athletic training paired with informal philosophical conversation | Gymnasium format | `06` §4 |
| **θέατρον** *theatron* | Dionysia festival; civic shared aesthetic + ethical reflection | Theater format | `06` §5, `09` §4.1 |
| **Παναθήναια** *Panathenaia* | The quadrennial polis-wide festival binding the city together | Festival format | `06` §6 |
| **κληρωτήριον** *klerōtērion* | The lot-drawing machine selecting citizens for civic office | Sortitioned governance + VRF-based draws | `14`, `08` |
| **βουλή** *boulē* | Council of 500, sortitioned, rotating annually | Citizen Assembly | `14` §1.1 |
| **ἐκκλησία** *ekklēsia* | Assembly of all citizens — large deliberative body | The platform's overall Citizen-Assembly + Agora layering | `11`, `14` |
| **ἡλιαία** *hēliaia* | Sortitioned popular courts for adjudication | Sortitioned-review-board for matching algorithm disputes | `14` §1.2 |
| **παιδεία** *paideia* | Lifelong cultivation of mind, character, judgment | Paideia layer — Theater, Chevruta, Teaching Circles, Lectures | `09` |
| **ἰσηγορία** *isēgoria* | Equal speech-time for every citizen | The Agora's wooden token + "nobody speaks twice" rule | `11` §2.3 |
| **παρρησία** *parrēsia* | Frank speech without fear; speaking truth even to power | The Agora's *eknomōs* convention + post-Agora silence rule | `11` §2.4 |
| **ἰσονομία** *isonomia* | Equal participation in civic affairs; equality before law | The Tier-4 invariants; no pay-to-play; sortition; cooperative articles | `03`, `14` |
| **εὐδαιμονία** *eudaimonia* | Human flourishing as the ultimate civic outcome | The Eudaimonia Index (5-question quarterly survey) | `01` §3 |
| **ἀρετή** *aretē* | Excellence of character | Eudaimonia question 1 (Arete-self) | `01` §3 |
| **φιλία** *philia* | Friendship; one of the four loves; Aristotelian central virtue | Tier-1 metrics #2 + #3 (tie formation); Symposium + Chevruta | `10` §3 |
| **ἀγάπη** *agape* | Selfless concern for another's flourishing | Welcoming Citizen role + Civic-value coefficient δ in matcher | `04` §3.2, `07` §2 |
| **ἔρως** *eros* | Romantic / erotic love | Romance-curious Symposium path (Tier-3 #15) | `10` §2 |
| **στοργή** *storgē* | Familial / parental love | Year-2+ family-format scope | `10` §5 |
| **σοφία** *sophia* | Wisdom — the deepest intellectual virtue | Eudaimonia question 3 (Sophia) | `01` §3, `09` |
| **τέλος** *telos* | The orientation toward one's intrinsic purpose | The Tier-1/2/3/4 metric structure; TCT alignment | `01` §2, `05` §3 |
| **νόμος** *nomos* | Custom, law, structured shared practice | Cooperative articles of incorporation + Tier-4 invariants | `14` §4 |
| **οἶκος** *oikos* | Household; private sphere | Explicitly out-of-scope (per `10` §5; we serve polis, not family) | `10` §5 |
| **ξενία** *xenia* | Sacred hospitality to strangers | Aleatory format; Welcoming Citizen rite; cross-polis visits | `06` §7, `04` §3.2 |
| **ἐφηβεία** *ephēbeia* | The two-year civic apprenticeship before citizenship | The Citizenship Reading + 90-day Newcomer-to-Established arc | `04` §3.5 |
| **παιδαγωγός** *paidagōgos* | The household guide-tutor for children | (Year-2+ adolescent-format role) | `10` §5 |
| **συμμαχία** *symmachia* | Inter-polis alliance assemblies | Cross-polis Agora; Polis Network gatherings | `11` §8, `18` Phase 4 |
| **χορηγία** *chorēgia* / **λειτουργία** *leitourgia* | Wealthy citizens' obligation to fund civic events (Festival sponsorship) | Patron-tier civic subscriptions | `13` §2 |
| **ἐκνόμως** *eknomōs* | The Athenian "what is said here stays here" convention | The Agora post-event silence rule | `11` §2.4 |
| **πολίτης** *politēs* | Citizen — defined by participation, not residency | Citizen membership in the cooperative; sortition pool | `04` §1 |
| **δικαιοσύνη** *dikaiosynē* | Justice as the structural virtue of the well-ordered polis | Tier-4 invariants + cooperative governance + sortitioned review | `03`, `14` |

## 2. Five preconditions Jiang names — our digital equivalents

Jiang's `4ehovUNrSrw` enumerates the five forces that made Greek creativity possible:

| Greek precondition | Digital equivalent |
|---|---|
| **The polis (small, face-to-face)** | City-anchored polis credentialing; neighborhood-radius matching (`12` §8) |
| **The alphabet (open knowledge)** | Open-source code; AGPL-licensed cooperative; open Paideia canon (`09`, `15` §8) |
| **Homer (shared empathic myth)** | The Citizenship Reading + Core Canon; shared text-experience across citizens (`04` §3.5, `09` §3.1) |
| **The agon (productive disagreement)** | The Agora protocol with bridging-rerank and mind-change-aggregate metric (`11`) |
| **Loss of the Mycenaean priesthood (no centralized doctrine)** | The Tier-4 invariants are constitutional, but the platform takes no theological / political-party position; doctrinal diversity is welcomed | (`11` §6) |

## 3. What we deliberately do NOT borrow from Athens

Honest acknowledgment of where the model breaks:

| Athenian feature | Why we reject |
|---|---|
| **Restriction of citizenship to free adult males of native birth** | Wholly disqualifying. Our citizenship is open to every adult resident of the polis without gender, ethnicity, religious, or national restriction. |
| **Slavery** | Wholly disqualifying. |
| **Military expansion as civic virtue** | We are civic infrastructure, not military. We have no equivalent of the Athenian phratry-as-recruitment unit. |
| **Direct-democracy at scale** | We use *sortitioned* representation, not direct democracy — direct democracy at modern scale is captured by intensity, per `14` §8. |
| **Ostracism** (exile-by-mass-vote) | We use sortitioned-review-board discipline, not mass-vote shaming, for civic violations. |
| **Theocratic city-cult** | We are religiously neutral. Citizens of any faith or none are welcome; the platform has no doctrinal religious position. |
| **Status-display economies** (chorēgia public works as ostentatious display) | We adapt this carefully: patron citizenship exists, but is *not publicly attributable* (Invariant I7). Donor lists are private. |
| **Aristocratic philosophical exclusivity** | We deliberately invert: Paideia is for every citizen, every welcoming-citizen-role rotates by sortition, no philosophical priesthood. |

## 4. Why this translation matters

A platform without philosophical genealogy is just a UI. Every feature in this design can be defended by citing a 2,500-year-old institution that had centuries of empirical record. When future designers debate features ("should we add a follower count?") they can be answered with both modern evidence (Putnam, Granovetter, Ovadya) and classical lineage ("there was no follower count in the Athenian agora; isēgoria forbids it"). The deep time of the lineage is itself a defense against capture.

Jiang's deeper claim, repeated across `EBWTRvjZ1dw` (Augustine), `aS-NfPSPMu8` (Dante), `XRP407WsA0w` (Aristotle), `6M1Z_V3WgOk` (the *Comedy*): the architecture of human flourishing has been *discovered*, painfully, across millennia. We are not inventing it. We are *building the next iteration*. The platform's job is to be the next entry in a list that includes the polis, the monastic order, the medieval university, the Renaissance humanist circle, the Quaker meeting, the 19th-century salon, the YMCA, the modern public library. None of those institutions invented human flourishing; each translated it into the medium of its own age.

## 5. Cross-references

| Topic | Doc |
|---|---|
| First-principles synthesis (Jiang + Dante + others) | `02_FIRST_PRINCIPLES.md` |
| Jiang's specific lectures and themes | `20_JIANG_DOCTRINE_INTEGRATION.md` |
| The Five Gathering Formats (Greek-traced) | `06_THE_FIVE_GATHERING_FORMATS.md` |
| Sortition (Greek-traced) | `14_GOVERNANCE_AND_SORTITION.md` |
| Bibliography including primary Greek texts | `21_SOURCES_AND_INFLUENCES.md` |

---

**Last updated:** 2026-05-24.
