---
date: 2026-05-11
task: global-infisical-wsl-auth
status: complete
---

# Session Report: Global Infisical WSL Auth

## Scope

Configured WSL user-level Infisical access so normal shell sessions do not require
manual `infisical login` or pasted tokens.

## Changes

- Installed `~/.local/bin/infisical` wrapper that shadows `/usr/bin/infisical`.
- Wrapper loads Universal Auth bootstrap variables from `~/.config/aiwonder.env`.
- Wrapper mints short-lived Infisical access tokens on demand and caches them in
  `~/.cache/infisical/aiwonder-prod.token` with mode `0600`.
- Wrapper refreshes the cached token when the decoded JWT expiry is within 60s.
- Wrapper injects default `projectId=c2d7e44c-d7d1-4b27-aa23-2ed5a97fa0b5` and
  `env=prod` for `infisical secrets ...` and `infisical run ...` commands.
- Added non-secret defaults to `~/.bashrc`: `INFISICAL_BOOTSTRAP_ENV`,
  `INFISICAL_PROJECT_ID`, and `INFISICAL_ENV`.

## Verification

- `command -v infisical` resolves to `/home/cabdru/.local/bin/infisical`.
- `infisical --version` reports the system CLI version through the wrapper.
- Deleted cached token and ran `infisical secrets get github_pat`; wrapper minted
  a fresh token and recreated cache files.
- Forced expiry by writing `1` to the exp cache, then reran secret lookup; wrapper
  refreshed the token.
- `infisical secrets get github_pat hf_hub_token --output=json --silent` returned
  only the expected key names.
- `infisical run -- bash -lc 'test -n "$github_pat" && test -n "$hf_hub_token"'`
  succeeded.
- Cache/source modes verified:
  - `~/.config/aiwonder.env`: `0600`
  - `~/.cache/infisical`: `0700`
  - token + exp cache files: `0600`

## Issue Filed

Because the GitHub and Hugging Face token values were pasted into chat, filed
GitHub issue #2 to track provider-side rotation. The issue records only key names
and verification steps, not token values.
