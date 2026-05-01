# 19 — Deployment & Rollout

**Authority over:** Vercel deploy commands, preview verification, production promotion, and rollback.

---

## 1. Vercel project context

| Field | Value |
|---|---|
| Project name | `teleox-site` |
| Project ID | `prj_7mBKBjfn2MyRxnoByQC1wJqHxyyr` |
| Org / team ID | `team_feWZfMPXqDJy0hmnbqKUDg2u` |
| Framework | Next.js 16 |
| Production domain | `teleox.ai` |
| Node version | 22.x (per `package.json` engines), 24.x (per Vercel project settings) |
| Auto-deploy from main | Yes |

---

## 2. Vercel CLI installation

If not installed globally:

```bash
npm install -g vercel
# OR (no global install)
npx vercel --version
```

For consistent behavior, prefer `npx vercel` so the version matches what Vercel ships in cloud builds.

---

## 3. Authentication

Use the Vercel Personal Access Token (PAT) provided out-of-band. **Do not commit the PAT to any file.** Pass via env var:

```bash
export VERCEL_TOKEN=<paste-token-here>
```

After deploy completes, **rotate the token** at https://vercel.com/account/tokens. PATs are bearer credentials — treat them like passwords.

---

## 4. Preview deploy (recommended first step)

From the repo root (`/home/cabdru/teleoxwebsite/teleox-website`):

```bash
cd /home/cabdru/teleoxwebsite/teleox-website
npx vercel deploy --token=$VERCEL_TOKEN --yes
```

Output:

```
✓ Build completed in /vercel/output [44s]
✓ Deployment complete: https://teleox-site-<hash>-<branch>.vercel.app
```

Capture the preview URL. Test there before promoting.

---

## 5. Verify preview URLs

Once preview is up, run this verification script (or each command manually):

```bash
PREVIEW_URL="<paste-preview-url-from-vercel-output>"

# Each route should return 200
for path in "/" "/research" "/paper" "/about" "/contact"; do
  code=$(curl -s -o /dev/null -w "%{http_code}" -L "$PREVIEW_URL$path")
  printf "%s  %s\n" "$code" "$path"
done

# Each retired route should return 308
for path in "/method" "/pricing" "/infrastructure" "/measured" "/sign" "/standard" "/verify" "/governance" "/mark" "/centroids" "/citations" "/feed" "/artifacts" "/priority-date" "/evidence-map" "/methodology" "/measurements"; do
  code=$(curl -s -o /dev/null -w "%{http_code}" "$PREVIEW_URL$path")
  printf "%s  %s\n" "$code" "$path"
done

# 404 page
code=$(curl -s -o /dev/null -w "%{http_code}" -L "$PREVIEW_URL/this-does-not-exist")
printf "%s  /this-does-not-exist (expect 404)\n" "$code"

# Sitemap
curl -s "$PREVIEW_URL/sitemap.xml" | head -30

# Robots
curl -s "$PREVIEW_URL/robots.txt"

# Private slugs (should still serve, even though they noindex)
for slug in "y-lecun" "k-cho" "ami"; do
  code=$(curl -s -o /dev/null -w "%{http_code}" "$PREVIEW_URL/$slug/")
  pdf_code=$(curl -s -o /dev/null -w "%{http_code}" "$PREVIEW_URL/$slug/ME-JEPAResearchPaper.pdf")
  printf "%s %s  /%s/  +  PDF\n" "$code" "$pdf_code" "$slug"
done
```

Expected output:
- Live routes: 200 across the board
- Retired routes: 308 (redirect)
- 404 path: 404
- Sitemap renders the 7 entries
- Robots includes the private-slug disallows
- Private slugs serve their landing pages + PDFs (200)

---

## 6. Visual verification on preview

Open the preview URL in:

- Desktop Chrome (1920×1080)
- Desktop Safari (1440×900)
- iPad Air (Chrome DevTools device mode, 820×1180)
- iPhone 15 (DevTools, 390×844)
- Android Pixel (DevTools)

Click through every nav item. Verify:
- Layout renders without overflow
- Mobile drawer opens / closes
- Sticky header stays sticky
- All CTAs work
- All external links open in new tab
- Footer renders correctly

---

## 7. Lighthouse on preview

Run Lighthouse against the preview URL:

```bash
# Via Chrome DevTools: open preview URL in Chrome, F12 → Lighthouse → Generate report
# Via CLI:
npx lighthouse "$PREVIEW_URL" --output=html --output-path=lighthouse-preview-$(date +%Y%m%d).html
```

Targets per `06_page_homepage.md` §10:
- Performance ≥ 95
- Accessibility ≥ 95
- Best Practices ≥ 95
- SEO 100

If any score drops, investigate before promoting.

---

## 8. OG meta verification

Use:
- https://www.opengraph.xyz/ — paste preview URL
- https://cards-dev.twitter.com/validator — paste preview URL
- https://www.linkedin.com/post-inspector/inspect/ — paste preview URL

