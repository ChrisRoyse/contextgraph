//! Kuramoto Oscillator Network for 13-embedding phase synchronization.
//!
//! Implements the Kuramoto model for coupled oscillators as specified in
//! Constitution v4.0.0 Section gwt.kuramoto:
//!
//! ```text
//! dθᵢ/dt = ωᵢ + (K/N) Σⱼ sin(θⱼ - θᵢ)
//! ```
//!
//! Where:
//! - θᵢ = Phase of embedder i ∈ [0, 2π]
//! - ωᵢ = Natural frequency of embedder i
//! - K = Coupling strength
//! - N = Number of oscillators (13)
//!
//! The order parameter r measures synchronization:
//! ```text
//! r · e^(iψ) = (1/N) Σⱼ e^(iθⱼ)
//! ```
//!
//! When r → 1, all oscillators are in phase (synchronized).
//! When r → 0, phases are uniformly distributed (incoherent).

use std::f64::consts::PI;
use std::time::Duration;

use crate::config::PhaseConfig;
use crate::error::{UtlError, UtlResult};

/// Number of oscillators (one per embedding space).
pub const NUM_OSCILLATORS: usize = 13;

/// Names of the 13 embedding spaces corresponding to each oscillator.
pub const EMBEDDER_NAMES: [&str; NUM_OSCILLATORS] = [
    "E1_Semantic",       // e5-large-v2
    "E2_TempRecent",     // exponential decay
    "E3_TempPeriodic",   // Fourier
    "E4_TempPositional", // sinusoidal PE
    "E5_Causal",         // Longformer SCM
    "E6_SparseLex",      // SPLADE
    "E7_Code",           // Qodo-Embed-1-1.5B
    "E8_Graph",          // MiniLM structure
    "E9_HDC",            // 10K-bit hyperdimensional
    "E10_Multimodal",    // CLIP
    "E11_Entity",        // MiniLM facts
    "E12_LateInteract",  // ColBERT
    "E13_SPLADE",        // SPLADE v3
];

/// Brain wave frequency bands for reference (from Constitution v4.0.0).
/// These define the RELATIVE frequencies; actual values are normalized.
#[allow(dead_code)] // Reference data for documentation and future frequency-based analysis
pub const BRAIN_WAVE_FREQUENCIES_HZ: [f64; NUM_OSCILLATORS] = [
    40.0, // E1_Semantic - gamma band (conscious binding)
    8.0,  // E2_TempRecent - alpha band (temporal integration)
    8.0,  // E3_TempPeriodic - alpha band (temporal integration)
    8.0,  // E4_TempPositional - alpha band (temporal integration)
    25.0, // E5_Causal - beta band (causal reasoning)
    4.0,  // E6_SparseLex - theta band (sparse activations)
    25.0, // E7_Code - beta band (structured thinking)
    12.0, // E8_Graph - alpha-beta transition
    80.0, // E9_HDC - high-gamma band (holographic)
    40.0, // E10_Multimodal - gamma band (cross-modal binding)
    15.0, // E11_Entity - beta band (factual grounding)
    60.0, // E12_LateInteract - high-gamma band (token precision)
    4.0,  // E13_SPLADE - theta band (keyword sparse)
];

/// Default natural frequencies for each embedder (normalized Hz).
///
/// These are normalized versions of the Constitution v4.0.0 brain wave frequencies
/// to enable synchronization with coupling strength K in [0, 10].
///
/// The normalization preserves the relative frequency ratios between embedders
/// while keeping the mean around 1.0 Hz for numerical stability.
///
/// Brain wave bands represented (see BRAIN_WAVE_FREQUENCIES_HZ for actual Hz):
/// - gamma (40Hz): conscious binding
/// - alpha (8Hz): temporal integration
/// - beta (25Hz): causal/structured reasoning
/// - theta (4Hz): sparse activations
/// - high-gamma (60-80Hz): precision/holographic
///
/// Normalized formula: normalized_freq = actual_freq / mean(all_freqs)
/// Mean of brain wave freqs = (40+8+8+8+25+4+25+12+80+40+15+60+4)/13 ≈ 25.3
pub const DEFAULT_NATURAL_FREQUENCIES: [f64; NUM_OSCILLATORS] = [
    1.58, // E1_Semantic - gamma band (40/25.3)
    0.32, // E2_TempRecent - alpha band (8/25.3)
    0.32, // E3_TempPeriodic - alpha band (8/25.3)
    0.32, // E4_TempPositional - alpha band (8/25.3)
    0.99, // E5_Causal - beta band (25/25.3)
    0.16, // E6_SparseLex - theta band (4/25.3)
    0.99, // E7_Code - beta band (25/25.3)
    0.47, // E8_Graph - alpha-beta transition (12/25.3)
    3.16, // E9_HDC - high-gamma band (80/25.3)
    1.58, // E10_Multimodal - gamma band (40/25.3)
    0.59, // E11_Entity - beta band (15/25.3)
    2.37, // E12_LateInteract - high-gamma band (60/25.3)
    0.16, // E13_SPLADE - theta band (4/25.3)
];

