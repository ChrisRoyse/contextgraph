# 13 — Content Schema (TypeScript Types)

**Authority over:** the TypeScript types that define the shape of route content, signal bundles, and other structured copy.
**File touched:** `src/types/route-copy.ts` (existing, may need extensions)

---

## 1. The `RouteCopy` type

The current `RouteCopy` type drives every page's `RouteCopy` file. The upgrade reuses it almost as-is, with three small additions.

### 1.1 Existing fields (verify against current `src/types/route-copy.ts`)

```ts
export type RouteCopy = {
  /** URL path (e.g. "/", "/research", "/paper") */
  path: string;

  /** Internal positioning statement; not rendered */
  purpose: string;

  /** Audience tier (primary / secondary / tertiary) */
  audience: "primary" | "secondary" | "tertiary";

  /** Audience taxonomies */
  playerClasses: PlayerClass[];

  /** Small uppercase eyebrow above hero h1 */
  eyebrow?: string;

  /** Primary CTA */
  primaryCta?: Cta;

  /** Secondary CTA */
  secondaryCta?: Cta;

  /** Signal-bundle IDs this page emits */
  signals: SignalId[];

  /** Scope-guard rules to enforce on rendered HTML */
  scopeGuards: ScopeGuard[];

  /** Hero h1 text */
  heroHeadline: string;

  /** Word(s) in heroHeadline to highlight; rendered with accent treatment */
  heroHeadlineLitWord?: string;

  /** Hero subhead paragraph */
  heroSubhead?: string;

  /** Hero alt text (for OG / accessibility) */
  heroAlt?: string;

  /** Body content blocks */
  bodyBlocks: BodyBlock[];

  /** Phrases the page must not contain (enforced by scan-claims.ts) */
  mustNotSay?: string[];
};

export type Cta = {
  label: string;
  href: string;
  newTab?: boolean;
  external?: boolean;
};

export type BodyBlock = {
  heading: string;
  paragraphs: string[];
};

export type PlayerClass =
  | "lab"
  | "researcher"
  | "enterprise"
  | "creator"
  | "investor"
  | "public";

export type SignalId =
  | `S${number}`;  // S1, S2, S3, ...

export type ScopeGuard = {
  rule: string;
  rationale: string;
};
```

### 1.2 Updates / additions for the upgrade

Add these optional fields to `RouteCopy` (verify against current file before adding to avoid duplication):

```ts
export type RouteCopy = {
  // ...existing fields above

  /** Optional artifact cards (used on /research) */
  artifactCards?: ArtifactCard[];

  /** Optional results data (used on / and /paper for ResultsBox) */
  results?: ResultsBoxContent;

  /** Optional bullet lists (used on /contact for what-gets-reply / what-doesnt) */
  bulletBlocks?: BulletBlock[];

  /** Optional structured-data injection */
  jsonLd?: object;
};

export type ArtifactCard = {
  eyebrow: string;
  title: string;
  oneLine: string;
  meta?: string;
  status: "private-preview" | "open-source" | "in-progress";
  cta: Cta;
};

export type ResultsBoxContent = {
  /** Each line of the results display, with bolded numbers */
  lines: ResultLine[];
};

export type ResultLine = {
  /** Plain text up to the first bold span */
  prefix: string;
  /** The bolded value */
  highlight: string;
  /** Plain text after the bold; may include another `highlight` segment */
  suffix?: string;
  /** Optional second highlight in same line */
  secondHighlight?: string;
  /** Plain text after the second highlight */
  secondSuffix?: string;
};

export type BulletBlock = {
  heading: string;
  /** Each list item, plain text */
  items: string[];
};
```

These additions are **optional** — pages that don't need them just omit. The existing pages don't break.

### 1.3 Updated PlayerClass enum

The new audience taxonomy is narrower. Update `PlayerClass`:

```ts
export type PlayerClass =
  | "lab"
  | "researcher"
  | "public";
```

Removed: `"enterprise"`, `"creator"`, `"investor"`. None of the new pages target these.

If existing routes (the ones being retired) reference removed values, the TypeScript compile will error — that's the signal to delete those files.

### 1.4 SignalId taxonomy

The existing site has signal-bundle IDs `S1` through `S6` (or similar). The upgrade does NOT redesign signals. Each new page reuses the existing IDs that fit:

| Page | Signal IDs |
|---|---|
| `/` | `S1, S3, S5` |
| `/research` | `S1, S3, S5` |
| `/paper` | `S1, S3, S5` |
| `/about` | `S1, S3` |
| `/contact` | `S1, S3` |

If a signal ID was specific to a retired page, it can be removed from the global signal taxonomy in `src/content/signals.ts` — verify before removing.

---

## 2. Per-route content file pattern

Every route content file follows this pattern:

```ts
// src/content/routes/<route>.ts
import type { RouteCopy } from "@/types/route-copy";

export const <route>Copy: RouteCopy = {
  path: "/<route>",
  purpose: "<one-line internal positioning statement>",
  audience: "primary" | "secondary",
  playerClasses: [...],
  eyebrow: "...",
  primaryCta: { label: "...", href: "..." },
  secondaryCta: { label: "...", href: "..." },
  signals: ["S1", "S3"],
  scopeGuards: [],
  heroHeadline: "...",
  heroHeadlineLitWord: "...",
  heroSubhead: "...",
  heroAlt: "...",
  bodyBlocks: [
    { heading: "...", paragraphs: [...] },
  ],
  mustNotSay: [...],
};

export default <route>Copy;
```