Each should render:
- Title: "Teleox.ai — Models can be opaque. Pipelines shouldn't be."
- Description: per spec
- Image: the new OG image (`og-image.png`)
- Domain: `teleox.ai`

If image doesn't appear, debug:
- Is the URL absolute (`https://teleox.ai/og-image.png`, not `/og-image.png`)?
- Is the file actually deployed (curl the URL)?
- Is it under the OG dimension limit (1200×630)?

---

## 9. Promote to production

After preview verification passes:

```bash
# Option A: via vercel CLI promote
npx vercel promote <preview-url> --token=$VERCEL_TOKEN

# Option B: via git merge to main (auto-deploy)
git checkout main
git merge upgrade/research-positioning-v3 --ff-only
git push origin main
```

Option B is cleaner because it keeps the git history straight and triggers a fresh production build with the same code. Option A is faster but may diverge if `main` has commits the preview didn't have.

**Recommended: Option B.**

---

## 10. Verify production

After production deploy completes (1–3 minutes):

```bash
PROD_URL="https://teleox.ai"

# Same verification script as §5 but against production
for path in "/" "/research" "/paper" "/about" "/contact"; do
  code=$(curl -s -o /dev/null -w "%{http_code}" -L "$PROD_URL$path")
  printf "%s  %s\n" "$code" "$path"
done
```

All 200s = production is healthy.

---

## 11. Post-deploy

### Step 11.1 Submit sitemap to Google Search Console

Go to https://search.google.com/search-console, select the `teleox.ai` property, navigate to Sitemaps, paste `https://teleox.ai/sitemap.xml`, click Submit.

Google should re-crawl within 24–72 hours.

### Step 11.2 Watch Vercel Analytics

For the next 48 hours, watch:
- Page views per route (homepage should dominate; research/paper next)
- Geographic distribution
- Bounce rate (homepage should be < 50% — research-y pages have higher bounce in absolute terms but the sticky reader is the metric that matters)
- Speed Insights — verify no performance regression

### Step 11.3 Monitor for 404s

Vercel logs will show any 404s in production. If retired routes are getting traffic, the redirect rules should catch them — but verify by looking at logs daily for the first week.

If a retired route doesn't redirect (typo in `redirects.ts`), it returns the friendly 404 page. Fix and redeploy.

---

## 12. Rollback (if needed)

### 12.1 Vercel dashboard rollback

Vercel keeps the last 100 deploys. To roll back:

1. Open https://vercel.com/<team>/<project>/deployments
2. Find the previous successful production deploy
3. Click "..." → "Promote to Production"

This is instant; no code changes required.

### 12.2 Git revert rollback

If the cause is a bad commit, revert it:

```bash
git checkout main
git revert <bad-commit-sha>  # creates a "Revert ..." commit
git push origin main
```

Vercel auto-deploys the reverted main.

### 12.3 Hotfix rollback

If the issue is a single small fix (typo, broken link), commit the fix to `main` directly:

```bash
git checkout main
# edit file
git commit -am "fix(home): correct broken link"
git push origin main
```

Vercel auto-deploys.

---

## 13. Cache invalidation

Vercel's edge cache serves the old version for ~10 seconds during the cutover, then catches up automatically. No manual invalidation typically needed.

If you need to force-purge a specific URL (e.g., the OG image):

```bash
curl -X PURGE "https://teleox.ai/og-image.png"
# or via Vercel dashboard: Project → Settings → Cache → Invalidate
```

---

## 14. DNS verification

The domain `teleox.ai` should resolve to Vercel:

```bash
dig teleox.ai +short
# Expect a Vercel IP (e.g., 76.76.21.21)

curl -I https://teleox.ai
# Expect HTTP/2 200, server: Vercel
```

If DNS is broken, the issue is upstream (not this deploy). Check Vercel project settings → Domains.

---

## 15. Security checklist

- [ ] HTTPS enforced (existing, via `next.config.ts` `Strict-Transport-Security` header)
- [ ] CSP header present
- [ ] X-Content-Type-Options: nosniff
- [ ] Referrer-Policy: strict-origin-when-cross-origin
- [ ] Permissions-Policy locked down (geolocation, microphone, camera = `()`)
- [ ] No secrets in deployed bundle (`grep -r "VERCEL_TOKEN\|API_KEY\|SECRET" .next/` should return nothing)

---

## 16. PAT rotation (after deploy)

After production deploy is verified, **immediately**:

1. Go to https://vercel.com/account/tokens
2. Find the PAT used for this deploy
3. Revoke it
4. If future deploys need a new PAT, generate a fresh one with the minimum required scope

---

## 17. Final deploy checklist

Before declaring "shipped":

- [ ] Preview deploy URL verified end-to-end (§5–§8)
- [ ] Lighthouse passes targets (§7)
- [ ] OG meta renders correctly across Twitter / LinkedIn / generic OG (§8)
- [ ] Production deploy verified (§10)
- [ ] Sitemap submitted to Search Console (§11.1)
- [ ] No 404s in production logs in first 24 hours (§11.3)
- [ ] PAT rotated (§16)
- [ ] Implementer / Chris notified