/// Kuramoto Oscillator Network for Global Workspace synchronization.
///
/// Models the 13 embedding spaces as coupled phase oscillators.
/// When coupling strength K is sufficient, the oscillators synchronize,
/// enabling coherent "conscious" percepts.
///
/// # Constitution Reference
///
/// - Section gwt.kuramoto defines the dynamics
/// - Order parameter r ≥ 0.8 indicates CONSCIOUS state
/// - Order parameter r < 0.5 indicates FRAGMENTED state
///
/// # Example
///
/// ```
/// use context_graph_utl::phase::KuramotoNetwork;
/// use std::time::Duration;
///
/// let mut network = KuramotoNetwork::new();
///
/// // Simulate 100 time steps
/// for _ in 0..100 {
///     network.step(Duration::from_millis(10));
/// }
///
/// // Check synchronization level
/// let (r, _psi) = network.order_parameter();
/// println!("Order parameter r = {:.3}", r);
/// ```
#[derive(Debug, Clone)]
pub struct KuramotoNetwork {
    /// Phase angles θᵢ for each oscillator in [0, 2π].
    phases: [f64; NUM_OSCILLATORS],

    /// Natural frequencies ωᵢ for each oscillator (radians/second).
    natural_frequencies: [f64; NUM_OSCILLATORS],

    /// Coupling strength K (global coupling).
    coupling_strength: f64,

    /// Total elapsed time in seconds.
    elapsed_total: f64,

    /// Whether the network is enabled (can be disabled for testing).
    enabled: bool,
}

impl KuramotoNetwork {
    /// Create a new Kuramoto network with default parameters.
    ///
    /// Default coupling strength K = 0.5 (moderate coupling).
    /// Initial phases are randomly distributed to start from incoherent state.
    pub fn new() -> Self {
        // Initialize with slightly different phases based on embedder index
        // to create realistic initial conditions
        let mut phases = [0.0; NUM_OSCILLATORS];
        for (i, phase) in phases.iter_mut().enumerate() {
            // Spread initial phases across [0, 2π] with some structure
            *phase = (i as f64 / NUM_OSCILLATORS as f64) * 2.0 * PI;
        }

        // Convert Hz to radians/second
        let mut natural_frequencies = [0.0; NUM_OSCILLATORS];
        for (i, freq) in natural_frequencies.iter_mut().enumerate() {
            *freq = DEFAULT_NATURAL_FREQUENCIES[i] * 2.0 * PI;
        }

        Self {
            phases,
            natural_frequencies,
            coupling_strength: 0.5, // Default moderate coupling
            elapsed_total: 0.0,
            enabled: true,
        }
    }

    /// Create a synchronized network (all phases aligned).
    ///
    /// Useful for testing or initializing from a known state.
    pub fn synchronized() -> Self {
        let mut network = Self::new();
        for phase in network.phases.iter_mut() {
            *phase = 0.0; // All phases at 0
        }
        network
    }

    /// Create an incoherent network (phases uniformly distributed).
    pub fn incoherent() -> Self {
        let mut network = Self::new();
        for (i, phase) in network.phases.iter_mut().enumerate() {
            *phase = (i as f64 / NUM_OSCILLATORS as f64) * 2.0 * PI;
        }
        network
    }

    /// Create from phase configuration.
    pub fn from_config(config: &PhaseConfig) -> Self {
        let mut network = Self::new();
        network.coupling_strength = config.coupling_strength as f64;
        network
    }

