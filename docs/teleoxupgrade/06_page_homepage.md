# 06 — Homepage Specification

**Route:** `/`
**File:** `src/app/page.tsx`
**Content:** `src/content/routes/home.ts`
**Audience:** Frontier-lab research leadership (primary), independent researchers (secondary)
**Single thing it must communicate:** This is a serious independent research operation; here is the work.

---

## 1. Page meta

| Field | Value |
|---|---|
| `title` | `Teleox.ai — Independent research on verifiable AI training pipelines` |
| `description` | `Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware, with cryptographic chain-of-custody from source to model weights.` |
| `og:title` | `Teleox.ai` |
| `og:description` | `Models can be opaque. Pipelines shouldn't be.` |
| `og:image` | `/og-image.png` (see `15_assets_inventory.md`) |
| `canonical` | `https://teleox.ai/` |

---

## 2. Page structure (top to bottom)

```
[ Header (sticky) ]
[ Hero ]
[ Section 1: Verifiable pipelines, not interpretable models ]
[ Section 2: Training-data bundles as a structured class ]
[ Section 3: Single-GPU empirical floor ]
[ CTAs (repeated below the fold) ]
[ Footer ]
```

Container width: `hero` (1200px max) for the hero block; `reading` (660px max) for the body sections.

---

## 3. Content (verbatim copy)

### 3.1 Eyebrow

```
Independent research
```

### 3.2 Hero headline (h1)

```
Models can be opaque. Pipelines shouldn't be.
```

### 3.3 Hero subhead

```
Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware — from source-data ingest to training bundle to model weights, with cryptographic chain-of-custody at every step.
```

### 3.4 Primary CTA

```
Label: Read current research
Href:  /research
Style: Primary CTA per design system §9.1
```

### 3.5 Secondary CTA

```
Label: Email Chris
Href:  mailto:chrisroyseai@gmail.com?subject=Teleox
Style: Secondary CTA per design system §9.2
```

---

### 3.6 Section 1

**Heading:** `Verifiable pipelines, not interpretable models`

**Body (2 paragraphs):**

```
Mechanistic interpretability of trained models is hard. The verifiability of the
pipelines that produced them does not have to be. Most of the safety,
reproducibility, and provenance properties researchers want from "interpretable
AI" can be obtained at the pipeline layer instead, today, on commodity hardware
— without waiting for representation-level interpretability to mature.

Every operation in a Teleox runtime — data ingest, multi-instrument latent
panel materialization, action-conditioned training, planning, surprise
recording, artifact provenance — writes typed rows into an append-only audit
log. A structurally separate verifier physically reads everything back.
```

---

### 3.7 Section 2

**Heading:** `Training-data bundles as a structured class`

**Body (2 paragraphs + ResultsBox):**

```
Each immutable bundle a runtime produces is itself a structured training-data
artifact. Most bytes are deterministic functions of a public source ZIP; the
small model-output slice is paired with the observed next-state and with
random-init / shuffled-target baselines in the same record; counterfactual
ablations and failed runs are persisted as first-class records.

The argument is that bundles of this shape are a distinct training-data class —
audited execution traces with cryptographic provenance — structurally outside
the recursion regime that produces model collapse. The current paper measures
this on a public-taxonomy benchmark.
```

**ResultsBox (after the two paragraphs):**

```
Career-taxonomy validation cosine: 0.905 ± 0.044 across 5 seeds (BCa 95% CI [0.861, 0.931])
Test cosine: 0.868 ± 0.068 (BCa [0.806, 0.912])
Action-removed ablation: collapses to validation cosine 0.790 — exactly as the action-conditioned predictor design requires.
```

(Reproduce with the existing ResultsBox component; bold the numbers via `<strong>`.)

---

### 3.8 Section 3

**Heading:** `Single-GPU empirical floor`

**Body (1 paragraph):**

```
Every reported number in current research is reproducible from a deterministic
Docker tar plus an Apptainer SIF on a single RTX 5090 (sm_120, CUDA 13.2).
Reviewers requiring source-level access for verification receive time-bounded
private grants. Public companion implementations demonstrate the multi-embedder
MCP architecture on adjacent domains.
```

---

### 3.9 Repeated CTAs (below the fold)

After the third section, repeat the primary + secondary CTAs in a final block before the footer. Same labels, same hrefs, same styling.

This gives a long-scroll reader the same call-to-action they had at the top, without forcing them to scroll back up.

---

## 4. The full `home.ts` RouteCopy file

Replace `src/content/routes/home.ts` with:

