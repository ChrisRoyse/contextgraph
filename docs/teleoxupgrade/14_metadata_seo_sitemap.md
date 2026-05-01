# 14 — Metadata, SEO, Sitemap, JSON-LD

**Authority over:** every meta tag, every JSON-LD blob, sitemap.xml, robots.txt, llms.txt, OG image references.

---

## 1. Root metadata in `layout.tsx`

The current `layout.tsx` has a metadata block. Update the relevant fields:

```tsx
// src/app/layout.tsx
export const metadata: Metadata = {
  metadataBase: new URL("https://teleox.ai"),
  applicationName: "Teleox.ai",
  title: {
    default: "Teleox.ai — Independent research on verifiable AI training pipelines",
    template: "%s — Teleox.ai",
  },
  description:
    "Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware, with cryptographic chain-of-custody from source to model weights.",
  keywords: [
    "JEPA",
    "joint-embedding predictive architecture",
    "world models",
    "AI training infrastructure",
    "verifiable AI",
    "training data provenance",
    "audited execution traces",
    "process supervision",
    "single-GPU reproducibility",
    "ME-JEPA",
  ],
  authors: [{ name: "Chris Royse", url: "https://teleox.ai/about" }],
  creator: "Chris Royse",
  publisher: "Teleox.ai",
  formatDetection: {
    email: false,
    address: false,
    telephone: false,
  },
  openGraph: {
    type: "website",
    siteName: "Teleox.ai",
    title: "Teleox.ai — Models can be opaque. Pipelines shouldn't be.",
    description:
      "Independent research on verifiable AI training pipelines.",
    url: "https://teleox.ai",
    images: [
      {
        url: "/og-image.png",
        width: 1200,
        height: 630,
        alt: "Teleox.ai — Models can be opaque. Pipelines shouldn't be.",
      },
    ],
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "Teleox.ai",
    description: "Models can be opaque. Pipelines shouldn't be.",
    images: ["/og-image.png"],
    // No site / creator handle — Teleox does not have a Twitter account
  },
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      "max-snippet": -1,
      "max-image-preview": "large",
      "max-video-preview": -1,
    },
  },
  alternates: {
    canonical: "https://teleox.ai",
  },
  icons: {
    icon: [
      { url: "/icon.svg", type: "image/svg+xml" },
      { url: "/favicon.ico", sizes: "any" },
    ],
    apple: "/apple-touch-icon.png",
  },
  manifest: "/manifest.json",
  category: "research",
};
```

---

## 2. Per-page metadata via `generateRouteMetadata`

Each `page.tsx` exports a `metadata` const:

```tsx
export const metadata: Metadata = generateRouteMetadata(homeCopy, {
  title: "Teleox.ai — Independent research on verifiable AI training pipelines",
  description: homeCopy.heroSubhead,
});
```

The helper merges:
- Root defaults from `layout.tsx`
- Route-specific overrides
- Per-route OG images if the route has a custom OG image (none currently)

The existing `src/lib/metadata.ts` already does this. Verify the implementation matches; if not, fix.

---

## 3. Per-page metadata table

| Route | `<title>` | `<meta description>` | OG title | OG description |
|---|---|---|---|---|
| `/` | `Teleox.ai — Independent research on verifiable AI training pipelines` | `Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware...` | `Teleox.ai — Models can be opaque. Pipelines shouldn't be.` | `Independent research on verifiable AI training pipelines.` |
| `/research` | `Research — Teleox.ai` | `Current and recent research artifacts published by Teleox: papers, reproducibility releases, and open-source companion implementations.` | `Teleox Research` | `Models can be opaque. Pipelines shouldn't be.` |
| `/paper` | `Dynamic / ME-JEPA — Teleox.ai` | `The current Teleox research paper: an audited, domain-portable JEPA runtime on a single RTX 5090, with a new class of training data.` | `Dynamic / ME-JEPA` | `Verifiable JEPA pipeline on a single RTX 5090.` |
| `/about` | `About — Teleox.ai` | `Teleox is an independent research operation founded by Chris Royse. Brief overview of the principal, the work, and adjacent projects.` | `About Teleox.ai` | `Independent research, founded by Chris Royse.` |
| `/contact` | `Contact — Teleox.ai` | `Direct email contact for research conversations about Teleox: chrisroyseai@gmail.com.` | `Contact Teleox` | `Direct email is the only contact channel.` |
| 404 | `Page not found — Teleox.ai` | (handled in `not-found.tsx`) | (n/a — `noindex`) | (n/a) |

---

## 4. JSON-LD schemas

The existing `JsonLd` component renders one or more JSON-LD blobs per page. Update `src/lib/json-ld.ts` to define the new schemas.

### 4.1 `organizationSchema` (rendered on every page via root layout)

