# Secrets Management + Infisical

> Phase 0 deliverable upgraded from "OS keychain" to **Infisical-first, local-`.env`-fallback**. Every secret the fleet uses (Anthropic API key, Slack bot token, Stripe keys, OAuth tokens for operator accounts, Markifact/Hyper MCP credentials) lives in Infisical in production. Build/test uses a gitignored `.env.local` for speed of iteration; operator migrates to Infisical with `bash secrets/setup-infisical.sh` once authenticated.

**Source spec docs:** `14_COMPLIANCE_AND_ETHICS.md §1 §7`, `16_TECH_STACK.md §11`, `02_FIRST_PRINCIPLES.md §3 §18 (rule 7)`.

---

## §1. Why secrets management matters more than the plan originally said

Original plan: "OS keychain on operator's host; never in agent prompts; rotated monthly." That's fine for solo-operator-single-host. It **fails** for:

| Scenario | OS-keychain failure mode |
|---|---|
| Contractor onboarding | No way to grant scoped access without sharing the operator's whole keychain |
| Multi-host (Synapse on Windows + LangGraph on Linux) | Keychain doesn't sync; manual replication is error-prone |
| Rotation drill | Manual; easy to forget a callsite |
| Audit ("who accessed Anthropic key last 30 days?") | Not available |
| Emergency revoke | Requires touching every host that has the credential |

**Infisical fixes all of these.** Centralized store, environment-scoped (dev/staging/prod), per-secret audit logs, time-bounded service tokens for non-operator agents, native CLI for rotation.

---

## §2. Architecture

```
┌──────────────────────────────────────────────────────────┐
│ Operator                                                   │
│   - generates secrets at provider consoles                 │
│   - `infisical login` (interactive browser auth)          │
│   - `infisical secrets set KEY=VALUE --env=ENV`           │
└────────────────────────────┬─────────────────────────────┘
                             │
                             ▼
┌──────────────────────────────────────────────────────────┐
│ Infisical (cloud or self-hosted)                          │
│   project: marketing-fleet                                 │
│   environments: dev | staging | prod                       │
│   secrets:                                                  │
│     ANTHROPIC_API_KEY                                      │
│     SLACK_BOT_TOKEN                                        │
│     SLACK_SIGNING_SECRET                                   │
│     STRIPE_API_KEY                                         │
│     MARKIFACT_MCP_TOKEN                                    │
│     HYPER_MCP_TOKEN                                        │
│     ...                                                     │
└────────────────────────────┬─────────────────────────────┘
                             │
                ┌────────────┼────────────┐
                ▼            ▼            ▼
       ┌────────────┐ ┌─────────────┐ ┌──────────────┐
       │ Orchestr.  │ │ Synapse host│ │ Playwright   │
       │ host       │ │ (Windows)   │ │ pool         │
       │            │ │             │ │              │
       │ infisical  │ │ infisical   │ │ infisical    │
       │ run --     │ │ run --      │ │ run --       │
       │ marketing  │ │ synapse-mcp │ │ playwright   │
       └────────────┘ └─────────────┘ └──────────────┘
                                                ▲
                                                │
                                  Each host gets a scoped
                                  service token; only the
                                  secrets that host needs.
```

The agent code never sees secret values in prompts. The `infisical run` wrapper injects them as env vars at process start. The `get_secret(name)` helper reads from `os.environ` and raises if missing.

---

## §3. The two modes

| Mode | Source | When |
|---|---|---|
| **Local `.env.local`** | Plain-text key/value at `marketing-fleet/.env.local` (gitignored) | Build/test only; fastest path to "agent has a key and runs" |
| **Infisical** | Centralized; environment-scoped; CLI-managed | Production, any contractor, any non-operator host, any rotation drill |

Both wire into the same `get_secret(name)` helper:

```python
# marketing-fleet/src/secrets/get_secret.py (sketch)
import os

def get_secret(name: str) -> str:
    """Read a secret from environment. Infisical injects in prod; .env.local in dev."""
    value = os.environ.get(name)
    if not value:
        raise SecretNotFoundError(
            f"Secret '{name}' not found. "
            f"Set it in Infisical (`infisical secrets set {name}=...`) "
            f"or in marketing-fleet/.env.local for build mode."
        )
    return value
```

The transition from `.env.local` → Infisical is invisible to agent code: same env-var names, same helper.

---

## §4. Bootstrap procedure (one-time, operator-driven)

