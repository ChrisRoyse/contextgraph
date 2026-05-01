# 15 — Assets Inventory

**Authority over:** every image, icon, font, manifest, and static file the site needs.
**Lives in:** `public/` directory of the Next.js project.

---

## 1. Currently in `public/` (verify, keep)

| File | Purpose | Action |
|---|---|---|
| `apple-touch-icon.png` | Apple touch icon (180×180) | Keep |
| `favicon.ico` | Legacy favicon | Keep |
| `icon.png` | Manifest 512×512 | Keep |
| `icon.svg` | Manifest SVG | Keep |
| `manifest.json` | Web app manifest | Update content (see `14_metadata_seo_sitemap.md` §8) |
| `og-image.png` | Open Graph image | **REPLACE** (new image needed) |
| `humans.txt` | Site credits | Update content (see `14_metadata_seo_sitemap.md` §9) |
| `llms.txt` | LLM-readable description | Update content (see `14_metadata_seo_sitemap.md` §7) |
| `robots.txt` | Generated; not a static file | Verify the dynamic `robots.ts` produces this |
| `_retired/` | Old assets, do not crawl | Keep (in robots disallow) |
| `images/` | Site imagery | Audit; remove anything tied to retired pages |
| `research/` | Research-related static assets | Audit |
| `standard/` | Standard-page assets | Remove (page retired) |
| `mark/` | Mark-page assets | Remove (page retired) |
| `blog/` | Blog static assets | Keep |
| `vercel.svg`, `next.svg`, `file.svg`, `globe.svg`, `window.svg`, `grain.png` | Default Next assets | Audit; remove if unused |
| `logo.png` | Logo PNG | Verify need; the wordmark is type-only, so a logo PNG is only used in JSON-LD (`organization.logo`) |

---

## 2. Asset to PRODUCE: new OG image

### 2.1 Spec

| Property | Value |
|---|---|
| File path | `public/og-image.png` |
| Dimensions | 1200 × 630 pixels |
| Format | PNG (24-bit, no alpha required) |
| File size target | ≤ 80 KB |
| Aspect ratio | 1.905:1 (Open Graph standard) |
| Color profile | sRGB |

### 2.2 Visual specification

```
┌──────────────────────────────────────────────────────────────┐
│                                                                │
│   (top padding: 80px)                                          │
│                                                                │
│   Models can be opaque.                                        │
│   Pipelines shouldn't be.                                      │
│   ↑ Source Serif 4 SemiBold, 64px, line-height 1.05            │
│   ↑ color: #F5F0E6 (cream)                                     │
│   ↑ left padding: 80px                                         │
│                                                                │
│                                                                │
│   ─────  (small horizontal accent line, 60px wide,             │
│           1px, color: #F3B85B amber, 24px below headline)      │
│                                                                │
│                                                                │
│   Independent research on verifiable AI training pipelines.    │
│   ↑ Source Serif 4 Regular, 28px, line-height 1.4              │
│   ↑ color: rgba(245,240,230,0.7) cream/70                      │
│                                                                │
│   ...                                                          │
│   ...                                                          │
│   ...                                                          │
│                                                                │
│   Teleox.ai                                  teleox.ai         │
│   ↑ Inter SemiBold, 32px,                    ↑ Inter Regular,  │
│     cream + amber period                       16px, cream/60  │
│   (bottom: 60px, left: 80px)                  (bottom: 60px,   │
│                                                  right: 80px)  │
│                                                                │
└──────────────────────────────────────────────────────────────┘

Background: solid #111713 (--color-ink-1)
```

### 2.3 Production options

**Option A: Static export from Figma**
- Design in Figma at 1200×630 with the exact specs above
- Export as PNG, run through ImageOptim or similar to get under 80 KB

**Option B: Programmatic generation via Vercel OG Image API (`@vercel/og`)**
- Create `src/app/og-image/route.tsx` that returns a generated image
- Pros: change once via code, no Figma round-trip
- Cons: uses Vercel runtime; may add cold-start cost

For the upgrade, **Option A is recommended** — a static PNG file is cacheable indefinitely, has zero runtime cost, and Vercel's edge cache delivers it fast.

### 2.4 Alt text

For OG meta tag:

```
"Teleox.ai — Models can be opaque. Pipelines shouldn't be. Independent research on verifiable AI training pipelines."
```

---

## 3. Asset to PRODUCE (optional): author photo for /about

Only if the decision in `09_page_about.md` §7 is to include a photo.

