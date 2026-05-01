# LeCun X Timeline Scout — 2026-05-01

**Source:** logged-in playwright snapshot of `https://x.com/ylecun`, captured ~17:25 UTC.
**Posts visible:** 12 (pinned + 11 timeline).
**Date range:** Apr 28 – May 1, 2026.

---

## Critical finding — the pinned post changes the playbook

> **@ylecun pinned (Nov 29, 2024):** "I do not write posts on X. I tweet links to posts on other platforms. I like and retweet (occasionally). I comment on friends' tweets (rarely). Follow me on... ⬇️⬇️⬇️"
> *(858 replies, 194 reposts, 1.7K likes, 1.5M views)*

**Implication for outreach:**
- His X account is a *broadcast amplifier*, not a conversation venue.
- An X thread tagging him (Tier 1.1 in the original strategy) has lower expected return than first assumed — he barely engages with X content he didn't already curate.
- His **original** content lives on LinkedIn, Threads, Facebook, and his personal site. The strategy should weight those higher.
- BUT: he *does* repost technical content from his orbit. **Getting reposted by someone he follows is the X path that actually reaches him.**

---

## Timeline content breakdown (12 posts visible)

| # | Date | Type | Topic | Engagement | LeCun-orbit fit |
|---|---|---|---|---|---|
| Pinned | Nov 29, 2024 | Original | "I don't write posts on X" meta-statement | 1.7K likes, 1.5M views | n/a — meta-rule |
| 1 | 10h ago | Repost (Daniel Jeffries) | Open-source defense, anti-distillation-ban | 106 likes | Open source theme — high amplification |
| 2 | 20h ago | Repost (Clement Delangue / HuggingFace) | "Pulling the ladder" — labs banning distillation after benefiting | 294 likes | Open source theme |
| 3 | Apr 30 | Repost (Daniel Jeffries) | Anti open-source ban | 107 likes | Same theme |
| 4 | **Apr 30** | **Repost (Jean-Rémi King)** | **Tribe v2 — Meta brain-response model demo** | **293 likes, 30K views** | **⭐ technical multi-modal** |
| 5 | Apr 29 | Repost (Captain Mark Kelly) | NSF Board firing | 814 likes | Politics |
| 6 | Apr 29 | Repost (Republicans against Trump) | FCC license threats | 13K likes, 532K views | Politics |
| 7 | Apr 29 | Repost (Kenneth Roth) | Politics | 199 likes | Politics |
| 8 | Apr 28 | Repost (Jessica Riedl) | Politics | 1.4K likes | Politics |
| 9 | **Apr 29** | **Original by @ylecun** | **"Trump's war on science" + Nature analysis on cut science panels** | 794 likes, 47K views | Politics-adjacent (science) |
| 10 | **Apr 28** | **Repost (Jitendra Malik @JitendraMalikCV)** | **"World models has a technical meaning - the transition model/dynamics model from Bellman/Kalman in the context of MDPs/state space approach to control theory ~1960. I gave a talk on this history."** | **288 likes, 55K views, 266 bookmarks** | **⭐⭐⭐⭐⭐ EXACT MATCH** |
| 11 | Apr 29 | Repost (James Pethokoukis) | "Radiologists make $500K/yr" — AI didn't replace them | 1.4K likes, 417K views | Economics/AI |

**Topic mix:** ~60% politics, ~30% open-source/distillation, **~10% direct technical content (only 2 posts in 4 days)**.

**Right now is NOT a high-signal window for technical pitches via X.** The political density means any reply you make will compete with hundreds of political-debate replies for his attention. But the *one* technical hook is gold.

---

## The single best engagement target — Malik's world-models thread

**Original post (Apr 28, ~JitendraMalikCV):**
> "Replying to @jon_barron — 'World models' has a technical meaning - the transition model/dynamics model from Bellman/Kalman in the context of MDPs/state space approach to control theory ~ 1960. I gave a talk on this history youtube.com..."

**Why this is the goldmine:**
1. LeCun reposted it (so he's already engaged with this exact framing)
2. Malik defines world models as **action-conditioned state-space dynamics models** — *exactly* the framing your ME-JEPA paper uses (`H=1` action-conditioned prediction, action-contrast accuracy, A3 ablation that removes the action and watches the model collapse)
3. 266 bookmarks suggests technical readers are saving it for reference — anyone who replies substantively gets seen by a high-quality cohort
4. Malik is at UC Berkeley — different orbit but adjacent. A reply on his thread that LeCun reposted threads the needle perfectly: you're in the conversation without cold-replying to LeCun himself
5. Malik is more likely to reply than LeCun is. If Malik replies → LeCun sees the exchange.

**Drafted reply (one variant — for your approval):**

> The ~1960 dynamics-model framing is exactly the angle that made our recent JEPA runtime experiment land: H=1 action-conditioned next-state prediction in a frozen-embedder latent panel, with the action-removed ablation as the diagnostic. On a career-taxonomy domain we see validation cosine 0.905 ± 0.044 across 5 seeds (BCa [0.861, 0.931]) collapse cleanly to 0.790 when the action is removed, exactly as the state-space framing predicts. Draft preprint if useful — happy to share privately.
>
> chrisroyseai@gmail.com

(Under 280 if you trim — but X allows longer for verified accounts. Check your account verification status.)

---

## Strategy adjustments

| Original strategy | Revised based on findings |
|---|---|
| Tier 1.1: Big X thread tagging @ylecun once | **DOWNGRADED** — he doesn't read original X content. Threads/LinkedIn cross-posts higher value |
| Tier 1.3: Reply to his X posts | **REFOCUSED** — reply to *original* posts that he reposted, not his reposts themselves. Catches him via thread notifications |
| New: Malik's world-models thread | **NEW TIER 1A** — single highest-fit engagement opportunity in the visible timeline |
| Threads / LinkedIn | **UPGRADED** — these are where his original content lives, per his own pinned admission |

---

## Recommended next moves (in order)

1. **Send the LeCun email** (already drafted: `email_lecun.md`) — this remains Tier 1 priority because the privileged-access framing works regardless of his X behavior.
2. **Reply to Malik's thread** (draft above) — costs 5 minutes, very high upside, low downside. Use this as the "you may have seen me on Malik's thread" hook in subsequent emails if needed.
3. **Send the Cho email 48-72h later** (already drafted: `email_cho.md`) — Cho is the warmest intra-NYU intermediary.
4. **Then** consider Threads/LinkedIn original posts referencing the Malik discussion — by that point if any of (1)–(3) hit, you have a much stronger second-touch position.
5. **Skip the X thread idea entirely for now** — wait for a denser technical-post window before posting an original thread tagging him.

---

## What I did NOT see (but worth checking)

- His Threads timeline (`@yannlecun` on threads.com — 301K followers, more original content per his pinned admission)
- His LinkedIn (`linkedin.com/in/yann-lecun` — long-form posts on AMI / JEPA)
- His Facebook public posts (older medium, but he still posts there)
- Recent "likes" on his X account (sometimes more revealing than reposts — shows what he reads but doesn't amplify)

If next iteration needed, scout Threads first — that's where the real conversation surface is.