Per `marketing-fleet/secrets/README.md`:

```bash
# 1. Operator authenticates (interactive browser flow)
infisical login

# 2. Initialize project link in the marketing-fleet directory
cd marketing-fleet
infisical init    # interactive: select project (create "marketing-fleet" if doesn't exist), select env (dev)

# 3. Push existing .env.local values to Infisical
bash secrets/setup-infisical.sh

# 4. After verifying with `infisical secrets --env=dev`, prompt deletes .env.local

# 5. Going forward, run the fleet via Infisical
infisical run --env=dev -- marketing goal create --spec=goals/test.md
```

The `setup-infisical.sh` script handles step 3: reads every `key=value` from `.env.local`, pushes each non-empty pair to Infisical, prompts to delete the local file after success.

---

## §5. The full secret inventory

| Secret | Source | Used by | Phase first needed |
|---|---|---|---|
| `ANTHROPIC_API_KEY` | console.anthropic.com | Every LLM-calling agent (all of them) | Phase 0 |
| `SLACK_BOT_TOKEN` | api.slack.com app config | M1 Operator Agent + M11 OperatorAsk | Phase 1 |
| `SLACK_SIGNING_SECRET` | api.slack.com app config | Webhook signature verification | Phase 1 |
| `LEAPABLE_MCP_URL` | operator's leapable.ai install | Memory layer (every agent that queries vaults) | Phase 0 |
| `LEAPABLE_API_TOKEN` | operator's leapable.ai admin | Leapable MCP authentication | Phase 0 |
| `GITHUB_TOKEN` | github.com user settings | D3 GitHub Maintainer (gh CLI auth) | Phase 2 |
| `GMAIL_OAUTH_CLIENT_ID` + `GMAIL_OAUTH_CLIENT_SECRET` | console.cloud.google.com | D4 Outreach Drafter (Gmail send) | Phase 2 |
| `MS_GRAPH_CLIENT_ID` + `MS_GRAPH_CLIENT_SECRET` | portal.azure.com | D4 (MS Graph send for Outlook contacts) | Phase 2 |
| `STRIPE_API_KEY` | dashboard.stripe.com | O3 Referral Engineer (Connect rev-share) | Phase 2 |
| `MARKIFACT_MCP_TOKEN` | markifact.com account | O4 Ad Operator (paid ad platforms) | Phase 2 |
| `HYPER_MCP_TOKEN` | hyper.mcp account | S1 Audience Sorter, S5 Competitive Intel, P3 Visual Generator | Phase 2 |
| `WINDSOR_MCP_TOKEN` | windsor.ai account | Cross-platform analytics (S5, O7) | Phase 3 |
| `SEARCHATLAS_MCP_TOKEN` | searchatlas account | O1 SEO + O2 GEO | Phase 3 |
| `OPENAI_API_KEY` (optional) | platform.openai.com | Cross-validation fallback (rare) | Phase 3 |
| `LANGSMITH_API_KEY` | smith.langchain.com | Phase 4 observability | Phase 4 |
| `PROMETHEUS_AUTH_TOKEN` | self-hosted | Phase 4 dashboard | Phase 4 |
| `S3_ARCHIVE_ACCESS_KEY` + `S3_ARCHIVE_SECRET` | AWS or compatible | Phase 4 audit-log + RocksDB snapshot archive | Phase 4 |

Each secret added to Infisical exactly once. The bootstrap script handles all of them in one pass per environment.

---

## §6. Per-environment secret scoping

| Env | Use case | Operator command |
|---|---|---|
| `dev` | Operator's local build/test | `infisical secrets set KEY=val --env=dev` |
| `staging` | Pre-production validation (Phase 4 onwards) | `--env=staging` |
| `prod` | Live operations against real accounts/channels | `--env=prod` |

The fleet refuses to publish to production accounts unless `INFISICAL_ENV=prod` AND the goal spec's `autonomy.level` permits it. Build-mode (dev) is safe-by-default: it can only publish to operator-designated test channels.

---

## §7. Rotation drill

Per `06_COMPUTER_USE_PLAYBOOK.md §3.3`: "Operator Agent rotates tokens monthly." With Infisical:

```bash
# Generate new key at provider console (e.g., console.anthropic.com)
infisical secrets set ANTHROPIC_API_KEY=<new-value> --env=dev
infisical secrets set ANTHROPIC_API_KEY=<new-value> --env=staging
infisical secrets set ANTHROPIC_API_KEY=<new-value> --env=prod

# Verify next agent run uses new key
infisical run --env=dev -- marketing wiki query "test"

# Invalidate the old key in provider console after 24h grace period
# (gives any in-flight processes time to pick up the new value)
```

