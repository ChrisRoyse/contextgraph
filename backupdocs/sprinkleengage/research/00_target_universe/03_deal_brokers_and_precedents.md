---
namespace: research
agent_id: agent-03-deal-brokers
timestamp: 2026-04-19T02:00:00Z
status: active
tags: brokers, precedents, acqui-hire, licensing, m-and-a, frontier-labs, teleox
---

# Deal Brokers & Precedent Playbook — AI $1B+ Transactions (2022–2026)

**Prepared for**: Chris Royse / Teleox.ai — $3B+ engagement scan
**Asset under transaction (primitive-first framing, Chris-confirmed 2026-04-19)**: the primitive (paper §0.2 counting identity + embedder-pool extensibility — see `research/05_executive_synthesis/THE_PRIMITIVE.md`). Case studies are exploratory proofs, not products. Sell-side mandate framing is **category-creating primitive in the AI data layer**, comparable in shape to Figma-Adobe (design-files category) or Scale AI (training-labels category) — not a case-study-product sell.
**Scope**: Reverse-engineer 9 precedent deals; rank 11 broker houses + key independents
**Method**: Open-source intelligence only; dated URLs cited; Tier classification per constraints.md
**Confidence framing**: Public sources may omit co-advisors and informal introducers; treat banker-per-deal as minimum-known, not complete set.

---

## Part 1 — Precedent Playbook: How Recent $1B+ AI Deals Got Done

Each precedent is reverse-engineered along five vectors:
1. **Advisor** (who ran it, buy- and sell-side)
2. **Trigger** (what caused the first conversation)
3. **Proof anchor** (what sealed it)
4. **Structure** (cash/stock/earnout/license/reverse-acqui-hire)
5. **Timeline** (first contact → close)

---

### P-1. Scale AI ← Meta ($14.3B, 2025) — LICENSING-VALUATION HYBRID

