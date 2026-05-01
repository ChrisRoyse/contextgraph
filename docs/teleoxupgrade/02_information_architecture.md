# 02 — Information Architecture

**Authority over:** sitemap, route map, navigation taxonomy, what stays / retires / changes.

---

## 1. Current state — full inventory

### 1.1 Routes that currently exist in `src/app/`

```
/                       → home.ts
/about                  → about.ts
/api/*                  → server-only (untouched)
/artifacts              → artifacts.ts
/blog                   → blog (separate content system)
/centroids              → centroids.ts
/citations              → citations.ts
/contact                → contact.ts
/evidence-map           → alignment-index.ts
/feed                   → feed.ts
/governance             → governance.ts
/infrastructure         → infrastructure.ts
/mark                   → mark.ts
/measured               → measured.ts
/measurements           → (alternate route — verify)
/method                 → method.ts
/methodology            → (alternate route — verify)
/paper                  → paper.ts
/pricing                → pricing.ts
/priority-date          → priority-date.ts
/research               → (folder exists in src/app — likely placeholder)
/sign                   → sign.ts
/sitemap.ts             → sitemap generator
/standard               → standard.ts
/robots.ts              → robots generator
/verify                 → verify.ts
```

That's **22 user-facing routes** in the current sitemap (from `src/app/sitemap.ts`):

- `/`
- `/method`
- `/paper`
- `/infrastructure`
- `/measured`
- `/pricing`
- `/contact`
- `/about`
- `/blog/rss.xml`
- `/llms.txt`

Plus several pages that exist but are not in the sitemap.

### 1.2 What each currently positions for

| Route | Current purpose (per its `RouteCopy.purpose`) | Retain in new IA? |
|---|---|---|
| `/` | "Position Teleox.ai as the company that turns owned data into AI signal, memory, and output checks." | YES — REPOSITION |
| `/about` | "Explain Teleox.ai as a founder-led company and clarify how Chris Royse and Steve Abbey contribute." | YES — REPOSITION |
| `/paper` | "Translate the research paper into practical value for buyers." | YES — REPOSITION |
| `/method` | "Explain meaning compression as a buyer benefit for model teams." | RETIRE (folds into `/research`) |
| `/infrastructure` | "Show the systems that make private AI memory and meaning compression useful." | RETIRE |
| `/contact` | "Ask buyers for the minimum information needed to judge a valuable data problem." | YES — REPOSITION |
| `/pricing` | (commercial pricing page) | RETIRE |
| `/measured` | (B2B credibility page) | RETIRE |
| `/sign` | (priority-date / IP-defense framing) | RETIRE |
| `/standard` | (regulatory positioning) | RETIRE |
| `/verify` | (B2B verification page) | RETIRE |
| `/governance` | (policy / governance positioning) | RETIRE |
| `/mark` | (trademark / IP positioning) | RETIRE |
| `/centroids` | (technical aside) | RETIRE |
| `/citations` | (citation aggregator) | RETIRE |
| `/feed` | (content feed) | RETIRE |
| `/artifacts` | (signed-artifacts page) | RETIRE |
| `/measurements` | (metrics dashboard) | RETIRE |
| `/methodology` | (alternate of /method) | RETIRE |
| `/priority-date` | (IP / patent positioning) | RETIRE |
| `/evidence-map` | (alignment-index) | RETIRE |
| `/blog` | (blog index) | KEEP AS-IS (out of conversion path) |
| `/research` | (folder placeholder) | BUILD OUT — primary new page |

That's **17 routes to retire**, **4 to reposition**, **1 to build new** (`/research`), **1 to leave alone** (`/blog`).

---

## 2. Target sitemap (post-upgrade)

```
/
├── /research               (NEW — primary research surface)
├── /paper                  (REPOSITIONED — current research paper, plain-language + technical)
├── /about                  (REPOSITIONED — Chris Royse, the work, what we are)
├── /contact                (REPOSITIONED — single email, no funnel)
├── /blog                   (UNCHANGED — content)
├── /llms.txt               (UNCHANGED — machine-readable site description)
├── /robots.txt             (REGENERATED — drop retired routes)
├── /sitemap.xml            (REGENERATED)
└── /api/*                  (UNCHANGED — server-only)
```

**5 user-facing routes** (`/`, `/research`, `/paper`, `/about`, `/contact`) plus the blog and machine-readable surfaces.

This is a deliberately small surface. A research site does not need 22 pages.

---

## 3. Navigation taxonomy

### 3.1 Primary navigation (header)

Visible in the header on every page, in this order, left-to-right:

```
[ Teleox.ai logo / wordmark ]    Research   Paper   About   Contact
```

### 3.2 Secondary navigation (footer)

```
Teleox.ai
chrisroyseai@gmail.com

Research   Paper   About   Contact   Blog
RSS        llms.txt
```

