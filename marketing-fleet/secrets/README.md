# marketing-fleet / secrets

> Where secrets live + how to wire Infisical. See `docs/marketingplan/implementation/12_SECRETS_AND_INFISICAL.md` for the full design.

## Two modes

| Mode | What | When |
|---|---|---|
| **Local `.env.local`** | Plain-text key/value file at `marketing-fleet/.env.local`. Gitignored. | Build/test only; the fastest path to "agent has a key and runs." |
| **Infisical** | Centralized secret store; CLI-managed; environment-scoped (dev/staging/prod) | Production; any contractor onboarding; rotation drills; audit-of-access |

Both modes are wired into the same code: agents call a `get_secret(name)` helper that reads from Infisical first (if `INFISICAL_PROJECT_ID` env is set), falls back to `.env.local`, raises if neither has the key.

## Bootstrap procedure (one-time)

1. **Install Infisical CLI** (already done if v0.43+; check with `infisical --version`)
2. **Operator logs in:**
   ```
   infisical login
   ```
   Opens browser; auth flow; saves credentials to `~/.infisical/`.
3. **Initialize project in this directory:**
   ```
   cd marketing-fleet
   infisical init
   ```
   Interactive: select "marketing-fleet" project (create if doesn't exist), select environment (dev).
4. **Push local secrets to Infisical:**
   ```
   bash secrets/setup-infisical.sh
   ```
   Reads `.env.local`, pushes each non-empty key/value pair, prompts to delete `.env.local` after success.
5. **Run the fleet via Infisical going forward:**
   ```
   infisical run --env=dev -- marketing goal create --spec=goals/test.md
   ```
   Injects all secrets as env vars; nothing on disk.

## Rotation drill

```bash
# Generate new key at https://console.anthropic.com/settings/keys
# Then push the new value:
infisical secrets set ANTHROPIC_API_KEY=<new-value> --env=dev
infisical secrets set ANTHROPIC_API_KEY=<new-value> --env=prod

# Optionally invalidate the old key in Anthropic console after verifying the new one works.
```

## Don't commit

`.gitignore` blocks:
- `.env`, `.env.*` (except `.env.example`)
- `secrets/*.local`
- `secrets/*.key`
- `*.pem`
- `credentials.json`

If you accidentally commit a secret, rotate it immediately — the git history persists indefinitely.

## Why both modes

- Local `.env.local` lets a solo operator on a fresh machine start in 30 seconds.
- Infisical adds: centralization, environment separation, audit logs, rotation tooling, per-contractor scoped tokens.
- The fleet code reads the same `get_secret()` helper; the source is transparent.