| Vector | Finding |
|---|---|
| **Announced** | June 12, 2025 |
| **Structure** | Meta buys 49% non-voting stake; valuation $29B; Scale remains independent; CEO Alexandr Wang and select team move to Meta to run new "superintelligence lab"; Jason Droege becomes interim Scale CEO. [[1]](https://scale.com/blog/scale-ai-announces-next-phase-of-company-evolution) [[2]](https://the-decoder.com/meta-invests-14b-in-scale-ai-acquires-49-stake-and-superintelligence-lab-leader/) |
| **Advisors** | **Centerview Partners — sole Wall Street bank** on Meta side. [[3]](https://www.bloomberg.com/news/articles/2025-06-13/centerview-is-sole-wall-street-bank-on-meta-s-14-billion-ai-bet) Wilson Sonsini Goodrich & Rosati = legal to Scale; Centerview also advised on the Scale transaction structure. [[4]](https://www.businesswire.com/news/home/20250612873954/en/Scale-AI-Announces-Next-Phase-of-Companys-Evolution) |
| **Trigger** | Zuckerberg's "frustration" with Meta's AI position after Llama 4 launch delays; Scale's unique access to human-AI evaluation pairs. [[5]](https://www.cnbc.com/2025/06/10/zuckerberg-makes-metas-biggest-bet-on-ai-14-billion-scale-ai-deal.html) |
| **Proof anchor** | Scale 2024 revenue ~$870M; 2025 forecast $2B; unique RLHF-data moat. [[1]](https://scale.com/blog/scale-ai-announces-next-phase-of-company-evolution) |
| **Timeline** | Scale raised $1B at $13.8B in May 2024; Meta stake announced 13 months later at >2× that valuation. |
| **Key insight for Teleox** | **49% non-voting + CEO move** is the regulator-evasion template; Centerview is the named broker for this pattern. Scale AI = data-labeling moat; Teleox = DDA-compression moat. Pattern-transferable. |

---

### P-2. Character.AI ← Google ($2.7B, August 2024) — LICENSE + REVERSE ACQUI-HIRE

| Vector | Finding |
|---|---|
| **Announced** | August 2, 2024 |
| **Structure** | Non-exclusive license to Character.AI tech; founders Noam Shazeer + Daniel De Freitas + research team return to Google DeepMind; investors paid out $88/share (~2.5× prior round); Character.AI pivots to post-training on open-source base models. [[6]](https://www.washingtonpost.com/technology/2024/08/02/google-character-ai-noam-shazeer/) [[7]](https://em360tech.com/tech-articles/character-ai-scraps-building-llms-after-google-deal) |
| **Advisors** | **Not publicly disclosed for either side.** No named investment bank in public filings — unusual for a deal of this size, consistent with it being structured as a license-plus-hire rather than acquisition (no proxy/S-4 required). |
| **Trigger** | Character.AI fundraising fatigue (the closed-loop LLM strategy was burning cash); Shazeer was the named anchor Google wanted back (he was the "Attention is All You Need" first-author). [[8]](https://www.axios.com/2024/08/05/google-characterai-venture-capital) |
| **Proof anchor** | **Shazeer's individual research reputation** — the Transformer paper (2017) was the technical credential; Character.AI's product MAU was secondary. [[9]](https://www.businesstoday.in/technology/news/story/meet-noam-shazeer-the-ai-genius-whom-google-reportedly-paid-27-billion-to-rehire-check-details-447863-2024-09-27) |
| **Timeline** | Character.AI last raised at $1B valuation March 2023 → deal closed 17 months later at 2.7× that number, implied by Shazeer's compensation. |
| **Key insight for Teleox** | **The named-researcher proof anchor dominates.** Chris's paper + LoRA must put Chris-as-individual-researcher front-and-center. Corporate-dev didn't need Character.AI product metrics — they needed Shazeer's track record. |

---

### P-3. Inflection AI ← Microsoft (~$650M, March 2024) — CANONICAL LICENSE-AND-ACQUIHIRE

| Vector | Finding |
|---|---|
| **Announced** | March 19–21, 2024 |
| **Structure** | $620M non-exclusive license for Inflection models (incl. Inflection 2.5) + $30M for legal/litigation waivers; MSFT hires Mustafa Suleyman (→ CEO Microsoft AI, leading Copilot), Karén Simonyan (→ Chief Scientist, Microsoft AI), and ~70 of 70 employees; Inflection "pivots" to B2B enterprise AI studio. [[10]](https://finance.yahoo.com/news/microsoft-agreed-pay-inflection-650-165547758.html) [[11]](https://www.techtimes.com/articles/302841/20240321/microsoft-pays-650-million-inflection-ai-software-licensing-staff-hiring.htm) |
| **Advisors** | **Not publicly disclosed for either side.** Deal was brokered internally via **Reid Hoffman** (Microsoft director + Inflection co-founder + Greylock GP — triple hat). [[12]](https://www.theinformation.com/articles/microsofts-inflection-deal-spotlights-reid-hoffmans-role) |
| **Trigger** | **Reid Hoffman phoned Satya Nadella directly.** Hoffman sits on Microsoft's board (since 2017), co-founded Inflection (2022), and invested via Greylock. He promised investors a "good outcome." [[13]](https://techcrunch.com/2024/03/21/microsoft-inflection-ai-investors-reid-hoffman-bill-gates/) |
| **Proof anchor** | Suleyman's DeepMind co-founder credential + Inflection 2.5 performing near GPT-4 on Pi chat benchmarks; investor compensation was the commercial anchor. |
| **Timeline** | Inflection raised $1.3B at $4B June 2023 → deal closed 9 months later. |
| **Key insight for Teleox** | **Board-member-introducer > banker.** Hoffman's dual directorship replaced the bank. For Teleox, the equivalent is finding a sitting board director of a hyperscaler who is also a TCT/DDA believer. No Reid Hoffman equivalent exists — yet. The bank-path is the default. |

---

### P-4. Adept AI ← Amazon (mid-2024) — REVERSE ACQUI-HIRE TEMPLATE

| Vector | Finding |
|---|---|
| **Announced** | June 28, 2024 |
| **Structure** | Amazon hires co-founders David Luan, Augustus Odena, Maxwell Nye, Erich Elsen, Kelsey Szot + employees; non-exclusive license to Adept tech, multimodal models, select datasets; Adept proceeds distributed so **investors made whole** (Luan personally oversaw the waterfall). [[14]](https://www.geekwire.com/2024/amazon-hires-founders-from-well-funded-enterprise-ai-startup-adept-to-boost-tech-giants-agi-team/) [[15]](https://www.semafor.com/article/08/02/2024/investors-in-adept-ai-will-be-paid-back-after-amazon-hires-startups-top-talent) |
| **Advisors** | **Not publicly disclosed.** FTC sent information request July 2024 probing the structure. [[16]](https://www.cnbc.com/2024/07/16/amazons-deal-with-ai-startup-adept-faces-ftc-scrutiny.html) |
| **Trigger** | Adept had raised $415M at $1B (March 2023) but was running out of runway for general-purpose agent training costs. Luan approached Amazon AGI lead **Rohit Prasad** (prior Alexa head, now leads Amazon AGI). |
| **Proof anchor** | Luan's OpenAI pedigree (VP Engineering, co-led GPT teams 2019–2021) + Adept's action-transformer research papers on browser agents. |
| **Timeline** | ~15 months from Adept's $1B round to Amazon deal close. |
| **Key insight for Teleox** | **Rohit Prasad is a repeating corp-dev name at Amazon for AI acqui-hires.** Amazon's pattern is acqui-hire into the AGI team, not M&A into AWS. For a $3B+ deal, Chris would need to reach Prasad or Andy Jassy. |

---

### P-5. Nuance ← Microsoft ($19.7B, 2021 announced; 2022 closed) — TRADITIONAL M&A

| Vector | Finding |
|---|---|
| **Announced** | April 12, 2021; closed March 4, 2022 |
| **Structure** | All-cash; $56.00/share; 23% premium; enterprise value $19.7B including net debt. [[17]](https://news.microsoft.com/source/2021/04/12/microsoft-accelerates-industry-cloud-strategy-for-healthcare-with-the-acquisition-of-nuance/) |
| **Advisors** | **Goldman Sachs & Co. LLC — exclusive financial advisor to Microsoft**; **Evercore — exclusive financial advisor to Nuance**. [[17]](https://news.microsoft.com/source/2021/04/12/microsoft-accelerates-industry-cloud-strategy-for-healthcare-with-the-acquisition-of-nuance/) [[18]](https://mercomcapital.com/microsoft-acquires-nuance-communications-for-19-7-billion/) |
| **Trigger** | 2019 MSFT–Nuance healthcare cloud partnership (DAX ambient clinical intelligence) → two years of joint go-to-market → acquisition made operational sense. |
| **Proof anchor** | Paid-commercial healthcare revenue (DAX was in 10,000+ US hospitals); Nuance doubled Microsoft's healthcare TAM to ~$500B. |
| **Timeline** | Partnership 2019 → Acquisition Announcement 2021 (24 months) → Close 2022. **~3 years end to end.** |
| **Key insight for Teleox** | **Pre-acquisition commercial partnership is the canonical warm-up.** Teleox's path: pilot DDA corpus production with a hyperscaler first, *then* M&A. Goldman TMT is Microsoft's default bank. |

---

### P-6. Mandiant ← Google ($5.4B, 2022) — TRADITIONAL M&A

| Vector | Finding |
|---|---|
| **Announced** | March 8, 2022; closed September 12, 2022 |
| **Structure** | All-cash; $23.00/share; 57% premium to undisturbed VWAP. [[19]](https://www.mergersight.com/post/google-s-5-4bn-acquisition-of-mandiant) [[20]](https://techcrunch.com/2022/09/12/google-closes-5-4b-mandiant-acquisition/) |
| **Advisors** | **Goldman Sachs — sell-side advisor (Mandiant)**. Buy-side: reported in-house at Google (Alphabet's corp-dev is historically self-advised for <$10B deals, uses external banks for >$10B). [[19]](https://www.mergersight.com/post/google-s-5-4bn-acquisition-of-mandiant) |
| **Trigger** | Post-SolarWinds market demand for cyber-incident-response; Microsoft's $500M cybersecurity revenue growth pressuring Google Cloud. |
| **Proof anchor** | Mandiant's named incident response cases (SolarWinds, Colonial Pipeline) — named-event proof. |
| **Timeline** | ~6 months announcement-to-close. Fast by tech-M&A standards. |
| **Key insight for Teleox** | **Goldman is ambidextrous: Goldman ran MSFT-buy (Nuance) and MANDIANT-sell (Google). Tier-1 bank with cross-hyperscaler credibility.** |

---

### P-7. Figma ← Adobe ($20B announced; BLOCKED 2023) — CAUTIONARY ANTITRUST

| Vector | Finding |
|---|---|
| **Announced** | September 15, 2022; **TERMINATED December 18, 2023** |
| **Structure** | $10B cash + $10B stock; reverse triangular merger via Saratoga Merger Sub I/II; **$1B breakup fee paid by Adobe** — a record reverse-termination fee. [[21]](https://news.adobe.com/news/news-details/2022/adobe-to-acquire-figma) [[22]](https://fortune.com/2023/12/18/adobe-figma-breakup-european-regulators-merger/) |
| **Advisors** | **Allen & Company LLC — financial advisor to Adobe**; Wachtell Lipton = Adobe legal. **Qatalyst Partners LP — financial advisor to Figma**; Fenwick & West = Figma legal. [[21]](https://news.adobe.com/news/news-details/2022/adobe-to-acquire-figma) |
| **Trigger** | Adobe viewed Figma as category-defining competitor to Adobe XD; classic "buy before displaced." |
| **Proof anchor** | Figma ARR crossing $400M (2022); ~50× revenue multiple — the proof was commercial, not research. |
| **Timeline** | 15 months announcement-to-termination (EC + UK CMA pushback). |
| **Key insight for Teleox** | **Allen & Co + Qatalyst = the Tier-1 pairing for $20B tech M&A.** Qatalyst (Frank Quattrone) is the standard sell-side tech bank; Allen is the buy-side relationship bank. **Regulatory risk is now a structural factor** — any Teleox $3B+ deal must be structured to avoid DOJ/FTC/EC blockage (hence the rise of license-and-acqui-hire). |

---

### P-8. News Corp ↔ OpenAI (>$250M / 5yr, 2024) — CONTENT LICENSING

| Vector | Finding |
|---|---|
| **Announced** | May 22, 2024 |
| **Structure** | 5-year multi-year global partnership; ~$50M/year cash + tech credits; covers WSJ, Barron's, MarketWatch, NY Post + UK/AU titles; display rights + fine-tuning rights on archive. [[23]](https://openai.com/index/news-corp-and-openai-sign-landmark-multi-year-global-partnership/) [[24]](https://variety.com/2024/digital/news/news-corp-openai-licensing-deal-1236013734/) |
| **Advisors** | **Not publicly disclosed.** Likely negotiated CBO-to-CBO (News Corp's Robert Thomson ↔ OpenAI's Brad Lightcap). |
| **Trigger** | NYT v. OpenAI lawsuit (Dec 2023) created pricing discovery event; News Corp negotiated **instead of suing**. |
| **Proof anchor** | WSJ/Dow Jones brand + 150+ years of curated financial archive. |
| **Timeline** | ~6 months from NYT lawsuit to News Corp deal. |
| **Key insight for Teleox** | **The NYT-lawsuit created the price floor (~$50M/yr/publisher).** Teleox's pitch must differentiate from Path-1 (licensing) by showing DDA's **91-signals-per-input** multiplicative leverage vs. linear per-document licensing. |

---

### P-9. Reddit ↔ Google ($60M/yr, 2024) — DATA LICENSING (IPO-COUPLED)

| Vector | Finding |
|---|---|
| **Announced** | February 22, 2024 (same day as Reddit S-1) |
| **Structure** | $60M/year cash; Google trains Vertex AI on Reddit corpus; Reddit gets Vertex access for internal search. [[25]](https://www.cbsnews.com/news/google-reddit-60-million-deal-ai-training/) [[26]](https://techcrunch.com/2024/02/22/reddit-says-its-made-203m-so-far-licensing-its-data/) |
| **Advisors** | Reddit IPO bankers = Morgan Stanley, Goldman Sachs, JPM, Bank of America; the AI licensing deal was likely structured alongside the IPO prep by the same roster. |
| **Trigger** | Reddit IPO required demonstrable non-ad revenue diversification — AI licensing became the pre-IPO revenue-narrative. |
| **Proof anchor** | Reddit's 18-year user-generated corpus (the only at-scale public human-conversation archive). |
| **Timeline** | S-1 filing and deal announcement were **same-day coordinated** — classic banker choreography. |
| **Key insight for Teleox** | **Licensing deals can be structured as revenue-narrative for an IPO.** Not relevant to Teleox unless pivoting to IPO-path. |

---

## Precedent Meta-Patterns (what every $1B+ AI deal shares)

1. **Named researcher > product metrics.** Shazeer (Character), Suleyman (Inflection), Luan (Adept) — in each case, the deal dollar value was indexed to one-to-three named researchers' publication track records, not to product ARR.
2. **License-and-acqui-hire is the new normal.** 3 of the 4 post-2024 frontier deals (Character, Inflection, Adept) avoided formal M&A to sidestep FTC/DOJ scrutiny — each drew subsequent regulator inquiry. [[27]](https://stepmark.ai/2025/11/03/how-big-tech-is-rewriting-ma-the-license-and-acqui-hire-era/)
3. **Advisor disclosure is asymmetric.** Traditional M&A (Nuance, Mandiant, Figma) discloses named banks. Acqui-hires (Character, Inflection, Adept) disclose no banks — introductions are board-member or direct-exec. **This is the single largest tactical insight**: for a $3B+ Teleox outcome, the "get-in" is probably NOT through a bank.
4. **~6–24 month timeline** from first executive contact to announced deal; traditional M&A skews longer, acqui-hires shorter.
5. **Commercial partnership as warm-up**: Microsoft–Nuance 2019 partnership → 2021 acquisition is the canonical example.
6. **FTC/DOJ scrutiny is now structural.** Every license-and-hire since 2024 drew regulator inquiry (Character DOJ, Adept FTC, Inflection FTC); any Teleox $3B+ structure must pre-negotiate the regulatory posture.

---

## Part 2 — Deal-Broker Universe for AI $3B+

Ranked by **verified recent AI mandate** + **frontier-lab relationship depth** + **accessibility-to-solo-founder-with-paper**.

### Ranking methodology
- **Recent AI deal cred** (last 24 months): 1–5
- **Frontier-lab relationship depth**: 1–5
- **Accessibility to cold paper/LoRA pitch**: 1–5
- **Named partner contactability**: 1–5
- **Total**: /20

---

### B-1. Centerview Partners — **CURRENT #1 FOR AI MEGA-DEALS**

| Dimension | Finding |
|---|---|
| **Score** | 17/20 |
| **Recent AI cred** | 5/5 — **Sole Wall Street bank on Meta-Scale AI ($14.3B)** [[3]](https://www.bloomberg.com/news/articles/2025-06-13/centerview-is-sole-wall-street-bank-on-meta-s-14-billion-ai-bet); facilitated OneStream→Hg (Apr 2026). |
| **Frontier-lab depth** | 4/5 — Meta relationship confirmed through Scale deal; also advised on multiple prior tech megadeals. |
| **Accessibility** | 3/5 — Founder-led boutique culture, but high-touch with corporate clients rather than founder walk-ins. |
| **Named partner** | **Blair Effron** (Founder, Co-Founder); **Robert Pruzan** (Founder); **Mark Robinson** (SF-based, tech). Contact via centerviewpartners.com. [[28]](https://www.centerviewpartners.com/) |
| **Teleox play** | Centerview represents **Meta's chosen bank for Scale-AI-style mega-data deals**. If Teleox pursues Meta, Centerview is the named intermediary. |

---

### B-2. Qatalyst Partners — **TIER-1 SELL-SIDE TECH BOUTIQUE**

| Dimension | Finding |
|---|---|
| **Score** | 16/20 |
| **Recent AI cred** | 4/5 — Sell-side advisor on Figma-Adobe [[21]](https://news.adobe.com/news/news-details/2022/adobe-to-acquire-figma); 213 lifetime deals, $1T+ aggregate; Figure AI Series B (Feb 2024); Thoma Bravo–PROS (Sep 2025). [[29]](https://tracxn.com/d/investment-bank/qatalyst-partners/) |
| **Frontier-lab depth** | 3/5 — Quattrone's network spans the old Netscape/Amazon/Cisco bench; newer partners building frontier-lab coverage. |
| **Accessibility** | 4/5 — Frank Quattrone is **publicly accessible** (20VC podcast, Jan 2026; regular conference speaker) and is known for taking the seller's call personally. |
| **Named partner** | **Frank Quattrone** (Founder/Executive Chairman); **Jonathan Turner** (Co-Founder); **Adrian Dollard** (Co-Founder); **George Boutros** (CEO 2016–present). [[30]](https://qatalyst.com/team/) |
| **Teleox play** | **If Chris wants a sell-side banker, Qatalyst is the default.** Frank Quattrone has publicly discussed "650 M&A deals worth $1T+" and emphasizes founder-direct outreach. [[31]](https://www.thetwentyminutevc.com/frank-quattrone) **Cold outbound path: email via Qatalyst.com or DM on Twitter/LinkedIn with paper + LoRA link.** |

---

### B-3. Allen & Company — **SUN VALLEY / TMT POWER CONCLAVE**

| Dimension | Finding |
|---|---|
| **Score** | 15/20 |
| **Recent AI cred** | 4/5 — Buy-side advisor to Adobe on Figma; Sun Valley 2024–2025 hosted Sam Altman + all FAANG CEOs + Altman/Nadella/Pichai. [[32]](https://variety.com/2024/biz/news/sun-valley-2024-shari-redstone-open-ai-sam-altman-blum-1236019877/) |
| **Frontier-lab depth** | 5/5 — **Sun Valley is the one place OpenAI, Microsoft, Google, Meta leadership meet in the same room once a year.** Historically advises News Corp, Paramount, tech-media intersection. |
| **Accessibility** | 1/5 — **Invitation-only conference; firm itself is extremely relationship-driven; no public pitch path.** A cold outreach is near-impossible; you need an existing portfolio-company CEO or LP introducer. |
| **Named partner** | **Herbert Allen III** (President/CEO); **Nancy Peretsman** (Managing Director — runs media/tech); **Ken Miller** (MD). |
| **Teleox play** | **Aspirational — not a cold-outreach target.** Allen & Co is useful as "who's in the room at Sun Valley" intelligence; Chris would reach them only via warm intro from an existing portfolio relationship. |

---

### B-4. Goldman Sachs TMT — **DEFAULT HYPERSCALER BANK**

| Dimension | Finding |
|---|---|
| **Score** | 15/20 |
| **Recent AI cred** | 5/5 — Advised Microsoft on Nuance ($19.7B); advised Mandiant on Google ($5.4B); advised Alphabet on Wiz ($32B); advised on Meta $27B JV AI-data-center campus (via Morgan Stanley co-mandate); **restructured TMT into Global Infrastructure Technology (Y. Coupal + J. Tofsky) + Global Internet & Media (B. Watkins + A. Uppalapati)** Dec 2025 explicitly to capture AI-infra deals. [[33]](https://money.usnews.com/investing/news/articles/2025-12-15/exclusive-goldman-sachs-reshapes-tmt-investment-group-to-focus-on-digital-infrastructure-and-ai-deals-memo-says) |
| **Frontier-lab depth** | 5/5 — Microsoft, Google, Amazon all use Goldman TMT for formal M&A. |
| **Accessibility** | 1/5 — Tier-1 bulge bracket; won't engage a solo founder without an existing CEO/CFO sponsor. |
| **Named partner** | **Yasmine Coupal** + **Jason Tofsky** (Co-heads, Global Infrastructure Technology); **Brandon Watkins** + **Alekhya Uppalapati** (Co-heads, Global Internet & Media); **Kyle Jessen** (Head Infra-Tech M&A + Semis). [[33]](https://money.usnews.com/investing/news/articles/2025-12-15/exclusive-goldman-sachs-reshapes-tmt-investment-group-to-focus-on-digital-infrastructure-and-ai-deals-memo-says) |
| **Teleox play** | **Goldman is where Teleox lands if it gets to a signed LOI — not before.** Use Goldman names in the narrative (credibility anchor) but don't cold-outbound. |

---

### B-5. Morgan Stanley TMT — **HYPERSCALER INFRA ADVISORY**

| Dimension | Finding |
|---|---|
| **Score** | 13/20 |
| **Recent AI cred** | 4/5 — Advised Meta on $27B AI-data-center JV (2025); IPO roster for Reddit (2024); multiple AI private-placement originations. [[34]](https://www.morganstanley.com/insights/articles/ai-market-trends-institute-2026) |
| **Frontier-lab depth** | 4/5 — Strong Meta + Apple relationships; weaker at pure-AI-lab layer (OpenAI, Anthropic). |
| **Accessibility** | 1/5 — Bulge bracket; same access profile as Goldman. |
| **Named partner** | **Ankur Luther** (recently to LionTree — see B-6); **Michael Wyatt** (Global Head TMT); **Angel Anta** (Private Markets AI). |
| **Teleox play** | Same role as Goldman — LOI-stage bank, not cold-outreach. |

---

### B-6. LionTree — **TMT/AI TECH BOUTIQUE, RECENTLY HEAVIED-UP**

| Dimension | Finding |
|---|---|
| **Score** | 13/20 |
| **Recent AI cred** | 3/5 — 270+ lifetime transactions, $800B+ aggregate; **Hired Ankur Luther from Morgan Stanley Oct 2024 explicitly to lead tech dealmaking** [[35]](https://money.usnews.com/investing/news/articles/2024-10-16/liontree-hires-luther-from-morgan-stanley-to-lead-tech-dealmaking-push); advised Paramount–Skydance ($8B, 2024); raised $1B+ investment fund 2023 with Saudi PIF anchor. [[36]](https://www.axios.com/2024/02/20/liontree-bourkoff-fund-saudi) |
| **Frontier-lab depth** | 3/5 — Traditionally media-heavy; tech buildout in progress; PIF relationship is sovereign-capital pipeline. |
| **Accessibility** | 3/5 — **Aryeh Bourkoff is publicly visible (Hollywood Reporter profiles, Boardroom podcast); founder-accessible relative to bulge bracket.** |
| **Named partner** | **Aryeh Bourkoff** (Founder/CEO); **Ankur Luther** (Head of Tech, from MS Oct 2024); **Ehren Stenzler** (Co-Founder). |
| **Teleox play** | **LionTree is the "I-want-sovereign-capital-syndicate" bank.** PIF + LionTree is a real path; Bourkoff's stated thesis is TMT convergence and he was early on AI media deals. Cold email via LinkedIn has known to land. |

---

### B-7. Evercore — **M&A-ADVISORY STRONGHOLD**

| Dimension | Finding |
|---|---|
| **Score** | 12/20 |
| **Recent AI cred** | 4/5 — Exclusive financial advisor to Nuance on MSFT $19.7B [[17]](https://news.microsoft.com/source/2021/04/12/microsoft-accelerates-industry-cloud-strategy-for-healthcare-with-the-acquisition-of-nuance/); Evercore ISI publishes influential AI research (Julian Emanuel, "2025 AI inflection year" call) [[37]](https://www.investing.com/news/stock-market-news/2025-is-the-critical-ai-adoption-inflection-year-evercore-says-3796841). |
| **Frontier-lab depth** | 3/5 — Strong enterprise-tech (Nuance, Snowflake IPO, MongoDB) but thinner at pure-frontier-lab. |
| **Accessibility** | 2/5 — Independent advisory culture; relationships-first. |
| **Named partner** | **Ralph Schlosstein** (Chairman); **John Weinberg** (CEO); **Roger Altman** (Founder); **Julian Emanuel** (ISI research lead). |
| **Teleox play** | Tier-2 preferred bank if Goldman is conflicted (e.g., buyer is already Goldman's client). Evercore's independence is the value prop. |

---

### B-8. Moelis & Company — **INDEPENDENT MID-MARKET-TO-MEGACAP**

| Dimension | Finding |
|---|---|
| **Score** | 11/20 |
| **Recent AI cred** | 3/5 — Ken Moelis stepped down as CEO June 2025; Navid Mahmoodzadegan took over. Firm investing heavily in tech/AI hiring; specific named frontier deals less visible in public record. [[38]](https://stocktwits.com/news-articles/markets/equity/ken-moelis-to-step-down-as-chief-executive-of-moelis-after-18-years/chlEkLZRbyK) |
| **Frontier-lab depth** | 3/5 — Deep PE/strategic relationships; less direct frontier-lab mandate visibility. |
| **Accessibility** | 3/5 — Moelis himself is publicly accessible via board/speaking circuit as Executive Chairman. |
| **Named partner** | **Ken Moelis** (Executive Chairman); **Navid Mahmoodzadegan** (CEO, 2025–); **Eric Cantor** (Vice Chairman). |
| **Teleox play** | Secondary if Qatalyst/Centerview conflicted. |

---

### B-9. JPMorgan TMT — **BULGE BRACKET AI-COVERAGE**

| Dimension | Finding |
|---|---|
| **Score** | 11/20 |
| **Recent AI cred** | 3/5 — JPM deploys $1.3B on internal AI (2024), $18B total tech budget 2025; TMT group active on Reddit IPO roster; no headline frontier-lab advisory in 24mo window. [[39]](https://www.klover.ai/jpmorgan-ai-strategy-chasing-ai-dominance/) |
| **Frontier-lab depth** | 3/5 — Relationship bank to public hyperscalers; lighter at pure-AI-lab layer. |
| **Accessibility** | 1/5 — Full bulge bracket. |
| **Named partner** | **Madhu Namburi** (Global Head of Tech IB); **Noah Wintroub** (Vice Chairman, Tech/Media/Internet). |
| **Teleox play** | Default if Goldman conflicted on a Meta/Amazon/Apple deal. |

---

### B-10. PJT Partners — **INDEPENDENT ADVISORY, STRATEGIC-FOCUSED**

| Dimension | Finding |
|---|---|
| **Score** | 10/20 |
| **Recent AI cred** | 3/5 — Strategic advisory on Core Scientific (AI-infra crypto-pivot); 2025 "second-best ever" strategic advisory year. [[40]](https://www.ainvest.com/news/pjt-partners-mastering-post-summer-landscape-restructuring-prowess-leadership-expansion-2509/) |
| **Frontier-lab depth** | 2/5 — Restructuring-biased historically; scaling advisory. |
| **Accessibility** | 3/5 — Paul Taubman is approachable via speaking circuit. |
| **Named partner** | **Paul Taubman** (Founder/CEO/Chairman); **Taylor Wright** (Head Strategic Advisory). |
| **Teleox play** | Secondary tier; useful if complex capital structure needed. |

---

### B-11. Lazard — **INDEPENDENT, AI-RESEARCH-FORWARD**

| Dimension | Finding |
|---|---|
| **Score** | 10/20 |
| **Recent AI cred** | 3/5 — Published "AI Impact on Technology M&A 2025" thesis; advises on tech M&A globally but pure-AI-lab headline deals less visible in window. [[41]](https://www.lazard.com/media/uifkfcwt/ai-impact-on-technology-ma_final.pdf) |
| **Frontier-lab depth** | 2/5 — European/global reach strong; Silicon Valley frontier-lab layer thinner. |
| **Accessibility** | 2/5 — Bulge-equivalent advisory. |
| **Named partner** | **Peter Orszag** (CEO); **Alexandra Soto** (Chief Operating Officer). |
| **Teleox play** | Secondary tier; useful if international/sovereign involvement needed. |

---

## Independent Advisors / Board-Director Brokers (the "Hoffman path")

These are **not banks** but individuals who have demonstrated the ability to broker a $1B+ AI deal via personal relationship:

| Name | Primary Affiliation | Cross-Board Role | Relevance to Teleox |
|---|---|---|---|
| **Reid Hoffman** | Greylock GP | Microsoft Director (since 2017); Inflection co-founder/director | **Single most-successful AI-deal-broker in 2024.** Demonstrated ability to route $650M from MSFT to Inflection. Not accessible cold; requires Greylock partner intro. [[12]](https://www.theinformation.com/articles/microsofts-inflection-deal-spotlights-reid-hoffmans-role) |
| **Marc Andreessen** | a16z Co-Founder | Meta Director (since 2008) | Known Meta-side AI connector; a16z portfolio includes many frontier labs. |
| **Frank Quattrone** | Qatalyst Founder | Executive Chairman | Publicly accessible via 20VC/conferences; will take founder direct outreach for serious deals. [[31]](https://www.thetwentyminutevc.com/frank-quattrone) |
| **Aryeh Bourkoff** | LionTree Founder/CEO | — | Sovereign-capital pipeline (PIF); media-convergence thesis overlaps TCT narrative. |
| **Rohit Prasad** | Amazon AGI SVP | Internal Amazon corp-dev gatekeeper for AGI acqui-hires | Path to Amazon = through Prasad; he personally oversaw Adept. [[14]](https://www.geekwire.com/2024/amazon-hires-founders-from-well-funded-enterprise-ai-startup-adept-to-boost-tech-giants-agi-team/) |
| **Brad Lightcap** | OpenAI COO | Runs OpenAI corp-dev + strategic partnerships | Signed News Corp, Condé Nast, Axel Springer licensing deals personally. |
| **Mustafa Suleyman** | Microsoft AI CEO (ex-Inflection) | Microsoft internal corp-dev influence | If MSFT is target, the acqui-hire playbook lives in his team. |

---

## Synthesis Table — Broker Ranking for Teleox $3B+ Engagement

| Rank | Firm/Person | Score | Teleox Use Case | Cold-Outreach Viable? |
|---|---|---|---|---|
| 1 | Centerview Partners | 17/20 | If target is **Meta** (Scale-AI precedent) | No — needs warm intro |
| 2 | Qatalyst Partners | 16/20 | **Default sell-side bank** | **Yes — Quattrone directly** |
| 3 | Allen & Company | 15/20 | Sun Valley access — aspirational | No |
| 4 | Goldman Sachs TMT | 15/20 | LOI-stage if target is MSFT/Google | No |
| 5 | Morgan Stanley TMT | 13/20 | LOI-stage if target is Meta/Apple | No |
| 6 | LionTree | 13/20 | **If sovereign-capital syndicate** (PIF) | **Yes — Bourkoff via LinkedIn** |
| 7 | Evercore | 12/20 | Tier-2 if Goldman conflicted | No |
| 8 | Moelis | 11/20 | Secondary sell-side | Partial — Ken Moelis |
| 9 | JPMorgan TMT | 11/20 | Default if Goldman conflicted | No |
| 10 | PJT Partners | 10/20 | Complex-cap-structure advisory | Partial |
| 11 | Lazard | 10/20 | International/sovereign deals | No |
| — | Reid Hoffman | (path, not firm) | **MSFT back-channel** | No — Greylock intro |
| — | Frank Quattrone | (path, not firm) | **Sell-side back-channel** | **Yes** |

---

## Final Recommendation — Top 3 for Teleox (sequenced)

### #1 — Frank Quattrone / Qatalyst Partners (DIRECT)

**Why**: Only Tier-1 sell-side banker who is **publicly accessible** and takes founder outreach. Track record includes Figma–Adobe (the canonical $20B tech deal). Quattrone's own framing ("650 deals, $1T, I take the call") is a public invitation to serious sellers.

**Cold approach**: Email (qatalyst.com) + LinkedIn DM + **personalised 1-pager** referencing Figma deal + Chris's measured anchor (Case 3 0.961 MEASURED WavLM SECS) + paper link + LoRA link.

**First ask**: 20-minute call to discuss whether the DDA/TCT/MC thesis warrants a formal sell-side mandate.

### #2 — Aryeh Bourkoff / LionTree (DIRECT)

**Why**: PIF (Saudi sovereign) fund anchor 2023 means LionTree is the only boutique with demonstrated sovereign-capital syndication for AI-scale checks. Bourkoff's TMT-convergence thesis maps directly to DDA's cross-modal positioning.

**Cold approach**: LinkedIn DM + referenced in newsletter commentary + paper link.

**First ask**: Is Teleox a fit for LionTree's tech-advisory roster (Luther's new mandate)?

### #3 — Centerview Partners (WARM INTRO REQUIRED)

**Why**: If the path-to-$3B runs through **Meta**, Centerview is the named gatekeeper (Scale-AI precedent). No cold path exists; warm intro via existing portfolio-company CEO is required.

**Cold approach**: **Do not attempt directly.** Instead, identify one Meta-adjacent company (Scale, Anthropic-via-AWS, etc.) and route an intro through an LP or advisor with Centerview exposure.

**First ask**: (via warm intro) "Would Centerview consider a mandate for a data-compression technology that extends the Scale-AI thesis?"

---

## Competing Interests Disclosure
Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.

---

## Sources

**Precedents:**
1. https://scale.com/blog/scale-ai-announces-next-phase-of-company-evolution — Scale AI official announcement
2. https://the-decoder.com/meta-invests-14b-in-scale-ai-acquires-49-stake-and-superintelligence-lab-leader/
3. https://www.bloomberg.com/news/articles/2025-06-13/centerview-is-sole-wall-street-bank-on-meta-s-14-billion-ai-bet — Centerview as Meta-Scale bank
4. https://www.businesswire.com/news/home/20250612873954/en/Scale-AI-Announces-Next-Phase-of-Companys-Evolution
5. https://www.cnbc.com/2025/06/10/zuckerberg-makes-metas-biggest-bet-on-ai-14-billion-scale-ai-deal.html
6. https://www.washingtonpost.com/technology/2024/08/02/google-character-ai-noam-shazeer/
7. https://em360tech.com/tech-articles/character-ai-scraps-building-llms-after-google-deal
8. https://www.axios.com/2024/08/05/google-characterai-venture-capital
9. https://www.businesstoday.in/technology/news/story/meet-noam-shazeer-the-ai-genius-whom-google-reportedly-paid-27-billion-to-rehire-check-details-447863-2024-09-27
10. https://finance.yahoo.com/news/microsoft-agreed-pay-inflection-650-165547758.html
11. https://www.techtimes.com/articles/302841/20240321/microsoft-pays-650-million-inflection-ai-software-licensing-staff-hiring.htm
12. https://www.theinformation.com/articles/microsofts-inflection-deal-spotlights-reid-hoffmans-role — Hoffman's role
13. https://techcrunch.com/2024/03/21/microsoft-inflection-ai-investors-reid-hoffman-bill-gates/
14. https://www.geekwire.com/2024/amazon-hires-founders-from-well-funded-enterprise-ai-startup-adept-to-boost-tech-giants-agi-team/
15. https://www.semafor.com/article/08/02/2024/investors-in-adept-ai-will-be-paid-back-after-amazon-hires-startups-top-talent
16. https://www.cnbc.com/2024/07/16/amazons-deal-with-ai-startup-adept-faces-ftc-scrutiny.html
17. https://news.microsoft.com/source/2021/04/12/microsoft-accelerates-industry-cloud-strategy-for-healthcare-with-the-acquisition-of-nuance/ — Goldman/Evercore advisors
18. https://mercomcapital.com/microsoft-acquires-nuance-communications-for-19-7-billion/
19. https://www.mergersight.com/post/google-s-5-4bn-acquisition-of-mandiant
20. https://techcrunch.com/2022/09/12/google-closes-5-4b-mandiant-acquisition/
21. https://news.adobe.com/news/news-details/2022/adobe-to-acquire-figma — Allen & Co / Qatalyst named
22. https://fortune.com/2023/12/18/adobe-figma-breakup-european-regulators-merger/
23. https://openai.com/index/news-corp-and-openai-sign-landmark-multi-year-global-partnership/
24. https://variety.com/2024/digital/news/news-corp-openai-licensing-deal-1236013734/
25. https://www.cbsnews.com/news/google-reddit-60-million-deal-ai-training/
26. https://techcrunch.com/2024/02/22/reddit-says-its-made-203m-so-far-licensing-its-data/
27. https://stepmark.ai/2025/11/03/how-big-tech-is-rewriting-ma-the-license-and-acqui-hire-era/ — acqui-hire pattern

**Brokers:**
28. https://www.centerviewpartners.com/
29. https://tracxn.com/d/investment-bank/qatalyst-partners/ — Qatalyst deal count
30. https://qatalyst.com/team/
31. https://www.thetwentyminutevc.com/frank-quattrone — Quattrone 20VC
32. https://variety.com/2024/biz/news/sun-valley-2024-shari-redstone-open-ai-sam-altman-blum-1236019877/
33. https://money.usnews.com/investing/news/articles/2025-12-15/exclusive-goldman-sachs-reshapes-tmt-investment-group-to-focus-on-digital-infrastructure-and-ai-deals-memo-says — Goldman TMT restructure
34. https://www.morganstanley.com/insights/articles/ai-market-trends-institute-2026
35. https://money.usnews.com/investing/news/articles/2024-10-16/liontree-hires-luther-from-morgan-stanley-to-lead-tech-dealmaking-push — LionTree tech push
36. https://www.axios.com/2024/02/20/liontree-bourkoff-fund-saudi — LionTree PIF fund
37. https://www.investing.com/news/stock-market-news/2025-is-the-critical-ai-adoption-inflection-year-evercore-says-3796841
38. https://stocktwits.com/news-articles/markets/equity/ken-moelis-to-step-down-as-chief-executive-of-moelis-after-18-years/chlEkLZRbyK
39. https://www.klover.ai/jpmorgan-ai-strategy-chasing-ai-dominance/
40. https://www.ainvest.com/news/pjt-partners-mastering-post-summer-landscape-restructuring-prowess-leadership-expansion-2509/
41. https://www.lazard.com/media/uifkfcwt/ai-impact-on-technology-ma_final.pdf

*Source count: 41 (exceeds 30 minimum: 27 precedent + 14 broker = 41)*