---

## 3. Per-route metadata file pattern

The existing `src/lib/metadata.ts` `generateRouteMetadata(routeCopy, overrides?)` function produces Next.js `Metadata`. Each `page.tsx`:

```tsx
import { generateRouteMetadata } from "@/lib/metadata";
import { homeCopy } from "@/content/routes/home";

export const metadata: Metadata = generateRouteMetadata(homeCopy, {
  title: "<override page-specific title>",
  description: "<override page-specific description>",
});
```

If a page omits the `overrides` argument, `generateRouteMetadata` derives sensible defaults from the `RouteCopy` (typically `${heroHeadline} — Teleox.ai` for title, `heroSubhead` for description).

---

## 4. The `index.ts` export aggregator

Update `src/content/routes/index.ts` to export only the live route copies:

```ts
// src/content/routes/index.ts
export { homeCopy } from "./home";
export { researchCopy } from "./research";
export { paperCopy } from "./paper";
export { aboutCopy } from "./about";
export { contactCopy } from "./contact";

// Retired (deleted): about.ts retains, but the following are removed:
// method.ts, infrastructure.ts, measured.ts, pricing.ts, sign.ts, standard.ts,
// verify.ts, governance.ts, mark.ts, centroids.ts, citations.ts, feed.ts,
// artifacts.ts, priority-date.ts, alignment-index.ts
```

---

## 5. The canonical-routes list

Used by the build-time validators. Update `src/lib/canonical-routes.mjs`:

```js
export const CANONICAL_ROUTES = [
  "/",
  "/research",
  "/paper",
  "/about",
  "/contact",
];
```

This is the source of truth for "what routes are part of the site." The validation scripts (`scripts/validate-content.mjs`, etc.) check against this list.

---

## 6. Type-generation / extraction patterns

### 6.1 Reading copy at build time

Pages import their copy:

```tsx
import { homeCopy } from "@/content/routes/home";
```

Static analysis at build time means typos in field access fail the TS compile. This is the safety net.

### 6.2 Reading copy from external scripts

Build-time scripts (in `scripts/`) read copy via dynamic import or by parsing the TS files. Existing scripts already handle this; do not modify the parser.

### 6.3 Adding a new content type

If a future page needs a new content shape (e.g. a publication list), add a new type to `src/types/route-copy.ts`, expose it as an optional field on `RouteCopy`, and provide a default render in the page. Do NOT introduce a parallel content system for one-off pages.

---

## 7. SignalBundle (machine-readable hook)

The existing site emits a `<meta name="signal-bundle">` tag with structured data per page (per `memory/decisions/agent-07-seo-metadata-lead--canonical-policy.md`). The upgrade preserves this but updates the schema to match the new audience.

Update `src/types/signal.ts` if needed; otherwise leave as-is.

---

## 8. JSON-LD schemas per page

Each page emits one or more JSON-LD blobs. The schemas:

| Page | JSON-LD types |
|---|---|
| `/` | `Organization` (root), `WebSite` (root), `WebPage` (page-specific) |
| `/research` | `CollectionPage` listing artifacts |
| `/paper` | `ScholarlyArticle` for the current paper |
| `/about` | `AboutPage` + `Person` for Chris Royse |
| `/contact` | `ContactPage` |
| `/blog` | `Blog` + per-post `BlogPosting` |

See `14_metadata_seo_sitemap.md` for the full schemas.

---

## 9. Content validation rules

The `scripts/validate-content.mjs` script enforces:

| Rule | What |
|---|---|
| Every canonical route has a `RouteCopy` file | Missing file = build fail |
| Every `RouteCopy` has a non-empty `heroHeadline` | Empty heading = build fail |
| Every `primaryCta.href` resolves to a known route or external URL | Broken internal link = build fail |
| Every `signals` entry is a valid `SignalId` | Invalid signal = build fail |
| Every paragraph in `bodyBlocks` is non-empty | Empty paragraph = build fail |
| `mustNotSay` phrases are checked against rendered HTML | Hit = build fail |

If you add new routes, make sure they pass these rules. Do not modify the validator unless you understand the implications.

---

## 10. Type-checking commands

```bash
npm run typecheck
```

Runs `tsc --noEmit` against the entire project. Should be 0 errors before deploy.

```bash
npm run validate:content
```

Runs the content validator. Should be 0 failures before deploy.

```bash
npm run scan:all
```

Runs all scanners (claims, public-posture, scope-guards, signals, event-names). Should be 0 violations before deploy.

---

## 11. Schema changes worth tracking

When modifying `src/types/route-copy.ts`, document the change in `memory/decisions/` per the existing pattern:

```
memory/decisions/agent-XX--upgrade-routecopy-fields.md
```

Include:
- What field was added/removed
- Why
- What pages need to update
- Date

This is the existing convention. Follow it.
