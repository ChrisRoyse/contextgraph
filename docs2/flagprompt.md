# Agent Issue-Flagging — GitHub Issues as the Source of Truth

**Audience:** AI agents + human operator. **Purpose:** make "find something wrong → file a tracked Issue" automatic. Agent sessions are amnesiac; if it isn't in the issue tracker, it dies with the session.

---

## 1. Platform facts (free tier)

- Unlimited issues on public + private repos. Templates, labels, milestones, sub-issues, types all free.
- REST + GraphQL, `gh` CLI, official MCP server (`github/github-mcp-server`) all free.
- Actions: 2,000 min/mo private, unlimited public.

**API rate limits (operational ceilings):**
- Unauthenticated: 60/hr (unusable).
- PAT/OAuth user: **5,000/hr**.
- `GITHUB_TOKEN` in Actions: **1,000/hr per repo**.
- GitHub App on GHEC org: 15,000/hr.
- Secondary: ≤900 pts/min REST, ≤2,000 pts/min GraphQL. POST/PATCH/PUT/DELETE = 5 pts; GET = 1.

---

## 2. Core rule

> If during a turn you observe anything matching §3 (Triggers) or §4 (Anomaly) and you are NOT fixing it this turn — open an Issue before ending the turn. No exceptions.

Two-question test:
1. Did I observe a defect/anomaly/smell/missing-test/dead-code/risk I'm not fixing now?
2. Is there an open issue for it?

If (1)=YES and (2)=NO → file now.

---

## 3. Trigger list (must file if not fixed this turn)

**3.1 Defects:** reproducible bug (crash, wrong output, off-by-one, race, leak, deadlock); error/stack trace in logs; any test flake (even once); FSV disagreement (SoT ≠ return value); uncovered 5xx/4xx anomalies.

**3.2 Smells:** dead code (unused exports, unreachable, unimported files); duplicated logic (same code in 2+ places); commented-out blocks older than this session; long methods (>30 lines); cyclomatic complexity >10; magic numbers/strings; TODO/FIXME/XXX/HACK (convert to issue, link from comment or delete); bad names; bare `catch`/`except: pass`; linter-silenced inconsistencies.

**3.3 Tech debt:** intentional shortcuts (file w/ rationale + payback note); outdated deps with CVEs; deprecated APIs / removed framework features / unsupported runtimes; missing tests on code you touched/read; stale or missing docs for behavior learned; workarounds for upstream bugs (track upstream).

**3.4 Architecture:** anti-patterns (distributed monolith symptoms, shared DB across services, God class, missing circuit breaker); SPOFs; tight coupling that should be independent; missing observability (no logs/metrics/traces); missing idempotency on retryable ops; schema/contract drift.

**3.5 Security (always P0/P1):** hardcoded secrets/tokens/keys (file even after removing — track rotation); missing auth/authz; SQL/NoSQL/OS/template injection; missing input validation, output encoding, CSRF; weak crypto (MD5, SHA1, DES, ECB, custom protocols); verbose error responses leaking internals; missing security headers / TLS / weak TLS. Public repo + actually-exploitable → use GitHub Security Advisories instead of Issues.

**3.6 Performance:** N+1 queries; unbounded list/loop/recursion; sync blocking I/O on hot path; missing pagination / rate limit / timeout; missing retry-with-backoff on flaky deps; cache stampede risk / no TTL jitter.

**3.7 Verification gaps:** function written without test; state change without FSV against SoT; uncovered boundary cases.

**3.8 Risks:** "will fail at scale X" (file w/ threshold); "will break when Y changes" (file w/ dependency); "hard to migrate later" (architectural debt).

Heuristic: if you think "someone should look at this someday" — that thought IS the trigger.

---

## 4. Anomaly detection (statistical)

A signal is anomalous if current value is >N stddev from rolling mean.

| Threshold | Sensitivity | Use for |
|---|---|---|
| ±2σ | ~5% of points | code-quality metrics, latency, error rates |
| ±3σ | ~0.3% | capacity/resource (less noise) |

