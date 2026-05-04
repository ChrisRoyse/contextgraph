# 31 - Monolith Definition And Scope

## What The Monolith Is

The monolith is a **single integrated AI infrastructure** that lives
in **one repository** at `/home/cabdru/contextgraph/`. As of
2026-04-30, the repo contains:

```text
contextgraph/
  Cargo.toml             # Rust workspace root
  crates/                # Rust crates: storage, embedders, dynamicjepa, cli, mcp, ...
  clipcannon/            # ClipCannon vendored from chrisroyse/clipcannon (2026-04-30)
    src/
      clipcannon/        # 23-stage video understanding/edit DAG
      phoenix/           # GPU-native avatar engine (FLAME, FACS, identity manifold)
      voiceagent/        # real-time meeting bot (Zoom/Teams)
      license_server/    # HMAC billing + Stripe
      chrome_extension/  # browser extension
    models/              # FLAME2020 etc. (~74MB; gitignore candidate)
    pyproject.toml       # Python package; BSL-1.1
    uv.lock
  configs/               # domain packs, train configs, centroid sets
  docs/5090jepa/         # ME-JEPA charter + smokedocs (this dir)
  docs2/                 # PAPER.md, alldata.md, leapableai_youtube_transcripts.md
  paper/                 # one-paper LaTeX (smokedoc 19)
  tests/                 # cross-language integration tests
  Dockerfile             # one container for the whole monolith
  Makefile               # cross-language build orchestrator
```

That tree is the monolith. Anything outside that tree is **not the
monolith**.

## What The Monolith Does (One Capability List)

The monolith does all of the following from one binary surface:

1. **Ingest** any corpus — text, video, audio, code, documents — under
   a typed domain pack with full source-of-truth provenance.
2. **Project** every input through the unified DDA panel:
   - 13 frozen text embedders (E1-E13) for text;
   - 7 frozen video modalities for video;
   - 99 pairwise interaction features ($\binom{13}{2}=78$ text +
     $\binom{7}{2}=21$ video);
   - up to 91 signals/text-input + 28/video-clip.
3. **Train** action-conditioned latent predictors using ME-JEPA
   (career taxonomy + any user-supplied domain pack), with FSV at
   every CF write.
4. **Build** frozen multi-modal centroid sets (TCT Phase 1) from a
   reference subset of any domain.
5. **Generate** identity-locked outputs:
   - voice clones (Qwen3-TTS + LoRA, 0.961 SECS per Case 3);
   - video clones (Phoenix + EchoMimicV3 LoRA, Case 2);
   - text style clones (LoRA on Gemma-4-E4B-it, Case 1: 5/5 FSV);
   - career-domain plans + predictions (ME-JEPA);
   - any other domain-pack-defined output.
6. **Validate** every output via Constellation Guard (per-output cosine
   threshold against frozen centroid set; reject if drift).
7. **Audit** every operation through a tamper-evident chain anchored
   to OpenTimestamps.
8. **Bundle** every run into one immutable schema (smokedoc 29) with
   one `bundle_root_hash` covering every artifact.
9. **Reproduce** any past run from the bundle + the Apptainer/Docker
   image (smokedoc 20).

All from one CLI, one MCP server, one HTTP API.

## What The Monolith Does Not Do (Explicit)

- Not multi-GPU or distributed at v1.0. Single RTX 5090 (32 GB) is
  the hardware claim. Multi-GPU is post-v1.0.
- Not real subject-level career data. Public taxonomy benchmark only
  (smokedoc 01). Phase 12 privacy gate (smokedoc 25) is post-v1.0.
- Not federated training across institutions. Single-tenant runtime.
- Not retroactive content moderation. Trust+safety lives at ingest +
  output; no post-hoc takedown service.
- Not a hosted SaaS at v1.0. The monolith is an artifact you run.
  Hosting is post-v1.0 and is not in this doc set.

## The Brand

The monolith is one product with one name. Working name candidates:

- **Teleox** (already an existing brand from PAPER.md and `docs2/`).
- **Leapable** (sibling brand from the YouTube channel).
- **ContextGraph** (the repo name; already brand-recognized via
  YouTube videos).

Pick **one** before any external launch. Recommendation: **Teleox**,
because:

- "Teleological" is the academic-paper anchor (TCT = Teleological
  Constellation Training in PAPER.md);
- it inherits the YouTube content gravity (already public on
  Leapable AI channel);
- "ContextGraph" is too generic for trademark and SEO;
- "Teleox" maps to a single .ai domain (already owned per PAPER.md
  affiliations).

The repo can stay `contextgraph` (developer brand); the public
product is **Teleox** (consumer brand).

## What The Monolith Replaces

The monolith replaces the following, **as one product**:

| What it replaces | Replacement role |
| --- | --- |
| ElevenLabs / Cartesia / Resemble (voice cloning) | Voice subcommand of the monolith with Constellation Guard |
| HeyGen / Synthesia / Tavus (video avatar) | Video subcommand with frozen identity manifold + per-frame guard |
| MLflow / W&B / Arize (ML observability) | FSV bundle + drift detection are upstream of observability |
| Hugging Face Datasets (data prep) | DDA panel produces 91-signal datasets |
| Eightfold / Gloat (career skill graphs) | ME-JEPA career-taxonomy benchmark with audit |
| InstructGPT/DPO/Constitutional AI (alignment) | Constellation Guard as alignment alternative |

This is not a list of competitors to attack one-by-one. It is a list of
**what the monolith does** that today requires stitching multiple
vendors together.

## Scope Of v1.0 Release

v1.0 ships **everything in §"What The Monolith Does"** at the
single-RTX-5090 envelope:

- text + video corpus ingest;
- DDA panel (13 + 7 + 99 pairs);
- ME-JEPA train + predict + plan + surprise;
- voice + video + text-style generation;
- Constellation Guard at every output;
- FSV bundle + reproducible artifact;
- one paper, one DOI, one Apptainer SIF, one Docker image.

What is **not** in v1.0:

- Multi-GPU.
- Phase 12 (real subject data; smokedoc 25).
- Multi-step rollouts (smokedoc 24; appendix only).
- A hosted/managed service (post-v1.0; the artifact is the deliverable).
- Domain pack marketplace.
- Phoenix avatar at full visual fidelity beyond Case 2 measurement
  (the EchoMimicV3 sign-bug fix is in scope; high-fidelity
  beyond-baseline rendering is not).

## What The Monolith Refuses To Be

- **Not a bundle of products with one logo.** Five product names
  glued together is not a monolith. The user rejected that framing.
- **Not three papers.** One paper, one citation anchor, one DOI.
- **Not a microservice mesh.** One process. Cross-language is in-process
  (smokedoc 33 specifies the boundary).
- **Not a GitHub-only release.** Zenodo DOI required (per ACM Available
  badging).
- **Not a closed source.** OSS engine; commercial license tier on top
  if and when hosting becomes a product (post-v1.0).
- **Not vapor**. Every claim in v1.0 is reproducible from the
  bundle.

## Acceptance

The monolith definition is **locked** when:

- the brand name is chosen and trademark-screened;
- this doc's "What it does" list is the same list in the paper
  abstract and the README;
- this doc's "What it does not do" list is the same list in the paper
  Limitations section and the README;
- no internal communication (commits, slack, sales) describes the
  monolith as multiple products.
