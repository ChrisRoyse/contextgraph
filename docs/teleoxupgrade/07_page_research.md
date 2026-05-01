# 07 — `/research` Page Specification

**Route:** `/research`
**File:** `src/app/research/page.tsx`
**Content:** `src/content/routes/research.ts` (NEW — does not yet exist)
**Audience:** Researchers wanting to evaluate the current artifacts
**Single thing it must communicate:** "Here is the work, listed and reachable."

---

## 1. Page meta

| Field | Value |
|---|---|
| `title` | `Research — Teleox.ai` |
| `description` | `Current and recent research artifacts published by Teleox: papers, reproducibility releases, and open-source companion implementations.` |
| `og:title` | `Teleox Research` |
| `og:description` | `Models can be opaque. Pipelines shouldn't be.` |
| `og:image` | `/og-image.png` |
| `canonical` | `https://teleox.ai/research` |

---

## 2. Page structure (top to bottom)

```
[ Header (sticky) ]
[ Hero (eyebrow + h1 + subhead) ]
[ Section: Current research ]
   [ ArtifactCard: ME-JEPA paper ]   [ ArtifactCard: ClipCannon ]
[ Section: How to access non-public artifacts ]
[ Section: Research direction ]
[ CTAs (Email Chris + Browse paper) ]
[ Footer ]
```

Container width: `standard` (1024px max).

---

## 3. Content (verbatim copy)

### 3.1 Eyebrow

```
Research index
```

### 3.2 H1

```
Current research and artifacts
```

### 3.3 Subhead

```
The work in progress at Teleox, listed in one place. Some artifacts are public; some are private previews shared with select reviewers. All numbers reported in published material are reproducible on commodity hardware.
```

---

### 3.4 Section: Current research

**Heading:** `Current research`

**Body (1 short paragraph before cards):**

```
Two artifacts cover the current research direction. The first is a JEPA-runtime
paper not yet posted publicly; the second is the open-source multi-embedder
companion that demonstrates the architecture in a different domain.
```

**Then a two-column grid of ArtifactCards:**

#### ArtifactCard 1 — ME-JEPA paper

```
eyebrow: Draft preprint
title:   Dynamic / ME-JEPA
oneLine: An audited, domain-portable JEPA runtime on a single RTX 5090. Headline:
         career-taxonomy validation cosine 0.905 ± 0.044 across 5 seeds
         (BCa CI [0.861, 0.931]); the action-removed ablation collapses to 0.790.
meta:    18 pages · 594 KB · May 2026 · Release candidate v1.0.0-rc5
status:  private-preview
cta:
  label: Request preview access
  href:  mailto:chrisroyseai@gmail.com?subject=ME-JEPA preview access
```

#### ArtifactCard 2 — ClipCannon

```
eyebrow: Open source
title:   ClipCannon
oneLine: MCP server for video understanding with 5 frozen embedders. Same
         multi-embedder architecture as the private ME-JEPA runtime, applied
         to a different domain.
meta:    Rust + Python · BSL 1.1 → Apache 2.0 (2030) · GitHub
status:  open-source
cta:
  label: github.com/ChrisRoyse/clipcannon
  href:  https://github.com/ChrisRoyse/clipcannon
  external: true
```

---

### 3.5 Section: How to access non-public artifacts

**Heading:** `Private-preview access`

**Body (3 short paragraphs):**

```
The Dynamic / ME-JEPA Rust runtime is currently maintained as a private
codebase. Reviewers, researchers, and lab members requiring source-level access
for verification can request a time-bounded private grant by writing to
chrisroyseai@gmail.com.

The reproducibility artifact for the current paper — a deterministic Docker tar
plus an Apptainer SIF, with per-seed bundles whose contents the paper's tables
are read directly from — is the canonical reproducible release. A reviewer with
an RTX 5090 host matching the environment fingerprint described in the paper
can reproduce every headline number from the artifact alone, without source-code
access.

The career-taxonomy data substrate derives from O*NET 30.2 (CC BY 4.0); the
source ZIP SHA-256 and per-edge walk hash are recorded in every generated
bundle and are deterministically reproducible from the public source.
```

---

### 3.6 Section: Research direction

**Heading:** `What this research is about`

**Body (3 short paragraphs):**