That's it. No multi-column footer with "Resources / Company / Legal / Social." A research site does not need that surface.

### 3.3 What is *not* in navigation

- No "Sign in" / "Login" / "Sign up"
- No "Pricing" link (route retired)
- No "Get started" CTA in nav
- No language switcher
- No newsletter signup
- No social media icons
- No search bar

### 3.4 Navigation state on different breakpoints

| Breakpoint | Navigation form |
|---|---|
| ≥ 1024px (desktop) | Horizontal text links in header |
| 640–1023px (tablet) | Same horizontal layout, condensed spacing |
| < 640px (mobile) | Hamburger → full-screen drawer with the same 4 items vertically + email at bottom |

See `12_navigation_and_footer.md` for component-level specifics.

---

## 4. Redirect plan

Every retired route returns a **308 Permanent Redirect** to the closest live route. This preserves any existing inbound links and sends Google a clean signal.

The redirect map lives in `src/lib/redirects.ts` (already exists). Append to `V1_TO_V2_REDIRECTS` (or create equivalent V2_TO_V3_REDIRECTS array — implementer's call):

```ts
export const V2_TO_V3_REDIRECTS: Redirect[] = [
  { source: "/method",          destination: "/research", permanent: true },
  { source: "/methodology",     destination: "/research", permanent: true },
  { source: "/infrastructure",  destination: "/research", permanent: true },
  { source: "/measured",        destination: "/research", permanent: true },
  { source: "/measurements",    destination: "/research", permanent: true },
  { source: "/pricing",         destination: "/contact",  permanent: true },
  { source: "/sign",            destination: "/about",    permanent: true },
  { source: "/standard",        destination: "/research", permanent: true },
  { source: "/verify",          destination: "/research", permanent: true },
  { source: "/governance",      destination: "/about",    permanent: true },
  { source: "/mark",            destination: "/about",    permanent: true },
  { source: "/centroids",       destination: "/research", permanent: true },
  { source: "/citations",       destination: "/research", permanent: true },
  { source: "/feed",            destination: "/blog",     permanent: true },
  { source: "/artifacts",       destination: "/research", permanent: true },
  { source: "/priority-date",   destination: "/about",    permanent: true },
  { source: "/evidence-map",    destination: "/research", permanent: true },
];
```

Wire into `next.config.ts` `redirects()` alongside the existing `V1_TO_V2_REDIRECTS` and `CANONICAL_HOST_REDIRECT`.

---

## 5. URL conventions

| Convention | Rule |
|---|---|
| Trailing slash | `false` (already configured in `next.config.ts`) — canonical is `/research` not `/research/` |
| Casing | All lowercase, hyphenated for multi-word slugs |
| Special private slugs (`/y-lecun`, `/k-cho`, `/ami`) | These are static HTML in `public/[slug]/index.html`, served directly without going through Next routing. They are **not** in the sitemap, **not** in the navigation, and have `<meta name="robots" content="noindex, nofollow">` |
| Per-recipient private URL pattern | `https://teleox.ai/{recipient-slug}` where the slug identifies the addressee (e.g. `y-lecun`, `k-cho`, `ami`). One recipient = one slug = one HTML page. |

---

## 6. Per-page purpose, audience, and primary CTA (target state)

This table is the source of truth for the next round of `RouteCopy` files.

| Route | Purpose | Audience | Primary CTA | Secondary CTA |
|---|---|---|---|---|
| `/` | Position Teleox as an independent research operation building verifiability infrastructure for predictive AI training pipelines | Frontier-lab research leadership; secondarily independent researchers | **Read current research** → `/research` | **Email Chris** → `mailto:chrisroyseai@gmail.com?subject=Teleox` |
| `/research` | Surface the current research artifacts: ME-JEPA paper (private preview), clipcannon companion, brief framing of the program | Researchers wanting to evaluate the work | **Read the paper** → `/paper` | **See the open-source companion** → `https://github.com/ChrisRoyse/clipcannon` |
| `/paper` | Translate the current paper into a plain-language summary alongside the technical paper itself | Researchers, technical leadership | **Download paper PDF** → ME-JEPAResearchPaper.pdf | **Email Chris** |
| `/about` | Identify Chris Royse as the principal, summarize prior work, name the company | Anyone curious about who is behind the work | **Email Chris** | **See research** → `/research` |
| `/contact` | Provide direct, low-friction email contact for research conversations | Anyone wanting to talk about the work | **Email Chris** | (none) |
| `/blog` | Publish occasional long-form posts | Anyone | (per-post CTAs) | — |

---

## 7. Sitemap.ts (target state)

Full file content. Replace `src/app/sitemap.ts` with:

```ts
import type { MetadataRoute } from "next";

const HOST = "https://teleox.ai";

type Entry = {
  path: string;
  changeFrequency: MetadataRoute.Sitemap[number]["changeFrequency"];
  priority: number;
};

const ROUTES: readonly Entry[] = [
  { path: "/",          changeFrequency: "weekly",  priority: 1.0  },
  { path: "/research",  changeFrequency: "weekly",  priority: 0.95 },
  { path: "/paper",     changeFrequency: "weekly",  priority: 0.92 },
  { path: "/about",     changeFrequency: "monthly", priority: 0.7  },
  { path: "/contact",   changeFrequency: "monthly", priority: 0.85 },
  { path: "/blog/rss.xml", changeFrequency: "weekly",  priority: 0.45 },
  { path: "/llms.txt",     changeFrequency: "weekly",  priority: 0.4  },
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

## 8. robots.txt (target state)

Update `src/app/robots.ts`:

```ts
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
        "/y-lecun/",   // private preview — never crawl
        "/k-cho/",
        "/ami/",
      ],
    },
    sitemap: `${HOST}/sitemap.xml`,
    host: HOST,
  };
}
```

---

## 9. Files to delete

After the redirect plan is in place and the new pages are live, delete:

```
src/app/method/
src/app/methodology/
src/app/infrastructure/
src/app/measured/
src/app/measurements/
src/app/pricing/
src/app/sign/
src/app/standard/
src/app/verify/
src/app/governance/
src/app/mark/
src/app/centroids/
src/app/citations/
src/app/feed/
src/app/artifacts/
src/app/priority-date/
src/app/evidence-map/