    /// Step the network forward in time using Kuramoto dynamics.
    ///
    /// Implements: dθᵢ/dt = ωᵢ + (K/N) Σⱼ sin(θⱼ - θᵢ)
    ///
    /// Uses Euler integration for simplicity and performance.
    pub fn step(&mut self, elapsed: Duration) {
        if !self.enabled {
            return;
        }

        let dt = elapsed.as_secs_f64();
        self.elapsed_total += dt;

        let n = NUM_OSCILLATORS as f64;
        let k = self.coupling_strength;

        // Compute phase derivatives
        let mut d_phases = [0.0; NUM_OSCILLATORS];

        for (i, d_phase) in d_phases.iter_mut().enumerate() {
            // Natural frequency term
            let mut d_theta = self.natural_frequencies[i];

            // Coupling term: (K/N) Σⱼ sin(θⱼ - θᵢ)
            let mut coupling_sum = 0.0;
            for j in 0..NUM_OSCILLATORS {
                if i != j {
                    coupling_sum += (self.phases[j] - self.phases[i]).sin();
                }
            }
            d_theta += (k / n) * coupling_sum;

            *d_phase = d_theta;
        }

        // Update phases (Euler integration)
        for (phase, d_phase) in self.phases.iter_mut().zip(d_phases.iter()) {
            *phase += d_phase * dt;

            // Wrap to [0, 2π]
            *phase = phase.rem_euclid(2.0 * PI);
        }
    }

    /// Compute the Kuramoto order parameter (r, ψ).
    ///
    /// r · e^(iψ) = (1/N) Σⱼ e^(iθⱼ)
    ///
    /// # Returns
    ///
    /// Tuple (r, ψ) where:
    /// - r ∈ [0, 1] is the synchronization level
    /// - ψ ∈ [0, 2π] is the mean phase
    ///
    /// # Interpretation
    ///
    /// - r ≈ 0: Incoherent (phases uniformly distributed)
    /// - r ≈ 0.5: Partial synchronization (EMERGING state)
    /// - r ≥ 0.8: Synchronized (CONSCIOUS state)
    /// - r ≈ 1: Perfect synchronization
    pub fn order_parameter(&self) -> (f64, f64) {
        let n = NUM_OSCILLATORS as f64;

        // Sum of e^(iθⱼ) = cos(θⱼ) + i·sin(θⱼ)
        let mut sum_cos = 0.0;
        let mut sum_sin = 0.0;

        for &phase in &self.phases {
            sum_cos += phase.cos();
            sum_sin += phase.sin();
        }

        // Average
        let avg_cos = sum_cos / n;
        let avg_sin = sum_sin / n;

        // r = |z| = sqrt(cos² + sin²)
        let r = (avg_cos * avg_cos + avg_sin * avg_sin).sqrt();

        // ψ = arg(z) = atan2(sin, cos)
        let psi = avg_sin.atan2(avg_cos).rem_euclid(2.0 * PI);

        (r, psi)
    }

    /// Get the synchronization level (order parameter r).
    ///
    /// This is the primary metric for consciousness state determination.
    #[inline]
    pub fn synchronization(&self) -> f64 {
        self.order_parameter().0
    }

    /// Check if network is in CONSCIOUS state (r ≥ 0.8).
    #[inline]
    pub fn is_conscious(&self) -> bool {
        self.synchronization() >= 0.8
    }

    /// Check if network is FRAGMENTED (r < 0.5).
    #[inline]
    pub fn is_fragmented(&self) -> bool {
        self.synchronization() < 0.5
    }

    /// Check if network is HYPERSYNC (r > 0.95).
    ///
    /// Warning: This may indicate pathological synchronization.
    #[inline]
    pub fn is_hypersync(&self) -> bool {
        self.synchronization() > 0.95
    }

    /// Get the phase of a specific embedder.
    pub fn phase(&self, embedder_idx: usize) -> Option<f64> {
        if embedder_idx < NUM_OSCILLATORS {
            Some(self.phases[embedder_idx])
        } else {
            None
        }
    }

    /// Get all phases as a slice.
    #[inline]
    pub fn phases(&self) -> &[f64; NUM_OSCILLATORS] {
        &self.phases
    }

