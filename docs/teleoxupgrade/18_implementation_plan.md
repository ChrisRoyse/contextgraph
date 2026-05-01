# 18 — Implementation Plan

**Authority over:** the order of operations for the implementing AI / engineer.
**Goal:** ship the upgrade as one atomic change set with zero half-states visible to the public.

---

## 1. Workflow choice

### 1.1 Branch strategy

Create a feature branch off `main`:

```bash
cd /home/cabdru/teleoxwebsite/teleox-website
git checkout -b upgrade/research-positioning-v3
```

All edits happen on this branch. Vercel automatically gives a preview deploy URL. No work happens on `main` until the preview is approved.

### 1.2 Commit cadence

One commit per logical step (per phase below). Use Conventional Commits:

```
feat(home): reposition homepage hero and body for research audience
feat(routes): add /research page and content
chore(routes): retire 17 commercial routes with redirects
fix(seo): update sitemap, robots, llms.txt, manifest
chore(content): update mustNotSay across all routes
```

Each commit should leave the codebase in a buildable state.

---

## 2. Phase 1 — Foundation (no public-facing changes yet)

These changes can happen in any order; they're additive to types and infrastructure.

### Step 1.1 Update content types

- File: `src/types/route-copy.ts`
- Change: narrow `PlayerClass` enum to `"lab" | "researcher" | "public"`. Add optional `artifactCards`, `results`, `bulletBlocks`, `jsonLd` fields per `13_content_schema.md` §1.2.
- Verify: `npm run typecheck` errors will appear in retired-route files — that's the signal to delete them in Phase 3.

### Step 1.2 Update canonical routes list

- File: `src/lib/canonical-routes.mjs`
- Change: replace the array with the 5-route list from `13_content_schema.md` §5.

### Step 1.3 Build the new components

Per `05_component_library.md`, create or verify:

- `src/components/layout/PageContainer.tsx` (new or update)
- `src/components/hero/ResearchHero.tsx` (new)
- `src/components/content-blocks/Section.tsx` (verify exists; build if not)
- `src/components/content-blocks/ResultsBox.tsx` (new)
- `src/components/content-blocks/ArtifactCard.tsx` (new)
- `src/components/layout/Header.tsx` (update — replace with new spec)
- `src/components/layout/Footer.tsx` (update — replace with new spec)
- `src/components/layout/RootLayoutShell.tsx` (verify wires Header + Footer)

Add unit tests for new components in `tests/components/`.

### Step 1.4 Update root layout

- File: `src/app/layout.tsx`
- Change: update `metadata` per `14_metadata_seo_sitemap.md` §1
- Change: add skip-link inside `<body>` per `12_navigation_and_footer.md` §4
- Change: ensure `JsonLd` for `organizationSchema` and `websiteSchema` use the updated schemas from `14_metadata_seo_sitemap.md` §4.1–4.2

### Step 1.5 Update json-ld.ts

- File: `src/lib/json-ld.ts`
- Change: replace `organizationSchema`, `websiteSchema`. Add `personSchema`, `paperSchema`, `researchCollectionSchema`. Per `14_metadata_seo_sitemap.md` §4.

---

## 3. Phase 2 — New and repositioned pages

### Step 2.1 Homepage

- File: `src/content/routes/home.ts`
- Action: replace entire file with spec from `06_page_homepage.md` §4
- File: `src/app/page.tsx`
- Action: replace entire file with spec from `06_page_homepage.md` §5

### Step 2.2 /research (NEW)

- File: `src/content/routes/research.ts`
- Action: create new file with spec from `07_page_research.md` §4
- File: `src/app/research/page.tsx`
- Action: create new file with spec from `07_page_research.md` §5
- Wire into `src/content/routes/index.ts` exports

### Step 2.3 /paper

- File: `src/content/routes/paper.ts`
- Action: replace entire file with spec from `08_page_paper.md` §4
- File: `src/app/paper/page.tsx`
- Action: replace entire file with spec from `08_page_paper.md` §5

### Step 2.4 /about

- File: `src/content/routes/about.ts`
- Action: replace entire file with spec from `09_page_about.md` §4
- File: `src/app/about/page.tsx`
- Action: replace entire file with spec from `09_page_about.md` §5

### Step 2.5 /contact

- File: `src/content/routes/contact.ts`
- Action: replace entire file with spec from `10_page_contact.md` §4
- File: `src/app/contact/page.tsx`
- Action: replace entire file with spec from `10_page_contact.md` §5

### Step 2.6 404 / error pages

- Files: `src/app/not-found.tsx`, `src/app/error.tsx`, `src/app/global-error.tsx`
- Action: replace per `11_page_404_error.md`

### Verification at end of Phase 2

- `npm run build` succeeds
- All 5 new/repositioned pages render correctly in dev (`npm run dev`)
- `npm run scan:all` passes (or fails on retired-route content — fix in Phase 3)

---

## 4. Phase 3 — Retire old routes