| Property | Value |
|---|---|
| File path | `public/images/chris-royse.jpg` (or `.webp`) |
| Dimensions | 240 × 240 pixels (displayed at 120×120, 2x for retina) |
| Format | JPEG quality 85 OR WebP quality 85 |
| File size target | ≤ 30 KB |
| Style | Headshot, eye-level, neutral expression, plain background (preferably the page background color or studio neutral) |

If no photo is provided, /about omits the image area entirely.

---

## 4. Fonts

All fonts are loaded via `next/font/google` in `layout.tsx`. No custom font files in `public/`.

| Family | Loaded as | Weights | Subsets |
|---|---|---|---|
| Inter | `next/font/google` | variable | latin |
| Source Serif 4 | `next/font/google` | variable | latin |
| JetBrains Mono | `next/font/google` | variable | latin |

If self-hosting fonts becomes preferred (for performance or privacy), the implementer can:
1. Download the font files from Google Fonts or the official sources
2. Place in `public/fonts/`
3. Replace `next/font/google` imports with `next/font/local`

This is **not required** for the upgrade.

---

## 5. Diagrams (none required)

The site does not need any diagrams for the new pages. The architecture description is in the paper itself.

If a future iteration adds an architecture diagram on `/research` or `/paper`, render as inline SVG with `currentColor` strokes — never as a PNG. This keeps it crisp at any size and inherits dark-mode color.

---

## 6. Icons (Lucide React only)

No icon files in `public/`. Icons are imported from `lucide-react`:

```tsx
import { ArrowRight, ExternalLink, Mail, Download, Menu, X } from "lucide-react";
```

The four icons whitelisted by `04_visual_design_system.md` §6.3:
- `ArrowRight`
- `ExternalLink`
- `Mail`
- `Download`

Plus two for nav:
- `Menu` (hamburger)
- `X` (close)

Total: 6 icons, all from Lucide. No custom SVG.

---

## 7. Favicon set

Existing favicon set in `public/`:

| File | Purpose |
|---|---|
| `favicon.ico` | Legacy browsers |
| `icon.svg` | Modern browsers (scalable) |
| `icon.png` | 512×512 PNG fallback |
| `apple-touch-icon.png` | iOS home screen |

These are referenced from `layout.tsx` metadata.icons (see `14_metadata_seo_sitemap.md` §1). No changes needed if they already match the brand.

If a fresh favicon is wanted to match the wordmark style:

- A simple `T` glyph in Source Serif 4, set in `--color-cream` on `--color-ink-1` background
- 32×32 base size, exported at 16, 32, 64, 128, 192, 512
- Tools: `realfavicongenerator.net` from a single source SVG

---

## 8. Static file: `public/_retired/`

This directory exists per `globals.css`:

```css
@source not "../../public/_retired";
```

Tailwind v4 explicitly excludes it from class extraction. Use it as a graveyard for assets you want to keep around but not deploy:

- Old OG images (move replaced ones here)
- Retired-page assets

These are blocked from crawling by `robots.txt` disallow.

---

## 9. Image optimization

Next.js's `Image` component handles automatic optimization. For pages that include images:

```tsx
import Image from "next/image";

<Image
  src="/images/chris-royse.jpg"
  alt="Chris Royse"
  width={120}
  height={120}
  className="rounded-full"
  priority={false}
/>
```

For OG images, the Open Graph meta tag references the static URL — Next does not optimize OG images automatically.

---

## 10. CDN and caching

Vercel automatically serves files from `public/` via its CDN with appropriate cache headers (the existing `next.config.ts` `headers()` function already configures cache-control). No additional configuration needed.

The OG image specifically gets:

```
Cache-Control: public, max-age=86400, must-revalidate
```

…via the favicon-pattern header rule. If a longer cache is desired (since the OG image rarely changes), update `next.config.ts` `headers()`:

```ts
{
  source: "/og-image.png",
  headers: [
    { key: "Cache-Control", value: "public, max-age=2592000, immutable" },
  ],
},
```

(30 days; only do this if you don't expect to change the OG image often.)

---

## 11. Asset checklist before launch

- [ ] `og-image.png` created and placed in `public/`
- [ ] `og-image.png` is ≤ 80 KB
- [ ] `manifest.json` updated with new name + description + theme color
- [ ] `llms.txt` updated to reflect new positioning
- [ ] `humans.txt` updated with current credits
- [ ] All retired-page assets removed from `public/standard/`, `public/mark/`, etc. (or moved to `_retired/`)
- [ ] No unused images in `public/images/`
- [ ] Favicon set is intact (icon.svg, icon.png, apple-touch-icon.png, favicon.ico)
- [ ] Twitter card validator shows OG image correctly
- [ ] LinkedIn post inspector shows OG image correctly
- [ ] OpenGraph.xyz debugger shows correct OG metadata