Asymmetric metrics (latency, error count) → upper bound only. Symmetric (RPS, return-value distribution) → both.

**Procedure:** collect last N points → compute μ, σ → normal = [μ−2σ, μ+2σ] → outside = anomaly → file → update window. **Robust variant (median+IQR):** anomaly when value is >1.5×IQR outside [Q1, Q3]. Prefer IQR when N<10 or distribution is skewed.

**Z-score recipe:** `z = (x−μ)/σ`. `|z|≥2` → file `type:anomaly` + (signal, μ, σ, current, z, cause hypothesis, scope). `|z|≥3` → also `priority:p1` (high).

**Signals an agent can compute without infra:**

| Signal | Source | Trigger (≥2σ) |
|---|---|---|
| File length | `git log --numstat` per file | single commit grows file >2σ |
| Function length | linter/static analysis | function >2σ above repo avg |
| Cyclomatic complexity | linter | >2σ above avg |
| Test count per module | repo scan | new module w/o proportional tests |
| PR diff size | `git log` | diff >2σ of recent PRs |
| Build time | CI logs | run >2σ longer than baseline |
| Test runtime | test report | single test >2σ above its history |
| Error rate / endpoint | logs/metrics | spike >2σ |
| Latency p95/p99 | metrics | spike >2σ |
| Dep count | lockfile diff | PR adds >2σ more deps |
| TODO/FIXME density | grep | file >2σ above repo avg |
| Open-issue age | issue list | issue older than μ+2σ → escalate |

**False-signal cautions:** N<10 → σ unreliable, use median/IQR. Seasonal data → deseasonalize first. Single `0` in count series may be a logging bug. Two-tailed thresholds wrong for one-sided metrics. When unsure → file anyway with `needs-triage`.

---

## 5. Repo setup (one-time)

```
.github/
├── ISSUE_TEMPLATE/
│   ├── config.yml
│   ├── 01-bug.yml … 07-test-gap.yml
├── workflows/
│   ├── dedupe-issues.yml
│   ├── auto-label.yml
│   └── stale-cleanup.yml
└── labels.yml
AGENTS.md   # repo-root operating rules
```

`.github/ISSUE_TEMPLATE/config.yml` — force template use:
```yaml
blank_issues_enabled: false
contact_links:
  - name: Discussion
    url: https://github.com/<owner>/<repo>/discussions
    about: Use Discussions for open-ended questions.
```

---

## 6. Labels (prefix:value, consistent colors per prefix)

| Label | Color | Meaning |
|---|---|---|
| `type:bug` | red | actual ≠ expected |
| `type:tech-debt` | gold | works but should improve |
| `type:dead-code` | gray | unused symbol/file/branch/column |
| `type:duplication` | gold | redundant logic |
| `type:security` | dark red | vuln/weakness |
| `type:performance` | orange | slow/inefficient |
| `type:architecture` | purple | structural/design |
| `type:test-gap` | yellow | missing/weak test |
| `type:docs` | blue | missing/wrong docs |
| `type:anomaly` | bright orange | statistical outlier |
| `type:risk` | yellow | foreseen failure |
| `source:agent` | light gray | filed by agent (always set) |
| `source:human` | dark gray | filed by person |
| `priority:p0` | crimson | drop everything (sec, prod outage) |
| `priority:p1` | red | this week |
| `priority:p2` | orange | this sprint |
| `priority:p3` | green | whenever |
| `status:needs-triage` | white | not yet validated |
| `status:confirmed` | light green | reproduced/verified |
| `status:in-progress` | blue | someone working it |
| `status:blocked` | black | waiting on something |
| `area:<module>` | per-area | subsystem (`area:api`, `area:auth`) |
| `agent:<name>` | light blue | which agent filed (claude/cursor/etc.) |