### Step 3.1 Delete retired route folders

```bash
cd /home/cabdru/teleoxwebsite/teleox-website
rm -rf src/app/method
rm -rf src/app/methodology
rm -rf src/app/infrastructure
rm -rf src/app/measured
rm -rf src/app/measurements
rm -rf src/app/pricing
rm -rf src/app/sign
rm -rf src/app/standard
rm -rf src/app/verify
rm -rf src/app/governance
rm -rf src/app/mark
rm -rf src/app/centroids
rm -rf src/app/citations
rm -rf src/app/feed
rm -rf src/app/artifacts
rm -rf src/app/priority-date
rm -rf src/app/evidence-map
```

### Step 3.2 Delete retired content files

```bash
rm src/content/routes/method.ts
rm src/content/routes/infrastructure.ts
rm src/content/routes/measured.ts
rm src/content/routes/pricing.ts
rm src/content/routes/sign.ts
rm src/content/routes/standard.ts
rm src/content/routes/verify.ts
rm src/content/routes/governance.ts
rm src/content/routes/mark.ts
rm src/content/routes/centroids.ts
rm src/content/routes/citations.ts
rm src/content/routes/feed.ts
rm src/content/routes/artifacts.ts
rm src/content/routes/priority-date.ts
rm src/content/routes/alignment-index.ts
```

### Step 3.3 Update route index exports

- File: `src/content/routes/index.ts`
- Action: remove all exports for deleted files. Final list per `13_content_schema.md` §4.

### Step 3.4 Add redirects

- File: `src/lib/redirects.ts`
- Action: add `V2_TO_V3_REDIRECTS` array (or extend existing) with all 17 retired→live mappings per `02_information_architecture.md` §4
- File: `next.config.ts`
- Action: include `V2_TO_V3_REDIRECTS` in the `redirects()` array

### Step 3.5 Verify

- `npm run typecheck` → 0 errors
- `npm run build` → success
- Visit `localhost:3000/method` → redirects to `/research`
- Visit `localhost:3000/pricing` → redirects to `/contact`
- All other retired routes redirect correctly

---

## 5. Phase 4 — SEO surfaces

### Step 4.1 Update sitemap

- File: `src/app/sitemap.ts`
- Action: replace ROUTES array per `02_information_architecture.md` §7

### Step 4.2 Update robots

- File: `src/app/robots.ts`
- Action: replace per `02_information_architecture.md` §8 (include `/y-lecun/`, `/k-cho/`, `/ami/` disallows)

### Step 4.3 Update llms.txt

- File: `public/llms.txt`
- Action: replace content per `14_metadata_seo_sitemap.md` §7

### Step 4.4 Update manifest

- File: `public/manifest.json`
- Action: replace content per `14_metadata_seo_sitemap.md` §8

### Step 4.5 Update humans.txt (optional)

- File: `public/humans.txt`
- Action: update credits per `14_metadata_seo_sitemap.md` §9

---

## 6. Phase 5 — Assets

### Step 5.1 Produce new OG image

Per `15_assets_inventory.md` §2. The image must be at `public/og-image.png` before the build runs.

If the OG image is not yet ready, ship the upgrade with the existing OG image and replace it later.

### Step 5.2 Audit and clean public/ directory

Per `15_assets_inventory.md` §1, remove:
- `public/standard/` (page retired)
- `public/mark/` (page retired)
- Any unused assets in `public/images/`

### Step 5.3 Verify favicon set

- `public/favicon.ico`, `public/icon.svg`, `public/icon.png`, `public/apple-touch-icon.png` all present
- All referenced from `layout.tsx` metadata

---

## 7. Phase 6 — Private-recipient slug pages

These already exist from previous outreach work:

```
public/y-lecun/index.html
public/y-lecun/ME-JEPAResearchPaper.pdf
public/k-cho/index.html
public/k-cho/ME-JEPAResearchPaper.pdf
public/ami/index.html
public/ami/ME-JEPAResearchPaper.pdf
```

Verify they are still in place after the cleanup in Phase 5.

Verify they are listed in `robots.txt` disallow.

---

## 8. Phase 7 — Validation

### Step 7.1 Local validation

```bash
cd /home/cabdru/teleoxwebsite/teleox-website
npm run typecheck         # 0 errors
npm run lint              # 0 errors
npm run validate:content  # 0 failures
npm run validate:assets   # 0 failures
npm run test              # all unit tests pass
npm run scan:all          # 0 violations
npm run build             # successful build
```

### Step 7.2 Local manual verification

Run dev server (`npm run dev`) and click through:

- `/` → renders new homepage
- `/research` → renders new research page with both ArtifactCards
- `/paper` → renders new paper page
- `/about` → renders new about page
- `/contact` → renders new contact page
- `/method`, `/pricing`, `/infrastructure`, etc. → 308 redirect to mapped destination
- `/this-does-not-exist` → 404 page
- Mobile drawer opens, navigates, closes
- All CTAs route correctly
- All external links open in new tab

