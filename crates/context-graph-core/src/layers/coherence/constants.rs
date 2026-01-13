//! Constants from Constitution (gwt.kuramoto)
//!
//! These constants define the Kuramoto synchronization parameters
//! and Global Workspace Theory thresholds.
//!
//! # Deprecation Notice
//!
//! The threshold constants (`GW_THRESHOLD`, `HYPERSYNC_THRESHOLD`, `FRAGMENTATION_THRESHOLD`)
//! are deprecated in favor of domain-aware [`GwtThresholds`](super::GwtThresholds).
//! Use `GwtThresholds::from_atc()` or `GwtThresholds::default_general()` instead.

/// Kuramoto coupling strength K from constitution (kuramoto_K: 2.0)
pub const KURAMOTO_K: f32 = 2.0;

/// Number of oscillators in Kuramoto network (one per embedding space E1-E13).
/// Constitution: gwt.kuramoto.frequencies (13 values)
pub const KURAMOTO_N: usize = 13;

/// Base frequencies for each oscillator (Hz).
///
/// Constitution mapping (gwt.kuramoto.frequencies):
/// - [0]  E1_Semantic       = 40.0 Hz (gamma_fast - perception binding)
/// - [1]  E2_TemporalRecent = 8.0 Hz  (theta_slow - memory consolidation)
/// - [2]  E3_TemporalPeriod = 8.0 Hz  (theta_2 - hippocampal rhythm)
/// - [3]  E4_Entity         = 8.0 Hz  (theta_3 - prefrontal sync)
/// - [4]  E5_Causal         = 25.0 Hz (beta_1 - motor planning)
/// - [5]  E6_Sparse         = 4.0 Hz  (delta - deep sleep)
/// - [6]  E7_Code           = 25.0 Hz (beta_2 - active thinking)
/// - [7]  E8_Emotional      = 12.0 Hz (alpha - relaxed awareness)
/// - [8]  E9_HDC            = 80.0 Hz (high_gamma - cross-modal binding)
/// - [9]  E10_Multimodal    = 40.0 Hz (gamma_mid - attention)
/// - [10] E11_EntityKG      = 15.0 Hz (beta_3 - cognitive control)
/// - [11] E12_LateInteract  = 60.0 Hz (gamma_low - sensory processing)
/// - [12] E13_SPLADE        = 4.0 Hz  (delta_slow - slow wave sleep)
pub const KURAMOTO_BASE_FREQUENCIES: [f32; KURAMOTO_N] = [
    40.0,  // E1  gamma_fast
    8.0,   // E2  theta_slow
    8.0,   // E3  theta_2
    8.0,   // E4  theta_3
    25.0,  // E5  beta_1
    4.0,   // E6  delta
    25.0,  // E7  beta_2
    12.0,  // E8  alpha
    80.0,  // E9  high_gamma
    40.0,  // E10 gamma_mid
    15.0,  // E11 beta_3
    60.0,  // E12 gamma_low
    4.0,   // E13 delta_slow
];

/// Default coupling strength for Kuramoto network.
/// Constitution: 2.0 (already correct in KURAMOTO_K)
pub const KURAMOTO_DEFAULT_COUPLING: f32 = 0.5;

/// Step interval for Kuramoto stepper (10ms = 100Hz update rate).
pub const KURAMOTO_STEP_INTERVAL_MS: u64 = 10;

/// Global workspace ignition threshold from constitution (coherence_threshold: 0.8)
/// Using 0.7 as per task spec for GW_THRESHOLD
///
/// # Deprecation
///
/// Use [`GwtThresholds::default_general().gate`](super::GwtThresholds::default_general) or
/// [`GwtThresholds::from_atc()`](super::GwtThresholds::from_atc) for domain-aware thresholds.
#[deprecated(
    since = "0.5.0",
    note = "Use GwtThresholds::default_general().gate or GwtThresholds::from_atc() instead"
)]
pub const GW_THRESHOLD: f32 = 0.7;

/// Time step for Kuramoto integration (dt)
pub const KURAMOTO_DT: f32 = 0.01;

/// Number of integration steps per process call
pub const INTEGRATION_STEPS: usize = 10;

/// Hypersync threshold (r > 0.95 is pathological)
///
/// # Deprecation
///
/// Use [`GwtThresholds::default_general().hypersync`](super::GwtThresholds::default_general) or
/// [`GwtThresholds::from_atc()`](super::GwtThresholds::from_atc) for domain-aware thresholds.
#[deprecated(
    since = "0.5.0",
    note = "Use GwtThresholds::default_general().hypersync or GwtThresholds::from_atc() instead"
)]
pub const HYPERSYNC_THRESHOLD: f32 = 0.95;

/// Fragmentation threshold (r < 0.5)
///
/// # Deprecation
///
/// Use [`GwtThresholds::default_general().fragmentation`](super::GwtThresholds::default_general) or
/// [`GwtThresholds::from_atc()`](super::GwtThresholds::from_atc) for domain-aware thresholds.
#[deprecated(
    since = "0.5.0",
    note = "Use GwtThresholds::default_general().fragmentation or GwtThresholds::from_atc() instead"
)]
pub const FRAGMENTATION_THRESHOLD: f32 = 0.5;