**Bootstrap script:**
```bash
gh label create "type:bug"        --color "d73a4a" --description "Defect"
gh label create "type:tech-debt"  --color "fbca04" --description "Working but should improve"
gh label create "type:dead-code"  --color "cccccc" --description "Unused code/data"
gh label create "type:duplication" --color "fbca04" --description "Redundant logic"
gh label create "type:security"   --color "b60205" --description "Vuln or weakness"
gh label create "type:performance" --color "d93f0b" --description "Slow / inefficient"
gh label create "type:architecture" --color "5319e7" --description "Structural concern"
gh label create "type:test-gap"   --color "fef2c0" --description "Missing/weak test"
gh label create "type:docs"       --color "0075ca" --description "Docs issue"
gh label create "type:anomaly"    --color "ff7619" --description "Statistical outlier"
gh label create "type:risk"       --color "fbca04" --description "Foreseen future failure"
gh label create "source:agent"    --color "e1e4e8" --description "Filed by an AI agent"
gh label create "source:human"    --color "586069" --description "Filed by a human"
gh label create "priority:p0"     --color "b60205" --description "Drop everything"
gh label create "priority:p1"     --color "d93f0b" --description "This week"
gh label create "priority:p2"     --color "fbca04" --description "This sprint"
gh label create "priority:p3"     --color "0e8a16" --description "Whenever"
gh label create "status:needs-triage" --color "ffffff" --description "Not yet validated"
gh label create "status:confirmed"    --color "c2e0c6" --description "Reproduced/verified"
gh label create "status:in-progress"  --color "0366d6" --description "Being worked on"
gh label create "status:blocked"      --color "000000" --description "Blocked"
```

---

## 7. Issue templates (`.github/ISSUE_TEMPLATE/`)

All templates use `source:agent` + `status:needs-triage` defaults.

### 7.1 `01-bug.yml`
```yaml
name: 🐛 Bug
description: Defect — actual behavior differs from expected.
title: "[BUG] "
labels: ["type:bug", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: summary
    attributes: {label: Summary, description: One paragraph.}
    validations: {required: true}
  - type: textarea
    id: reproduce
    attributes: {label: Steps to reproduce, description: Numbered, exact commands or inputs.}
    validations: {required: true}
  - type: textarea
    id: expected
    attributes: {label: Expected}
    validations: {required: true}
  - type: textarea
    id: actual
    attributes: {label: Actual (with evidence), description: Logs/screenshots/SoT read showing disagreement.}
    validations: {required: true}
  - type: input
    id: location
    attributes: {label: Location, description: File:line, commit SHA, URL.}
  - type: textarea
    id: scope
    attributes: {label: Suspected scope / blast radius}
  - type: textarea
    id: workaround
    attributes: {label: Workaround (if any)}
  - type: input
    id: agent
    attributes: {label: Filing agent, placeholder: "e.g., claude-opus-4.7"}
```

### 7.2 `02-tech-debt.yml`
```yaml
name: 🧹 Tech debt
description: Working but should be improved (smells, complexity, deprecated patterns).
title: "[DEBT] "
labels: ["type:tech-debt", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: what
    attributes: {label: What is the debt}
    validations: {required: true}
  - type: textarea
    id: where
    attributes: {label: Where (file paths, modules)}
    validations: {required: true}
  - type: textarea
    id: why-bad
    attributes: {label: Why it's a problem (interest rate), description: How is it slowing future work?}
    validations: {required: true}
  - type: textarea
    id: proposal
    attributes: {label: Suggested refactor}
  - type: textarea
    id: effort
    attributes: {label: Effort estimate, placeholder: "small (<2h), medium (1–2d), large (1+w)"}
```

### 7.3 `03-dead-code.yml`
```yaml
name: 💀 Dead code
description: Unused symbol, file, branch, column, or feature flag.
title: "[DEAD] "
labels: ["type:dead-code", "status:needs-triage", "source:agent"]
body:
  - type: input
    id: symbol
    attributes: {label: Symbol / file / table / flag}
    validations: {required: true}
  - type: textarea
    id: evidence
    attributes: {label: Evidence it is unused, description: "grep -r, lint, coverage, DB 0-read/write query"}
    validations: {required: true}
  - type: textarea
    id: indirect
    attributes: {label: Indirect-consumer check, description: "Reflection? DI? Generated code? External callers?"}
    validations: {required: true}
  - type: textarea
    id: deletion-plan
    attributes: {label: Proposed deletion plan}
```