```ts
import type { RouteCopy } from "@/types/route-copy";

export const homeCopy: RouteCopy = {
  path: "/",
  purpose:
    "Position Teleox as an independent research operation building verifiability infrastructure for JEPA-style and predictive-AI training pipelines.",
  audience: "primary",
  playerClasses: ["lab", "researcher"],
  eyebrow: "Independent research",

  primaryCta: {
    label: "Read current research",
    href: "/research",
  },
  secondaryCta: {
    label: "Email Chris",
    href: "mailto:chrisroyseai@gmail.com?subject=Teleox",
  },

  signals: ["S1", "S3", "S5"], // verify against existing signal taxonomy; trim if S6 was buyer-only

  scopeGuards: [],

  heroHeadline: "Models can be opaque. Pipelines shouldn't be.",
  heroHeadlineLitWord: "Pipelines shouldn't be",
  heroSubhead:
    "Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware — from source-data ingest to training bundle to model weights, with cryptographic chain-of-custody at every step.",
  heroAlt:
    "Teleox.ai — independent research on verifiable AI training pipelines.",

  bodyBlocks: [
    {
      heading: "Verifiable pipelines, not interpretable models",
      paragraphs: [
        "Mechanistic interpretability of trained models is hard. The verifiability of the pipelines that produced them does not have to be. Most of the safety, reproducibility, and provenance properties researchers want from \"interpretable AI\" can be obtained at the pipeline layer instead, today, on commodity hardware — without waiting for representation-level interpretability to mature.",
        "Every operation in a Teleox runtime — data ingest, multi-instrument latent panel materialization, action-conditioned training, planning, surprise recording, artifact provenance — writes typed rows into an append-only audit log. A structurally separate verifier physically reads everything back.",
      ],
    },
    {
      heading: "Training-data bundles as a structured class",
      paragraphs: [
        "Each immutable bundle a runtime produces is itself a structured training-data artifact. Most bytes are deterministic functions of a public source ZIP; the small model-output slice is paired with the observed next-state and with random-init / shuffled-target baselines in the same record; counterfactual ablations and failed runs are persisted as first-class records.",
        "The argument is that bundles of this shape are a distinct training-data class — audited execution traces with cryptographic provenance — structurally outside the recursion regime that produces model collapse. The current paper measures this on a public-taxonomy benchmark.",
      ],
      // ResultsBox is rendered separately in page.tsx; see §5 below
    },
    {
      heading: "Single-GPU empirical floor",
      paragraphs: [
        "Every reported number in current research is reproducible from a deterministic Docker tar plus an Apptainer SIF on a single RTX 5090 (sm_120, CUDA 13.2). Reviewers requiring source-level access for verification receive time-bounded private grants. Public companion implementations demonstrate the multi-embedder MCP architecture on adjacent domains.",
      ],
    },
  ],

  mustNotSay: [
    "guarantees AGI",
    "solves the data wall",
    "model collapse is over",
    "synthetic data replacement",
    "revolutionary",
    "revolutionizes",
    "game-changing",
    "first ever",
    "world's first",
    "world-first",
    "disrupt",
    "unleash",
    "transform AI",
    "the future of AI",
    "end of LLMs",
    "AGI is here",
  ],
};

export default homeCopy;
```

---

## 5. The full `page.tsx` file

Replace `src/app/page.tsx` with:

```tsx
import type { Metadata } from "next";
import { homeCopy } from "@/content/routes/home";
import { generateRouteMetadata } from "@/lib/metadata";
import { PageContainer } from "@/components/layout/PageContainer";
import { ResearchHero } from "@/components/hero/ResearchHero";
import { Section } from "@/components/content-blocks/Section";
import { ResultsBox } from "@/components/content-blocks/ResultsBox";

export const metadata: Metadata = generateRouteMetadata(homeCopy, {
  // Override defaults for the homepage
  title: "Teleox.ai — Independent research on verifiable AI training pipelines",
  description: homeCopy.heroSubhead,
});

export default function HomePage() {
  return (
    <PageContainer width="hero">
      <ResearchHero
        eyebrow={homeCopy.eyebrow}
        headline={homeCopy.heroHeadline}
        subhead={homeCopy.heroSubhead}
        primaryCta={homeCopy.primaryCta}
        secondaryCta={homeCopy.secondaryCta}
      />

      <div className="max-w-[660px]">
        <Section heading={homeCopy.bodyBlocks[0].heading}>
          {homeCopy.bodyBlocks[0].paragraphs.map((p, i) => (
            <p key={i}>{p}</p>
          ))}
        </Section>

        <Section heading={homeCopy.bodyBlocks[1].heading}>
          {homeCopy.bodyBlocks[1].paragraphs.map((p, i) => (
            <p key={i}>{p}</p>
          ))}
          <ResultsBox>
            Career-taxonomy validation cosine: <strong>0.905 ± 0.044</strong> across 5 seeds (BCa 95% CI <strong>[0.861, 0.931]</strong>)
            <br />
            Test cosine: <strong>0.868 ± 0.068</strong> (BCa [0.806, 0.912])
            <br />
            Action-removed ablation: collapses to validation cosine <strong>0.790</strong> — exactly as the action-conditioned predictor design requires.
          </ResultsBox>
        </Section>

        <Section heading={homeCopy.bodyBlocks[2].heading}>
          {homeCopy.bodyBlocks[2].paragraphs.map((p, i) => (
            <p key={i}>{p}</p>
          ))}
        </Section>

        <div className="mt-16 md:mt-20 pt-8 border-t border-mauve-fog">
          <ResearchHero
            headline=""
            primaryCta={homeCopy.primaryCta}
            secondaryCta={homeCopy.secondaryCta}
          />
        </div>
      </div>
    </PageContainer>
  );
}
```

(The empty-headline trick at the bottom uses `ResearchHero` to render the CTA pair consistently; if a cleaner pattern is preferred, extract a `CtaRow` component.)

---

## 6. Layout dimensions

| Element | Desktop (≥ 1024px) | Tablet (640–1023px) | Mobile (< 640px) |
|---|---|---|---|
| Hero h1 | 84px | 56px | 40px |
| Hero subhead | 20px | 18px | 18px |
| Section h2 | 40px | 32px | 28px |
| Body p | 18px | 16px | 16px |
| Hero block bottom margin | 96px | 64px | 64px |
| Section top margin (after first) | 56px | 48px | 48px |
| Repeated-CTA block top margin | 80px | 64px | 64px |

---

## 7. What appears above the fold

On a 1280×900 desktop viewport:

- Header (sticky, ~64px tall)
- Eyebrow
- Hero h1 (full)
- Hero subhead (full)
- Both CTAs

The first body section h2 may peek at the bottom; that's acceptable. The user must see the headline + subhead + both CTAs without scrolling.

On a 375×667 mobile viewport:

- Header (~56px tall)
- Eyebrow
- Hero h1 (full, may wrap to 4 lines at 40px)
- Hero subhead (may be 4–5 lines)
- Primary CTA may be the only one above fold; secondary CTA acceptable below

---

## 8. Visual reference (ASCII mockup)

```
┌──────────────────────────────────────────────────────┐
│  Teleox.ai           Research  Paper  About  Contact │ ← Header (sticky)
├──────────────────────────────────────────────────────┤
│                                                       │
│  INDEPENDENT RESEARCH                                 │ ← Eyebrow
│                                                       │
│  Models can be opaque.                                │ ← H1 (84px serif)
│  Pipelines shouldn't be.                              │
│                                                       │
│  Teleox publishes infrastructure that makes           │ ← Subhead (20px)
│  JEPA-style and predictive-AI training pipelines      │
│  verifiable end-to-end on commodity hardware...       │
│                                                       │
│  [ Read current research → ]   [ Email Chris ]        │ ← CTAs
│                                                       │
│  ─────────────────────────────────                    │ ← h2 divider
│                                                       │
│  Verifiable pipelines, not interpretable              │ ← Section 1 H2 (40px)
│  models                                                │
│                                                       │
│  Mechanistic interpretability of trained models       │ ← body
│  is hard. The verifiability of the pipelines...       │
│                                                       │
│  Every operation in a Teleox runtime — data           │
│  ingest, multi-instrument latent panel...             │
│                                                       │
│  ─────────────────────────────────                    │
│                                                       │
│  Training-data bundles as a structured class          │ ← Section 2 H2
│                                                       │
│  Each immutable bundle a runtime produces is...       │
│                                                       │
│  ┌─────────────────────────────────────────────────┐  │
│  │ Career-taxonomy validation cosine: 0.905 ± ...  │  │ ← ResultsBox
│  │ Test cosine: 0.868 ± 0.068 (BCa [0.806, ...])   │  │
│  │ Action-removed ablation: collapses to 0.790...  │  │
│  └─────────────────────────────────────────────────┘  │
│                                                       │
│  The argument is that bundles of this shape are       │
│  a distinct training-data class...                    │
│                                                       │
│  ─────────────────────────────────                    │
│                                                       │
│  Single-GPU empirical floor                           │ ← Section 3 H2
│                                                       │
│  Every reported number in current research is         │
│  reproducible from a deterministic Docker tar...      │
│                                                       │
│  ─────────────────────────────────                    │
│                                                       │
│  [ Read current research → ]   [ Email Chris ]        │ ← Repeated CTAs
│                                                       │
├──────────────────────────────────────────────────────┤
│  Teleox.ai                                            │ ← Footer
│  chrisroyseai@gmail.com                               │
│  Research  Paper  About  Contact  Blog  RSS           │
└──────────────────────────────────────────────────────┘
```

