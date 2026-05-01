# 21 — AI Agent Execution Plan

**Authority over:** the operational, atomic phase-by-phase work order designed for an AI coding agent to execute without losing context between sessions.
**Differs from `18_implementation_plan.md`:** that doc is human-oriented and time-budgeted; this doc is AI-agent-oriented and atomic-task-budgeted, with explicit context windows, definition-of-done checks, and resumability.

---

## How to use this document

Each phase below is **self-contained** and **resumable**. An AI agent can:

- Pick up any phase cold (read this doc + the specific spec docs it references)
- Execute it without reference to the other phases in flight
- Verify completion via the explicit "Definition of done" checklist
- Hand off to the next phase by telling the user "Phase N complete; ready for Phase N+1"

If a phase is too large, the agent should split it into sub-phases (1.a, 1.b) without modifying the canonical phase numbers.

**Critical rule:** the agent should not jump ahead. Each phase has dependencies; running them out of order will cause type errors or broken builds.

---

## Phase 0 — Pre-flight (must run first)

### Scope
Verify the working environment is sane before starting.

### Inputs
- Read: `README.md` (this directory), `00_README.md` (this dir, the index)
- Verify access to `/home/cabdru/teleoxwebsite/teleox-website/`

### Steps
1. Confirm git is clean: `cd /home/cabdru/teleoxwebsite/teleox-website && git status`
2. Confirm node version: `node --version` (expect v22.x or v24.x)
3. Confirm dependencies installed: `ls node_modules/ | head` (expect non-empty)
4. Confirm dev server can start: `npm run dev` then visit `http://localhost:3000` — should render the current (pre-upgrade) site. Kill the dev server.
5. Confirm `npm run build` succeeds against the current main: `npm run build` (this takes 1–3 minutes)
6. Confirm Vercel CLI works: `npx vercel --version`
7. Create the upgrade branch: `git checkout -b upgrade/research-positioning-v3`

### Definition of done
- [ ] On a fresh branch `upgrade/research-positioning-v3`
- [ ] Current main builds successfully
- [ ] Dev server runs locally
- [ ] No uncommitted changes from prior work

### Gotchas
- If `node_modules` is stale (after recent dependency changes), run `npm install` first
- If `npm run build` fails on main, the upgrade is not the cause; fix first or escalate to user

---

## Phase 1 — Update content schema and canonical routes

### Scope
Type definitions and the canonical-routes registry. No UI changes yet.

### Inputs
- Read: `13_content_schema.md` §1, §3, §4, §5

### Steps

1. Read current `src/types/route-copy.ts` to confirm what fields already exist
2. Update `src/types/route-copy.ts`:
   - Narrow `PlayerClass` to `"lab" | "researcher" | "public"`
   - Add optional fields per `13_content_schema.md` §1.2: `artifactCards`, `results`, `bulletBlocks`, `jsonLd`
3. Update `src/lib/canonical-routes.mjs` to the 5-route list:
   ```js
   export const CANONICAL_ROUTES = [
     "/",
     "/research",
     "/paper",
     "/about",
     "/contact",
   ];
   ```