### 7.4 `04-security.yml`
```yaml
name: 🔒 Security
description: Vulnerability, weakness, or unsafe pattern. Triage as P0/P1.
title: "[SEC] "
labels: ["type:security", "priority:p1", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: finding
    attributes: {label: Finding}
    validations: {required: true}
  - type: input
    id: cwe
    attributes: {label: CWE / OWASP category (if known)}
  - type: textarea
    id: impact
    attributes: {label: Impact if exploited}
    validations: {required: true}
  - type: textarea
    id: reproduce
    attributes: {label: How to reproduce / observe}
  - type: textarea
    id: fix
    attributes: {label: Suggested fix}
  - type: checkboxes
    id: disclosure
    attributes:
      label: Disclosure
      options:
        - label: May need private disclosure (omit exploit details in public-repo issues).
```

### 7.5 `05-anomaly.yml`
```yaml
name: 📊 Anomaly
description: A signal outside its typical range (≥ 2σ).
title: "[ANOMALY] "
labels: ["type:anomaly", "status:needs-triage", "source:agent"]
body:
  - type: input
    id: signal
    attributes: {label: Signal name, placeholder: "p95 latency, build time, error rate"}
    validations: {required: true}
  - type: input
    id: stats
    attributes: {label: μ / σ / current / z-score, placeholder: "μ=1.2s σ=0.3s current=2.8s z=5.3"}
    validations: {required: true}
  - type: textarea
    id: window
    attributes: {label: Window used, description: How many points, over what period.}
  - type: textarea
    id: hypothesis
    attributes: {label: Hypothesis for cause}
  - type: textarea
    id: link
    attributes: {label: Link to evidence, description: Dashboard URL, log query, CI run, commit range.}
```

### 7.6 `06-architecture.yml`
```yaml
name: 🏛️ Architecture
description: Structural or design concern. Coupling, SPOF, anti-pattern, missing seam.
title: "[ARCH] "
labels: ["type:architecture", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: concern
    attributes: {label: Concern}
    validations: {required: true}
  - type: textarea
    id: evidence
    attributes: {label: Evidence (dependency graph, traces, code refs)}
    validations: {required: true}
  - type: textarea
    id: risk
    attributes: {label: Risk if left}
  - type: textarea
    id: options
    attributes: {label: Options considered}
```

### 7.7 `07-test-gap.yml`
```yaml
name: 🧪 Test gap
description: Code path lacks adequate test or FSV coverage.
title: "[TEST] "
labels: ["type:test-gap", "status:needs-triage", "source:agent"]
body:
  - type: input
    id: target
    attributes: {label: Function / endpoint / module}
    validations: {required: true}
  - type: textarea
    id: gap
    attributes: {label: What is uncovered}
    validations: {required: true}
  - type: textarea
    id: cases
    attributes: {label: Cases that should exist, description: "Happy path? Boundary? Concurrency? Auth variants? FSV against SoT?"}
```

---

## 8. Authentication

**Fine-grained PAT (single user, recommended):** Settings → Developer settings → PAT → Fine-grained. Repo access: target repos only. Permissions: `Issues: Read/Write`, `Metadata: Read`, `Contents: Read` (or Write if committing). Max expiration 90d → schedule rotation. Rate limit: 5,000/hr shared across all tools that user uses.

**Classic PAT (avoid):** `repo` scope grants issue access but also full read/write. Use only if forced.

**GitHub App (multi-agent, shared repo):** higher isolated rate limits per installation; short-lived auto-refreshed tokens; appears as `<app>[bot]`. Permissions: `Issues: Read/Write`, `Metadata: Read`.

**`GITHUB_TOKEN` (Actions only):** auto-provided per-run. Set `permissions: { issues: write }` at job level. 1,000/hr per repo.