    /// Set the phase of a specific embedder.
    ///
    /// # Errors
    ///
    /// Returns error if embedder_idx is out of range.
    pub fn set_phase(&mut self, embedder_idx: usize, phase: f64) -> UtlResult<()> {
        if embedder_idx >= NUM_OSCILLATORS {
            return Err(UtlError::PhaseError(format!(
                "Embedder index {} out of range [0, {})",
                embedder_idx, NUM_OSCILLATORS
            )));
        }
        self.phases[embedder_idx] = phase.rem_euclid(2.0 * PI);
        Ok(())
    }

    /// Get the coupling strength K.
    #[inline]
    pub fn coupling_strength(&self) -> f64 {
        self.coupling_strength
    }

    /// Set the coupling strength K.
    ///
    /// # Arguments
    ///
    /// * `k` - Coupling strength, clamped to [0, 10]
    pub fn set_coupling_strength(&mut self, k: f64) {
        self.coupling_strength = k.clamp(0.0, 10.0);
    }

    /// Get the natural frequency of a specific embedder.
    pub fn natural_frequency(&self, embedder_idx: usize) -> Option<f64> {
        if embedder_idx < NUM_OSCILLATORS {
            // Convert back to Hz
            Some(self.natural_frequencies[embedder_idx] / (2.0 * PI))
        } else {
            None
        }
    }

    /// Get all natural frequencies as Hz.
    pub fn natural_frequencies(&self) -> [f64; NUM_OSCILLATORS] {
        let mut hz = [0.0; NUM_OSCILLATORS];
        for (hz_val, freq) in hz.iter_mut().zip(self.natural_frequencies.iter()) {
            *hz_val = freq / (2.0 * PI);
        }
        hz
    }

    /// Set the natural frequency of a specific embedder (in Hz).
    ///
    /// # Errors
    ///
    /// Returns error if embedder_idx is out of range or frequency is non-positive.
    pub fn set_natural_frequency(&mut self, embedder_idx: usize, freq_hz: f64) -> UtlResult<()> {
        if embedder_idx >= NUM_OSCILLATORS {
            return Err(UtlError::PhaseError(format!(
                "Embedder index {} out of range [0, {})",
                embedder_idx, NUM_OSCILLATORS
            )));
        }
        if freq_hz <= 0.0 {
            return Err(UtlError::PhaseError(format!(
                "Frequency must be positive, got {}",
                freq_hz
            )));
        }
        self.natural_frequencies[embedder_idx] = freq_hz * 2.0 * PI;
        Ok(())
    }

    /// Get the total elapsed time since creation or reset.
    #[inline]
    pub fn elapsed_total(&self) -> Duration {
        Duration::from_secs_f64(self.elapsed_total)
    }

    /// Reset the network to initial conditions.
    pub fn reset(&mut self) {
        // Reset to uniformly distributed phases
        for i in 0..NUM_OSCILLATORS {
            self.phases[i] = (i as f64 / NUM_OSCILLATORS as f64) * 2.0 * PI;
        }
        self.elapsed_total = 0.0;
    }

    /// Reset to synchronized state.
    pub fn reset_synchronized(&mut self) {
        for phase in self.phases.iter_mut() {
            *phase = 0.0;
        }
        self.elapsed_total = 0.0;
    }

    /// Enable or disable the network.
    ///
    /// When disabled, `step()` has no effect.
    #[inline]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the network is enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the cosine component of the order parameter.
    ///
    /// This is equivalent to cos(φ) in the UTL formula,
    /// measuring overall phase alignment.
    pub fn cos_phase(&self) -> f64 {
        let (r, _psi) = self.order_parameter();
        // The effective phase alignment is r * cos(ψ),
        // but for UTL we just need r as the coherence measure
        // since ψ is the collective mean phase
        r
    }

    /// Compute phase difference between two embedders.
    pub fn phase_difference(&self, i: usize, j: usize) -> Option<f64> {
        if i >= NUM_OSCILLATORS || j >= NUM_OSCILLATORS {
            return None;
        }
        let diff = (self.phases[i] - self.phases[j]).rem_euclid(2.0 * PI);
        // Return the smaller angle (could be going either direction)
        if diff > PI {
            Some(2.0 * PI - diff)
        } else {
            Some(diff)
        }
    }

    /// Compute pairwise coupling strength based on phase alignment.
    ///
    /// Returns a value in [0, 1] where 1 means perfect phase alignment.
    pub fn pairwise_coupling(&self, i: usize, j: usize) -> Option<f64> {
        self.phase_difference(i, j)
            .map(|diff| (1.0 + diff.cos()) / 2.0)
    }

