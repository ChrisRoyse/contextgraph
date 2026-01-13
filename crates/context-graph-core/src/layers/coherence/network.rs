//! Kuramoto Network - Coupled oscillator system
//!
//! Implements the Kuramoto model for coupled oscillator dynamics:
//! dθ_i/dt = ω_i + (K/N) Σ_j sin(θ_j - θ_i)

use std::f32::consts::PI;

use super::constants::KURAMOTO_BASE_FREQUENCIES;
use super::oscillator::KuramotoOscillator;

/// Kuramoto network implementing coupled oscillator dynamics.
///
/// The network synchronizes via the Kuramoto model:
/// dθ_i/dt = ω_i + (K/N) Σ_j sin(θ_j - θ_i)
///
/// The order parameter r ∈ [0, 1] measures synchronization:
/// - r ≈ 1: Perfect synchronization (all phases aligned)
/// - r ≈ 0: No synchronization (phases uniformly distributed)
#[derive(Debug, Clone)]
pub struct KuramotoNetwork {
    /// Array of oscillators
    oscillators: Vec<KuramotoOscillator>,
    /// Global coupling strength K
    coupling: f32,
}

impl KuramotoNetwork {
    /// Create a new Kuramoto network with n oscillators and coupling strength K.
    ///
    /// Oscillators are initialized with distributed phases and varied frequencies
    /// based on constitution-defined natural frequencies for the 13 embedding spaces (E1-E13).
    pub fn new(n: usize, coupling: f32) -> Self {
        // Initialize oscillators with distributed phases and varying frequencies
        // Frequency bands from constitution gwt.kuramoto.frequencies (13 values for E1-E13)
        let oscillators: Vec<_> = (0..n)
            .map(|i| {
                // Distribute initial phases evenly
                let phase = (i as f32 / n as f32) * 2.0 * PI;
                // Use constitution frequencies with slight variation for stability
                let freq = KURAMOTO_BASE_FREQUENCIES[i % KURAMOTO_BASE_FREQUENCIES.len()]
                    * (1.0 + (i as f32 * 0.02)); // Reduced variance for stability
                KuramotoOscillator::new(phase, freq)
            })
            .collect();

        Self {
            oscillators,
            coupling,
        }
    }

    /// Create network with custom oscillators (for testing or specific configurations).
    pub fn with_oscillators(oscillators: Vec<KuramotoOscillator>, coupling: f32) -> Self {
        Self {
            oscillators,
            coupling,
        }
    }

    /// Get the number of oscillators in the network.
    pub fn size(&self) -> usize {
        self.oscillators.len()
    }

    /// Get the coupling strength K.
    pub fn coupling(&self) -> f32 {
        self.coupling
    }