### Step 7.3 Lighthouse

Run Lighthouse (Chrome DevTools or `npm run test:lhci`):

- Performance ≥ 95
- Accessibility ≥ 95
- Best Practices ≥ 95
- SEO 100

### Step 7.4 Accessibility

```bash
npm run test:a11y
```

Should show 0 axe-core violations on every page.

---

## 9. Phase 8 — Deploy preview to Vercel

Per `19_deployment_rollout.md`. Summary:

```bash
git add -A
git commit -m "feat(positioning): reposition Teleox.ai for frontier-lab research audience"
git push origin upgrade/research-positioning-v3
```

Vercel auto-builds the preview. Capture the preview URL.

---

## 10. Phase 9 — Manual review of preview

Open the preview URL on:
- Desktop Chrome
- Desktop Safari
- iPhone (real device or DevTools simulator)
- Android (DevTools simulator)

Test:
- All 5 pages render correctly
- Wordmark + nav layout correct
- Mobile drawer works
- All CTAs route correctly
- OG meta renders correctly when shared (Twitter card validator)
- Performance acceptable

---

## 11. Phase 10 — Promote to production

Per `19_deployment_rollout.md`. After preview approval:

```bash
git checkout main
git merge upgrade/research-positioning-v3 --ff-only
git push origin main
```

Vercel auto-deploys main → production.

Verify in production:
- All pages render
- All redirects work
- DNS / TLS intact
- Sitemap regenerated and accessible at `https://teleox.ai/sitemap.xml`
- robots.txt updated at `https://teleox.ai/robots.txt`

---

## 12. Phase 11 — Post-deploy housekeeping

- Submit new sitemap to Google Search Console
- Update social/profile links if any reference Teleox.ai
- Notify the implementer / Chris that the deploy is complete

---

## 13. Rollback plan

If a critical issue is discovered post-deploy:

```bash
# In Vercel dashboard: promote the previous deployment back to production
# OR via CLI:
vercel rollback --token=$VERCEL_TOKEN

# Or via git revert:
git revert <merge-commit-sha>
git push origin main
```

The retired-route content files are deleted in Phase 3; rolling back via git restores them. The redirect rules are also reverted.

---

## 14. Estimated effort

| Phase | Estimated time (one experienced dev) |
|---|---|
| Phase 1 (foundation) | 2–3 hours |
| Phase 2 (5 pages) | 3–4 hours |
| Phase 3 (retire) | 1 hour |
| Phase 4 (SEO surfaces) | 30 minutes |
| Phase 5 (assets) | 1–2 hours (mainly OG image) |
| Phase 6 (verify private slugs) | 15 minutes |
| Phase 7 (validation) | 1 hour |
| Phase 8 (deploy preview) | 15 minutes |
| Phase 9 (manual review) | 1 hour |
| Phase 10 (promote) | 15 minutes |
| Phase 11 (housekeeping) | 30 minutes |
| **Total** | **~10–14 hours of focused work** |

For an AI implementer, the same work proceeds without the same fatigue / context-switch cost — most of the time is review and verification.

---

## 15. Per-step task IDs (suggested for tracking)

```
upgrade-1.1  Update RouteCopy types
upgrade-1.2  Update canonical-routes.mjs
upgrade-1.3  Build PageContainer, ResearchHero, Section, ResultsBox, ArtifactCard
upgrade-1.4  Update Header.tsx, Footer.tsx, RootLayoutShell.tsx
upgrade-1.5  Update layout.tsx (metadata, skip link)
upgrade-1.6  Update json-ld.ts schemas
upgrade-2.1  Replace home.ts and home page.tsx
upgrade-2.2  Create /research route + content
upgrade-2.3  Replace paper.ts and paper page.tsx
upgrade-2.4  Replace about.ts and about page.tsx
upgrade-2.5  Replace contact.ts and contact page.tsx
upgrade-2.6  Replace 404, error, global-error
upgrade-3.1  Delete 17 retired route folders
upgrade-3.2  Delete 15 retired content files
upgrade-3.3  Update routes/index.ts exports
upgrade-3.4  Add V2_TO_V3 redirects
upgrade-4.1  Update sitemap.ts
upgrade-4.2  Update robots.ts
upgrade-4.3  Update llms.txt
upgrade-4.4  Update manifest.json
upgrade-4.5  Update humans.txt
upgrade-5.1  Produce og-image.png
upgrade-5.2  Audit / clean public/
upgrade-5.3  Verify favicon set
upgrade-6.1  Verify private slug pages intact
upgrade-7.1  Run all local validators
upgrade-7.2  Manual click-through
upgrade-7.3  Lighthouse
upgrade-7.4  Accessibility tests
upgrade-8.1  Deploy preview to Vercel
upgrade-9.1  Cross-browser preview review
upgrade-10.1 Promote to production
upgrade-11.1 Post-deploy housekeeping
```

The implementing AI should track progress via these IDs.