Schedule: monthly cron via operator's calendar. Reflector files a `rotation_drill_complete.md` for the audit log.

---

## §8. Service tokens for non-operator hosts

When the Synapse host (Windows) or the Playwright pool needs Infisical access without the operator interactively logging in:

```bash
# At Infisical web console:
# 1. Create a Machine Identity with role "viewer" scoped to marketing-fleet/dev (or prod)
# 2. Generate Universal Auth client ID + client secret
# 3. Set on the target host:
INFISICAL_CLIENT_ID=...
INFISICAL_CLIENT_SECRET=...
# 4. Run with:
infisical run --token "$(infisical login --method=universal-auth --client-id=$INFISICAL_CLIENT_ID --client-secret=$INFISICAL_CLIENT_SECRET --plain)" --env=prod -- <command>
```

Per-host scoping limits blast radius if a host is compromised.

---

## §9. Audit + access log

Infisical web console shows for every secret:
- When it was created / updated / read
- By which user or machine identity
- Source IP

Operator queries this quarterly as part of the `marketing audit --quarterly` per `19 §6`:

```bash
# Manual export from Infisical web console; or via API:
infisical audit-logs --project=marketing-fleet --since="30 days ago" --format=json > /tmp/secrets-audit.json
```

Any unexpected access → investigate immediately.

---

## §10. What this REPLACES in the original plan

| Original (Phase 0) | New |
|---|---|
| `~/.marketing-fleet/keychain/` (OS keychain) | Infisical project `marketing-fleet`, env `dev`/`staging`/`prod` |
| "Keychain on operator's host" | Infisical-first; `.env.local` fallback only for solo-operator-build-mode |
| "Per-agent scoped tokens" (vague) | Infisical Machine Identities per non-operator host with viewer-only role |
| "Monthly rotation" (manual) | Scripted rotation via `infisical secrets set --env=...` for every env |

---

## §11. Tradeoffs vs alternatives

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| **Infisical (chosen)** | Open-source; self-hostable; CLI-native; good audit; per-env scoping | Yet-another-service to maintain | Net positive — outweighs the maintenance cost |
| AWS Secrets Manager | AWS-native; battle-tested | Pulls in AWS dependency; less ergonomic CLI | Reject — operator's stack isn't AWS-centric |
| HashiCorp Vault | Industry-standard; very flexible | Heavyweight; significant ops burden | Reject — overkill for single-operator scale |
| 1Password CLI | Operator already familiar; ergonomic | Per-seat licensing for non-operator hosts | Reject — Machine Identity story isn't as clean |
| Plain `.env.local` only | Zero infrastructure | No audit; no rotation; no scoping; doesn't scale past solo-operator | Use as fallback, not as primary |

---

## §12. Operator action items (one-time)

- [ ] Run `infisical login` (interactive)
- [ ] Run `cd marketing-fleet && infisical init` (create project + select dev env)
- [ ] Run `bash secrets/setup-infisical.sh` (pushes `.env.local` values)
- [ ] Verify `infisical secrets --env=dev` shows the expected keys
- [ ] Delete `.env.local` (script prompts after successful push)
- [ ] Add Machine Identity for Synapse host (Phase 2; Synapse is on Windows desktop)
- [ ] Add Machine Identity for any future Playwright pool host (Phase 3)
- [ ] Schedule monthly rotation drill in calendar

---

## §13. Cross-references

- Bootstrap script: `marketing-fleet/secrets/setup-infisical.sh`
- Bootstrap README: `marketing-fleet/secrets/README.md`
- `.env.local` (current build/test secrets): `marketing-fleet/.env.local` (gitignored)
- Compliance bound (audit log requirements): `docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md §7`
- Security posture (the rest of `16 §11` security points): `docs/marketingplan/16_TECH_STACK.md §11`
- Doctrine — "trust the harness, never secrets in prompts": `docs/marketingplan/02_FIRST_PRINCIPLES.md §3 §18 (rule 7)`
- Phase 0 build (where Infisical bootstrap lands): `01_PHASE_0_FOUNDATIONS.md §2.1 §3 Day 5`
- Tool Gateway integration (where get_secret is called): `10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md`