```
Teleox publishes infrastructure that makes JEPA-style and predictive-AI training
pipelines verifiable end-to-end. The current direction has three working axes.

The first axis is pipeline verifiability — making every step from source-data
ingest through training and planning physically auditable on commodity hardware.
The current paper develops a Full-State Verification (FSV) protocol where a
structurally separate verifier physically re-reads every persisted output as an
independent code path.

The second axis is training-data structure — treating the bundles produced by an
audited runtime as a distinct training-data class, with cryptographic chain-of-
custody from public source ZIPs through to model weights. The third axis is
single-GPU reproducibility, ensuring that external verification is feasible
without lab-scale compute.
```

---

### 3.7 CTA pair (footer of page, before the global footer)

```
Primary:   Email Chris    →   mailto:chrisroyseai@gmail.com?subject=Teleox research
Secondary: See the paper  →   /paper
```

---

## 4. The full `research.ts` RouteCopy file

Create `src/content/routes/research.ts`:

```ts
import type { RouteCopy } from "@/types/route-copy";

export const researchCopy: RouteCopy = {
  path: "/research",
  purpose:
    "Surface the current research artifacts (ME-JEPA paper, ClipCannon) and provide a path to private-preview access.",
  audience: "primary",
  playerClasses: ["lab", "researcher"],
  eyebrow: "Research index",

  primaryCta: {
    label: "Email Chris",
    href: "mailto:chrisroyseai@gmail.com?subject=Teleox research",
  },
  secondaryCta: {
    label: "See the paper",
    href: "/paper",
  },

  signals: ["S1", "S3", "S5"],
  scopeGuards: [],

  heroHeadline: "Current research and artifacts",
  heroHeadlineLitWord: "Current research",
  heroSubhead:
    "The work in progress at Teleox, listed in one place. Some artifacts are public; some are private previews shared with select reviewers. All numbers reported in published material are reproducible on commodity hardware.",
  heroAlt: "Teleox research index — current artifacts.",

  bodyBlocks: [
    {
      heading: "Current research",
      paragraphs: [
        "Two artifacts cover the current research direction. The first is a JEPA-runtime paper not yet posted publicly; the second is the open-source multi-embedder companion that demonstrates the architecture in a different domain.",
      ],
    },
    {
      heading: "Private-preview access",
      paragraphs: [
        "The Dynamic / ME-JEPA Rust runtime is currently maintained as a private codebase. Reviewers, researchers, and lab members requiring source-level access for verification can request a time-bounded private grant by writing to chrisroyseai@gmail.com.",
        "The reproducibility artifact for the current paper — a deterministic Docker tar plus an Apptainer SIF, with per-seed bundles whose contents the paper's tables are read directly from — is the canonical reproducible release. A reviewer with an RTX 5090 host matching the environment fingerprint described in the paper can reproduce every headline number from the artifact alone, without source-code access.",
        "The career-taxonomy data substrate derives from O*NET 30.2 (CC BY 4.0); the source ZIP SHA-256 and per-edge walk hash are recorded in every generated bundle and are deterministically reproducible from the public source.",
      ],
    },
    {
      heading: "What this research is about",
      paragraphs: [
        "Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end. The current direction has three working axes.",
        "The first axis is pipeline verifiability — making every step from source-data ingest through training and planning physically auditable on commodity hardware. The current paper develops a Full-State Verification (FSV) protocol where a structurally separate verifier physically re-reads every persisted output as an independent code path.",
        "The second axis is training-data structure — treating the bundles produced by an audited runtime as a distinct training-data class, with cryptographic chain-of-custody from public source ZIPs through to model weights. The third axis is single-GPU reproducibility, ensuring that external verification is feasible without lab-scale compute.",
      ],
    },
  ],

  mustNotSay: [
    "guarantees AGI",
    "solves the data wall",
    "model collapse is over",
    "synthetic data replacement",
    "revolutionary",
    "first ever",
    "world's first",
    "the future of AI",
  ],
};

export default researchCopy;
```

---

## 5. The full `page.tsx` file

Create `src/app/research/page.tsx`:

```tsx
import type { Metadata } from "next";
import { researchCopy } from "@/content/routes/research";
import { generateRouteMetadata } from "@/lib/metadata";
import { PageContainer } from "@/components/layout/PageContainer";
import { ResearchHero } from "@/components/hero/ResearchHero";
import { Section } from "@/components/content-blocks/Section";
import { ArtifactCard } from "@/components/content-blocks/ArtifactCard";

export const metadata: Metadata = generateRouteMetadata(researchCopy, {
  title: "Research — Teleox.ai",
  description:
    "Current and recent research artifacts published by Teleox: papers, reproducibility releases, and open-source companion implementations.",
});

export default function ResearchPage() {
  return (
    <PageContainer width="standard">
      <ResearchHero
        eyebrow={researchCopy.eyebrow}
        headline={researchCopy.heroHeadline}
        subhead={researchCopy.heroSubhead}
      />

      <Section heading={researchCopy.bodyBlocks[0].heading}>
        {researchCopy.bodyBlocks[0].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-8">
        <ArtifactCard
          eyebrow="Draft preprint"
          title="Dynamic / ME-JEPA"
          oneLine="An audited, domain-portable JEPA runtime on a single RTX 5090. Headline: career-taxonomy validation cosine 0.905 ± 0.044 across 5 seeds (BCa CI [0.861, 0.931]); the action-removed ablation collapses to 0.790."
          meta="18 pages · 594 KB · May 2026 · Release candidate v1.0.0-rc5"
          status="private-preview"
          cta={{
            label: "Request preview access",
            href: "mailto:chrisroyseai@gmail.com?subject=ME-JEPA preview access",
          }}
        />

        <ArtifactCard
          eyebrow="Open source"
          title="ClipCannon"
          oneLine="MCP server for video understanding with 5 frozen embedders. Same multi-embedder architecture as the private ME-JEPA runtime, applied to a different domain."
          meta="Rust + Python · BSL 1.1 → Apache 2.0 (2030) · GitHub"
          status="open-source"
          cta={{
            label: "github.com/ChrisRoyse/clipcannon",
            href: "https://github.com/ChrisRoyse/clipcannon",
            external: true,
          }}
        />
      </div>

      <Section heading={researchCopy.bodyBlocks[1].heading}>
        {researchCopy.bodyBlocks[1].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <Section heading={researchCopy.bodyBlocks[2].heading}>
        {researchCopy.bodyBlocks[2].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <div className="mt-16 md:mt-20 pt-8 border-t border-mauve-fog">
        <ResearchHero
          headline=""
          primaryCta={researchCopy.primaryCta}
          secondaryCta={researchCopy.secondaryCta}
        />
      </div>
    </PageContainer>
  );
}
```

---

## 6. Wire into route exports

Add to `src/content/routes/index.ts`:

```ts
export { researchCopy } from "./research";
```

Add to `src/lib/canonical-routes.mjs` (the JS validator-side list — verify the exact format):

```js
"/research",  // include in CANONICAL_ROUTES export
```

Add to `src/app/sitemap.ts` ROUTES array (already specified in `02_information_architecture.md`).

---

## 7. Visual reference (ASCII mockup, desktop)