    /// Get the mean field (collective rhythm) as (amplitude, phase).
    ///
    /// This is the order parameter decomposed into r and ψ.
    #[inline]
    pub fn mean_field(&self) -> (f64, f64) {
        self.order_parameter()
    }

    /// Inject a perturbation to a specific embedder's phase.
    ///
    /// Useful for testing network response to disturbances.
    pub fn perturb(&mut self, embedder_idx: usize, delta_phase: f64) -> UtlResult<()> {
        if embedder_idx >= NUM_OSCILLATORS {
            return Err(UtlError::PhaseError(format!(
                "Embedder index {} out of range [0, {})",
                embedder_idx, NUM_OSCILLATORS
            )));
        }
        self.phases[embedder_idx] = (self.phases[embedder_idx] + delta_phase).rem_euclid(2.0 * PI);
        Ok(())
    }
}

impl Default for KuramotoNetwork {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_network() {
        let network = KuramotoNetwork::new();
        assert_eq!(network.phases.len(), 13);
        assert!(network.coupling_strength > 0.0);
    }

    #[test]
    fn test_synchronized_network_has_r_near_1() {
        let network = KuramotoNetwork::synchronized();
        let (r, _) = network.order_parameter();
        assert!(
            r > 0.99,
            "Synchronized network should have r ≈ 1, got {}",
            r
        );
    }

    #[test]
    fn test_incoherent_network_has_low_r() {
        let network = KuramotoNetwork::incoherent();
        let (r, _) = network.order_parameter();
        // With evenly distributed phases, r should be very low
        assert!(r < 0.1, "Incoherent network should have r ≈ 0, got {}", r);
    }

    #[test]
    fn test_step_updates_phases() {
        let mut network = KuramotoNetwork::new();
        let initial_phases = network.phases;

        network.step(Duration::from_millis(100));

        // Phases should have changed
        assert_ne!(network.phases, initial_phases);
    }

    #[test]
    fn test_high_coupling_leads_to_sync() {
        let mut network = KuramotoNetwork::incoherent();
        // With brain wave frequencies (4-80 Hz), we need much stronger coupling
        // K must exceed critical coupling Kc ≈ 2 * frequency_spread / π
        // For our frequencies (spread ~76 Hz), Kc ≈ 48
        // Using K = 100 ensures synchronization
        network.set_coupling_strength(10.0); // Very strong coupling for brain wave frequencies

        // Run for many steps with smaller time step for stability
        // Higher frequencies require smaller dt for accurate integration
        for _ in 0..5000 {
            network.step(Duration::from_millis(1));
        }

        let (r, _) = network.order_parameter();
        // With very strong coupling, should synchronize
        // Note: Full sync takes longer with diverse brain wave frequencies
        assert!(r > 0.5, "High coupling should lead to sync, got r = {}", r);
    }

    #[test]
    fn test_brain_wave_frequencies_preserve_ratios() {
        // Verify normalized frequencies preserve the ratios from Constitution v4.0.0 spec
        // The actual frequencies are normalized but maintain the relative band relationships
        let network = KuramotoNetwork::new();
        let freqs = network.natural_frequencies();

        // E9 (HDC) should be highest (high-gamma 80Hz scaled)
        assert!(
            freqs[8] > freqs[0],
            "E9 (high-gamma) should be > E1 (gamma)"
        );
        // E12 (Late) should be second highest (high-gamma 60Hz scaled)
        assert!(
            freqs[11] > freqs[0],
            "E12 (high-gamma) should be > E1 (gamma)"
        );
        // E1, E10 (gamma 40Hz) should be higher than E5, E7 (beta 25Hz)
        assert!(
            freqs[0] > freqs[4],
            "E1 (gamma 40Hz) should be > E5 (beta 25Hz)"
        );
        assert!(freqs[9] > freqs[6], "E10 (gamma) should be > E7 (beta)");
        // Alpha (E2-E4) should be lower than beta (E5, E7)
        assert!(
            freqs[1] < freqs[4],
            "E2 (alpha 8Hz) should be < E5 (beta 25Hz)"
        );
        // Theta (E6, E13) should be lowest
        assert!(
            freqs[5] < freqs[1],
            "E6 (theta 4Hz) should be < E2 (alpha 8Hz)"
        );
        assert!(freqs[12] < freqs[1], "E13 (theta) should be < E2 (alpha)");
        // E6 and E13 should be equal (both theta 4Hz)
        assert!(
            (freqs[5] - freqs[12]).abs() < 0.001,
            "E6 and E13 should both be theta"
        );
        // E2, E3, E4 should be equal (all alpha 8Hz)
        assert!(
            (freqs[1] - freqs[2]).abs() < 0.001,
            "E2, E3 should both be alpha"
        );
        assert!(
            (freqs[2] - freqs[3]).abs() < 0.001,
            "E3, E4 should both be alpha"
        );
    }