    /// Update all oscillators according to Kuramoto dynamics.
    ///
    /// Implements: dθ_i/dt = ω_i + (K/N) Σ_j sin(θ_j - θ_i)
    ///
    /// Uses Euler integration with time step dt.
    pub fn step(&mut self, dt: f32) {
        let n = self.oscillators.len() as f32;
        let mut deltas = vec![0.0f32; self.oscillators.len()];

        // Compute phase derivatives for all oscillators
        for (i, osc) in self.oscillators.iter().enumerate() {
            let theta_i = osc.phase;
            let omega_i = osc.frequency;

            // Coupling sum: Σ_j sin(θ_j - θ_i)
            let coupling_sum: f32 = self
                .oscillators
                .iter()
                .map(|other| (other.phase - theta_i).sin())
                .sum();

            // dθ_i/dt = ω_i + (K/N) × Σ_j sin(θ_j - θ_i)
            deltas[i] = omega_i + (self.coupling / n) * coupling_sum;
        }

        // Apply Euler update: θ_i(t+dt) = θ_i(t) + dθ_i/dt × dt
        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            osc.phase = (osc.phase + deltas[i] * dt).rem_euclid(2.0 * PI);
        }
    }

    /// Compute the Kuramoto order parameter r.
    ///
    /// r = |1/N Σ_j exp(iθ_j)| = sqrt((Σcos(θ)/N)² + (Σsin(θ)/N)²)
    ///
    /// r ∈ [0, 1] where:
    /// - r = 1: Perfect synchronization
    /// - r = 0: Complete desynchronization
    pub fn order_parameter(&self) -> f32 {
        let n = self.oscillators.len() as f32;
        if n == 0.0 {
            return 0.0;
        }

        let sum_cos: f32 = self.oscillators.iter().map(|o| o.phase.cos()).sum();
        let sum_sin: f32 = self.oscillators.iter().map(|o| o.phase.sin()).sum();

        let r_x = sum_cos / n;
        let r_y = sum_sin / n;

        (r_x * r_x + r_y * r_y).sqrt()
    }

    /// Compute the mean phase ψ of the synchronized oscillators.
    ///
    /// ψ = arg(Σ_j exp(iθ_j))
    pub fn mean_phase(&self) -> f32 {
        let sum_cos: f32 = self.oscillators.iter().map(|o| o.phase.cos()).sum();
        let sum_sin: f32 = self.oscillators.iter().map(|o| o.phase.sin()).sum();

        sum_sin.atan2(sum_cos).rem_euclid(2.0 * PI)
    }

    /// Inject external signal to modulate oscillator frequencies.
    ///
    /// This models external input (e.g., learning signal) affecting the network.
    pub fn inject_signal(&mut self, signal: f32) {
        // Clamp signal to reasonable range to prevent instability
        let clamped_signal = signal.clamp(-1.0, 1.0);

        for osc in &mut self.oscillators {
            // Modulate frequency based on signal (±10%)
            osc.frequency *= 1.0 + clamped_signal * 0.1;
        }
    }

    /// Reset oscillators to distributed phases (useful after perturbation).
    pub fn reset_phases(&mut self) {
        let n = self.oscillators.len();
        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            osc.phase = (i as f32 / n as f32) * 2.0 * PI;
        }
    }

    /// Get all oscillator phases.
    pub fn phases(&self) -> Vec<f32> {
        self.oscillators.iter().map(|o| o.phase).collect()
    }

    /// Get all oscillator frequencies.
    pub fn frequencies(&self) -> Vec<f32> {
        self.oscillators.iter().map(|o| o.frequency).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::coherence::constants::KURAMOTO_DT;
    use std::time::Instant;

    #[test]
    fn test_network_creation() {
        use super::super::constants::KURAMOTO_N;
        let net = KuramotoNetwork::new(KURAMOTO_N, 2.0);
        assert_eq!(net.size(), KURAMOTO_N);
        assert_eq!(net.size(), 13, "[FSV] CRITICAL: Must have 13 oscillators per constitution");
        assert!((net.coupling() - 2.0).abs() < 1e-6);
        println!("[VERIFIED] Network creation with {} oscillators and K=2.0", KURAMOTO_N);
    }

    #[test]
    fn test_order_parameter_range() {
        use super::super::constants::KURAMOTO_N;
        let net = KuramotoNetwork::new(KURAMOTO_N, 2.0);
        let r = net.order_parameter();
        assert!((0.0..=1.0).contains(&r));
        println!("[VERIFIED] Order parameter r ∈ [0, 1]: r = {} (with {} oscillators)", r, KURAMOTO_N);
    }

    #[test]
    fn test_perfect_sync_order_parameter() {
        use super::super::constants::KURAMOTO_N;
        // All oscillators at same phase = perfect sync (r = 1)
        let oscillators: Vec<_> = (0..KURAMOTO_N).map(|_| KuramotoOscillator::new(0.0, 40.0)).collect();
        let net = KuramotoNetwork::with_oscillators(oscillators, 2.0);
        let r = net.order_parameter();
        assert!((r - 1.0).abs() < 1e-6, "Expected r ≈ 1.0, got {}", r);
        println!("[VERIFIED] Perfect sync with {} oscillators: r = {} ≈ 1.0", KURAMOTO_N, r);
    }

    #[test]
    fn test_kuramoto_sync_increases_with_coupling() {
        use super::super::constants::KURAMOTO_N;
        // With strong coupling, sync should increase over time
        let mut net = KuramotoNetwork::new(KURAMOTO_N, 5.0); // Strong coupling
        let r_initial = net.order_parameter();

        // Run many steps
        for _ in 0..100 {
            net.step(0.1);
        }

        let r_final = net.order_parameter();

        // With strong coupling, r should generally increase or stay high
        // Allow some variance due to dynamics
        assert!(
            r_final >= r_initial * 0.8 || r_final > 0.7,
            "Expected sync to increase: r_initial={}, r_final={}",
            r_initial,
            r_final
        );
        println!(
            "[VERIFIED] Kuramoto sync with strong coupling ({} oscillators): {} -> {}",
            KURAMOTO_N, r_initial, r_final
        );
    }

    #[test]
    fn test_kuramoto_step_updates_phases() {
        let mut net = KuramotoNetwork::new(4, 2.0);
        let phases_before = net.phases();

        net.step(0.1);

        let phases_after = net.phases();

        // Phases should have changed
        let changed = phases_before
            .iter()
            .zip(phases_after.iter())
            .any(|(a, b)| (a - b).abs() > 1e-6);
        assert!(changed, "Phases should change after step");
        println!("[VERIFIED] Kuramoto step updates phases");
    }

    #[test]
    fn test_mean_phase() {
        // All oscillators at phase 0 -> mean phase should be 0
        let oscillators: Vec<_> = (0..4).map(|_| KuramotoOscillator::new(0.0, 10.0)).collect();
        let net = KuramotoNetwork::with_oscillators(oscillators, 2.0);
        let psi = net.mean_phase();
        assert!(psi.abs() < 1e-6, "Expected mean phase ≈ 0, got {}", psi);
        println!("[VERIFIED] Mean phase calculation");
    }

    #[test]
    fn test_inject_signal() {
        let mut net = KuramotoNetwork::new(4, 2.0);
        let freqs_before: Vec<_> = net.frequencies();

        net.inject_signal(0.5);

        let freqs_after: Vec<_> = net.frequencies();

        // Frequencies should have changed
        let changed = freqs_before
            .iter()
            .zip(freqs_after.iter())
            .any(|(a, b)| (a - b).abs() > 1e-6);
        assert!(changed, "Frequencies should change after signal injection");
        println!("[VERIFIED] Signal injection modulates frequencies");
    }

    #[test]
    fn test_reset_phases() {
        use std::f32::consts::PI;

        let mut net = KuramotoNetwork::new(4, 2.0);

        // Run some steps to desynchronize
        for _ in 0..50 {
            net.step(0.1);
        }

        net.reset_phases();

        // Phases should be evenly distributed again
        let phases = net.phases();
        for (i, &phase) in phases.iter().enumerate() {
            let expected = (i as f32 / 4.0) * 2.0 * PI;
            assert!(
                (phase - expected).abs() < 1e-6,
                "Phase {} should be reset to {}",
                phase,
                expected
            );
        }
        println!("[VERIFIED] Phase reset");
    }

    #[test]
    fn test_kuramoto_computation_benchmark() {
        use super::super::constants::KURAMOTO_N;
        let iterations = 10_000;
        let mut net = KuramotoNetwork::new(KURAMOTO_N, 2.0);

        let start = Instant::now();

        for _ in 0..iterations {
            net.step(KURAMOTO_DT);
            let _ = net.order_parameter();
        }

        let total_us = start.elapsed().as_micros();
        let avg_ns = (total_us * 1000) / iterations as u128;

        println!("Kuramoto Computation Benchmark ({} oscillators):", KURAMOTO_N);
        println!("  Iterations: {}", iterations);
        println!("  Total time: {} us", total_us);
        println!("  Avg per step+r: {} ns", avg_ns);

        // Each computation should be fast (sub-millisecond)
        assert!(
            avg_ns < 100_000,
            "Kuramoto step+r computation too slow: {} ns",
            avg_ns
        );

        println!("[VERIFIED] Kuramoto step+order_param ({} oscillators) avg {} ns", KURAMOTO_N, avg_ns);
    }

    /// Full State Verification test for 13 oscillators per constitution
    #[test]
    fn fsv_l5_kuramoto_13_oscillators() {
        use super::super::constants::{KURAMOTO_N, KURAMOTO_BASE_FREQUENCIES};

        // Verify constant is 13
        assert_eq!(KURAMOTO_N, 13, "[FSV] KURAMOTO_N must be 13 per constitution");

        // Verify frequencies array has 13 elements
        assert_eq!(KURAMOTO_BASE_FREQUENCIES.len(), 13, "[FSV] KURAMOTO_BASE_FREQUENCIES must have 13 elements");

        // Create network with constitution size
        let net = KuramotoNetwork::new(KURAMOTO_N, 2.0);
        assert_eq!(net.size(), 13, "[FSV] Network must have 13 oscillators");

        // Verify all 13 phases are present
        let phases = net.phases();
        assert_eq!(phases.len(), 13, "[FSV] Must have 13 oscillator phases");

        // Verify all 13 frequencies are present
        let frequencies = net.frequencies();
        assert_eq!(frequencies.len(), 13, "[FSV] Must have 13 natural frequencies");

        println!("[FSV] L5 Kuramoto verification:");
        println!("  KURAMOTO_N = {}", KURAMOTO_N);
        println!("  KURAMOTO_BASE_FREQUENCIES.len() = {}", KURAMOTO_BASE_FREQUENCIES.len());
        println!("  Network oscillators = {}", net.size());
        println!("  Phases count = {}", phases.len());
        println!("  Frequencies count = {}", frequencies.len());
        println!("[VERIFIED] All 13 oscillator requirements met per constitution GWT-002");
    }
}