**Storage rules:** never commit tokens (pre-commit `gitleaks`); CI → encrypted Secrets; workstation → `gh auth login` (OS keychain) or vault env-var. Any leak = P0 → revoke immediately.

---

## 9. Three ways to create issues (equivalent)

### 9.1 `gh` CLI
```bash
gh auth login --with-token < $TOKEN_FILE

gh issue create \
  --repo "owner/repo" \
  --title "[BUG] Login retries fail after 3rd attempt" \
  --body-file ./issue-body.md \
  --label "type:bug,priority:p1,source:agent,agent:claude" \
  --assignee "@me"

gh issue list --repo "owner/repo" --state open \
  --search "login retries in:title,body" \
  --json number,title,url

gh issue comment 123 --repo "owner/repo" --body "Reproduced at $(date -u)."

gh issue close 123 --repo "owner/repo" --reason "completed" --comment "Fixed in #456."
```

### 9.2 REST API
`POST https://api.github.com/repos/{owner}/{repo}/issues`
```bash
curl -X POST \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  https://api.github.com/repos/owner/repo/issues \
  -d '{
    "title": "[DEAD] handleLegacyAuth() unused since v3.2",
    "body": "## Evidence\n- grep -r returns 0 callers\n\n## Indirect check\n- No reflection/DI/external SDK\n",
    "labels": ["type:dead-code","source:agent","status:needs-triage"]
  }'
```

Python helpers:
```python
import os, requests
HDRS = {"Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}",
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28"}

def create_issue(owner, repo, title, body, labels):
    r = requests.post(f"https://api.github.com/repos/{owner}/{repo}/issues",
                      headers=HDRS, json={"title":title,"body":body,"labels":labels}, timeout=15)
    r.raise_for_status(); return r.json()

def search_issues(owner, repo, query):  # search endpoint has its own lower rate limit
    q = f"repo:{owner}/{repo} is:open is:issue {query}"
    r = requests.get("https://api.github.com/search/issues",
                     headers=HDRS, params={"q":q}, timeout=15)
    r.raise_for_status(); return r.json()["items"]
```

### 9.3 GitHub MCP Server (best for Claude/Cursor/agentic IDEs)
Official: `github/github-mcp-server`. Tools: `create_issue`, `list_issues`, `search_issues`, `update_issue`, `add_issue_comment`, plus PR/repo/Dependabot access.

Claude config:
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {"GITHUB_PERSONAL_ACCESS_TOKEN": "<fine-grained-PAT>"}
    }
  }
}
```

---

## 10. Mandatory dedupe (run before every create)

1. Pick **3–6 distinctive keywords** (symbol names, error strings, file paths). Avoid `bug`, `error`, `issue`, `failure`.
2. Search open AND recently closed:
   ```bash
   gh issue list --repo o/r --state all --limit 50 \
     --search "handleLegacyAuth oauth retry in:title,body"
   ```
3. Score each result for similarity (embeddings cosine, token overlap, or LLM 0–10 rating).
4. **Thresholds:**
   - ≥8/10 → **do not file**. Comment on existing: `"Re-observed at SHA abc1234 running scenario X. New detail: …"`.
   - 5–7 → file new, link related in body: `"Possibly related to #123, distinct because …"`.
   - <5 → file as new.
5. If a duplicate slips through, close explicitly:
   ```bash
   gh issue close 456 --repo o/r --reason "not planned" \
     --comment "Duplicate of #123. Closing; follow #123 for updates."
   ```

GitHub's `Duplicate of #N` comment syntax auto-marks the new one as duplicate.

---

## 11. Per-turn protocol (5 steps — paste into `AGENTS.md`)

**Step 1 — Read backlog:**
```bash
gh issue list --repo $REPO --state open --label "source:agent" --limit 50
```
Skim titles → mental list of what's already tracked (prevents dupes later).

**Step 2 — Do the work** (whatever the human asked).

**Step 3 — Observe surroundings.** Note §3 triggers and §4 anomalies you are NOT fixing this turn.

