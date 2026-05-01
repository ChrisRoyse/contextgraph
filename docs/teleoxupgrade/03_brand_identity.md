# 03 — Brand Identity

**Authority over:** the brand name, the wordmark, brand voice in copy, brand "personality."
**Distinct from:** visual design system (colors / type / spacing), which is `04_visual_design_system.md`.

---

## 1. Brand name

**Teleox** (rendered as **Teleox.ai** when the domain is meaningful, otherwise just **Teleox**).

Pronunciation: `TEL-ee-ocks`. The "Tel" rhymes with "tell"; the "eox" mirrors the suffix in "Toledox" / "telos" / Greek-rooted naming patterns.

### 1.1 Etymology and meaning

The name combines:
- **télos** (Greek τέλος) — purpose, end, completion. Used in philosophy of science to describe goal-directed processes (teleology).
- **-ox** — a terminal suffix that is short, hard, and lands cleanly on the page.

The implicit meaning: *purpose-driven, goal-directed AI infrastructure* — the runtime is built to make end-state-defined behavior verifiable.

### 1.2 How to write the name

| Context | Form | Example |
|---|---|---|
| First mention on a page | Teleox.ai | "Teleox.ai publishes research on…" |
| Subsequent mentions | Teleox | "Teleox runtimes are written in Rust." |
| In code / URLs | `teleox.ai` | `https://teleox.ai/research` |
| In `<title>` and metadata | Teleox.ai | `<title>About — Teleox.ai</title>` |
| Logo / wordmark | "Teleox.ai" with the period as part of the mark | (see §2) |
| Possessive | Teleox's | "Teleox's research direction is…" |

Never:
- TELEOX (all caps) — too aggressive
- teleox (all lowercase) — sloppy
- Teleox AI (with space) — wrong
- TeleOx (camelcase) — wrong
- Teleox.AI (dot-AI all caps) — wrong

---

## 2. Wordmark / logo

The wordmark **is** the brand identifier. There is no separate icon-mark logo.

### 2.1 Wordmark specification

```
Teleox.ai
```

- Set in **Inter Variable** at weight `600` (semibold)
- Letter-spacing: `-0.02em` (slightly tightened)
- Font feature settings: `"ss01"` if available (alternative `a` if Inter offers it)
- Color: `--color-cream` on dark surfaces, `--color-ink-1` on light surfaces
- The period `.` between `Teleox` and `ai` is set in `--color-amber` (the brand accent); this is the **only** decorative typographic moment in the entire site

### 2.2 In TSX

```tsx
<Link href="/" className="font-sans font-semibold tracking-tight text-cream text-lg">
  Teleox<span className="text-amber">.</span>ai
</Link>
```

### 2.3 Sizing

| Context | Wordmark size |
|---|---|
| Header nav | 18–20px |
| Footer | 16px |
| OG image | 64px |
| favicon | (separate — see `15_assets_inventory.md`) |

### 2.4 Clearspace

Minimum padding around the wordmark equal to the cap height of the type. No other element may encroach within that space.

### 2.5 What the wordmark must NOT do

- Animate (no glow, pulse, shimmer)
- Combine with a tagline in any locked-up unit (use surrounding copy instead)
- Rotate, skew, or tilt
- Use any color other than `cream`, `ink-1`, or the amber dot
- Be reproduced in a competitor's brand colors (e.g. Anthropic warm-amber-orange)

---

## 3. Brand positioning statement

This is the internal positioning paragraph — it does not appear on the site verbatim, but anchors all copy.

> Teleox is an independent research operation founded by Chris Royse. We publish infrastructure and research artifacts that make JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware, with cryptographic chain-of-custody from source data through training to model weights. We exist to demonstrate that the verifiability properties researchers want from "interpretable AI" can be obtained at the pipeline layer, today, without waiting for representation-level interpretability to mature. We are research-first, commercial-implicit. Our primary audience is research leadership at frontier AI labs.

Use this paragraph to test any new copy. If it contradicts a sentence in here, the new copy is wrong.

---

## 4. Brand voice (the "who is speaking")

The voice on the site is **a single named technically-rigorous independent researcher** — Chris Royse — speaking as "I" in some places (about, contact) and as "Teleox" in institutional contexts (homepage, research, paper).