```ts
export const organizationSchema = {
  "@context": "https://schema.org",
  "@type": "Organization",
  "@id": "https://teleox.ai/#organization",
  name: "Teleox.ai",
  alternateName: "Teleox",
  url: "https://teleox.ai",
  logo: "https://teleox.ai/icon.png",
  description: "Independent research on verifiable AI training pipelines.",
  founder: {
    "@type": "Person",
    name: "Chris Royse",
    email: "chrisroyseai@gmail.com",
    url: "https://teleox.ai/about",
  },
  email: "chrisroyseai@gmail.com",
  sameAs: [
    "https://github.com/ChrisRoyse",
  ],
};
```

### 4.2 `websiteSchema` (rendered on every page via root layout)

```ts
export const websiteSchema = {
  "@context": "https://schema.org",
  "@type": "WebSite",
  "@id": "https://teleox.ai/#website",
  url: "https://teleox.ai",
  name: "Teleox.ai",
  description: "Models can be opaque. Pipelines shouldn't be.",
  publisher: {
    "@id": "https://teleox.ai/#organization",
  },
  inLanguage: "en-US",
};
```

### 4.3 `personSchema` (used on /about)

```ts
export const personSchema = {
  "@context": "https://schema.org",
  "@type": "Person",
  "@id": "https://teleox.ai/about#chris-royse",
  name: "Chris Royse",
  email: "chrisroyseai@gmail.com",
  url: "https://teleox.ai/about",
  jobTitle: "Independent researcher",
  affiliation: {
    "@type": "Organization",
    "@id": "https://teleox.ai/#organization",
  },
  sameAs: [
    "https://github.com/ChrisRoyse",
  ],
};
```

### 4.4 `paperSchema` (used on /paper)

```ts
export const paperSchema = {
  "@context": "https://schema.org",
  "@type": "ScholarlyArticle",
  "@id": "https://teleox.ai/paper#me-jepa-2026",
  headline: "Dynamic / ME-JEPA: Source-of-Truth Verified Latent World Modeling on a Single RTX 5090",
  alternativeHeadline: "An audited, domain-portable JEPA runtime on a single RTX 5090",
  author: {
    "@id": "https://teleox.ai/about#chris-royse",
  },
  datePublished: "2026-05-01",
  dateModified: "2026-05-01",
  inLanguage: "en-US",
  publisher: {
    "@id": "https://teleox.ai/#organization",
  },
  isAccessibleForFree: false,
  about: [
    "Joint-Embedding Predictive Architecture",
    "World models",
    "Training-data provenance",
    "Reproducibility",
  ],
  keywords: [
    "JEPA",
    "world models",
    "auditable ML",
    "training data",
    "model collapse",
    "process supervision",
  ],
  url: "https://teleox.ai/paper",
};
```

### 4.5 `researchCollectionSchema` (used on /research)

```ts
export const researchCollectionSchema = {
  "@context": "https://schema.org",
  "@type": "CollectionPage",
  "@id": "https://teleox.ai/research#collection",
  name: "Research — Teleox.ai",
  description: "Current and recent research artifacts.",
  url: "https://teleox.ai/research",
  isPartOf: {
    "@id": "https://teleox.ai/#website",
  },
  hasPart: [
    {
      "@id": "https://teleox.ai/paper#me-jepa-2026",
    },
    {
      "@type": "SoftwareSourceCode",
      name: "ClipCannon",
      codeRepository: "https://github.com/ChrisRoyse/clipcannon",
      programmingLanguage: ["Rust", "Python"],
      license: "https://opensource.org/licenses/BSL-1.1",
    },
  ],
};
```

### 4.6 Wiring JSON-LD into pages

The existing `<JsonLd>` component (in `src/components/structured-data/`) renders one schema. The root `layout.tsx` already renders `organizationSchema` and `websiteSchema`:

```tsx
<JsonLd data={organizationSchema} />
<JsonLd data={websiteSchema} />
```

Each page that has additional JSON-LD adds its own:

```tsx
// In /research/page.tsx
import { JsonLd } from "@/components/structured-data/JsonLd";
import { researchCollectionSchema } from "@/lib/json-ld";

export default function ResearchPage() {
  return (
    <>
      <JsonLd data={researchCollectionSchema} />
      <PageContainer>...</PageContainer>
    </>
  );
}
```

---

## 5. Sitemap.xml

Already specified in `02_information_architecture.md` §7. Reproduced here for completeness:

```ts
// src/app/sitemap.ts
import type { MetadataRoute } from "next";

const HOST = "https://teleox.ai";

const ROUTES = [
  { path: "/",          changeFrequency: "weekly",  priority: 1.0  },
  { path: "/research",  changeFrequency: "weekly",  priority: 0.95 },
  { path: "/paper",     changeFrequency: "weekly",  priority: 0.92 },
  { path: "/about",     changeFrequency: "monthly", priority: 0.7  },
  { path: "/contact",   changeFrequency: "monthly", priority: 0.85 },
  { path: "/blog/rss.xml", changeFrequency: "weekly", priority: 0.45 },
  { path: "/llms.txt",     changeFrequency: "weekly", priority: 0.4  },
] as const;

export default function sitemap(): MetadataRoute.Sitemap {
  const lastModified = new Date();
  return ROUTES.map(({ path, changeFrequency, priority }) => ({
    url: path === "/" ? HOST : `${HOST}${path}`,
    lastModified,
    changeFrequency,
    priority,
  }));
}
```