---

## 9. SEO + structured data

The homepage emits an `Organization` JSON-LD blob via the existing `JsonLd` component in `layout.tsx`. Update `src/lib/json-ld.ts` `organizationSchema` to:

```ts
{
  "@context": "https://schema.org",
  "@type": "Organization",
  "name": "Teleox.ai",
  "url": "https://teleox.ai",
  "logo": "https://teleox.ai/icon.png",
  "founder": {
    "@type": "Person",
    "name": "Chris Royse",
    "email": "chrisroyseai@gmail.com",
    "url": "https://teleox.ai/about"
  },
  "description": "Independent research on verifiable AI training pipelines.",
  "sameAs": [
    "https://github.com/ChrisRoyse"
  ]
}
```

The homepage also emits a `WebSite` JSON-LD blob (existing pattern). Update `websiteSchema`:

```ts
{
  "@context": "https://schema.org",
  "@type": "WebSite",
  "name": "Teleox.ai",
  "url": "https://teleox.ai",
  "description": "Models can be opaque. Pipelines shouldn't be."
}
```

---

## 10. Performance budget

| Metric | Target |
|---|---|
| First Contentful Paint | < 1.0s on 4G |
| Largest Contentful Paint | < 1.8s on 4G |
| Cumulative Layout Shift | < 0.05 |
| Total page weight | < 150 KB compressed |
| JavaScript bundle | < 80 KB compressed |
| Lighthouse Performance | ≥ 95 |
| Lighthouse Accessibility | ≥ 95 |
| Lighthouse Best Practices | ≥ 95 |
| Lighthouse SEO | 100 |

The homepage has no images other than the OG image (referenced, not loaded). No heavy JS. All this is achievable.

---

## 11. Validation gate compliance

The page must pass:

| Scanner | Pass criterion |
|---|---|
| `validate-content.mjs` | Route entry exists; signals defined; hero/subhead non-empty |
| `scan-claims.ts` | No claim phrases that lack evidence anchors |
| `scan-public-posture.ts` | No internal-material leak; allow-list clean |
| `scan-scope-guards.ts` | All scope-guard rules pass (none expected) |
| `scan-public-posture` | No "auction" / mechanism-design vocabulary |
| `mustNotSay` enforcement | No phrase in the `mustNotSay` array appears in body |

If a scanner flags new copy, the priority is: (1) verify the copy is actually accurate, (2) if accurate, update the scanner's allow-list (document in `memory/decisions/`).

---

## 12. Test plan

After implementation:

- [ ] `https://teleox.ai/` returns 200
- [ ] Page title contains "Teleox.ai" and "verifiable AI training pipelines"
- [ ] OG image renders correctly when shared on Twitter / LinkedIn / Slack
- [ ] Hero h1 and subhead render at correct sizes on all breakpoints
- [ ] Both CTAs are clickable and route correctly
- [ ] All three section headings render with the spec'd top divider
- [ ] ResultsBox displays the three metric lines with bolded numbers
- [ ] Repeated CTAs at the bottom render identically to the top
- [ ] No console errors
- [ ] No layout shift after font load
- [ ] Lighthouse scores meet targets in §10
- [ ] `npm run scan:all` passes
- [ ] Vitest suite passes
- [ ] Playwright e2e for homepage passes (verifies headline text, CTA hrefs, no console errors)

---

## 13. What this page deliberately does NOT have

- No carousel
- No video embed
- No "trusted by" logo strip
- No animated background
- No newsletter signup form
- No Calendly embed
- No chat bubble
- No "see pricing" CTA
- No "book a demo" CTA
- No "trusted by enterprise" badge row
- No customer testimonials
- No "AI for X" headline rotator

If the implementer is tempted to add any of the above, refer back to the four tests in `01_strategic_positioning.md` §10.
