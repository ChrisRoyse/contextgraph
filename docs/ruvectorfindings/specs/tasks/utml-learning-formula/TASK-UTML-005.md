# TASK-UTML-005 — `compute_delta_omega` + sub-functions (effective_plasticity, landscape_health, stability_floor, agent_state_score)

```xml
<task_spec id="TASK-UTML-005" version="1.0">

<metadata>
  <title>compute_delta_omega geometric product (plasticity^0.4 · landscape^0.4 · stability^0.1 · agent_state^0.1) + sub-functions for each component</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-UTML-04">ΔΩ geometric weighting; sub-components</item>
    <item ref="AC-UTML-04-01">Geometric weighting non-negotiable; weights sum to 1.0</item>
    <item ref="AC-UTML-04-02">landscape_health = sigmoid(-λ_max(H_t)) · inverted_U(‖g_t‖)</item>
    <item ref="AC-UTML-04-03">stability_floor ∈ {1.0, 0.4, 0.1, 0.0}; 0.0 → step abort</item>
    <item ref="AC-UTML-04-04">agent_state_score defaults to 0.7 (NEVER 1.0) when E17 unavailable</item>
    <item ref="EC-UTML-03">ΔΩ=0 step ABORT + ALERT</item>
    <item ref="EC-UTML-10">E17 transcript window unavailable → 0.7 default</item>
    <item ref="component:DeltaOmega">crates/context-graph-mejepa-train/src/learning_signal/delta_omega.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
ΔΩ measures optimizer-substrate health: ReLU dormancy (Sutton 2024 plasticity loss), Hessian top-eigenvalue (SAM-style sharpness, Foret 2021), discrete stability_floor (NaN/Goodhart hard gate), and E17 AffectText agent_state_score (doc 12 §5.3). Geometric weighting is non-negotiable so any sub-component at 0 forces ΔΩ=0. The full Hessian power-iteration and continual-backprop dormant-unit detection are Phase 5 dependencies; this task ships the assembly and discrete-stability-floor logic plus the agent_state_score with mandatory 0.7 default.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy: MEJEPA_OBSERVE_BATCH_NAN)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-04; AC-UTML-04-01..04; EC-UTML-03, EC-UTML-10)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component DeltaOmega — compute_delta_omega + sub-functions)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §5.3 (E17 AffectText)</file>
  <file purpose="prior_tasks">TASK-UTML-001.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/delta_omega.rs` with `compute_delta_omega`, `compute_landscape_health`, `compute_stability_floor`, `compute_effective_plasticity`, `compute_agent_state_score`
    - Define `GsnrSummary` struct (healthy_fraction: f32) used by plasticity helper
    - Define `TranscriptWindow` placeholder type (alias or simple struct) for agent_state probe; real E17 forward call lives in TASK-UTML-008/Phase 5
    - All five compute functions per TECH-UTML behavior tables
    - Validate stability_floor ∈ {1.0, 0.4, 0.1, 0.0}; UtmlFactorOutOfRange otherwise
    - Validate inputs in [0, 1] for plasticity/landscape/agent_state
    - Geometric mean for plasticity = (dormancy * rank * gsnr).powf(1.0/3.0)
    - inverted_U helper: returns ~1.0 in [band_low, band_high], drops outside; configurable bands
    - compute_agent_state_score returns (f32, AgentStateSource) tuple where AgentStateSource is `String` per TECH-UTML data_models (or new enum re-using TASK-UTML-002 pattern)
    - When transcript_window=None, return (0.7, "default_neutral_no_transcript") — NEVER 1.0
    - Unit tests: each sub-component at 0 → ΔΩ=0; healthy state ΔΩ ≈ 0.92 (Career-Taxonomy anchor); stability_floor discrete table; agent_state_score None case returns 0.7
  </in_scope>
  <out_of_scope>
    - Hessian power iteration (Phase 5)
    - Continual-backprop dormant-unit re-init (TASK-UTML-009)
    - E17 forward inference (Phase 1c neural backend)
    - L_step assembly (TASK-UTML-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/delta_omega.rs">
use crate::error::{UtmlError, UtmlErrorCode};

#[derive(Debug, Clone, Copy)]
pub struct GsnrSummary { pub healthy_fraction: f32 }

#[derive(Debug, Clone)]
pub struct TranscriptWindow { pub utterances: Vec&lt;String&gt; } // placeholder

pub fn compute_delta_omega(
    plasticity: f32,
    landscape: f32,
    stability: f32,
    agent_state: f32,
) -&gt; Result&lt;f32, UtmlError&gt;;

pub fn compute_landscape_health(
    lambda_max: f32,
    grad_norm: f32,
    band_low: f32,    // default 0.1
    band_high: f32,   // default 10.0
) -&gt; f32;

pub fn compute_stability_floor(
    nan_or_inf_detected: bool,
    validation_rising_steps: u64,
    grad_norm: f32,
    grad_norm_warning_threshold: f32,
    goodhart_proxy_decoupled: bool,
) -&gt; f32;

pub fn compute_effective_plasticity(
    relu_dormancy_fraction: f32,
    grad_cov_effective_rank: f32,
    max_rank: f32,
    gsnr: &amp;GsnrSummary,
) -&gt; f32;

pub fn compute_agent_state_score(
    transcript_window: Option&lt;&amp;TranscriptWindow&gt;,
    e17_forward: Option&lt;&amp;dyn Fn(&amp;TranscriptWindow) -&gt; (f32, f32)&gt;, // (valence, arousal)
) -&gt; (f32, &amp;'static str);

fn sigmoid(x: f32) -&gt; f32;
fn inverted_u(x: f32, low: f32, high: f32) -&gt; f32;
    </signature>
  </signatures>
  <constraints>
    - Validate stability ∈ {1.0, 0.4, 0.1, 0.0}; UtmlFactorOutOfRange on any other value
    - Validate plasticity, landscape, agent_state in [0, 1]; UtmlFactorOutOfRange otherwise
    - delta_omega = plasticity^0.4 * landscape^0.4 * stability^0.1 * agent_state^0.1
    - Clamp to [0, 1] (f32 rounding may overshoot by ε)
    - 0.4+0.4+0.1+0.1 = 1.0 (verified in test)
    - compute_agent_state_score: if transcript_window.is_none() OR e17_forward.is_none() → return (0.7, "default_neutral_no_transcript")
    - else: (valence, arousal) = e17_forward(transcript); arousal_yu = inverted_u(arousal, 0.3, 0.7); score = clamp(valence * arousal_yu, 0, 1); return (score, "e17_transcript")
    - File &lt; 400 lines
  </constraints>
  <verification>
    - cargo test shows ≥ 10 tests pass: each sub-component at 0 zeros ΔΩ; healthy state ≈ 0.92; stability_floor 4-row table; transcript None → 0.7; geometric mean of plasticity sub-parts; inverted_U and sigmoid math
  </verification>
</definition_of_done>

<pseudo_code>
fn compute_delta_omega(p, l, s, a) -&gt; Result&lt;f32, UtmlError&gt; {
    for (name, v) in [("plasticity", p), ("landscape", l), ("agent_state", a)] {
        if !(0.0..=1.0).contains(&amp;v) { return Err(UtmlError::new(UtmlFactorOutOfRange, format!("{name}={v}"))); }
    }
    if !matches!(s, 1.0 | 0.4 | 0.1 | 0.0) { return Err(...); }
    let omega = p.powf(0.4) * l.powf(0.4) * s.powf(0.1) * a.powf(0.1);
    Ok(omega.clamp(0.0, 1.0))
}

fn compute_stability_floor(nan, rising, gn, warn, proxy) -&gt; f32 {
    if nan || proxy { return 0.0; }
    if rising > 10000 { return 0.1; }
    if gn > warn { return 0.4; }
    1.0
}

fn compute_effective_plasticity(dormancy, rank, max_rank, gsnr) -&gt; f32 {
    let pd = (1.0 - dormancy).clamp(0.0, 1.0);
    let pr = (rank / max_rank).clamp(0.0, 1.0);
    let pg = gsnr.healthy_fraction.clamp(0.0, 1.0);
    (pd * pr * pg).powf(1.0/3.0).clamp(0.0, 1.0)
}

fn compute_landscape_health(lambda_max, gn, low, high) -&gt; f32 {
    let sharp = sigmoid(-lambda_max);
    let yu = inverted_u(gn, low, high);
    (sharp * yu).clamp(0.0, 1.0)
}

fn compute_agent_state_score(window, e17) -&gt; (f32, &amp;'static str) {
    match (window, e17) {
        (Some(w), Some(fwd)) => {
            let (val, arou) = fwd(w);
            let yu = inverted_u(arou, 0.3, 0.7);
            ((val * yu).clamp(0.0, 1.0), "e17_transcript")
        }
        _ => (0.7, "default_neutral_no_transcript"),
    }
}

fn sigmoid(x: f32) -&gt; f32 { 1.0 / (1.0 + (-x).exp()) }
fn inverted_u(x, low, high) -&gt; f32 {
    if x &lt; low { return (x / low).clamp(0.0, 1.0); }
    if x &gt; high { return (1.0 - (x - high)/high).clamp(0.0, 1.0); }
    1.0
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/delta_omega.rs">all 5 compute fns + GsnrSummary + TranscriptWindow + helpers + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod delta_omega; pub use delta_omega::*;</file>
</files_to_modify>

<validation_criteria>
  <criterion>Geometric weight exponents sum to exactly 1.0</criterion>
  <criterion>Each sub-component at 0 zeros ΔΩ</criterion>
  <criterion>stability_floor table maps inputs to {1.0, 0.4, 0.1, 0.0} correctly</criterion>
  <criterion>agent_state_score with None returns (0.7, "default_neutral_no_transcript") — NEVER 1.0</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_omega</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