---

## 6. robots.txt

Already specified in `02_information_architecture.md` §8. Reproduced:

```ts
// src/app/robots.ts
import type { MetadataRoute } from "next";

const HOST = "https://teleox.ai";

export default function robots(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: "*",
      allow: "/",
      disallow: [
        "/_retired/",
        "/api/",
        "/y-lecun/",
        "/k-cho/",
        "/ami/",
        // Add any future per-recipient slugs here
      ],
    },
    sitemap: `${HOST}/sitemap.xml`,
    host: HOST,
  };
}
```

---

## 7. llms.txt

A short machine-readable description for LLM crawlers. Update `public/llms.txt`:

```
# Teleox.ai

Teleox publishes infrastructure and research that makes JEPA-style and
predictive-AI training pipelines verifiable end-to-end on commodity hardware,
with cryptographic chain-of-custody from source data through training to model
weights.

## What lives here
- /research  Current research artifacts (papers, open-source companions)
- /paper     The current paper (Dynamic / ME-JEPA), plain-language overview
- /about     Who runs Teleox (Chris Royse, independent researcher)
- /contact   Direct email contact

## How to use this
- Cite the canonical paper URL: https://teleox.ai/paper
- Reach the principal directly at chrisroyseai@gmail.com
- The runtime described in the paper is private; companion code is at
  github.com/ChrisRoyse/clipcannon

## What this site is not
- Not a SaaS product
- Not a consulting firm
- Not a competitor to any major AI lab — complementary infrastructure
```

---

## 8. manifest.json

Update `public/manifest.json`:

```json
{
  "name": "Teleox.ai — Independent research on verifiable AI training pipelines",
  "short_name": "Teleox",
  "description": "Models can be opaque. Pipelines shouldn't be.",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#111713",
  "theme_color": "#111713",
  "icons": [
    {
      "src": "/icon.svg",
      "sizes": "any",
      "type": "image/svg+xml"
    },
    {
      "src": "/icon.png",
      "sizes": "512x512",
      "type": "image/png"
    },
    {
      "src": "/apple-touch-icon.png",
      "sizes": "180x180",
      "type": "image/png"
    }
  ]
}
```

---

## 9. humans.txt (optional)

```
/* TEAM */
    Founder & researcher: Chris Royse
    Email: chrisroyseai@gmail.com

/* SITE */
    Last update: 2026-05-01
    Standards: HTML5, CSS3, ES2024
    Components: Next.js 16, React 19, Tailwind v4
    Hosted on: Vercel
```

If Steve Abbey stays involved, add him.

---

## 10. Open Graph image

A new OG image is needed. Spec in `15_assets_inventory.md`. Reference for layout teams:

- Dimensions: 1200×630 PNG
- Background: `--color-ink-1` (#111713)
- Headline (large, Source Serif 4, 64px, semibold, cream): "Models can be opaque. Pipelines shouldn't be."
- Wordmark (Inter, 32px, semibold, cream + amber dot): "Teleox.ai" — bottom-left
- URL (Inter, 16px, cream/60): "teleox.ai" — bottom-right

No background graphics, no people, no abstract gradients. Pure typography.

---

## 11. Verification

After all metadata is wired:

- [ ] `https://teleox.ai/sitemap.xml` lists exactly 7 entries
- [ ] `https://teleox.ai/robots.txt` includes `/y-lecun/`, `/k-cho/`, `/ami/` in disallow
- [ ] `https://teleox.ai/llms.txt` reflects new positioning
- [ ] `https://teleox.ai/manifest.json` has updated name + description
- [ ] View source on `/`: confirm `<title>` matches table in §3
- [ ] View source on `/`: confirm `<meta name="description">` matches table in §3
- [ ] View source on `/`: confirm `<meta property="og:title">` matches
- [ ] View source on `/`: confirm `<meta property="og:image">` is `https://teleox.ai/og-image.png` (absolute URL)
- [ ] View source on `/`: confirm at least 2 `<script type="application/ld+json">` blocks (Organization + WebSite)
- [ ] View source on `/research`: confirm 3+ JSON-LD blocks (Organization + WebSite + CollectionPage)
- [ ] View source on `/paper`: confirm 3+ JSON-LD blocks (Organization + WebSite + ScholarlyArticle)
- [ ] View source on `/about`: confirm 3+ JSON-LD blocks (Organization + WebSite + Person/AboutPage)
- [ ] Twitter card validator: paste `https://teleox.ai/`; verify large-image card with correct title/description/image
- [ ] LinkedIn post inspector: same
- [ ] OpenGraph debug tool: same
- [ ] Google rich results test: paste each page URL; no errors
- [ ] Search Console: validate the new sitemap once submitted