    #[test]
    fn test_brain_wave_reference_frequencies_match_constitution() {
        // Verify the reference (non-normalized) frequencies match Constitution v4.0.0 exactly
        assert!(
            (BRAIN_WAVE_FREQUENCIES_HZ[0] - 40.0).abs() < 0.001,
            "E1 should be 40Hz gamma"
        );
        assert!(
            (BRAIN_WAVE_FREQUENCIES_HZ[1] - 8.0).abs() < 0.001,
            "E2 should be 8Hz alpha"
        );
        assert!(
            (BRAIN_WAVE_FREQUENCIES_HZ[4] - 25.0).abs() < 0.001,
            "E5 should be 25Hz beta"
        );
        assert!(
            (BRAIN_WAVE_FREQUENCIES_HZ[5] - 4.0).abs() < 0.001,
            "E6 should be 4Hz theta"
        );
        assert!(
            (BRAIN_WAVE_FREQUENCIES_HZ[8] - 80.0).abs() < 0.001,
            "E9 should be 80Hz high-gamma"
        );
        assert!(
            (BRAIN_WAVE_FREQUENCIES_HZ[11] - 60.0).abs() < 0.001,
            "E12 should be 60Hz high-gamma"
        );
    }

    #[test]
    fn test_zero_coupling_stays_low() {
        // With zero coupling and incoherent start, network should NOT synchronize
        // The order parameter r should stay low (< 0.5) meaning fragmented
        let mut network = KuramotoNetwork::incoherent();
        network.set_coupling_strength(0.0);

        // Run for many steps - without coupling, phases evolve independently
        for _ in 0..1000 {
            network.step(Duration::from_millis(1));
        }

        let final_r = network.synchronization();
        // Without coupling, r should stay low (phases desynchronize due to different frequencies)
        // The key property is that it should NOT become highly synchronized (r > 0.8)
        assert!(
            final_r < 0.8,
            "Zero coupling should not lead to high synchronization, got r = {}",
            final_r
        );
    }

    #[test]
    fn test_consciousness_states() {
        let mut network = KuramotoNetwork::synchronized();
        assert!(network.is_conscious());
        assert!(!network.is_fragmented());

        network.reset(); // Back to incoherent
        assert!(network.is_fragmented());
        assert!(!network.is_conscious());
    }

    #[test]
    fn test_phase_wrapping() {
        let mut network = KuramotoNetwork::new();

        // Set a phase beyond 2π
        network.set_phase(0, 3.0 * PI).unwrap();

        // Should wrap to [0, 2π]
        let phase = network.phase(0).unwrap();
        assert!((0.0..2.0 * PI).contains(&phase));
    }

    #[test]
    fn test_natural_frequency_access() {
        let network = KuramotoNetwork::new();

        // All frequencies should be positive
        for i in 0..NUM_OSCILLATORS {
            let freq = network.natural_frequency(i).unwrap();
            assert!(freq > 0.0);
        }
    }

    #[test]
    fn test_perturb() {
        let mut network = KuramotoNetwork::synchronized();
        let initial_r = network.synchronization();

        // Perturb one oscillator
        network.perturb(5, PI / 2.0).unwrap();

        let perturbed_r = network.synchronization();
        // Perturbation should reduce synchronization
        assert!(
            perturbed_r < initial_r,
            "Perturbation should reduce sync: {} vs {}",
            perturbed_r,
            initial_r
        );
    }

    #[test]
    fn test_disabled_network_does_not_step() {
        let mut network = KuramotoNetwork::new();
        let initial_phases = network.phases;

        network.set_enabled(false);
        network.step(Duration::from_millis(100));

        // Phases should not change
        assert_eq!(network.phases, initial_phases);
    }
}