### 4.1 Voice attributes

| Attribute | Calibration |
|---|---|
| **Confidence** | High — but only when warranted by reported evidence. Hedge precisely; do not bluff |
| **Specificity** | Always. Use numbers, paper titles, file names, hardware names. Never vague |
| **Self-effacement** | Use sparingly: "this is not a representational contribution" — when scoping the work honestly |
| **Humor** | Almost never. Dry, only when zero-cost: "failed runs are persisted as first-class records, not discarded" |
| **Sentiment** | Neutral-to-positive. No excitement signaling, no emojis, no exclamation points |
| **First person plural ("we")** | Permitted on institutional pages (homepage, research). One-author-stand-in, not feigning a team |
| **First person singular ("I")** | Permitted on author-facing pages (about). When Chris speaks |
| **Second person ("you")** | Use sparingly, in invitational contexts ("If forwarding to anyone in CILVR is useful, please do") |

### 4.2 Sentence-level patterns

GOOD:
> "Each immutable bundle the runtime produces is itself a structured training-data artifact."

(Specific noun, present-tense, no adjectives.)

GOOD:
> "Career-taxonomy validation cosine 0.905 ± 0.044 across 5 seeds (BCa CI [0.861, 0.931])."

(Reported evidence, named metric, confidence interval.)

GOOD:
> "The contribution lives in the bundle format, not the predictor weights."

(Honest scoping, names what it is and is not.)

BAD:
> "Teleox is unlocking the next generation of trustworthy AI."

(Marketing verb, vague noun, generational-claim hubris.)

BAD:
> "Our cutting-edge platform empowers AI labs to extract maximum value from their data."

(Hype adjectives, "platform," "extract value," "data" as marketing object.)

BAD:
> "We're excited to share that Teleox has solved the data wall."

(Excited, "solved," absolute claim.)

---

## 5. Brand personality

If Teleox were a person, they would be:

- A 38-year-old independent researcher who used to work at one of the big labs
- Reads arxiv every morning over coffee
- Has strong opinions, rarely shares them on Twitter
- Owns one nice serif typeface and uses it everywhere
- Writes long emails on careful occasions, short emails the rest of the time
- Would rather show you a graph than give you a slide deck
- Doesn't own a hoodie with a company logo

Reference brand archetypes:
- Stripe Press
- Distill.pub
- transformer-circuits.pub
- Chris Olah's blog (back when it was active)
- Yann LeCun's NYU page (the very minimal version)
- Christopher Ré's Stanford CRFM page

Anti-archetypes:
- Any series-B AI startup with a gradient-purple brand
- Any "AI for X" landing page with stock photos of glowing brains
- Any "we're hiring rocket-scientists" hiring page with bouncy animations

---

## 6. Brand voice across surfaces

| Surface | Voice mode | Person | Example |
|---|---|---|---|
| Homepage hero | Institutional, declarative | "Teleox" / "we" | "Teleox publishes infrastructure that makes…" |
| Homepage body | Institutional, technical | "Teleox" / "we" | "Each immutable bundle the runtime produces is itself a structured training-data artifact." |
| `/research` page | Institutional, descriptive | "Teleox" / "we" + paper-author "Chris Royse" by name | "The current draft preprint, *Dynamic / ME-JEPA*, asks…" |
| `/paper` page | Plain-language explanatory | "Teleox" institutional + named paper author | "The paper (Royse, 2026) reports…" |
| `/about` page | Personal, narrative | "I" (Chris) | "I started this work because…" |
| `/contact` page | Direct, transactional | "I" (Chris) | "Reach me at chrisroyseai@gmail.com." |
| Footer | Minimal, factual | None | "Teleox.ai · chrisroyseai@gmail.com · 2026" |
| Error pages | Brief, not-cute | None | "This page does not exist. Try /research." |

---

## 7. Brand voice across recipient-types

The site does not personalize by visitor (we have no analytics-driven personalization). But the voice anticipates how different visitor types should read each page.

