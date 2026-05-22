# Pinned GitHub Action SHAs

Scope: Phase F workflows `.github/workflows/{e2e,chaos,load,soak}.yml`.

| Action | Source ref resolved | Pinned SHA | Used by |
|---|---|---|---|
| `actions/checkout` | `refs/tags/v4` | `34e114876b0b11c390a56381ad16ebd13914f8d5` | e2e, chaos, load, soak |
| `actions/cache` | `refs/tags/v4` | `0057852bfaa89a56745cba8c7296529d2fc39830` | e2e, chaos, load, soak |
| `dtolnay/rust-toolchain` | `refs/heads/stable` | `29eef336d9b2848a0b548edc03f92a220660cdb8` | e2e, chaos, load, soak |

Rotation cadence: quarterly, or immediately when upstream publishes a security advisory for one of these actions.

Rotation procedure:

1. Resolve the upstream ref with `git ls-remote`.
2. Replace workflow pins with the new 40-character commit SHA.
3. Run direct `rg` readback over `.github/workflows/` for the pinned SHAs.
4. Record the old and new SHAs in the issue comment and manual verification notes.