4. Run `npm run typecheck` — expect errors in retired-route content files (this is correct; they'll be deleted in Phase 6)

### Definition of done
- [ ] `src/types/route-copy.ts` has narrowed PlayerClass
- [ ] `src/types/route-copy.ts` has new optional fields
- [ ] `src/lib/canonical-routes.mjs` has 5-route list
- [ ] `npm run typecheck` errors are limited to retired routes (method.ts, infrastructure.ts, etc.) — no errors in `home.ts`, `about.ts`, `paper.ts`, `contact.ts`, types files, or libs

### Commit
`feat(types): narrow PlayerClass enum and add upgrade content fields`

---

## Phase 2 — Build new components

### Scope
Create the 5 new shared components: PageContainer, ResearchHero, Section, ResultsBox, ArtifactCard. (Plus the new Header and Footer, which replace existing files.)

### Inputs
- Read: `04_visual_design_system.md` §3, §9 (typography, CTAs)
- Read: `05_component_library.md` §2–§8 (full component specs)
- Read: `12_navigation_and_footer.md` §1 (header), §2 (footer), §3 (layout shell)

### Steps

For each component below, create the file with the verbatim spec from `05_component_library.md`. Copy the JSX block exactly.

1. `src/components/layout/PageContainer.tsx` (per `05_component_library.md` §2.2)
2. `src/components/hero/ResearchHero.tsx` (per `05_component_library.md` §3.2)
3. `src/components/content-blocks/Section.tsx` (per `05_component_library.md` §4.2 — verify if exists; replace if older API)
4. `src/components/content-blocks/ResultsBox.tsx` (per `05_component_library.md` §5.2)
5. `src/components/content-blocks/ArtifactCard.tsx` (per `05_component_library.md` §6.2)
6. `src/components/layout/Header.tsx` (per `12_navigation_and_footer.md` §1.6) — replaces existing
7. `src/components/layout/Footer.tsx` (per `12_navigation_and_footer.md` §2.4) — replaces existing
8. Verify `src/components/layout/RootLayoutShell.tsx` wires Header + Footer correctly (per `12_navigation_and_footer.md` §3)

### Verify after each file

- TypeScript: `npm run typecheck` should not introduce new errors
- ESLint: `npm run lint` should pass for the file

### Definition of done
- [ ] All 8 components created/updated
- [ ] `npm run typecheck` shows no new errors
- [ ] `npm run lint` passes
- [ ] Components compile (try importing in a test file)

### Commit
`feat(components): build PageContainer, ResearchHero, Section, ResultsBox, ArtifactCard, Header, Footer for research-positioning upgrade`

---

## Phase 3 — Update root layout and JSON-LD

### Scope
Update `layout.tsx` metadata and `json-ld.ts` schemas.

### Inputs
- Read: `14_metadata_seo_sitemap.md` §1 (root metadata), §4 (JSON-LD schemas)

### Steps

1. Update `src/app/layout.tsx`:
   - Replace `metadata` block per `14_metadata_seo_sitemap.md` §1
   - Add the skip-link inside `<body>` per `12_navigation_and_footer.md` §4
   - Verify `<JsonLd>` blocks remain (organization, website schemas — values updated in step 2)
2. Update `src/lib/json-ld.ts`:
   - Replace `organizationSchema` per `14_metadata_seo_sitemap.md` §4.1
   - Replace `websiteSchema` per `14_metadata_seo_sitemap.md` §4.2
   - Add `personSchema` per §4.3
   - Add `paperSchema` per §4.4
   - Add `researchCollectionSchema` per §4.5

### Definition of done
- [ ] `layout.tsx` metadata reflects new positioning
- [ ] Skip link added inside `<body>`
- [ ] `json-ld.ts` exports 5 schemas: organization, website, person, paper, researchCollection
- [ ] `npm run typecheck` passes
- [ ] `npm run build` succeeds (this is a real test; if metadata is malformed, build fails)

### Commit
`feat(seo): update root metadata and JSON-LD schemas for research positioning`

---

## Phase 4 — Reposition homepage

### Scope
Replace `home.ts` and `app/page.tsx` for the new homepage.

### Inputs
- Read: `06_page_homepage.md` (full spec) — every section, especially §4 (RouteCopy file) and §5 (page.tsx file)

### Steps

1. Replace `src/content/routes/home.ts` with the verbatim content from `06_page_homepage.md` §4
2. Replace `src/app/page.tsx` with the verbatim content from `06_page_homepage.md` §5
3. Run `npm run dev`, visit `http://localhost:3000`, verify:
   - Hero renders headline + subhead at correct size
   - Three sections render with top dividers
   - ResultsBox renders with bolded numbers
   - Both CTA pairs (top + bottom) render correctly
   - No console errors

### Definition of done
- [ ] `home.ts` has the new RouteCopy content verbatim
- [ ] `page.tsx` renders the homepage matching the ASCII mockup in `06_page_homepage.md` §8
- [ ] Lighthouse scores ≥ 95 on performance and accessibility
- [ ] `npm run scan:all` passes for the homepage (or fails on retired routes — that's Phase 6)

### Commit
`feat(home): reposition homepage for frontier-lab research audience`

---

## Phase 5 — Build /research, reposition /paper, /about, /contact, error pages

### Scope
The four other live pages and the error pages.

### Inputs
- Read: `07_page_research.md`, `08_page_paper.md`, `09_page_about.md`, `10_page_contact.md`, `11_page_404_error.md`

### Steps

For each page below, follow the same pattern as Phase 4:

1. **`/research` (NEW)**:
   - Create `src/content/routes/research.ts` per `07_page_research.md` §4
   - Create `src/app/research/page.tsx` per `07_page_research.md` §5
   - Add `export { researchCopy } from "./research";` to `src/content/routes/index.ts`

2. **`/paper`**:
   - Replace `src/content/routes/paper.ts` per `08_page_paper.md` §4
   - Replace `src/app/paper/page.tsx` per `08_page_paper.md` §5

3. **`/about`**:
   - Replace `src/content/routes/about.ts` per `09_page_about.md` §4
   - Replace `src/app/about/page.tsx` per `09_page_about.md` §5

4. **`/contact`**:
   - Replace `src/content/routes/contact.ts` per `10_page_contact.md` §4
   - Replace `src/app/contact/page.tsx` per `10_page_contact.md` §5

5. **Error pages**:
   - Replace `src/app/not-found.tsx` per `11_page_404_error.md` §1.2
   - Replace `src/app/error.tsx` per `11_page_404_error.md` §2.2
   - Replace `src/app/global-error.tsx` per `11_page_404_error.md` §3.2

### Verify after each page
- Visit the route in dev: layout matches mockup, no console errors

### Definition of done
- [ ] All 4 routes (`/research`, `/paper`, `/about`, `/contact`) render correctly in dev
- [ ] 404 page renders correctly when visiting nonexistent URL
- [ ] No new TypeScript errors
- [ ] Lighthouse passes on each new page

### Commits (one per page)
- `feat(routes): add /research page`
- `feat(paper): reposition /paper for research audience`
- `feat(about): reposition /about for research audience`
- `feat(contact): reposition /contact for research audience`
- `feat(errors): reposition 404, error, global-error pages`

---

## Phase 6 — Retire old routes

### Scope
Delete 17 retired route folders and their content files. Add redirects.

### Inputs
- Read: `02_information_architecture.md` §4 (redirects), §9 (files to delete)

### Steps

1. Delete the route folders:
   ```bash
   cd /home/cabdru/teleoxwebsite/teleox-website
   rm -rf src/app/{method,methodology,infrastructure,measured,measurements,pricing,sign,standard,verify,governance,mark,centroids,citations,feed,artifacts,priority-date,evidence-map}
   ```

2. Delete the content files:
   ```bash
   rm src/content/routes/{method.ts,infrastructure.ts,measured.ts,pricing.ts,sign.ts,standard.ts,verify.ts,governance.ts,mark.ts,centroids.ts,citations.ts,feed.ts,artifacts.ts,priority-date.ts,alignment-index.ts}
   ```

3. Update `src/content/routes/index.ts` to export only:
   ```ts
   export { homeCopy } from "./home";
   export { researchCopy } from "./research";
   export { paperCopy } from "./paper";
   export { aboutCopy } from "./about";
   export { contactCopy } from "./contact";
   ```

4. Add the `V2_TO_V3_REDIRECTS` array per `02_information_architecture.md` §4 to `src/lib/redirects.ts`
5. Wire the array into `next.config.ts` `redirects()` (alongside existing arrays)
6. Run `npm run typecheck` — should now have ZERO errors (since the broken types in retired files are gone)
7. Run `npm run build` — should succeed
8. Test redirects in dev:
   - `localhost:3000/method` → 308 to `/research`
   - `localhost:3000/pricing` → 308 to `/contact`
   - All 17 retired routes redirect correctly

### Definition of done
- [ ] All 17 retired route folders deleted
- [ ] All 15 retired content files deleted
- [ ] `src/content/routes/index.ts` exports only 5 routes
- [ ] `V2_TO_V3_REDIRECTS` added to `redirects.ts`
- [ ] Wired into `next.config.ts`
- [ ] `npm run typecheck` shows 0 errors
- [ ] `npm run build` succeeds
- [ ] Test redirects in dev: 308 to mapped destination for each retired route

### Commit
`chore(routes): retire 17 commercial routes with 308 redirects`

---

## Phase 7 — Update SEO surfaces

### Scope
Sitemap, robots, llms.txt, manifest.

### Inputs
- Read: `02_information_architecture.md` §7 (sitemap), §8 (robots)
- Read: `14_metadata_seo_sitemap.md` §5 (sitemap), §6 (robots), §7 (llms.txt), §8 (manifest)

### Steps

1. Replace `src/app/sitemap.ts` per `02_information_architecture.md` §7
2. Replace `src/app/robots.ts` per `02_information_architecture.md` §8
3. Replace `public/llms.txt` per `14_metadata_seo_sitemap.md` §7
4. Replace `public/manifest.json` per `14_metadata_seo_sitemap.md` §8
5. Optionally update `public/humans.txt` per `14_metadata_seo_sitemap.md` §9
6. Run `npm run dev`, visit:
   - `http://localhost:3000/sitemap.xml` — should list 7 entries
   - `http://localhost:3000/robots.txt` — should disallow `/y-lecun/`, `/k-cho/`, `/ami/`
   - `http://localhost:3000/llms.txt` — should reflect new positioning
   - `http://localhost:3000/manifest.json` — should have new name + description

### Definition of done
- [ ] `sitemap.xml` lists 7 entries
- [ ] `robots.txt` disallows private slugs
- [ ] `llms.txt` reflects new positioning
- [ ] `manifest.json` updated
- [ ] No build errors

### Commit
`chore(seo): update sitemap, robots, llms.txt, manifest`

---

## Phase 8 — Produce OG image

### Scope
Create the new `public/og-image.png` per `15_assets_inventory.md` §2.

### Inputs
- Read: `15_assets_inventory.md` §2

### Steps

The AI agent has limited ability to produce a PNG file. Two options:

**Option A: Generate programmatically**
- Create `src/app/og-image/route.tsx` using `@vercel/og` (Next.js OG Image API)
- Generate the image dynamically per `15_assets_inventory.md` §2.2 spec
- Update root metadata `openGraph.images[0].url` to point to `/og-image` route

**Option B: Skip for this upgrade; user produces externally**
- Leave existing `public/og-image.png` in place
- Open a follow-up TODO for the user to produce the new image in Figma
- Document in commit message

For an AI agent without image-generation capability: **Option A is preferred** — the agent can write the TSX route file and Vercel handles the image generation at request time.

### Steps for Option A

1. Create `src/app/og-image/route.tsx`:
   ```tsx
   import { ImageResponse } from "next/og";

   export const runtime = "edge";

   export async function GET() {
     return new ImageResponse(
       (
         <div
           style={{
             height: "100%",
             width: "100%",
             display: "flex",
             flexDirection: "column",
             justifyContent: "space-between",
             padding: "80px",
             background: "#111713",
             color: "#F5F0E6",
             fontFamily: "Georgia, serif",
           }}
         >
           <div>
             <div style={{ fontSize: 64, fontWeight: 600, lineHeight: 1.05 }}>
               Models can be opaque.
               <br />
               Pipelines shouldn't be.
             </div>
             <div style={{ width: 60, height: 1, background: "#F3B85B", marginTop: 32 }} />
             <div
               style={{
                 fontSize: 28,
                 lineHeight: 1.4,
                 color: "rgba(245,240,230,0.7)",
                 marginTop: 24,
               }}
             >
               Independent research on verifiable AI training pipelines.
             </div>
           </div>
           <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-end" }}>
             <div style={{ fontFamily: "system-ui, sans-serif", fontSize: 32, fontWeight: 600 }}>
               Teleox<span style={{ color: "#F3B85B" }}>.</span>ai
             </div>
             <div style={{ fontFamily: "system-ui, sans-serif", fontSize: 16, color: "rgba(245,240,230,0.6)" }}>
               teleox.ai
             </div>
           </div>
         </div>
       ),
       { width: 1200, height: 630 },
     );
   }
   ```

2. Update root metadata `openGraph.images[0].url` to `https://teleox.ai/og-image`

### Definition of done
- [ ] Either Option A: `src/app/og-image/route.tsx` exists and returns a valid PNG when visited at `/og-image`
- [ ] Or Option B: existing `og-image.png` left in place; user task created for replacement
- [ ] Open Graph debugger renders the image correctly

### Commit
`feat(seo): generate dynamic OG image via @vercel/og` (Option A)
or
`chore: defer OG image refresh to follow-up task` (Option B)

---

## Phase 9 — Verify private slug pages still in place

### Scope
The three private outreach pages (`/y-lecun/`, `/k-cho/`, `/ami/`) and their PDFs should be untouched by this upgrade. Verify.

### Inputs
- N/A; just file existence checks

### Steps

```bash
ls public/y-lecun/index.html public/y-lecun/ME-JEPAResearchPaper.pdf
ls public/k-cho/index.html public/k-cho/ME-JEPAResearchPaper.pdf
ls public/ami/index.html public/ami/ME-JEPAResearchPaper.pdf
```

All 6 files should exist. Spot-check one HTML file renders correctly when visited locally:

```bash
curl -s http://localhost:3000/y-lecun/ | head -20
```

Should return the HTML for the LeCun preview page.

### Definition of done
- [ ] All 6 files (3 HTML + 3 PDFs) present
- [ ] All 3 routes accessible in dev
- [ ] All listed in `robots.txt` disallow

### Commit
None — verification only.

---

## Phase 10 — Local validation gauntlet

### Scope
Run every local validator and ensure the entire branch is green.

### Inputs
- Read: `17_validation_and_scanners.md` §8 (pre-deploy checklist)

### Steps

```bash
cd /home/cabdru/teleoxwebsite/teleox-website
npm run typecheck
npm run lint
npm run validate:content
npm run validate:assets
npm run test
npm run scan:all
npm run build
```

Each command must exit 0.

If any fail, **do not proceed to deploy**. Fix and re-run.

### Definition of done
- [ ] All 7 commands exit 0
- [ ] No warnings that would indicate latent issues

### Commit
None — verification only.

---

## Phase 11 — Deploy preview to Vercel

### Scope
Push the branch, capture the Vercel preview URL.

### Inputs
- Read: `19_deployment_rollout.md` §3 (auth), §4 (preview deploy), §5 (verify URLs)

### Steps

1. Confirm the branch is committed and pushed:
   ```bash
   git status                                # should be clean
   git push origin upgrade/research-positioning-v3
   ```

2. Vercel auto-builds on push. Wait for build to complete (~1–3 min).

3. Capture the preview URL from Vercel dashboard or via CLI:
   ```bash
   npx vercel ls --token=$VERCEL_TOKEN | head -5
   ```

4. Run the verification script from `19_deployment_rollout.md` §5 against the preview URL.

### Definition of done
- [ ] Branch pushed
- [ ] Vercel preview URL captured
- [ ] All 5 main routes return 200 on preview
- [ ] All 17 retired routes return 308 on preview
- [ ] Sitemap, robots, llms.txt, manifest all return correctly on preview
- [ ] Private slugs (`/y-lecun/`, `/k-cho/`, `/ami/`) all return 200 on preview

### Commit
None — deploy operation.

---

## Phase 12 — Manual cross-browser verification

### Scope
Click through the preview URL in multiple browsers and devices.

### Inputs
- Read: `19_deployment_rollout.md` §6 (visual verification)

### Steps

Open the preview URL in:
1. Desktop Chrome (1920×1080)
2. Desktop Safari (1440×900)
3. Mobile Chrome (DevTools mobile emulation OR real device)

For each, verify:
- Homepage hero renders correctly
- Mobile drawer opens/closes (mobile only)
- All nav links work
- All CTAs work
- All external links open in new tab
- No layout shift after font load
- No console errors
- Footer renders correctly

This is **manual** — the AI agent should ask the user to perform this verification, or the user can delegate to playwright.

### Definition of done
- [ ] User confirms preview looks correct on all tested devices
- [ ] No layout / functional regressions noted

---

## Phase 13 — Lighthouse + Open Graph verification

### Scope
Performance, accessibility, SEO, and OG meta verification.

### Inputs
- Read: `19_deployment_rollout.md` §7 (Lighthouse), §8 (OG verification)

### Steps

1. Run Lighthouse against preview URL on Chrome DevTools (or `npx lighthouse <preview-url>`):
   - Performance ≥ 95
   - Accessibility ≥ 95
   - Best Practices ≥ 95
   - SEO 100

2. Paste preview URL into:
   - https://www.opengraph.xyz/
   - https://cards-dev.twitter.com/validator
   - https://www.linkedin.com/post-inspector/inspect/

3. Verify each renders the OG image and correct title/description.

### Definition of done
- [ ] All 4 Lighthouse scores at or above target
- [ ] OG meta renders correctly on all 3 social validators

---

## Phase 14 — Promote to production

### Scope
Merge the branch to `main`, trigger production deploy.

### Inputs
- Read: `19_deployment_rollout.md` §9 (promote)

### Steps

```bash
git checkout main
git merge upgrade/research-positioning-v3 --ff-only
git push origin main
```

Vercel auto-deploys main → production. Wait for build to complete (~1–3 min).

### Definition of done
- [ ] Branch merged to main with fast-forward
- [ ] Production deploy succeeded in Vercel dashboard

---

## Phase 15 — Verify production

### Scope
Same verification as Phase 11 but against `https://teleox.ai`.

### Inputs
- Read: `19_deployment_rollout.md` §10 (verify production)

### Steps

Run the verification script from `19_deployment_rollout.md` §5, replacing `PREVIEW_URL` with `https://teleox.ai`.

### Definition of done
- [ ] All 5 main routes return 200 in production
- [ ] All 17 retired routes return 308 in production
- [ ] Sitemap, robots, llms.txt, manifest all serve correctly
- [ ] Private slugs accessible
- [ ] No 404s in Vercel logs in first hour

---

## Phase 16 — Post-deploy housekeeping

### Scope
Submit sitemap, rotate PAT, notify user.

### Inputs
- Read: `19_deployment_rollout.md` §11 (post-deploy), §16 (PAT rotation)

### Steps

1. Submit `https://teleox.ai/sitemap.xml` to Google Search Console
2. Rotate the Vercel PAT used for the deploy
3. Notify the user that the deploy is complete; provide:
   - Production URL: https://teleox.ai
   - Preview URL (for reference)
   - Summary of what shipped

### Definition of done
- [ ] Sitemap submitted to GSC
- [ ] PAT rotated
- [ ] User notified with deploy summary

---

## Phase ⊥ (rollback) — Only if needed

### When to run

If a critical issue is found post-deploy that cannot be hot-fixed within 30 minutes.

### Steps

Per `19_deployment_rollout.md` §12. Use Vercel dashboard rollback (instant) for fastest recovery, then investigate.

---

## Total phase count

**16 phases** to ship + 1 rollback contingency.

For a focused AI agent, each phase is a 30-minute to 2-hour block. The full upgrade is achievable in 1–2 working days of agent time.

---

## Resumability checklist

Between sessions, the next agent should be able to identify "where am I?" by:

1. Running `git log --oneline -20` — recent commits indicate which phases completed
2. Running `git status -sb` — current branch + dirty state
3. Running the Phase 10 validation gauntlet — if it passes, you're past Phase 10
4. Asking the user "what was the last phase confirmed complete?"

Each phase commit message indicates which phase finished. The phase IDs in commits (e.g., `feat(home): reposition homepage` → corresponds to Phase 4) make resumption straightforward.

---

## When this plan is wrong

The plan is wrong when:

- **A phase fails verification and the AI tries to ship anyway.** Do not. Fix or escalate.
- **A spec doc contradicts another spec doc.** Both can't be right. Escalate to user; do not pick one and proceed.
- **The user changes requirements mid-phase.** Pause the phase, document the change, get explicit re-confirmation before resuming.
- **The build environment doesn't match the spec.** E.g., Tailwind v4 was downgraded. Stop and reconcile.

The plan is right by default; the world can change. Pause and re-confirm rather than improvising.
