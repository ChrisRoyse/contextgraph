# Reaching Yann LeCun — Outreach Strategy for Context Graph

**Date:** 2026-05-01
**Target:** Yann LeCun — ACM Turing Laureate, NYU Silver Professor, Executive Chairman / co-founder AMI Labs (Advanced Machine Intelligence Labs), ex-Chief AI Scientist at Meta (FAIR).
**Goal:** Get his attention on Context Graph as an aligned implementation of persistent-memory + JEPA-style world-modeling, so it lands on his radar (best case: a reply / quote tweet / intro to AMI staff; stretch: collaboration, hire, or investment conversation).

---

## 1. The Situation (Why Now Is The Right Moment)

LeCun left Meta in late 2025 and unveiled **AMI Labs** in January 2026 with a **$1.03B seed at $3.5B pre-money** — one of the largest seed rounds ever. The company is built on **four explicit pillars** ([amilabs.xyz](https://amilabs.xyz/)):

1. **Understand the real world**
2. **Have persistent memory**
3. **Reason and plan**
4. **Are controllable and safe**

Context Graph hits all four with shipped Rust code:

| AMI pillar | Context Graph evidence |
|---|---|
| Understand the real world | 14 production embedders (E1–E14) covering semantic, causal, code, graph, entity, multilingual; 7 learner-state slots (E15–E21) for affect/physiological grounding |
| Persistent memory | RocksDB with 73 column families, single-writer Unix flock, append-only audit log, merge history, importance history, embedding registry — *real* persistent memory primitives |
| Reason and plan | E5 cause↔effect asymmetric embeddings (1.2× cause→effect, 0.8× reverse), `get_causal_chain` with 0.9^hop attenuation, LLM-gated causal/graph discovery (Qwen2.5-3B), constellation centroids |
| Controllable and safe | Operator attribution on every write, audit_log CF, soft-delete with 30-day recovery, merge reversal hashes, fail-fast validation, single-writer guarantee |

**Plus the Learning-as-UTL subsystem** computes ΔS·ΔC·ΔE → L with 6 diagnostic states (confused/optimal/stuck/boring/dysregulated/dissipating) — this is *exactly* the energy-based, objective-driven framing LeCun has been pushing in his "A Path Towards Autonomous Machine Intelligence" vision paper.

This is a real moment. He has $1B and is hiring. He is publicly arguing LLMs are a dead end. You have a shipped artifact that prefigures what AMI is trying to build.

---

## 2. Pathway Ranking (By Probability × Reward, Highest First)

### Tier 1 — Highest Leverage

#### 1.1. Write a public thread on X showing Context Graph as a JEPA-compatible memory substrate
- **Why it works:** LeCun's primary discovery channel is X ([@ylecun](https://x.com/ylecun), 1M+ followers). He actively engages with substantive technical posts, especially anything that demonstrates JEPA-aligned thinking or contradicts LLM-only orthodoxy.
- **Key:** He filters hard for *technical substance* and *original work*. He's openly hostile to vague hype and "AI doomers." Show code, benchmarks, fingerprints.
- **Format:** A 6–10 tweet thread with one diagram (the 14-embedder fingerprint with the four AMI pillars overlaid), one short loom/screen recording of `search_graph` running on synthetic + real data, one link to the GitHub repo, one link to the spec docs in `/docs2/contextgraphspecs/`.
- **Hook line examples (pick one, A/B test):**
  - *"Built a 14-embedder persistent memory substrate that maps 1:1 onto the four AMI Labs pillars. Open source. Here's how it works →"*
  - *"If world models need persistent memory, here's what 'persistent memory' actually has to look like in storage. 73 RocksDB CFs, audit log, merge lineage. Code below."*
  - *"@ylecun A working JEPA-style memory layer in Rust: 14 representation spaces (no pixel reconstruction), causal-asymmetric retrieval, learner-state-conditioned routing. Spec + code in repl."*
- **CTA:** Link to repo + spec docs + a single one-page PDF "Context Graph for JEPA agents."
- **Tagging:** Tag `@ylecun` *once* in the first tweet (more = filter), and tag `@sainingxie` (CSO of AMI), `@MichaelGRabbat` (VP World Models, AMI), and `@kchonyc` (NYU collaborator).
- **Timing:** Post Tue/Wed 9–11 ET when he is most active.

#### 1.2. Apply for the AMI Scientist or AMI Engineer role with Context Graph as your portfolio
- **Why it works:** Five open positions ([jobs.ashbyhq.com/ami](https://jobs.ashbyhq.com/ami)), "AMI Scientist" and "AMI Engineer" both list NY/Paris/Montreal/Singapore. The job description literally says: *"You will work with a team on world model research efforts, including: Self-supervised learning methods… new architectures… scalable algorithms for pre-processing and curating video data… evaluations for benchmarking world model understanding, prediction, and planning… efficient algorithms for model-based planning and reasoning."*
- **Move:** Apply with a cover letter framing Context Graph as *"a working persistent-memory + multi-representation retrieval layer that any JEPA-trained world model can plug into."*
- **Reality:** Even if you don't want the job, the application is read by the recruiter (currently hiring an "AI Research Recruiter (NY based)"), and exceptional submissions get forwarded to founders. This is a *legitimate* surface-area increase regardless of intent.
- **Direct contacts:**
  - General: `contact@amilabs.xyz`
  - Press (use only if you have a real release): `press@amilabs.xyz`
  - Apply: `https://jobs.ashbyhq.com/ami`

#### 1.3. Reply substantively to one of his recent posts with a code/benchmark, not opinion
- **Why it works:** He's said publicly: *"Scientific debates on social media are like a human form of bidirectional RLHF."* He rewards specific, evidenced replies with quote-tweets. He punishes generic argumentation by ignoring it.
- **The replies he engages with** look like: a graph, a benchmark number, a counter-example with code, a paper link with one-sentence summary.
- **Watch list:** Set up a follow on his X, his [Threads](https://www.threads.com/@yannlecun), his [Facebook](https://www.facebook.com/yann.lecun/), and his [LinkedIn](https://www.linkedin.com/in/yann-lecun/). He cross-posts but Threads/FB are higher signal-to-noise (less hostility, more thoughtful).
- **Pattern:** When he posts about JEPA/world models/persistent memory/EBMs/criticism of LLMs/open source — within 30 minutes, drop a focused reply: one sentence + one image/code link.

### Tier 2 — Good Leverage, Slower

#### 2.1. Get inside the NYU CILVR / Global AI Frontier Lab orbit
- **Lab:** Computational Intelligence, Learning, Vision, and Robotics — co-founded by LeCun and Rob Fergus, currently 18 PIs. Hosts a weekly seminar series ([wp.nyu.edu/cilvr](https://wp.nyu.edu/cilvr/)) that LeCun attends when in town.
- **Inroads via collaborators (all reachable, all reply more than LeCun does):**
  - **Joan Bruna** — `bruna@cs.nyu.edu`, 60 Fifth Ave Office 612. Mathematical foundations of deep learning.
  - **Kyunghyun Cho** — `kyunghyun.cho@nyu.edu`, 60 Fifth Ave Office 616, X: [@kchonyc](https://twitter.com/kchonyc). Co-leads CILVR with LeCun. **Heads the Global AI Frontier Lab with LeCun.** Most accessible high-status NYU CILVR person.
  - **Rob Fergus** — co-founder of CILVR, also at NYU.
  - **Brenden Lake**, **Sam Bowman**, **Rajesh Ranganath** — CILVR co-leads.
- **Move:** Email Cho first. He's known for being responsive and posts his email publicly. Pitch a 15-min Zoom or an in-person meeting at a CILVR seminar. If Cho engages, ask him whether the Frontier Lab seminar would host a brown-bag on Context Graph.
- **Timing:** NYU semester runs through May; if pursuing this, contact in late August for fall semester slot.

#### 2.2. Submit a workshop paper at a conference where he is keynoting / on the board
- **He is on the ICLR 2026 Board** (NYU + AMI affiliation). Conference dates:
  - **ICLR 2026** — April 23–25 (already passed; aim ICLR 2027)
  - **ICML 2026** — July 6–11, Seoul, South Korea
  - **NeurIPS 2026** — Sydney (Dec 6–12), Atlanta (Dec 8–13), Paris (Dec 9–13). He'll be at Paris with high probability (AMI is HQ'd there).
- **Best workshops to target** (most likely to draw him):
  - "Self-Supervised Learning: Theory and Practice" (recurs at NeurIPS / ICML)
  - "World Models Workshop"
  - "Memory in Foundation Models" (new, watch for CFP)
  - Anything with JEPA, EBM, or persistent-memory in the title
- **Move:** Even a 4-page workshop submission with a poster gets you a 90-second hallway conversation if he passes by. Bring printed one-pagers.

#### 2.3. Publish a comparison paper / arxiv preprint
- **Title-as-bait pattern:** "Context Graph: A 14-Representation Persistent Memory Substrate for Objective-Driven AI Agents"
- Co-cite his vision paper (*A Path Towards Autonomous Machine Intelligence*, openreview 2022) and the LeWorldModel paper (Maes/LeLidec/Scieur/LeCun/Balestriero, March 2026, [arxiv 2603.19312](https://arxiv.org/abs/2603.19312)).
- Post the arxiv link on X tagging him *once*. Arxiv links have higher engagement than blog posts.
- **Critical:** Don't claim it's "the missing piece of JEPA" — claim it's *"a candidate persistent-memory module compatible with JEPA-trained encoders."* Frame as collaboration, not replacement.

### Tier 3 — Lower probability, useful as supplements

#### 3.1. Email NYU directly with a tight pitch
- **Address:** `yann@cs.nyu.edu`. Office: 60 Fifth Ave, Office 516. Phone: 212-998-3283 (do not call).
- **Reality:** He almost certainly gets 50+ cold emails per day and has admitted publicly to ignoring most. Probability of personal reply ≈ 1–3%.
- **However:** A *very* short email (< 120 words), with a one-sentence hook, one link to a 1-min loom, and one PDF spec attachment, occasionally gets forwarded to a postdoc or grad student. That's still a useful outcome.
- **Subject line pattern:** Specific and provocative — e.g., *"Persistent memory layer for JEPA agents — 73 CFs, 14 spaces, MIT-licensed Rust"*

#### 3.2. LinkedIn message
- He's active on LinkedIn ([linkedin.com/in/yann-lecun](https://www.linkedin.com/in/yann-lecun/)). Only useful if you're in his second-degree network.
- **Better tactic:** Comment substantively on his LinkedIn posts (he reads top comments). Don't DM cold.

#### 3.3. Through an investor in AMI Labs
- AMI investors include **Cathay Capital**, **C4 Ventures**, **Toyota** (strategic). If you have any LP/founder relationships into these funds, ask for a portfolio-company-curiosity intro.
- Investors love forwarding "interesting builders" to portfolio founders. Lower threshold than a cold email.

#### 3.4. Through Lex Fridman
- LeCun has been on the [Lex Fridman Podcast](https://lexfridman.com/yann-lecun-3/) three times (most recent: ep #416, March 2024). Lex frequently amplifies indie ML projects.
- DM Lex on X with a tight pitch — if he amplifies Context Graph, LeCun sees it.
- **Probability: low. Reach: very high if it hits.**

#### 3.5. Speaker bureau intermediaries
- He's bookable through agencies (Champions Speakers UK, AI Speakers Agency). These are *not* contact channels — listed for completeness only. Do not use.

---

## 3. The Pitch (One Page You Can Reuse Everywhere)

**Headline:** *Context Graph — a persistent memory + multi-representation retrieval layer that fits the AMI Labs four-pillar definition of intelligence.*

**Three-line elevator:**
> Context Graph stores every memory in 14 partly-orthogonal representation spaces (semantic, causal, code, graph, entity, paraphrase, sparse, token, HDC, multilingual, temporal) — none of which try to reconstruct pixels — and exposes a 108-tool MCP surface that any agent or world-model can use as its persistent memory layer. Built in Rust, 73 RocksDB column families, audit-logged, MIT-licensed. The four-pillar mapping: *understand the real world* → 14 embedders + 7 learner-state slots; *persistent memory* → durable RocksDB with merge lineage; *reason and plan* → E5 cause↔effect asymmetric retrieval + causal chain traversal; *controllable and safe* → operator-attributed audit log on every write.

**One-sentence why-it's-different:** Most "memory for agents" is RAG over one embedding model; Context Graph is a *substrate* with 14 perspectives, learner-state-conditioned routing, and storage primitives that survive crashes, schema migrations, and adversarial inputs.

**Asks (in increasing audacity):** technical feedback / amplification on X / a 15-min Zoom with an AMI scientist / pilot integration with AMI's evaluation harness.

---

## 4. Tactical Messaging Templates

### 4.1. The X Reply (when he posts about JEPA / world models / memory)
> Built this as the persistent-memory substrate side of that picture — 14 representation spaces, no pixel reconstruction, RocksDB-durable, MIT-licensed Rust. Repo: [link]. Would love your read on the E5 asymmetric (cause→effect 1.2×, reverse 0.8×) design.

(< 280 chars, has a *specific technical detail*, ends with a precise question.)

### 4.2. The X Original Post Thread (1.1)
1. *Ten months ago I started building what a "persistent memory layer for JEPA-style agents" would actually have to look like in production. Here's what shipped. (1/n)*
2. *14 representation spaces per memory. None reconstruct pixels. Each captures a different invariant: semantic, causal-asymmetric, code, graph, entity, paraphrase, sparse-keyword, token-level, HDC-noise-robust, multilingual.*
3. *Causal retrieval is asymmetric: cause→effect 1.2×, effect→cause 0.8×. Mirrors the directional nature of mechanism. E5 stores dual vectors; HNSW indexes both directions separately.*
4. *Persistent memory: 73 RocksDB column families. Append-only audit log keyed by big-endian timestamp. Permanent merge history. Operator attribution. Single-writer Unix flock.*
5. *State-conditioned routing: when a learner's stress-floor crosses 0.60, retrieval automatically swaps to an "affect_repair" weight profile. The four AMI Labs pillars, but for memory.*
6. *108 MCP tools, including `search_graph` (multi-space RRF fusion), `get_causal_chain` (0.9^hop attenuation), `compile_constellation` (centroid + spread over arbitrary memory selectors).*
7. *MIT-licensed. Rust. Spec docs: [link]. Repo: [link]. Would love feedback from anyone working on world models / memory architectures. cc @ylecun @sainingxie*

### 4.3. The Cold Email (3.1)
> Subject: Persistent memory layer for JEPA agents — Rust, 14 spaces, MIT
>
> Dr. LeCun,
>
> I've built a persistent memory substrate that maps to the four AMI Labs pillars: 14 embedding spaces (none reconstruct pixels), 73 RocksDB CFs with audit log + merge lineage, E5 asymmetric cause→effect retrieval, learner-state-conditioned routing.
>
> Spec: [link to /docs2/contextgraphspecs/]
> Repo: [link]
> 90-second loom: [link]
>
> If interesting, I'd value 15 minutes with anyone on the AMI memory/eval team — happy to fly to Paris or NY.
>
> [name] · [github] · [email]

(115 words. One subject hook. Three artifacts. One ask. No CV-dump, no cofounder bio paragraph.)

### 4.4. The Email to Kyunghyun Cho (2.1) — most likely to actually reply
> Subject: Quick technical question on persistent memory architectures for objective-driven agents
>
> Dr. Cho,
>
> I've been building an open-source persistent-memory layer (Context Graph, MIT Rust) intended as the storage substrate for JEPA-style agents — 14 representation spaces, durable RocksDB, learner-state-conditioned retrieval. Two questions where your work on retrieval + LM agents would be uniquely useful:
>
> 1. Whether the 91-pairwise-cross-correlation feature in the training export is the right shape for downstream learning — or whether something Tucker-decomposed would be better.
> 2. Whether the CILVR seminar would be open to a 25-min brown-bag once the eval harness lands.
>
> Spec: [link]. No reply expected if too tangential.
>
> [name]

---

## 5. Don'ts (LeCun-Specific Trip Wires)

- **Don't pitch "AGI"**, "alignment as existential threat," or "we solved X" — he is publicly hostile to the doomer/hype vocabulary.
- **Don't tag him in every tweet.** Tag once. He filters notifications.
- **Don't send marketing decks.** Send code, papers, benchmarks, and one diagram.
- **Don't compare yourself to GPT or claim to "replace LLMs."** AMI's pitch is not "LLM killer," it's "different architecture for different problems."
- **Don't email asking for advice or mentorship** — he gets dozens of these per week and has openly said he ignores them. Email with an *artifact* and an *offer* (here is what I built, would value your read).
- **Don't post screenshots of LLM hype takedowns** to try to get his attention. He's said he finds the dunking culture tiresome even when it's pointed at people he disagrees with.
- **Don't spam multiple channels in the same week.** Pick one, wait 2 weeks, escalate to the next if no reply.

---

## 6. Sequencing — A Concrete 30-Day Plan

| Day | Action | Channel |
|---|---|---|
| 1 | Push Context Graph repo to public visibility; ensure README has a clear "What this is for AMI/JEPA agents" section at top | GitHub |
| 2 | Record 90-second loom demonstrating store_memory → search_graph → get_causal_chain on synthetic data | Loom |
| 3 | Write the one-pager PDF (§3 of this doc, formatted) | Local |
| 4 | Apply to AMI Scientist + AMI Engineer (general application form) with cover letter referencing the four pillars | jobs.ashbyhq.com/ami |
| 5 | Email Kyunghyun Cho (template §4.4) | Email |
| 7 | Publish the X thread (template §4.2) on a Tuesday 09:30 ET. Tag @ylecun once + @sainingxie + @MichaelGRabbat. Cross-post to LinkedIn + Threads. | X / LinkedIn / Threads |
| 10 | If no engagement on X thread by day 10, write the second thread variant (different hook) and try again on a Wednesday 09:00 ET | X |
| 14 | Watch for any LeCun X post on memory / world-models / JEPA. Reply within 30 min using template §4.1. | X |
| 17 | Cold-email LeCun directly (template §4.3) — only if no other channel has produced engagement | Email |
| 21 | Submit arxiv preprint titled "Context Graph: A Persistent Memory Substrate for Objective-Driven AI Agents." Post arxiv link to X with @ylecun tag. | Arxiv |
| 25 | If still no engagement: reach out to AMI investors (Cathay Capital, C4 Ventures) via LinkedIn intro requests | LinkedIn |
| 30 | Retrospect. Re-prioritize remaining tier 2/3 paths. | — |

---

## 7. Monitoring (What "Engagement" Looks Like, Sorted By Value)

| Signal | What it means | Next move |
|---|---|---|
| Personal reply or quote-tweet from @ylecun | High signal, conversation is open | Reply within 1h with a 90-sec loom or a paper link. Move to email. |
| Like from @ylecun | Low signal, but he saw it | Don't reply. Post a follow-up thread in 7 days. |
| Reply / quote from @sainingxie, @MichaelGRabbat, @kchonyc | High signal — closer to AMI than LeCun himself | Email them directly within 24h with the one-pager. |
| AMI recruiter contact via LinkedIn | High signal | Take the call. Show repo + spec, not resume. |
| Cho replies to email | Best practical outcome | Schedule the brown-bag. Bring a printed one-pager for everyone. |
| Lex Fridman likes / replies | Career-changing | Pitch him a podcast slot featuring the architecture, not yourself. |

---

## 8. Why Context Graph Specifically Lands With Him

LeCun's vision paper *A Path Towards Autonomous Machine Intelligence* describes six modules: configurator, perception, world model, **cost**, **short-term memory**, **actor**. The two italicized are what most labs handwave. Context Graph is *literally* an implementation of the short-term-memory + persistent-memory layer with a built-in cost-aware retrieval policy (state-conditioned weight routing).

The pitch isn't *"my project is JEPA"* — it's *"my project is the storage and retrieval module that any JEPA-trained encoder can plug into, and here's working Rust to prove it."* That framing is rare, complementary, and exactly the surface area an early-stage research lab building world models actually needs.

That is the angle of least resistance.

---

## Sources

- [AMI Labs official site](https://amilabs.xyz/) — pillars, team, contact, mission
- [AMI Labs jobs board](https://jobs.ashbyhq.com/ami) — 5 open roles incl. AMI Scientist + AMI Engineer
- [AMI Scientist job description](https://jobs.ashbyhq.com/ami/da976389-9875-4eb6-b866-8097baed3178) — exact language LeCun's team uses
- [Yann LeCun NYU profile](https://cims.nyu.edu/people/profiles/LECUN_Yann.html) — `yann@cs.nyu.edu`, 60 Fifth Ave Office 516, 212-998-3283
- [Yann LeCun personal site](http://yann.lecun.com/)
- [Yann LeCun on X (@ylecun)](https://x.com/ylecun) — primary channel
- [Yann LeCun on Threads (@yannlecun)](https://www.threads.com/@yannlecun) — 301K followers
- [Yann LeCun on LinkedIn](https://www.linkedin.com/in/yann-lecun/)
- [Yann LeCun on Facebook](https://www.facebook.com/yann.lecun/)
- [Yann LeCun GitHub (@ylecun)](https://github.com/ylecun)
- [CILVR Lab at NYU](https://wp.nyu.edu/cilvr/) — co-founded by LeCun + Fergus, weekly seminar
- [Kyunghyun Cho's site](https://kyunghyuncho.me/) — co-leads CILVR + Global AI Frontier Lab with LeCun
- [LeWorldModel paper (arxiv 2603.19312, March 2026)](https://arxiv.org/abs/2603.19312) — Maes / Le Lidec / Scieur / LeCun / Balestriero
- [VL-JEPA paper (arxiv 2512.10942, Feb 2026)](https://arxiv.org/abs/2512.10942) — Chen / … / LeCun
- [I-JEPA Meta announcement](https://ai.meta.com/blog/yann-lecun-ai-model-i-jepa/)
- [V-JEPA Meta announcement](https://ai.meta.com/blog/v-jepa-yann-lecun-ai-model-video-joint-embedding-predictive-architecture/)
- [Lex Fridman Podcast #416 transcript](https://lexfridman.com/yann-lecun-3-transcript/) — most recent long-form conversation
- [MIT Tech Review: AMI Labs profile (Jan 2026)](https://www.technologyreview.com/2026/01/22/1131661/yann-lecuns-new-venture-ami-labs/)
- [TechCrunch: AMI Labs $1.03B seed (March 2026)](https://techcrunch.com/2026/03/09/yann-lecuns-ami-labs-raises-1-03-billion-to-build-world-models/)
- [TechCrunch: who's behind AMI Labs](https://techcrunch.com/2026/01/23/whos-behind-ami-labs-yann-lecuns-world-model-startup/)
- [LeCun X post unveiling AMI seed](https://x.com/ylecun/status/2031268686984527936)
- [ICLR 2026 Board (LeCun listed)](https://iclr.cc/Conferences/2026/Board)
- [NeurIPS 2026 dates](https://neurips.cc/Conferences/2026/Dates) — Sydney / Atlanta / Paris (likely)
- [Princeton: Cracking Cold Emails to Professors](https://pcur.princeton.edu/2025/04/cracking-cold-emails-reaching-out-to-professors-in-a-way-that-works/) — general cold-email best practices