```
┌───────────────────────────────────────────────────────────────┐
│  Teleox.ai           Research  Paper  About  Contact          │
├───────────────────────────────────────────────────────────────┤
│                                                                │
│  RESEARCH INDEX                                                │
│                                                                │
│  Current research and artifacts                                │ ← H1
│                                                                │
│  The work in progress at Teleox, listed in one place. Some     │
│  artifacts are public; some are private previews shared with   │
│  select reviewers. All numbers reported in published material  │
│  are reproducible on commodity hardware.                       │
│                                                                │
│  ────────────────────────────────────────────────────────      │
│                                                                │
│  Current research                                              │ ← H2
│                                                                │
│  Two artifacts cover the current research direction. The       │
│  first is a JEPA-runtime paper not yet posted publicly; the    │
│  second is the open-source multi-embedder companion that       │
│  demonstrates the architecture in a different domain.          │
│                                                                │
│  ┌──────────────────────────┐  ┌──────────────────────────┐   │
│  │ DRAFT PREPRINT · PRIVATE │  │ OPEN SOURCE · PUBLIC     │   │
│  │                          │  │                          │   │
│  │ Dynamic / ME-JEPA        │  │ ClipCannon               │   │
│  │                          │  │                          │   │
│  │ An audited, domain-      │  │ MCP server for video     │   │
│  │ portable JEPA runtime... │  │ understanding with 5     │   │
│  │                          │  │ frozen embedders. Same   │   │
│  │ 18 pages · 594 KB · May  │  │ multi-embedder...        │   │
│  │ 2026 · RC v1.0.0-rc5     │  │                          │   │
│  │                          │  │ Rust + Python · BSL 1.1  │   │
│  │ Request preview access → │  │ github.com/ChrisRoyse/   │   │
│  │                          │  │ clipcannon  ↗            │   │
│  └──────────────────────────┘  └──────────────────────────┘   │
│                                                                │
│  ────────────────────────────────────────────────────────      │
│                                                                │
│  Private-preview access                                        │ ← H2
│                                                                │
│  The Dynamic / ME-JEPA Rust runtime is currently maintained    │
│  as a private codebase. Reviewers... can request a time-       │
│  bounded private grant by writing to chrisroyseai@gmail.com.   │
│                                                                │
│  The reproducibility artifact for the current paper — a        │
│  deterministic Docker tar plus an Apptainer SIF...             │
│                                                                │
│  The career-taxonomy data substrate derives from O*NET...      │
│                                                                │
│  ────────────────────────────────────────────────────────      │
│                                                                │
│  What this research is about                                   │ ← H2
│                                                                │
│  Teleox publishes infrastructure that makes JEPA-style and     │
│  predictive-AI training pipelines verifiable end-to-end...     │
│                                                                │
│  ────────────────────────────────────────────────────────      │
│                                                                │
│  [ Email Chris → ]   [ See the paper ]                         │
│                                                                │
├───────────────────────────────────────────────────────────────┤
│  Footer                                                        │
└───────────────────────────────────────────────────────────────┘
```

---

## 8. Mobile layout

On mobile (< 768px), the two-up grid stacks into one column. Card content is identical; spacing tightens via the `md:` breakpoint utilities.

---

## 9. SEO / structured data

The `/research` page emits a `CollectionPage` JSON-LD blob (extension to `src/lib/json-ld.ts`):

```ts
{
  "@context": "https://schema.org",
  "@type": "CollectionPage",
  "name": "Research — Teleox.ai",
  "description": "Current and recent research artifacts.",
  "url": "https://teleox.ai/research",
  "isPartOf": {
    "@type": "WebSite",
    "name": "Teleox.ai",
    "url": "https://teleox.ai"
  },
  "hasPart": [
    {
      "@type": "ScholarlyArticle",
      "name": "Dynamic / ME-JEPA: Source-of-Truth Verified Latent World Modeling on a Single RTX 5090",
      "author": {
        "@type": "Person",
        "name": "Chris Royse",
        "email": "chrisroyseai@gmail.com"
      },
      "datePublished": "2026-05",
      "isAccessibleForFree": false
    },
    {
      "@type": "SoftwareSourceCode",
      "name": "ClipCannon",
      "codeRepository": "https://github.com/ChrisRoyse/clipcannon",
      "license": "https://opensource.org/licenses/BSL-1.1"
    }
  ]
}
```

---

## 10. Test plan

- [ ] `https://teleox.ai/research` returns 200
- [ ] Page renders the hero, intro paragraph, and two ArtifactCards
- [ ] ArtifactCards render in a two-column grid on desktop, single column on mobile
- [ ] "Request preview access" CTA opens email client with correct subject
- [ ] "github.com/ChrisRoyse/clipcannon" CTA opens in new tab
- [ ] Three section headings render with the spec'd top divider
- [ ] Bottom CTA pair renders correctly
- [ ] No console errors
- [ ] Lighthouse Performance ≥ 95
- [ ] `npm run scan:all` passes
- [ ] Vitest passes for the new ArtifactCard component
- [ ] Playwright e2e for `/research` passes

---

## 11. What this page deliberately does NOT have

- No paper preview iframe (the PDF is reached via the per-recipient slug or email request, not directly)
- No "View full paper" inline expand
- No author photo
- No "subscribe to research updates" form
- No comments
- No related-work auto-suggestions
- No "people who viewed this also viewed"
- No download counter
- No social share buttons