**Step 4 — Triage:** trivial + you're here anyway → fix now (boy-scout). Else → queue for filing.

**Step 5 — File before ending turn.** For each queued item: run dedupe (§10) → pick template (§7) → fill out specific/evidence-rich → create → note issue # in response.

### 11.2 Title rules
- Specific (name the symbol/file/endpoint).
- Imperative or declarative — describes state, not the fix.
- Prefixed: `[BUG]`/`[DEBT]`/`[DEAD]`/`[SEC]`/`[ANOMALY]`/`[ARCH]`/`[TEST]`.
- ≤80 chars.

✅ `[BUG] /orders POST returns 200 but row not inserted when amount==0`
✅ `[DEAD] PaymentLegacyProcessor unused since v4.0 migration (Apr 2024)`
✅ `[ANOMALY] checkout endpoint p99 jumped 8σ after deploy abc1234`
❌ `Bug in orders` / `Fix the payment thing`

### 11.3 Body checklist (always include)
1. Evidence (log line, file:line, diff, query output, dashboard URL, commit SHA).
2. Expected vs observed (FSV-style).
3. Scope estimate.
4. Repro steps if non-trivial.
5. Suggested next action (even "investigate further").
6. Footer:
   ```
   ---
   Filed by: claude-opus-4.7
   Session: <date or correlation-id>
   Commit at time of filing: abc1234
   ```

### 11.4 Same-turn fixes
Reference in commit/PR: `Fixes #N` / `Closes #N` auto-closes on merge. Small fix + no prior issue → optionally skip filing (commit msg suffices). Non-trivial fix → file first, reference from PR for reviewer context.

### 11.5 Priority heuristics
- **p0** — security-exploitable now, prod outage, data loss possible.
- **p1** — user-facing bug, security weakness w/o immediate exploit, anomaly ≥3σ.
- **p2** — tech debt slowing dev, anomaly 2–3σ, test gap on real path.
- **p3** — cosmetic, micro-opt, far-future risk.
- Default **p2** when unsure.

---

## 12. Multi-agent handoff

### 12.1 Start-of-session ritual
```bash
# Open agent-filed issues
gh issue list --repo $REPO --state open --label "source:agent" \
  --sort updated --limit 20 --json number,title,labels,updatedAt

# Recent agent activity (open + closed)
gh issue list --repo $REPO --state all --label "source:agent" \
  --sort updated --limit 10 --json number,title,state,closedAt

# High-priority queue
gh issue list --repo $REPO --state open \
  --label "priority:p0,priority:p1" --json number,title,labels
```

### 12.2 "Continue where you left off"
1. Filter: `state:open` AND `source:agent` AND `assignee:none`.
2. Sort: `priority` asc (p0 first), then `updated` asc (oldest first).
3. Pick top → `gh issue edit N --add-assignee @me`.
4. Swap `status:needs-triage` → `status:in-progress`.
5. Comment: `"Picking this up — agent X — commit Y — plan: …"`.
6. Work it.
7. PR with `Closes #N`.

### 12.3 Pausing mid-task
Comment a **resume note**: what was tried, what was learned, file/line to start at, hypothesis to test next. Set `status:blocked` if waiting on something; else leave `status:in-progress`. Unassign only if you won't return.

This is the single highest-leverage habit in multi-agent work.

---

## 13. Automation backstop (Actions)

### 13.1 SAST → Issues
`.github/workflows/sast-to-issues.yml`:
```yaml
name: SAST → Issues
on:
  schedule: [{cron: "0 4 * * 1"}]
  workflow_dispatch: {}
permissions:
  contents: read
  issues: write
  security-events: read
jobs:
  scan-and-file:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Semgrep
        run: |
          pipx install semgrep
          semgrep --config=auto --json > findings.json
      - name: File issues for new findings
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          REPO: ${{ github.repository }}
        run: python .github/scripts/findings_to_issues.py findings.json
```
Script must hash `rule_id + file + line range` as fingerprint, search title field for it, only file new ones (same dedupe contract as agents).