| Visitor type | What they see | What they should feel |
|---|---|---|
| Frontier-lab principal (LeCun, Olah, Hassabis, etc.) | Restrained homepage, real numbers, real artifact | "This is serious. I'll spend 90 seconds." |
| Frontier-lab researcher (postdoc, RE) | Same homepage, paper one click away | "I should send this to my advisor / DM it to a colleague." |
| Independent academic | Same homepage, clipcannon as engineering proof | "There's a real engineer behind this." |
| AI Twitter person | Lands via shared link, hopefully on the paper | "There's no clickbait here, but the paper is interesting." |
| Recruiter | Hits about, sees solo principal | "OK, this is one person. Move on." (this is fine — recruiters are not the audience) |
| Random VC | Hits homepage, no Calendly, no pitch | "There's no funnel here." (this is fine — VCs are not the audience) |
| Existing paying customer | (none — pre-revenue) | n/a |

---

## 8. Tagline

There is no tagline locked up with the wordmark. The hero headline serves the tagline function on the homepage and changes if positioning evolves.

**Current hero headline (which serves as informal tagline):**

> *"Models can be opaque. Pipelines shouldn't be."*

This sentence is the closest thing Teleox has to a tagline. It can appear:
- As the homepage h1
- In an OG image (see `15_assets_inventory.md`)
- In `meta description`
- In `humans.txt`

It should NOT appear:
- In the page header / nav (cluttered)
- In the footer (overuse)
- Locked up with the wordmark in any logo unit

---

## 9. Brand-adjacent properties

### 9.1 leapable.ai

A separate property the principal also operates. Mentioned on `/about` only. Not in nav. Not on homepage.

The phrasing on `/about` should be:

> "Chris also operates leapable.ai, a public AI memory product on the same underlying infrastructure."

(Linked as `https://leapable.ai`.) That's the entire mention.

### 9.2 Chris Royse personal sites

If Chris has a personal academic page, link to it from `/about`. Otherwise the about page IS the personal page within Teleox.

### 9.3 GitHub

`github.com/ChrisRoyse` and the `clipcannon` repo are linked from `/research` and `/about`. Not from homepage. Not from nav.

---

## 10. Brand assets inventory

| Asset | Location | Spec |
|---|---|---|
| Wordmark | Inline TSX (no SVG file needed) | See §2 |
| Favicon | `public/favicon.ico` (existing — keep) | Existing — keep |
| Apple touch icon | `public/apple-touch-icon.png` | Existing — keep |
| Manifest icons | `public/icon.png`, `public/icon.svg` | Existing — keep |
| OG image | `public/og-image.png` | NEW — see `15_assets_inventory.md` |
| Logo PNG (for press kits, etc.) | NOT NEEDED — the wordmark is the only logo |

---

## 11. Brand violations to actively avoid

| Violation | Why it's bad |
|---|---|
| Swapping the amber period for any other color | The amber dot is the *one* place the brand has personality; changing it dilutes what little decoration exists |
| Adding a slogan / tagline below the wordmark in nav | Visual clutter, signals "startup" |
| Animating the wordmark | Anti-research |
| Using a stock-photo hero (people, abstract gradients, brain illustrations) | All instantly read as marketing |
| Putting customer logos on the homepage ("trusted by Anthropic, Google, …") | Even if true, signals SaaS |
| Adding a chat bubble (Intercom, Drift) | Signals SaaS |
| Adding "Sign in" / "Sign up" anywhere | Signals SaaS |
| Using rounded-corner radius > 8px on cards | Visual mismatch with Ember palette and serif typography |
| Using emojis in body copy | Signals casual; Anthropic-orbit reads as low-effort |

---

## 12. Brand verification checklist

After implementation, walk through every public page and check:

- [ ] Wordmark renders as `Teleox<amber>.</amber>ai` in nav
- [ ] No emoji anywhere in body copy
- [ ] No "transform / unleash / disrupt / revolutionary / first-ever" anywhere
- [ ] No exclamation points anywhere
- [ ] No customer logos
- [ ] No newsletter signup
- [ ] No "Sign in" / "Sign up"
- [ ] No chat bubble overlay
- [ ] No stock photos of brains, networks, or abstract gradients
- [ ] First-person reference is "I" only on about and contact; "we" on research, paper, homepage
- [ ] Email link uses `chrisroyseai@gmail.com` consistently
- [ ] All retired-route links removed