src/content/routes/method.ts
src/content/routes/infrastructure.ts
src/content/routes/measured.ts
src/content/routes/pricing.ts
src/content/routes/sign.ts
src/content/routes/standard.ts
src/content/routes/verify.ts
src/content/routes/governance.ts
src/content/routes/mark.ts
src/content/routes/centroids.ts
src/content/routes/citations.ts
src/content/routes/feed.ts
src/content/routes/artifacts.ts
src/content/routes/priority-date.ts
src/content/routes/alignment-index.ts
```

Also remove these from `src/content/routes/index.ts` exports.

Components that are only used by retired pages can be deleted; components that might be reused elsewhere should be left alone for now (a separate cleanup PR can prune them later).

---

## 10. The /research page is new

Currently a folder placeholder may exist at `src/app/research/`. The implementer needs to:

1. Create `src/content/routes/research.ts` with full `RouteCopy`
2. Create `src/app/research/page.tsx` consuming that copy via the standard pattern (see `13_content_schema.md` and `06_page_homepage.md` for the pattern)
3. Wire into `src/content/routes/index.ts` exports

See `07_page_research.md` for the full content + component spec.

---

## 11. Out-of-IA surfaces

These are not "pages" but matter:

| Surface | Status |
|---|---|
| `/llms.txt` | Update to reflect new positioning (one paragraph max — see `14_metadata_seo_sitemap.md`) |
| OG image | New OG image needed — see `15_assets_inventory.md` |
| Favicon | Existing favicon stays |
| `/manifest.json` | Update `name`, `short_name`, `description` to match new positioning |
| `humans.txt` | Update credits if needed |
| `404 / not-found` | Friendly research-toned 404 — see `11_page_404_error.md` |

---

## 12. Internal-link policy

A page on this site should link to:

| Link target | When |
|---|---|
| Internal page on this site | When the link extends or supports the current page's claim |
| `clipcannon` GitHub | Once on `/research`, once on `/paper`, optional once on `/about`. Not on the homepage |
| The ME-JEPA paper PDF (`/research/...pdf` or per-recipient slug) | On `/research`, `/paper` (if public preview at that point) |
| `mailto:chrisroyseai@gmail.com` | On every page; as primary or secondary CTA |
| `leapable.ai` | On `/about` only, in a "Related" or "Other" subsection. Not in nav, not on home |
| External research papers (arxiv, OpenReview, etc.) | Only when directly cited in body copy |

A page on this site should **not** link to:
- Random AI news sites
- Twitter / X (the platform changes too fast to embed reliably)
- Newsletter platforms
- Other vendor sites

---

## 13. Verification — when IA is done

1. Visit each route in target sitemap. All return 200.
2. Visit each retired route. All return 308 to the mapped destination.
3. `npm run build` completes successfully (no missing imports for retired routes).
4. `npm run scan:all` passes.
5. `https://teleox.ai/sitemap.xml` lists exactly the 7 entries.
6. `https://teleox.ai/robots.txt` lists the disallows for `/y-lecun/`, `/k-cho/`, `/ami/`.
7. Header navigation shows exactly 4 items: Research, Paper, About, Contact.
8. No retired-route links remain anywhere in `src/`.
