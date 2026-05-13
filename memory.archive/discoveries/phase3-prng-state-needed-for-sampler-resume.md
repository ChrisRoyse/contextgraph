# Discovery: Phase 0 SplitMix64 Needed Public State For Phase 3 Resume

Date: 2026-05-10

Phase 3 `BatchSampler` requires deterministic weighted sampling and checkpoint/resume. The existing Phase 0 `SplitMix64` implementation had a private module and no public state accessors, so downstream code could not save and restore exact RNG state.

Change made:
- `context_graph_mejepa_corpus::prng` is now public.
- `SplitMix64::{from_state,state,next_unit_f32,next_f32_signed}` were added.

This keeps one shared PRNG implementation across corpus generation, sampler replay, and counterfactual perturbation instead of duplicating RNG logic.
