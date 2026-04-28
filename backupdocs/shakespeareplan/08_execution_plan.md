# 08 Execution Plan

## Phase 0: Freeze Baseline And Artifacts

Outputs:

- `baseline_v21_browser_fsv.md`
- `v42_artifact_audit.md`
- model hashes for v2.1, v4.1, v4.2

Actions:

1. Confirm HF default and local v2.1 adapter.
2. Run a small v2.1 browser FSV to verify services still work.
3. Sample v4.2 data and mark it unsafe or reusable.
4. Do not train until data audit is complete.

## Phase 1: Build Clean Canon And Manifests

Outputs:

- `shakespeare_clean_canon.jsonl`
- `shakespeare_canon_manifest.json`
- `contamination_report.md`

Actions:

1. Re-clean source Shakespeare text.
2. Strip stage directions and headers.
3. Split by passage type and work.
4. Build anti-memorization index.
5. Hash everything.

## Phase 2: Rebuild Style Constellations

Outputs:

- `sonnets.constellation`
- `soliloquy.constellation`
- `comic_prose.constellation`
- `tragic_blank_verse.constellation`
- `constellation_fsv.md`

Actions:

1. Select clean members.
2. Compute core 14 embedder projections where available.
3. Build centroids and covariance.
4. Calibrate thresholds on held-out Shakespeare and modern controls.
5. Verify dirty scene headers are not accepted as style exemplars.

## Phase 3: Build SFT Data

Outputs:

- `shakespeare_v5_sft_tct.jsonl`
- `shakespeare_v5_sft_manifest.parquet`
- `dataset_fsv.md`

Actions:

1. Select real Shakespeare target passages from cleaned derived-signal records.
2. Score every candidate through TCT reward.
3. Apply contamination and anti-memorization gates.
4. Balance by category.
5. Reserve eval prompts before training.

Strict source rule:

- assistant targets are Shakespeare's writing only,
- scene titles, act headers, stage directions, speaker labels, and boilerplate
  are hard rejects,
- generated/adversarial/composition rows are eval or guard fixtures, not SFT
  target rows,
- duplicate target passages from multiple derived labels are deduplicated or
  explicitly downweighted.

## Phase 4: Build CAMD-DPO Data

Outputs:

- `shakespeare_v5_camd_dpo.jsonl`
- `camd_pair_report.md`

Actions:

1. Mine content-equivalent candidates using E1/E10/E14.
2. Rank style using calibrated style panel.
3. Create chosen/rejected pairs.
4. Add adversarial, code, structured, loop-negative pairs.
5. Hard reject dirty or unrelated pairs.
6. Sample 200 pairs manually in the report.

Do not reuse current DPO exports without rebuilding. The current physical audit
shows missing score traces and contaminated negative/positive rows. Rebuild from
clean, score-traced, derived-signal candidates.

## Phase 5: SFT Refresh

Outputs:

- `~/.contextgraph/models/shakespeare_lora_v5_tct/sft/`
- `training_sft_report.md`

Actions:

1. Stop daemon if VRAM requires it.
2. Train from v2.1 adapter.
3. Save checkpoints every 250 steps.
4. Run small direct FSV after every checkpoint.

## Phase 6: CAMD-DPO

Outputs:

- `~/.contextgraph/models/shakespeare_lora_v5_tct/dpo/`
- `training_dpo_report.md`

Actions:

1. Train conservative DPO from SFT checkpoint.
2. Use SFT regularization.
3. Use label smoothing for mined labels.
4. Monitor reward margins and style/content validation.

## Phase 7: Runtime Calibration

Outputs:

- `runtime_config_v5.json`
- `decoder_sweep_report.md`

Actions:

1. Sweep `repetition_penalty`, `top_p`, `temperature`, `max_tokens`.
2. Calibrate MVAG thresholds against v2.1 good outputs, modern controls, and v4.1 loops.
3. Derive SPLADE logit boost dictionary.
4. Validate code/JSON grammar constraints.

## Phase 8: Production FSV

Outputs:

- `browser_fsv_v5.md`
- screenshots or raw output logs
- `publish_decision.md`

Actions:

1. Start daemon, inference, dashboard.
2. Run full Playwright FSV.
3. Run 3-seed subset.
4. Run anti-memorization scan.
5. Decide publish or reject.

## Phase 9: Publish Only If Gate Passes

Outputs:

- HF model update,
- model card,
- reproducibility bundle,
- exact FSV numbers.

Actions:

1. Upload adapter and inference config.
2. Include constrained decoder code.
3. Include exact system prompt and runtime parameters.
4. Mark v4.1/v4.2 as research if retained.
