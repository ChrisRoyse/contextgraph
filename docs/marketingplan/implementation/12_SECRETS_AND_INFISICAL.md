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

Status legend: ✅ captured in `.env.local` (gitignored) · 🟡 partial / deferred · ⬜ not yet needed (later phase) · 📌 operator-managed elsewhere

| Secret | Source | Used by | Phase first needed | Status (2026-05-27) |
|---|---|---|---|---|
| `ANTHROPIC_API_KEY` | console.anthropic.com | Every LLM-calling agent | Phase 0 | 📌 already in operator's Infisical |
| `SLACK_BOT_TOKEN` (xoxb-...) | api.slack.com app `A0B6GJXT6R2` | M1 Operator Agent + M11 OperatorAsk | Phase 1 | ✅ |
| `SLACK_APP_LEVEL_TOKEN` (xapp-...) | Slack app config | M11 Socket Mode transport | Phase 1 | ✅ |
| `SLACK_SIGNING_SECRET` | Slack app Basic Information | Webhook signature verification | Phase 1 | ✅ |
| `SLACK_CLIENT_ID` + `SLACK_CLIENT_SECRET` | Slack app Basic Information | OAuth flows (future installs) | Phase 1 | ✅ |
| `SLACK_VERIFICATION_TOKEN` | Slack app Basic Information | Legacy webhook verification | Phase 1 | ✅ |
| `SLACK_WORKSPACE_ID` + `SLACK_OPERATOR_ASK_CHANNEL` | Slack URL inspection | OperatorAsk routing | Phase 1 | ✅ workspace `T0B3LAJFYGY` channel `C0B2Q0CSJTY` |
| `LEAPABLE_SERVICE_URL` | operator's leapable.ai install | Marketplace + Discover surfaces | Phase 0 | ✅ `https://leapable.ai` |
| `LEAPABLE_MCP_TRANSPORT` + `LEAPABLE_MCP_COMMAND` | operator's local MCP binary | Memory layer (every agent that queries vaults) | Phase 0 | ✅ stdio / `leapable-mcp` |
| `LEAPABLE_API_TOKEN` | leapable.ai admin (only for remote ops) | Marketplace publish, remote vault ops | Phase 2 | ⬜ deferred (local MCP doesn't need it) |
| `GITHUB_TOKEN` (fine-grained PAT) | github.com/settings/personal-access-tokens | D3 GitHub Maintainer | Phase 2 | ✅ scopes: Contents/Issues/Pull requests/Workflows/Discussions R+W; All repos; No expiration |
| `STRIPE_ACCOUNT_ID` | dashboard.stripe.com | Account identifier | Phase 2 | ✅ `acct_1TbZaYRR6EhrWnFx` (marketing-fleet sandbox) |
| `STRIPE_SECRET_KEY` (sk_test_...) | Stripe Developers → API Keys | Admin operations (product creation, etc.) | Phase 2 | ✅ test mode |
| `STRIPE_PUBLISHABLE_KEY` (pk_test_...) | Stripe Developers → API Keys | Client-side checkout init | Phase 2 | ✅ test mode |
| `STRIPE_RESTRICTED_KEY` (rk_test_...) | Stripe Developers → API Keys | O3 Referral Engineer day-to-day ops | Phase 2 | ✅ test mode (auto-generated by Stripe; AI-agent-scoped purpose-built key deferred to live mode) |
| `GMAIL_OAUTH_CLIENT_ID` + `GMAIL_OAUTH_CLIENT_SECRET` | console.cloud.google.com | D4 Outreach Drafter (Gmail send) | Phase 2 | ⬜ |
| `MS_GRAPH_CLIENT_ID` + `MS_GRAPH_CLIENT_SECRET` | portal.azure.com | D4 (MS Graph for Outlook contacts) | Phase 2 | ⬜ |
| `MARKIFACT_MCP_TOKEN` | markifact.com account | O4 Ad Operator (paid ad platforms) | Phase 2 | ⬜ business decision pending (subscription tier) |
| `HYPER_MCP_TOKEN` | hyper.mcp account | S1 Audience Sorter, S5 Competitive Intel, P3 Visual Generator | Phase 2 | ⬜ business decision pending |
| `WINDSOR_MCP_TOKEN` | windsor.ai account | Cross-platform analytics (S5, O7) | Phase 3 | ⬜ business decision pending |
| `SEARCHATLAS_MCP_TOKEN` | searchatlas account | O1 SEO + O2 GEO | Phase 3 | ⬜ business decision pending |
| `OPENAI_API_KEY` (optional) | platform.openai.com | Cross-validation fallback (rare) | Phase 3 | ⬜ |
| `LANGSMITH_API_KEY` | smith.langchain.com | Phase 4 observability | Phase 4 | ⬜ |
| `PROMETHEUS_AUTH_TOKEN` | self-hosted | Phase 4 dashboard | Phase 4 | ⬜ |
| `S3_ARCHIVE_ACCESS_KEY` + `S3_ARCHIVE_SECRET` | AWS or compatible | Phase 4 audit-log + RocksDB snapshot archive | Phase 4 | ⬜ |

Each secret added to Infisical exactly once. The bootstrap script handles all of them in one pass per environment.

### §5.1 2026-05-27 capture session evidence

Captured via Playwright-driven headed Chrome with operator handling logins/2FA. All values written to `marketing-fleet/.env.local` (gitignored). To migrate to Infisical: `infisical login` → `cd marketing-fleet && infisical init` → `bash secrets/setup-infisical.sh`.

Slack app artifacts:
- App: `marketing-fleet-bot` (ID `A0B6GJXT6R2`) in workspace `T0B3LAJFYGY` (Leapable)
- Bot user: `marketing-fleet` (always_online)
- OAuth scopes: `app_mentions:read, channels:history, channels:read, chat:write, chat:write.public, groups:history, groups:read, im:history, im:read, im:write, mpim:history, mpim:read, mpim:write, reactions:read, reactions:write, users:read`
- Socket Mode enabled (no public webhook URL needed); App-Level token has `connections:write` scope
- Bot invited to channel `C0B2Q0CSJTY` (operator action confirmed 2026-05-27)

GitHub PAT artifacts:
- Name: `marketing-fleet`; Owner: `ChrisRoyse`; All repositories; No expiration (Infisical-rotation-discipline-based)
- Scopes: Metadata (R, required) + Contents R+W + Issues R+W + Pull requests R+W + Workflows R+W + Discussions R+W

Stripe artifacts:
- Account: `acct_1TbZaYRR6EhrWnFx` ("marketing-fleet sandbox", test mode)
- Onboarding 8/8 complete, Managed Payments enabled (3.5% add-on in live mode), Marketplace business model for Connect
- 8 products + recurring prices live: see `marketing-fleet/state/stripe-products.json` (Leapable Subscriber/Creator Pro/Creator Plus, Synapse Individual Pro/Team, Polis Plus/Host/Curator)
- Connect Marketplace mode enabled for Creator (Leapable) + Host (Polis) revshare
- Operator's prior socialmedia2.com account (`acct_1Tb1gWPe6KXK0yCp`) untouched; remains separate

Operator action items remaining:
- [ ] `infisical login` then `bash marketing-fleet/secrets/setup-infisical.sh` to migrate `.env.local` → Infisical
- [ ] Delete `marketing-fleet/.env.local` after Infisical migration confirmed
- [ ] (Recommended) rotate the originally-exposed Anthropic key at console.anthropic.com
- [ ] When ready for live: Stripe "Switch to live account" → identity verify + bank link
- [ ] When Phase 2 begins: Gmail/MS Graph OAuth, Markifact/Hyper subscription decisions

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