### 13.2 Dependabot
Enable under Settings → Security → Code security: alerts (free, all repos), security updates (auto-PRs), version updates (`.github/dependabot.yml`). Convert unfixed alerts → `type:security` issues after N days so they hit the same backlog.

### 13.3 Stale cleanup
`.github/workflows/stale-cleanup.yml`:
```yaml
name: Stale issues
on:
  schedule: [{cron: "30 1 * * *"}]
permissions: {issues: write}
jobs:
  stale:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/stale@v9
        with:
          days-before-issue-stale: 60
          days-before-issue-close: 14
          stale-issue-label: "status:stale"
          exempt-issue-labels: "priority:p0,priority:p1,status:in-progress"
          stale-issue-message: |
            No activity for 60 days. Closing in 14 days unless reactivated.
            Comment or remove the `status:stale` label to keep open.
          close-issue-message: |
            Closed as stale. Reopen if still relevant.
```

### 13.4 Auto-labeling
`actions/labeler` applies labels by file-path patterns — use for `area:*`.

### 13.5 Title fingerprint dedupe (cheap)
Embed stable hash in title: `[SEC] python.lang.security.audit.dangerous-eval at api/handler.py:142 [fp:semgrep:py-eval-handler-142]`
Search:
```bash
gh issue list --search "[fp:semgrep:py-eval-handler-142] in:title" --state all
```
Match → comment; no match → file new.

---

## 14. Hygiene

- **Weekly 5-min triage** (human or designated agent): scan `status:needs-triage`, confirm or close; reprioritize drifters; close obvious-fix-not-auto-closed.
- **Closing dupes:** always link the kept issue in the closing comment (makes future dedupe easier).
- **Label cap:** 1 `type:*` + 1 `priority:*` + 1 `status:*` + 1–2 `area:*` + `source:*`. That's the cap. Don't over-label.
- **Milestones** for sweeps: group all issues from "harden auth module" sweep → milestone page = sweep report.
- **Sub-issues** for trackers: parent issue + children via REST `POST /repos/{o}/{r}/issues/{n}/sub_issues` using the issue's numeric `id` (not number). CLI doesn't fully support yet.
- **Trends to watch:** which `area:*` accumulates most → where real debt lives. Time-to-close per `type:*` → surface-only vs deep refactoring pattern. Agents vs humans hit ratio → tune triggers/templates.

---

## 15. Copy-paste artifacts

### 15.1 `AGENTS.md` (top of repo)
```markdown
# AGENTS.md — Operating Rules for AI Agents in This Repo

## Non-negotiable rule
If you observe a bug, defect, smell, dead code, redundant code, tech debt,
anomaly, security weakness, architectural concern, or test gap that you are
NOT fixing in this same turn — you MUST open a GitHub Issue before ending
your turn. See `.github/ISSUE_TEMPLATE/` for templates.

## Before opening
1. Search existing open AND closed issues with 3–6 distinctive keywords.
2. Clear duplicate → comment on existing with new evidence; do not file new.
3. Partial match → file but link the related issue in body.

## Required content
- Specific prefixed title (`[BUG]`/`[DEBT]`/`[DEAD]`/`[SEC]`/`[ANOMALY]`/`[ARCH]`/`[TEST]`).
- Evidence (file:line, log, query output, commit SHA).
- Expected vs observed (FSV style).
- Suspected scope/blast radius.
- Suggested next action.
- Footer: agent name, session id, commit SHA.

## Required labels
- One `type:*` (bug, tech-debt, dead-code, duplication, security,
  performance, architecture, test-gap, docs, anomaly, risk).
- One `priority:*` (default `priority:p2`).
- `source:agent` + `agent:<your-name>`.
- `status:needs-triage` new; `status:in-progress` when you start work.

## Anomaly flagging
Any signal >2σ from rolling mean → file `type:anomaly` with μ, σ, current, z-score, window.

## Multi-agent handoff
Before working: list open `source:agent` issues, pick highest-priority unassigned,
assign self, set `status:in-progress`, comment with plan.
Pausing mid-task: comment what tried/learned/where-to-resume.

## Tools
- `gh` CLI (preferred for shell-capable agents).
- REST/GraphQL (any language).
- GitHub MCP server (Claude/Cursor/agentic IDEs).

## Reference
See `docs/AGENT_ISSUE_FLAGGING_GUIDE.md` for full protocol.
```

