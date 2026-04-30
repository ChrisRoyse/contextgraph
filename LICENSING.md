# Licensing Boundary

This repository is a monorepo with multiple license domains. The license for
each file is determined by the nearest applicable license file or by the
subtree listed below.

## Repository Root

- Path: `/`
- License file: `LICENSE`
- License: PolyForm Noncommercial License 1.0.0
- Required notice: `Copyright 2025 Chris Royse`

## ClipCannon

- Path: `clipcannon/`
- License file: `clipcannon/LICENSE`
- Package metadata: `license = "BUSL-1.1"` in `clipcannon/pyproject.toml`
- License: Business Source License 1.1
- Change license: Apache License, Version 2.0
- Change date: 2030-03-31

## Research Artifacts And Public Data

- O*NET text database ZIPs are not committed. Local runs verify the O*NET
  license marker and source ZIP hash in each bundle's
  `onet_source_preflight.json`.
- Generated DynamicJEPA bundles under `tmp/` are local reproducibility
  artifacts and are not source-controlled.
- Any future checked-in third-party data license text belongs under
  `THIRD_PARTY_LICENSES/`.

## Paper Materials

- Path: `paper/` when present
- Intended license: CC BY 4.0 for paper text, figures, and supplementary
  material unless a file states otherwise.

## Operational Rule

Do not move code across these boundaries without updating this file and the
affected package metadata. If a dependency or generated artifact has a license
that cannot be represented by one of the boundaries above, fail the release
gate until the boundary is documented.
