# Agent Issue-Flagging System — GitHub Issues as the Source of Truth

> **Audience:** AI agents (Claude, Cursor, Aider, Copilot, etc.) and the human operator coordinating them.
> **Purpose:** make "find something wrong → file a tracked GitHub Issue" an automatic, unmissable habit so nothing slips between agent sessions.
> **Why GitHub Issues:** free on all plans (public and private repos), unlimited issues, full REST + GraphQL API, native CLI (`gh`), official MCP server — meaning your agents can read, write, search, and close issues directly during a session.

---

## Table of Contents

1. [Why GitHub Issues, Why Free](#1-why-github-issues-why-free)
2. [The Core Rule — "If You See It, You File It"](#2-the-core-rule--if-you-see-it-you-file-it)
3. [What Always Gets Flagged (the Trigger List)](#3-what-always-gets-flagged-the-trigger-list)
4. [Anomaly Detection — "Outside Standard Deviations"](#4-anomaly-detection--outside-standard-deviations)
5. [Repository Setup — One-Time Configuration](#5-repository-setup--one-time-configuration)
6. [The Label Taxonomy](#6-the-label-taxonomy)
7. [The Issue Templates](#7-the-issue-templates)
8. [Authentication — Tokens, Scopes, Free-Tier Limits](#8-authentication--tokens-scopes-free-tier-limits)
9. [Three Ways for Agents to Create Issues](#9-three-ways-for-agents-to-create-issues)
10. [The Mandatory Deduplication Step](#10-the-mandatory-deduplication-step)
11. [The Agent Operating Protocol](#11-the-agent-operating-protocol)
12. [Multi-Agent Handoff — Picking Up Where the Last One Left Off](#12-multi-agent-handoff--picking-up-where-the-last-one-left-off)
13. [Automation Backstop — GitHub Actions Catches What Agents Miss](#13-automation-backstop--github-actions-catches-what-agents-miss)
14. [Issue Hygiene — Keeping the Backlog Useful](#14-issue-hygiene--keeping-the-backlog-useful)
15. [Copy-Paste Pack — Ready-to-Use Files](#15-copy-paste-pack--ready-to-use-files)

---

## 1. Why GitHub Issues, Why Free

GitHub Issues is the right answer for this use case because every property you need is included in the free tier.

| Feature | Free tier reality |
|---|---|
| Issue creation | Unlimited on public and private repos. No quota. |
| Private repos | Unlimited, with unlimited collaborators (since April 2020). |
| Issue API (REST + GraphQL) | Full access on all plans. |
| GitHub CLI (`gh`) | Free, no plan gating. |
| GitHub Actions | 2,000 free minutes/month on private repos; **unlimited** on public repos. Enough for the kind of lightweight automation in this doc. |
| Issue templates | Free, stored in `.github/ISSUE_TEMPLATE/`. |
| Labels, milestones, sub-issues, types | All free. Issue types via REST API were added in 2025. |
| Issue search | Full-text + filters via the API, free. |

**API rate limits** to know up front (these are the real operational ceilings):
- Unauthenticated: 60 requests/hour per IP. Treat as unusable.
- Authenticated user (PAT or OAuth): **5,000 requests/hour**. Plenty for agents.
- `GITHUB_TOKEN` inside GitHub Actions: **1,000 requests/hour per repository**.
- GitHub App on a GHEC organization: 15,000 requests/hour.
- Secondary limits exist (≤ 900 points/min on REST, ≤ 2,000 points/min on GraphQL). POST/PATCH/PUT/DELETE cost 5 points; GET costs 1.

Translation: a token-authenticated agent can create thousands of issues per hour. The bottleneck is *your* signal-to-noise discipline, not GitHub's wallet.

---

## 2. The Core Rule — "If You See It, You File It"

> **Rule of the house:** during any task, if an agent observes anything that meets a Trigger (§3) or is statistically abnormal (§4) and does not fix it in the same turn, the agent **must** open a GitHub Issue before ending its turn. No exceptions.

Reasoning:
- Agent sessions are amnesiac. State that lives only in chat history dies with the session.
- The next agent (or the human) cannot fix what isn't tracked.
- "I'll mention it in the response" is not tracking. A response is not a queue.
- GitHub Issues are the **Source of Truth** for outstanding work. Everything else is conversation.

The two-question test the agent runs in its head:

1. Did I observe a defect, anomaly, smell, missing test, dead code path, or risk I am not fixing right now?
2. Is there an open issue that already covers it?

If (1) is YES and (2) is NO → **open the issue now**.

---

## 3. What Always Gets Flagged (the Trigger List)

These are the categories that *must* produce an issue if not fixed in the same turn. Treat as a hard rule, not a guideline.

### 3.1 Defects and unexpected behavior
- Any reproducible bug — crash, wrong output, off-by-one, race condition, memory leak, infinite loop, deadlock.
- Any error or stack trace observed in logs that is not already an open issue.
- Any test that flakes — even once.
- Any FSV check (see the System Hardening Guide) where the Source of Truth disagrees with the return value.
- Any 500 / 4xx anomaly in monitoring not already covered.

### 3.2 Code-quality smells
- **Dead code:** unused exports, unreachable branches, files imported by nothing.
- **Redundant / duplicated code:** the same logic in 2+ places when it should be one.
- **Commented-out code blocks** older than the session.
- **Long methods** (> 30 lines) or **high cyclomatic complexity** (> 10) added or encountered.
- **Magic numbers / strings** without named constants.
- **TODO / FIXME / XXX / HACK** comments — convert to issues, then keep or remove the comment with a link to the issue.
- **Bad names** (single letters, vague nouns, misleading verbs) that obscure intent.
- **Bare `catch` / `except: pass`** swallowing errors.
- **Inconsistent style** that the linter would catch but is silenced.

### 3.3 Technical debt
- Any **intentional shortcut** taken in this session — file it with rationale and a payback note.
- **Outdated dependencies** with known CVEs (Dependabot alerts not converted to issues).
- **Outdated patterns** (deprecated APIs, removed framework features, unsupported runtime versions).
- **Missing tests** for code paths the agent touched or read.
- **Missing or stale docs** for behavior the agent learned about.
- **Workarounds** for upstream bugs — track the upstream resolution.

### 3.4 Architectural and structural concerns
- Newly observed **anti-patterns** (distributed monolith symptoms, shared database access across services, God class, missing circuit breaker, etc. — see Hardening Guide §5.2).
- **Single points of failure** discovered.
- **Tight coupling** between modules that should be independent.
- **Missing observability** — code paths with no logs, metrics, or traces.
- **Missing idempotency** on retryable operations.
- **Schema / contract drift** between services.

### 3.5 Security concerns (always P0/P1 — fix-now if possible, file if not)
- Hardcoded secrets, tokens, keys, passwords — *and* file the issue even if you also remove them, so the rotation/audit work is tracked.
- Missing authentication or authorization on an endpoint.
- SQL/NoSQL/OS/template injection vectors.
- Missing input validation, output encoding, or CSRF protection.
- Insecure crypto (MD5, SHA1, DES, ECB, custom protocols).
- Verbose error responses leaking internals.
- Missing security headers, missing TLS, weak TLS config.

### 3.6 Performance and operational concerns
- N+1 queries.
- Unbounded list / loop / recursion.
- Synchronous blocking I/O on a hot path.
- Missing pagination, missing rate limit, missing timeout.
- Missing retry-with-backoff on a flaky dependency.
- Cache stampede risk; no jitter on TTL.

### 3.7 Verification gaps
- Any function written without a test.
- Any state change without an FSV check that reads the Source of Truth.
- Any edge case from the boundary list (§6.3 of the Hardening Guide) not exercised.

### 3.8 Risks / foresight items
- "Will fail at scale X" — file with the threshold.
- "Will break when Y changes" — file with the dependency.
- "Will be hard to migrate later" — file as architectural debt.

If the agent finds itself thinking "someone should look at this someday" — that thought *is* the trigger. File it.

---

## 4. Anomaly Detection — "Outside Standard Deviations"

The instruction "anything outside typical standard deviations" is a real, well-defined practice. Here is how an agent should operationalize it.

### 4.1 What "anomalous" means in practice

A signal is anomalous when its current value lies more than **N standard deviations from the rolling mean** of its recent history. The most common thresholds:

| Threshold | Sensitivity | Use for |
|---|---|---|
| **±2σ** | Higher — flags ~5% of points in a normal distribution | Code-quality metrics, latency, error rates |
| **±3σ** | Lower — flags ~0.3% | Capacity and resource use; reduces noise |

For asymmetric metrics (latency, error count) use only the *upper* bound. For symmetric ones (return-value distributions, RPS) use both.

The classic procedure (works without any ML infrastructure):

1. Collect the last *N* measurements (e.g., 30 builds, 90 days of metric).
2. Compute mean (μ) and standard deviation (σ).
3. Define normal range: `[μ − 2σ, μ + 2σ]`.
4. New point outside the range → **anomaly → open an issue**.
5. Update the rolling window.

A more robust variant uses **median + IQR** (interquartile range) — less sensitive to outliers that themselves drag the mean. Anomaly when value is more than `1.5 × IQR` outside the [Q1, Q3] band.

### 4.2 What an agent should compute and watch

The agent doesn't need a metrics platform. It can compute these from artifacts an LLM session already has access to:

| Signal | Source | Anomaly trigger (≥ 2σ) → open issue |
|---|---|---|
| File length (lines) | git history of the file | A single commit grows a file by > 2σ |
| Function length | static analysis output | Any function > 2σ above repo average |
| Cyclomatic complexity | linter output | Same |
| Test count per module | repo scan | Module added without a proportional test count |
| PR diff size (lines) | git log | Diff > 2σ of recent PRs (likely too big to review) |
| Build time | CI logs | Build runs > 2σ longer than baseline |
| Test runtime | test report | Single test > 2σ above its history |
| Error rate per endpoint | logs / metrics if available | Spike > 2σ during the session |
| Latency p95 / p99 | metrics if available | Spike > 2σ |
| Dependency count | lockfile diff | A PR adds > 2σ more deps than usual |
| TODO/FIXME density | grep | A file > 2σ above repo TODO density |
| Open-issue age | issue list | Issue older than μ+2σ → escalate, don't just leave |

### 4.3 The agent's anomaly recipe (no infrastructure required)

When an agent suspects anomaly territory:

1. Pull the historical series (`git log --numstat`, lint output, CI log, etc.).
2. Compute mean and stdev in code or in head.
3. Compute the z-score of the new point: `z = (x − μ) / σ`.
4. If `|z| ≥ 2`, file an issue tagged `anomaly` with: signal name, μ, σ, current value, z-score, plausible cause, suspected scope.
5. If `|z| ≥ 3`, also add `priority:high`.

### 4.4 Beware of false signals

- Small windows (< 10 data points) give unreliable σ. Use median/IQR instead.
- Seasonal data needs deseasonalizing before σ is meaningful.
- A single value of `0` in a count series can be a logging bug, not an event drop.
- Two-tailed thresholds are wrong for one-sided metrics like "error count."

When unsure → file the issue anyway with a `needs-triage` label. A noisy issue is cheaper than a missed regression.

---

## 5. Repository Setup — One-Time Configuration

Do this once per repo. Then agents have everything they need.

### 5.1 Directory layout

```
.github/
├── ISSUE_TEMPLATE/
│   ├── config.yml
│   ├── 01-bug.yml
│   ├── 02-tech-debt.yml
│   ├── 03-dead-code.yml
│   ├── 04-security.yml
│   ├── 05-anomaly.yml
│   ├── 06-architecture.yml
│   └── 07-test-gap.yml
├── workflows/
│   ├── dedupe-issues.yml
│   ├── auto-label.yml
│   └── stale-cleanup.yml
└── labels.yml
AGENTS.md   # Agent operating instructions (top of repo)
```

### 5.2 Disable blank issues to force template use

`.github/ISSUE_TEMPLATE/config.yml`:

```yaml
blank_issues_enabled: false
contact_links:
  - name: Discussion
    url: https://github.com/<owner>/<repo>/discussions
    about: Use Discussions for open-ended questions.
```

This makes every new issue choose a template — including the ones agents file.

---

## 6. The Label Taxonomy

Use **prefix:value** so labels group naturally and don't sprawl. Pick consistent colors per prefix.

### 6.1 The minimum useful set

| Label | Color | Meaning |
|---|---|---|
| **`type:bug`** | red | Defect: actual ≠ expected |
| **`type:tech-debt`** | gold | Working but should be improved |
| **`type:dead-code`** | gray | Unused symbol, file, branch, or column |
| **`type:duplication`** | gold | Redundant logic; should consolidate |
| **`type:security`** | dark red | Vulnerability or weakness |
| **`type:performance`** | orange | Slow or inefficient |
| **`type:architecture`** | purple | Structural / design concern |
| **`type:test-gap`** | yellow | Missing or weak test |
| **`type:docs`** | blue | Missing or wrong documentation |
| **`type:anomaly`** | bright orange | Statistical outlier; needs explanation |
| **`type:risk`** | yellow | Foreseen future failure |
| **`source:agent`** | light gray | Filed by an automated agent (always set) |
| **`source:human`** | dark gray | Filed by a person |
| **`priority:p0`** | crimson | Drop everything (security, prod outage) |
| **`priority:p1`** | red | This week |
| **`priority:p2`** | orange | This sprint |
| **`priority:p3`** | green | Whenever |
| **`status:needs-triage`** | white | Not yet validated |
| **`status:confirmed`** | light green | Reproduced or verified |
| **`status:in-progress`** | blue | Someone is working on it |
| **`status:blocked`** | black | Waiting on something |
| **`area:<module>`** | per-area | Subsystem tag (e.g., `area:api`, `area:auth`) |
| **`agent:claude`** / **`agent:cursor`** / etc. | light blue | Which agent filed it (for trend analysis) |

### 6.2 Bootstrap labels via the CLI

```bash
# Run once per repo
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

## 7. The Issue Templates

Templates live in `.github/ISSUE_TEMPLATE/`. YAML form templates (`.yml`) give validated fields and auto-attach labels. Below are the seven templates the agent system actually uses.

### 7.1 Bug template

`.github/ISSUE_TEMPLATE/01-bug.yml`:

```yaml
name: 🐛 Bug
description: Defect — actual behavior differs from expected.
title: "[BUG] "
labels: ["type:bug", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: summary
    attributes:
      label: Summary
      description: One paragraph describing the bug.
    validations:
      required: true
  - type: textarea
    id: reproduce
    attributes:
      label: Steps to reproduce
      description: Numbered, exact commands or inputs.
    validations:
      required: true
  - type: textarea
    id: expected
    attributes:
      label: Expected
    validations:
      required: true
  - type: textarea
    id: actual
    attributes:
      label: Actual (with evidence)
      description: Logs, screenshots, or — for FSV — the Source-of-Truth read showing the disagreement.
    validations:
      required: true
  - type: input
    id: location
    attributes:
      label: Location
      description: File path(s), line(s), commit SHA, or URL.
  - type: textarea
    id: scope
    attributes:
      label: Suspected scope / blast radius
  - type: textarea
    id: workaround
    attributes:
      label: Workaround (if any)
  - type: input
    id: agent
    attributes:
      label: Filing agent
      placeholder: e.g., claude-opus-4.7
```

### 7.2 Tech-debt template

`.github/ISSUE_TEMPLATE/02-tech-debt.yml`:

```yaml
name: 🧹 Tech debt
description: Working but should be improved (smells, complexity, deprecated patterns).
title: "[DEBT] "
labels: ["type:tech-debt", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: what
    attributes:
      label: What is the debt
    validations: {required: true}
  - type: textarea
    id: where
    attributes:
      label: Where (file paths, modules)
    validations: {required: true}
  - type: textarea
    id: why-bad
    attributes:
      label: Why it's a problem (interest rate)
      description: How is it slowing future work or risking failure?
    validations: {required: true}
  - type: textarea
    id: proposal
    attributes:
      label: Suggested refactor
  - type: textarea
    id: effort
    attributes:
      label: Effort estimate
      placeholder: e.g., small (<2h), medium (1–2 days), large (1+ weeks)
```

### 7.3 Dead-code template

`.github/ISSUE_TEMPLATE/03-dead-code.yml`:

```yaml
name: 💀 Dead code
description: Unused symbol, file, branch, column, or feature flag.
title: "[DEAD] "
labels: ["type:dead-code", "status:needs-triage", "source:agent"]
body:
  - type: input
    id: symbol
    attributes:
      label: Symbol / file / table / flag
    validations: {required: true}
  - type: textarea
    id: evidence
    attributes:
      label: Evidence it is unused
      description: e.g., `grep -r` result, lint output, runtime coverage data, DB query showing 0 reads/writes.
    validations: {required: true}
  - type: textarea
    id: indirect
    attributes:
      label: Indirect-consumer check
      description: Reflection? DI? Generated code? External callers? — list what you ruled out.
    validations: {required: true}
  - type: textarea
    id: deletion-plan
    attributes:
      label: Proposed deletion plan
```

### 7.4 Security template

`.github/ISSUE_TEMPLATE/04-security.yml`:

```yaml
name: 🔒 Security
description: Vulnerability, weakness, or unsafe pattern. Triage as P0/P1.
title: "[SEC] "
labels: ["type:security", "priority:p1", "status:needs-triage", "source:agent"]
body:
  - type: textarea
    id: finding
    attributes:
      label: Finding
    validations: {required: true}
  - type: input
    id: cwe
    attributes:
      label: CWE / OWASP category (if known)
  - type: textarea
    id: impact
    attributes:
      label: Impact if exploited
    validations: {required: true}
  - type: textarea
    id: reproduce
    attributes:
      label: How to reproduce / observe
  - type: textarea
    id: fix
    attributes:
      label: Suggested fix
  - type: checkboxes
    id: disclosure
    attributes:
      label: Disclosure
      options:
        - label: This finding may need private disclosure (do not include exploit details in this issue if it's a public repo).
```

> Note: for public repos, use **GitHub Security Advisories** for actually-exploitable vulns, not Issues. Configure under `Security → Advisories → New draft`.

### 7.5 Anomaly template

`.github/ISSUE_TEMPLATE/05-anomaly.yml`:

```yaml
name: 📊 Anomaly
description: A signal outside its typical range (≥ 2σ).
title: "[ANOMALY] "
labels: ["type:anomaly", "status:needs-triage", "source:agent"]
body:
  - type: input
    id: signal
    attributes:
      label: Signal name
      placeholder: e.g., p95 latency, build time, error rate
    validations: {required: true}
  - type: input
    id: stats
    attributes:
      label: μ / σ / current / z-score
      placeholder: μ=1.2s σ=0.3s current=2.8s z=5.3
    validations: {required: true}
  - type: textarea
    id: window
    attributes:
      label: Window used
      description: How many points, over what period.
  - type: textarea
    id: hypothesis
    attributes:
      label: Hypothesis for cause
  - type: textarea
    id: link
    attributes:
      label: Link to evidence
      description: Dashboard URL, log query, CI run, commit range.
```

### 7.6 Architecture template

`.github/ISSUE_TEMPLATE/06-architecture.yml`:

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

### 7.7 Test-gap template

`.github/ISSUE_TEMPLATE/07-test-gap.yml`:

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
    attributes:
      label: Cases that should exist
      description: Happy path? Boundary? Concurrency? Auth variants? FSV against the SoT?
```

---

## 8. Authentication — Tokens, Scopes, Free-Tier Limits

Agents need authenticated access. Pick one of three patterns by use case.

### 8.1 Personal Access Token (PAT) — simplest

For a single user / single workstation / a single agent. Two flavors:

- **Fine-grained PAT** (recommended): scoped to specific repos, with specific permissions. Create at `Settings → Developer settings → Personal access tokens → Fine-grained tokens`.
  - **Repository access:** select your repo(s) only.
  - **Permissions → Repository:** `Issues: Read and Write`, `Metadata: Read-only`, `Contents: Read-only` (or Write if the agent also commits).
  - **Expiration:** 90 days max. Schedule rotation.
- **Classic PAT** (legacy): `repo` scope grants issue access but also full repo read/write. Avoid unless you must.

Rate limit: 5,000 requests/hour, shared across **all** tools the user uses.

### 8.2 GitHub App — best for multi-agent / shared use

When several agents (or several people running agents) share one repo, use a GitHub App. Benefits:
- Higher and isolated rate limits (per installation, not per user).
- Per-repo install with explicit permissions.
- Tokens are short-lived and refreshed automatically.
- Filings appear as `<app-name>[bot]`, easy to filter.

Setup: `Settings → Developer settings → GitHub Apps → New GitHub App` → set permissions `Issues: Read/Write`, `Metadata: Read` → install on the target repo(s).

### 8.3 `GITHUB_TOKEN` — inside GitHub Actions only

When the agent runs as a GitHub Action, no PAT is needed. The runner exposes a per-run `GITHUB_TOKEN`. Set `permissions: { issues: write }` at the job level.

Rate limit: 1,000/hour per repository. Plenty for normal automation.

### 8.4 Token storage rules

- Never commit a token. Pre-commit `gitleaks`.
- In CI: use the repo's encrypted Secrets.
- On a workstation: `gh auth login` (stores in OS keychain) or env-var loaded from a vault, never a plain `.env` you might commit.
- Treat any leak as P0 → revoke immediately at `Settings → Developer settings → Tokens → Revoke`.

---

## 9. Three Ways for Agents to Create Issues

Pick whichever the agent already has. All three are equivalent for our purposes.

### 9.1 The `gh` CLI (most ergonomic for shell-capable agents)

The CLI is preinstalled on every GitHub-hosted runner and on most dev machines.

```bash
# Auth once
gh auth login --with-token < $TOKEN_FILE

# Create an issue
gh issue create \
  --repo "owner/repo" \
  --title "[BUG] Login retries fail after 3rd attempt" \
  --body-file ./issue-body.md \
  --label "type:bug,priority:p1,source:agent,agent:claude" \
  --assignee "@me"

# Search for existing issues (dedupe)
gh issue list --repo "owner/repo" --state open \
  --search "login retries in:title,body" \
  --json number,title,url

# Comment on an existing one
gh issue comment 123 --repo "owner/repo" --body "Reproduced again at $(date -u)."

# Close with a reason
gh issue close 123 --repo "owner/repo" --reason "completed" --comment "Fixed in #456."
```

### 9.2 The REST API (works from any language / any agent)

Endpoint: `POST https://api.github.com/repos/{owner}/{repo}/issues`

```bash
curl -X POST \
  -H "Authorization: Bearer $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  https://api.github.com/repos/owner/repo/issues \
  -d '{
    "title": "[DEAD] handleLegacyAuth() unused since v3.2",
    "body": "## Evidence\n- grep -r returns 0 callers\n- Removed in OAuth migration ...\n\n## Indirect check\n- No reflection usage\n- No external SDK calls\n\n/cc @maintainer",
    "labels": ["type:dead-code", "source:agent", "status:needs-triage"]
  }'
```

Python (using `requests`):

```python
import os, requests, json

def create_issue(owner, repo, title, body, labels):
    url = f"https://api.github.com/repos/{owner}/{repo}/issues"
    r = requests.post(
        url,
        headers={
            "Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
        json={"title": title, "body": body, "labels": labels},
        timeout=15,
    )
    r.raise_for_status()
    return r.json()
```

Search for dupes first:

```python
def search_issues(owner, repo, query):
    # Search is its own endpoint with its own (lower) rate limit
    url = "https://api.github.com/search/issues"
    q = f"repo:{owner}/{repo} is:open is:issue {query}"
    r = requests.get(
        url,
        headers={"Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}"},
        params={"q": q},
        timeout=15,
    )
    r.raise_for_status()
    return r.json()["items"]
```

### 9.3 The GitHub MCP Server (best for Claude / Cursor / agentic IDEs)

GitHub maintains an official MCP server (`github/github-mcp-server`) that gives any MCP-capable agent direct natural-language access to:
- Create / update / search / close issues
- Read repo files and history
- Manage PRs, labels, assignees
- Read Dependabot alerts

For Claude (desktop or Claude Code), add to MCP config:

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "<your-fine-grained-PAT>"
      }
    }
  }
}
```

Once installed, the agent can call tools like `create_issue`, `list_issues`, `search_issues`, `update_issue`, `add_issue_comment` directly — no shell, no `curl`.

> If the agent is being told "search for relevant connectors," it should look up the **GitHub** connector specifically — the GitHub MCP is first-party and free.

---

## 10. The Mandatory Deduplication Step

Filing duplicate issues poisons the backlog. The agent must run a dedupe check before every create.

### 10.1 The dedupe algorithm (do this every time)

1. **Pick 3–6 distinctive keywords** from the finding. Avoid generic words like "bug" or "issue."
   - Good: `handleLegacyAuth`, `OAuth migration`, `retry exhaustion`
   - Bad: `error`, `failure`, `broken`
2. **Search open AND recently closed** issues:
   ```bash
   gh issue list --repo o/r --state all --limit 50 \
     --search "handleLegacyAuth oauth retry in:title,body"
   ```
3. **Score** each result for similarity. If the agent has access to embeddings, use cosine similarity on title+first-1KB-of-body. Without embeddings, use simple token overlap or have the LLM rate similarity 0–10.
4. **Threshold:**
   - Score ≥ 8 / 10 → **do not file**. Add a comment to the existing issue with new evidence. Use the format: "Re-observed at SHA `abc1234` running scenario X. New detail: ..."
   - Score 5–7 → file but link the related issue in the body ("Possibly related to #123, but distinct because ...").
   - Score < 5 → file as new.
5. **If you choose to file** despite a partial match, link the related issues explicitly. GitHub's "Duplicate of #N" syntax in a comment automatically marks the new one as a duplicate.

### 10.2 Why this matters

Without dedupe, a hardening pass that runs 5 agents over a week creates the same "long method in `auth.py`" issue five times. With dedupe, the second through fifth runs add a comment with "still present in commit X" — which is actually useful signal about whether the issue is being deprioritized.

### 10.3 Closing duplicates explicitly

When the agent decides a duplicate slipped through, it must explicitly close it:

```bash
gh issue close 456 --repo o/r --reason "not planned" \
  --comment "Duplicate of #123. Closing this one; please follow #123 for updates."
```

---

## 11. The Agent Operating Protocol

This is the runtime contract every agent follows. Paste this section into the project's `AGENTS.md` (top-level file most agentic tools auto-read).

### 11.1 The five-step turn cycle

For every meaningful turn that touches code or state:

**Step 1 — Read the open backlog.**
- `gh issue list --repo $REPO --state open --label "source:agent" --limit 50`
- Skim titles; build a mental list of what's already tracked. This **prevents** duplicate filings later in the turn.

**Step 2 — Do the work.**
- Whatever the human asked for: implement, refactor, debug, review.

**Step 3 — Observe the surroundings.**
- While reading code, note triggers from §3 and anomalies from §4 *that you are not fixing in this turn*.

**Step 4 — Triage in your head.**
For each observation:
- **Trivial + I'm here anyway** → fix it now (boy-scout rule).
- **Anything else** → queue it for filing.

**Step 5 — File before ending the turn.**
For each queued item:
1. Run dedupe (§10.1).
2. Pick the right template (§7).
3. Fill it out — **specific** title, evidence-rich body, correct labels.
4. Create the issue.
5. Note the issue number in your response so the human can see what was filed.

### 11.2 Writing the title

A good title is:
- **Specific** — names the symbol/file/endpoint involved.
- **Imperative or declarative** — describes the state of the world, not the fix.
- **Prefixed** with the template prefix (`[BUG]`, `[DEBT]`, `[DEAD]`, `[SEC]`, `[ANOMALY]`, `[ARCH]`, `[TEST]`).
- **Under ~80 characters.**

Examples:
- ✅ `[BUG] /orders POST returns 200 but row not inserted when amount==0`
- ✅ `[DEAD] PaymentLegacyProcessor unused since v4.0 migration (Apr 2024)`
- ✅ `[ANOMALY] checkout endpoint p99 jumped 8σ after deploy abc1234`
- ❌ `Bug in orders`  (too vague)
- ❌ `Fix the payment thing`  (talks about fix, not evidence)

### 11.3 Writing the body

Whatever the template, the body should always include:
1. **Evidence** the agent observed — log line, file:line, diff, query output, dashboard URL, commit SHA.
2. **What was expected** vs **what was observed** (FSV-style).
3. **Scope estimate** — what else might be affected.
4. **Repro steps** if non-trivial.
5. **Suggested next action** — even if it's "investigate further."
6. **Agent attribution footer:**
   ```
   ---
   Filed by: claude-opus-4.7
   Session: <date or correlation-id>
   Commit at time of filing: abc1234
   ```

### 11.4 When the agent does fix the issue in the same turn

Even then, **still mention it** in the PR/commit. Use `Fixes #N` / `Closes #N` in the commit message so GitHub auto-closes when the change merges. If no issue existed and the fix is small, the agent can either:
- Skip filing (the commit message describes the work).
- File-then-close-with-fix-in-same-PR (preferred for traceability of recurring problems).

For non-trivial fixes, *always* file the issue first and reference it in the PR — this gives reviewers context the diff alone can't carry.

### 11.5 Priority assignment heuristics

- **`priority:p0`** — security-exploitable now, prod outage, data loss possible.
- **`priority:p1`** — bug affecting users, security weakness with no immediate exploit, anomaly ≥ 3σ.
- **`priority:p2`** — tech debt slowing development, anomaly 2–3σ, test gap on a real code path.
- **`priority:p3`** — cosmetic, micro-optimization, far-future risk.

Default to **p2** when unsure. Humans can retriage; better than no priority.

---

## 12. Multi-Agent Handoff — Picking Up Where the Last One Left Off

The whole point is to make agent sessions composable. Here's how the next agent finds the work.

### 12.1 The agent's start-of-session ritual

Every agent's *very first* action on a repo:

```bash
# 1. What's open from prior agent runs?
gh issue list --repo $REPO --state open --label "source:agent" \
  --sort updated --limit 20 \
  --json number,title,labels,updatedAt

# 2. What did the last few agents work on?
gh issue list --repo $REPO --state all --label "source:agent" \
  --sort updated --limit 10 \
  --json number,title,state,closedAt

# 3. What's marked high-priority?
gh issue list --repo $REPO --state open \
  --label "priority:p0,priority:p1" --json number,title,labels
```

### 12.2 The "next-up" selection rule

When the human says "continue where you left off" or "work on the open issues":

1. Filter to `state:open` AND `source:agent` AND `assignee:none`.
2. Sort by `priority` ascending (p0 first), then `updated` ascending (oldest first).
3. Pick the top item; assign yourself (`gh issue edit N --add-assignee @me`).
4. Change `status:needs-triage` → `status:in-progress`.
5. Comment on the issue with start-of-work note: *"Picking this up — agent X — commit Y — plan: ..."*.
6. Work it.
7. On completion: open PR with `Closes #N`. The status label gets removed when the PR merges.

### 12.3 Handing off mid-task

If an agent runs out of context window or hits an interrupt:
- Comment on the issue with a **resume note**: what was tried, what was learned, what file/line to start at, what hypothesis to test next.
- Add label `status:blocked` if waiting on something, otherwise leave as `status:in-progress`.
- Unassign self only if you know you won't come back.

This is the single most valuable habit in multi-agent work: leave the next agent enough breadcrumbs to be productive in 30 seconds, not 30 minutes.

---

## 13. Automation Backstop — GitHub Actions Catches What Agents Miss

Agents are conscientious but not perfect. Layer in automation so static-analysis findings, dependency vulns, and stale issues are filed/managed without an agent in the loop.

### 13.1 SAST → Issues

Use Semgrep, CodeQL, or whichever scanner you already trust. Wire a workflow to convert each finding above a severity threshold into a GitHub Issue, with dedupe based on rule ID + file path.

`.github/workflows/sast-to-issues.yml` (skeleton):

```yaml
name: SAST → Issues
on:
  schedule: [{cron: "0 4 * * 1"}]   # weekly Monday 04:00 UTC
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
        run: |
          python .github/scripts/findings_to_issues.py findings.json
```

The Python script reads `findings.json`, hashes `rule_id + file + line range` as a fingerprint, searches existing issues for that fingerprint in the title, and only files new ones — same dedupe contract as agents.

### 13.2 Dependabot is free and on by default

Enable in `Settings → Security → Code security and analysis`:
- **Dependabot alerts** — free on all repos.
- **Dependabot security updates** — auto-opens PRs for vuln fixes.
- **Dependabot version updates** — set up via `.github/dependabot.yml`.

For each alert that isn't auto-fixed within N days, a small workflow can convert it to a `type:security` issue so it shows up in the same backlog the agents are watching.

### 13.3 Stale issue management

Don't let the backlog rot. Use the official `actions/stale` action.

`.github/workflows/stale-cleanup.yml`:

```yaml
name: Stale issues
on:
  schedule: [{cron: "30 1 * * *"}]
permissions:
  issues: write
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
            This issue has had no activity for 60 days. It will close in 14 days
            unless reactivated. Comment or remove the `status:stale` label to keep it open.
          close-issue-message: |
            Closed as stale. Reopen if still relevant.
```

### 13.4 Auto-labeling

`actions/labeler` applies labels by file path patterns automatically — useful for `area:*` labels.

### 13.5 Issue title fingerprinting for dedupe

For agents that don't compute embeddings, a cheap trick: include a structured fingerprint in the title.

Example for a SAST finding:
```
[SEC] python.lang.security.audit.dangerous-eval at api/handler.py:142 [fp:semgrep:py-eval-handler-142]
```

The `[fp:...]` token is a stable hash you can grep against in the title field. The next agent / workflow doing dedupe just does:

```bash
gh issue list --search "[fp:semgrep:py-eval-handler-142] in:title" --state all
```

If a result comes back, comment on it instead of filing new.

---

## 14. Issue Hygiene — Keeping the Backlog Useful

The backlog is only valuable if it stays trustworthy. Some practices to bake in.

### 14.1 The "Confirm or close" weekly review (5 minutes, human-led)

Once a week, the human (or a designated triage agent) does:
- Glance at issues with `status:needs-triage`. Either confirm (`status:confirmed`) or close with a reason.
- Re-prioritize anything that drifted.
- Close anything obviously fixed but not auto-closed.

### 14.2 Don't punish reporters (including agents)

Adam Cogan's rule for handling duplicates: even when closing a duplicate, leave a friendly note pointing to the tracker issue. The same applies to agent-filed duplicates — when you close one, the closing comment should reference the kept issue. This makes the next agent's dedupe work easier (the closed dup links to the canonical).

### 14.3 Don't over-label

Five to ten labels per issue is hard to read. Stick to: 1 `type:*`, 1 `priority:*`, 1 `status:*`, 1–2 `area:*`, plus `source:*`. That's the cap.

### 14.4 Use Milestones for hardening sweeps

Group all issues filed during a "harden the auth module" sweep into a Milestone of the same name. The Milestone page becomes the report of that sweep — closed vs open count, burn-down, sub-issue tree.

### 14.5 Use sub-issues (issue hierarchy)

GitHub now supports parent/sub-issue relationships natively. For a multi-issue sweep, create one parent **tracking issue** and link each finding as a sub-issue. The parent becomes the roll-up the human watches.

CLI doesn't fully support this yet (as of 2025); use the REST API with `POST /repos/{o}/{r}/issues/{n}/sub_issues` and the issue's numeric `id` (not number).

### 14.6 Trends to watch

Once you have a few weeks of agent-filed issues, ask:
- Which `area:*` accumulates the most issues? → That's where the real debt lives.
- What's the average time-to-close per `type:*`? → If `type:dead-code` is fast and `type:tech-debt` is slow, your team is doing surface cleanup without deep refactoring.
- Are agents finding things humans miss, or vice versa? → Tune the trigger list (§3) and templates (§7) accordingly.

---

## 15. Copy-Paste Pack — Ready-to-Use Files

These are minimal, working artifacts to drop into a repo today.

### 15.1 `AGENTS.md` (top of repo)

```markdown
# AGENTS.md — Operating Rules for AI Agents in This Repo

## The non-negotiable rule
If you observe a bug, defect, smell, dead code, redundant code, tech debt,
anomaly, security weakness, architectural concern, or test gap that you are
NOT fixing in this same turn — you MUST open a GitHub Issue before ending
your turn. See `.github/ISSUE_TEMPLATE/` for templates.

## Before opening an issue
1. Search existing open AND closed issues with 3–6 distinctive keywords.
2. If a clear duplicate exists, comment on the existing issue with new
   evidence instead of opening a new one.
3. If a partial match exists, link it in the new issue body.

## Required content for every issue you file
- Specific, prefixed title (e.g., `[BUG] ...`, `[DEBT] ...`, `[DEAD] ...`).
- Evidence (file:line, log line, query output, commit SHA).
- Expected vs observed (FSV style).
- Suspected scope / blast radius.
- Suggested next action.
- Footer: agent name, session id, commit SHA.

## Labels every agent issue must have
- One `type:*` label (bug, tech-debt, dead-code, duplication, security,
  performance, architecture, test-gap, docs, anomaly, risk).
- One `priority:*` label (default `priority:p2` when unsure).
- `source:agent` and `agent:<your-name>`.
- `status:needs-triage` for new ones; `status:in-progress` when you start work.

## Anomaly flagging
Any signal more than 2σ from its rolling mean → file as `type:anomaly`.
Include μ, σ, current value, z-score, and the window used.

## Multi-agent handoff
Before working: list open `source:agent` issues, pick the highest-priority
unassigned one, assign yourself, set `status:in-progress`, comment with your
plan.

When pausing mid-task: comment with what you tried, what you learned, and
where to resume.

## Tools at your disposal
- `gh` CLI (preferred for shell-capable agents).
- GitHub REST/GraphQL API (any language).
- GitHub MCP server (Claude / Cursor / agentic IDEs).

## Reference
See `docs/AGENT_ISSUE_FLAGGING_GUIDE.md` for the full protocol.
```

### 15.2 Minimal create-issue script (Python, dedupe-aware)

`.github/scripts/file_issue.py`:

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
"""
import argparse, json, os, sys
import urllib.parse, urllib.request

API = "https://api.github.com"

def gh(method, path, payload=None):
    req = urllib.request.Request(
        f"{API}{path}",
        method=method,
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
    res = gh("GET", f"/search/issues?{enc}")
    return res.get("items", [])

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--repo", required=True)
    p.add_argument("--title", required=True)
    p.add_argument("--body-file", required=True)
    p.add_argument("--labels", default="")
    p.add_argument("--keywords", required=True,
                   help="3–6 distinctive words for dedupe search")
    p.add_argument("--similarity-threshold", type=float, default=0.6)
    args = p.parse_args()

    body = open(args.body_file).read()
    labels = [l.strip() for l in args.labels.split(",") if l.strip()]

    # Dedupe
    matches = search_existing(args.repo, args.keywords)
    if matches:
        print(f"⚠ Possible duplicates found ({len(matches)}):")
        for m in matches[:5]:
            print(f"  #{m['number']} {m['state']} — {m['title']} — {m['html_url']}")
        # Conservative behavior: refuse to file. Caller can re-run with
        # --keywords narrowed or comment on the existing issue instead.
        sys.exit(2)

    owner, name = args.repo.split("/", 1)
    issue = gh("POST", f"/repos/{owner}/{name}/issues",
               {"title": args.title, "body": body, "labels": labels})
    print(f"✓ Filed #{issue['number']}: {issue['html_url']}")

if __name__ == "__main__":
    main()
```

Exit codes: `0` = filed; `2` = dedupe match found, did NOT file (agent should comment on the existing issue instead).

### 15.3 Quick start checklist (one-pass setup)

- [ ] Create labels (§6.2 script).
- [ ] Add 7 issue templates from §7.
- [ ] Add `.github/ISSUE_TEMPLATE/config.yml` disabling blank issues.
- [ ] Create `AGENTS.md` at repo root (§15.1).
- [ ] Generate a fine-grained PAT scoped to this repo with `Issues: Read/Write`.
- [ ] Store the PAT in the agent's credential vault / MCP config.
- [ ] Enable Dependabot alerts under Security settings.
- [ ] Add the `actions/stale` workflow (§13.3).
- [ ] (Optional) Install GitHub MCP server for Claude/Cursor.
- [ ] Test: run `gh issue create --title "[TEST] verifying setup" --body "test" --label "type:docs"`.
- [ ] Close the test issue.
- [ ] Tell your agent: *"From now on, file an issue for anything you see that isn't normal. Read AGENTS.md."*

You now have a free, durable, multi-agent backlog system.

---

## Final Notes

- **Cost:** $0/month, regardless of issue volume.
- **Limits to remember:** 5,000 API calls/hour authenticated; 1,000/hour in Actions; free private repos limited to 3 *collaborators with write access* (does not affect agents that authenticate as the owner). For private repos with more than 3 humans needing write, upgrade to Pro/Team — Issues themselves remain free.
- **The discipline, not the tool, is the work.** GitHub Issues makes filing free and frictionless. It's still up to the agents (and you) to actually file, dedupe, triage, and close. Re-read §2 if the backlog starts decaying.
- **Trust the system.** Once issues are the SoT, your prompts can shrink: "harden the auth module" becomes "work through `priority:p1` issues with `area:auth`," and the agent's first action is to read the queue. That's the win.