### 15.2 `.github/scripts/file_issue.py` — dedupe-aware filer
```python
#!/usr/bin/env python3
"""Dedupe-aware issue filer for agents.

Usage:
    GITHUB_TOKEN=... python file_issue.py \
        --repo owner/name \
        --title "[BUG] X" \
        --body-file body.md \
        --labels type:bug,priority:p2,source:agent \
        --keywords "foo bar baz"

Exit codes: 0 = filed; 2 = dedupe match found, did NOT file (caller should
comment on existing issue instead).
"""
import argparse, json, os, sys
import urllib.parse, urllib.request

API = "https://api.github.com"

def gh(method, path, payload=None):
    req = urllib.request.Request(
        f"{API}{path}", method=method,
        headers={
            "Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "agent-issue-filer",
        },
        data=json.dumps(payload).encode() if payload else None,
    )
    with urllib.request.urlopen(req, timeout=20) as r:
        return json.loads(r.read().decode())

def search_existing(repo, keywords):
    q = f"repo:{repo} is:issue {keywords}"
    enc = urllib.parse.urlencode({"q": q, "per_page": 5})
    return gh("GET", f"/search/issues?{enc}").get("items", [])

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--repo", required=True)
    p.add_argument("--title", required=True)
    p.add_argument("--body-file", required=True)
    p.add_argument("--labels", default="")
    p.add_argument("--keywords", required=True, help="3–6 distinctive words for dedupe")
    p.add_argument("--similarity-threshold", type=float, default=0.6)
    args = p.parse_args()

    body = open(args.body_file).read()
    labels = [l.strip() for l in args.labels.split(",") if l.strip()]

    matches = search_existing(args.repo, args.keywords)
    if matches:
        print(f"⚠ Possible duplicates ({len(matches)}):")
        for m in matches[:5]:
            print(f"  #{m['number']} {m['state']} — {m['title']} — {m['html_url']}")
        sys.exit(2)

    owner, name = args.repo.split("/", 1)
    issue = gh("POST", f"/repos/{owner}/{name}/issues",
               {"title": args.title, "body": body, "labels": labels})
    print(f"✓ Filed #{issue['number']}: {issue['html_url']}")

if __name__ == "__main__":
    main()
```

### 15.3 Quick-start checklist
- [ ] Create labels (§6 script).
- [ ] Add 7 issue templates from §7.
- [ ] Add `.github/ISSUE_TEMPLATE/config.yml` (disable blank issues).
- [ ] Create `AGENTS.md` at repo root (§15.1).
- [ ] Generate fine-grained PAT scoped to repo with `Issues: R/W`.
- [ ] Store PAT in agent vault / MCP config.
- [ ] Enable Dependabot alerts (Settings → Security).
- [ ] Add `actions/stale` workflow (§13.3).
- [ ] (Optional) Install GitHub MCP server.
- [ ] Smoke test: `gh issue create --title "[TEST] verify setup" --body "test" --label "type:docs"` then close.
- [ ] Instruct agent: *"From now on, file an issue for anything you see that isn't normal. Read AGENTS.md."*

---

## Limits to remember

- **Cost:** $0/mo regardless of issue volume.
- **API:** 5,000/hr user-auth; 1,000/hr `GITHUB_TOKEN` in Actions.
- **Free private repos:** 3 collaborators w/ write access cap (does not affect agents authed as the owner). Issues themselves free.
- **The discipline is the work.** GitHub Issues makes filing free and frictionless; the agents must actually file, dedupe, triage, close.
- **Win condition:** once Issues are SoT, prompts shrink — "harden auth module" becomes "work `priority:p1` + `area:auth`."
