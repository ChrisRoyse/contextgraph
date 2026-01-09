//! NeuromodulationState - Central manager for all neuromodulators
//!
//! This module provides:
//! 1. `NeuromodulationState`: Complete state snapshot of all modulators
//! 2. `NeuromodulationManager`: Coordinates all neuromodulators
//! 3. `ModulatorType`: Enum for identifying modulators
//!
//! ## Architecture
//!
//! The manager owns DA, 5HT, and NE modulators. ACh is read-only, as it is
//! managed by the GWT MetaCognitiveLoop. The manager can:
//! - Get state snapshots for all modulators
//! - Apply homeostatic decay to all modulators
//! - Process trigger events
//! - Adjust individual modulators

use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use super::dopamine::{DopamineLevel, DopamineModulator, DA_BASELINE, DA_MAX, DA_MIN};
use super::noradrenaline::{
    NoradrenalineLevel, NoradrenalineModulator, NE_BASELINE, NE_MAX, NE_MIN,
};
use super::serotonin::{
    SerotoninLevel, SerotoninModulator, NUM_EMBEDDING_SPACES, SEROTONIN_BASELINE, SEROTONIN_MAX,
    SEROTONIN_MIN,
};

/// Complete neuromodulation state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuromodulationState {
    /// Dopamine level and state
    pub dopamine: DopamineLevel,
    /// Serotonin level and state
    pub serotonin: SerotoninLevel,
    /// Noradrenaline level and state
    pub noradrenaline: NoradrenalineLevel,
    /// Acetylcholine level (read from GWT MetaCognitiveLoop)
    pub acetylcholine: f32,
    /// Timestamp of this state snapshot
    #[serde(with = "instant_serde")]
    pub timestamp: Instant,
}

// Custom serde for Instant (convert to/from duration since UNIX_EPOCH)
mod instant_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Instant;

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as elapsed duration in nanoseconds from some reference point
        let elapsed = instant.elapsed().as_nanos() as u64;
        elapsed.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as now - duration (approximation)
        let _nanos = u64::deserialize(deserializer)?;
        Ok(Instant::now())
    }
}

impl NeuromodulationState {
    /// Get Hopfield beta value (from dopamine)
    pub fn hopfield_beta(&self) -> f32 {
        self.dopamine.value
    }

    /// Get attention temperature (from noradrenaline)
    pub fn attention_temp(&self) -> f32 {
        self.noradrenaline.value
    }

    /// Get UTL learning rate (from acetylcholine)
    pub fn utl_learning_rate(&self) -> f32 {
        self.acetylcholine
    }

    /// Get serotonin-scaled weight for embedding space
    pub fn get_space_weight(&self, space_index: usize) -> f32 {
        if space_index >= NUM_EMBEDDING_SPACES {
            return 0.0;
        }
        let base_weight = self.serotonin.space_weights[space_index];
        let scaling = 0.5 + 0.5 * self.serotonin.value;
        base_weight * scaling
    }

    /// Check if system is in high alert state
    pub fn is_alert(&self) -> bool {
        self.noradrenaline.value > NE_BASELINE + 0.2
    }

    /// Check if system is in elevated learning state
    pub fn is_learning_elevated(&self) -> bool {
        self.acetylcholine > 0.001 + (0.002 - 0.001) / 2.0
    }
}

/// Modulator type enum for identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModulatorType {
    /// Dopamine - controls hopfield.beta
    Dopamine,
    /// Serotonin - controls space_weights
    Serotonin,
    /// Noradrenaline - controls attention.temp
    Noradrenaline,
    /// Acetylcholine - controls utl.lr (read-only, managed by GWT)
    Acetylcholine,
}

impl std::fmt::Display for ModulatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModulatorType::Dopamine => write!(f, "Dopamine (DA)"),
            ModulatorType::Serotonin => write!(f, "Serotonin (5HT)"),
            ModulatorType::Noradrenaline => write!(f, "Noradrenaline (NE)"),
            ModulatorType::Acetylcholine => write!(f, "Acetylcholine (ACh)"),
        }
    }
}

/// Manager for all neuromodulators
#[derive(Debug)]
pub struct NeuromodulationManager {
    dopamine: DopamineModulator,
    serotonin: SerotoninModulator,
    noradrenaline: NoradrenalineModulator,
    last_update: Instant,
}

impl NeuromodulationManager {
    /// Create a new neuromodulation manager with all modulators at baseline
    pub fn new() -> Self {
        Self {
            dopamine: DopamineModulator::new(),
            serotonin: SerotoninModulator::new(),
            noradrenaline: NoradrenalineModulator::new(),
            last_update: Instant::now(),
        }
    }

    /// Get current state snapshot
    ///
    /// # Arguments
    /// - `ach_from_gwt`: Acetylcholine value read from GWT MetaCognitiveLoop
    pub fn get_state(&self, ach_from_gwt: f32) -> NeuromodulationState {
        NeuromodulationState {
            dopamine: self.dopamine.level().clone(),
            serotonin: self.serotonin.level().clone(),
            noradrenaline: self.noradrenaline.level().clone(),
            acetylcholine: ach_from_gwt,
            timestamp: Instant::now(),
        }
    }

    /// Apply homeostatic decay to all modulators
    pub fn decay_all(&mut self, delta_t: Duration) {
        self.dopamine.decay(delta_t);
        self.serotonin.decay(delta_t);
        self.noradrenaline.decay(delta_t);
        self.last_update = Instant::now();
    }

    /// Update with automatic delta_t calculation
    pub fn update(&mut self) {
        let delta_t = self.last_update.elapsed();
        self.decay_all(delta_t);
    }

    /// Adjust a specific modulator by delta
    ///
    /// # Arguments
    /// - `modulator`: Which modulator to adjust
    /// - `delta`: Amount to adjust (positive or negative)
    ///
    /// # Returns
    /// New value after adjustment
    ///
    /// # Errors
    /// - `CoreError::FeatureDisabled` if trying to adjust ACh (read-only)
    pub fn adjust(&mut self, modulator: ModulatorType, delta: f32) -> CoreResult<f32> {
        match modulator {
            ModulatorType::Dopamine => {
                let new_value = (self.dopamine.value() + delta).clamp(DA_MIN, DA_MAX);
                self.dopamine.set_value(new_value);
                Ok(new_value)
            }
            ModulatorType::Serotonin => {
                self.serotonin.adjust(delta);
                Ok(self.serotonin.value())
            }
            ModulatorType::Noradrenaline => {
                let new_value = (self.noradrenaline.value() + delta).clamp(NE_MIN, NE_MAX);
                self.noradrenaline.set_value(new_value);
                Ok(new_value)
            }
            ModulatorType::Acetylcholine => Err(CoreError::FeatureDisabled {
                feature:
                    "Direct ACh adjustment is disabled. ACh is managed by GWT MetaCognitiveLoop."
                        .to_string(),
            }),
        }
    }

    /// Set a specific modulator to exact value
    pub fn set(&mut self, modulator: ModulatorType, value: f32) -> CoreResult<f32> {
        match modulator {
            ModulatorType::Dopamine => {
                self.dopamine.set_value(value);
                Ok(self.dopamine.value())
            }
            ModulatorType::Serotonin => {
                self.serotonin.set_value(value);
                Ok(self.serotonin.value())
            }
            ModulatorType::Noradrenaline => {
                self.noradrenaline.set_value(value);
                Ok(self.noradrenaline.value())
            }
            ModulatorType::Acetylcholine => Err(CoreError::FeatureDisabled {
                feature: "Direct ACh setting is disabled. ACh is managed by GWT MetaCognitiveLoop."
                    .to_string(),
            }),
        }
    }

    /// Get current value of a modulator
    pub fn get(&self, modulator: ModulatorType) -> Option<f32> {
        match modulator {
            ModulatorType::Dopamine => Some(self.dopamine.value()),
            ModulatorType::Serotonin => Some(self.serotonin.value()),
            ModulatorType::Noradrenaline => Some(self.noradrenaline.value()),
            ModulatorType::Acetylcholine => None, // Must be read from GWT
        }
    }

    /// Get modulator range (min, baseline, max)
    pub fn get_range(modulator: ModulatorType) -> (f32, f32, f32) {
        match modulator {
            ModulatorType::Dopamine => (DA_MIN, DA_BASELINE, DA_MAX),
            ModulatorType::Serotonin => (SEROTONIN_MIN, SEROTONIN_BASELINE, SEROTONIN_MAX),
            ModulatorType::Noradrenaline => (NE_MIN, NE_BASELINE, NE_MAX),
            ModulatorType::Acetylcholine => (0.001, 0.001, 0.002),
        }
    }

    // Event handlers

    /// Handle workspace entry event (triggers dopamine)
    pub fn on_workspace_entry(&mut self) {
        self.dopamine.on_workspace_entry();
    }

    /// Handle threat detection event (triggers noradrenaline)
    pub fn on_threat_detected(&mut self) {
        self.noradrenaline.on_threat_detected();
    }

    /// Handle threat detection with severity
    pub fn on_threat_detected_with_severity(&mut self, severity: f32) {
        self.noradrenaline
            .on_threat_detected_with_severity(severity);
    }

    /// Handle safety confirmation (calms noradrenaline)
    pub fn on_safety_confirmed(&mut self) {
        self.noradrenaline.on_safety_confirmed();
    }

    /// Handle positive mood event (increases serotonin)
    pub fn on_positive_event(&mut self, magnitude: f32) {
        self.serotonin.on_positive_event(magnitude);
    }

    /// Handle negative mood event (decreases serotonin)
    pub fn on_negative_event(&mut self, magnitude: f32) {
        self.serotonin.on_negative_event(magnitude);
    }

    // Parameter accessors

    /// Get Hopfield beta (dopamine)
    pub fn get_hopfield_beta(&self) -> f32 {
        self.dopamine.get_hopfield_beta()
    }

    /// Get attention temperature (noradrenaline)
    pub fn get_attention_temp(&self) -> f32 {
        self.noradrenaline.get_attention_temp()
    }

    /// Get space weight for embedding index (serotonin)
    pub fn get_space_weight(&self, space_index: usize) -> f32 {
        self.serotonin.get_space_weight(space_index)
    }

    /// Get all space weights (serotonin)
    pub fn get_all_space_weights(&self) -> [f32; NUM_EMBEDDING_SPACES] {
        self.serotonin.get_all_space_weights()
    }

    /// Set base weight for embedding space
    pub fn set_space_weight(&mut self, space_index: usize, weight: f32) {
        self.serotonin.set_space_weight(space_index, weight);
    }

    // Alert states

    /// Check if system is alert (high NE)
    pub fn is_alert(&self) -> bool {
        self.noradrenaline.is_alert()
    }

    /// Check if system is in high alert (very high NE)
    pub fn is_high_alert(&self) -> bool {
        self.noradrenaline.is_high_alert()
    }

    /// Reset all modulators to baseline
    pub fn reset_all(&mut self) {
        self.dopamine.reset();
        self.serotonin.reset();
        self.noradrenaline.reset();
        self.last_update = Instant::now();
    }

    /// Get dopamine modulator reference
    pub fn dopamine(&self) -> &DopamineModulator {
        &self.dopamine
    }

    /// Get serotonin modulator reference
    pub fn serotonin(&self) -> &SerotoninModulator {
        &self.serotonin
    }

    /// Get noradrenaline modulator reference
    pub fn noradrenaline(&self) -> &NoradrenalineModulator {
        &self.noradrenaline
    }
}

impl Default for NeuromodulationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = NeuromodulationManager::new();

        assert!((manager.get_hopfield_beta() - DA_BASELINE).abs() < f32::EPSILON);
        assert!((manager.get_attention_temp() - NE_BASELINE).abs() < f32::EPSILON);
        assert!((manager.serotonin.value() - SEROTONIN_BASELINE).abs() < f32::EPSILON);
    }

    #[test]
    fn test_manager_get_state() {
        let manager = NeuromodulationManager::new();
        let state = manager.get_state(0.001);

        assert!((state.hopfield_beta() - DA_BASELINE).abs() < f32::EPSILON);
        assert!((state.attention_temp() - NE_BASELINE).abs() < f32::EPSILON);
        assert!((state.utl_learning_rate() - 0.001).abs() < f32::EPSILON);
    }

    #[test]
    fn test_manager_adjust_dopamine() {
        let mut manager = NeuromodulationManager::new();

        let result = manager.adjust(ModulatorType::Dopamine, 0.5);
        assert!(result.is_ok());
        assert!((result.unwrap() - (DA_BASELINE + 0.5)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_manager_adjust_ach_fails() {
        let mut manager = NeuromodulationManager::new();

        let result = manager.adjust(ModulatorType::Acetylcholine, 0.0005);
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_decay_all() {
        let mut manager = NeuromodulationManager::new();

        // Set all to max
        manager.dopamine.set_value(DA_MAX);
        manager.noradrenaline.set_value(NE_MAX);
        manager.serotonin.set_value(SEROTONIN_MAX);

        // Decay
        for _ in 0..50 {
            manager.decay_all(Duration::from_millis(100));
        }

        // All should be closer to baseline
        assert!(manager.dopamine.value() < DA_MAX);
        assert!(manager.noradrenaline.value() < NE_MAX);
        assert!(manager.serotonin.value() < SEROTONIN_MAX);
    }

    #[test]
    fn test_manager_workspace_entry() {
        let mut manager = NeuromodulationManager::new();
        let initial = manager.get_hopfield_beta();

        manager.on_workspace_entry();

        assert!(manager.get_hopfield_beta() > initial);
    }

    #[test]
    fn test_manager_threat_detection() {
        let mut manager = NeuromodulationManager::new();
        let initial = manager.get_attention_temp();

        manager.on_threat_detected();

        assert!(manager.get_attention_temp() > initial);
        assert!(manager.is_alert());
    }

    #[test]
    fn test_manager_get_range() {
        let (min, baseline, max) = NeuromodulationManager::get_range(ModulatorType::Dopamine);
        assert!((min - DA_MIN).abs() < f32::EPSILON);
        assert!((baseline - DA_BASELINE).abs() < f32::EPSILON);
        assert!((max - DA_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn test_manager_reset_all() {
        let mut manager = NeuromodulationManager::new();

        manager.on_workspace_entry();
        manager.on_threat_detected();
        manager.on_positive_event(1.0);

        manager.reset_all();

        assert!((manager.get_hopfield_beta() - DA_BASELINE).abs() < f32::EPSILON);
        assert!((manager.get_attention_temp() - NE_BASELINE).abs() < f32::EPSILON);
        assert!((manager.serotonin.value() - SEROTONIN_BASELINE).abs() < f32::EPSILON);
    }

    #[test]
    fn test_modulator_type_display() {
        assert_eq!(ModulatorType::Dopamine.to_string(), "Dopamine (DA)");
        assert_eq!(ModulatorType::Serotonin.to_string(), "Serotonin (5HT)");
        assert_eq!(
            ModulatorType::Noradrenaline.to_string(),
            "Noradrenaline (NE)"
        );
        assert_eq!(
            ModulatorType::Acetylcholine.to_string(),
            "Acetylcholine (ACh)"
        );
    }

    #[test]
    fn test_state_space_weights() {
        let manager = NeuromodulationManager::new();
        let state = manager.get_state(0.001);

        // Test all space weights are valid
        for i in 0..NUM_EMBEDDING_SPACES {
            let weight = state.get_space_weight(i);
            assert!((0.0..=1.0).contains(&weight));
        }

        // Invalid index should return 0
        assert!((state.get_space_weight(100) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_state_alert_states() {
        let mut manager = NeuromodulationManager::new();
        let state = manager.get_state(0.001);
        assert!(!state.is_alert());

        manager.on_threat_detected();
        let state = manager.get_state(0.001);
        assert!(state.is_alert());
    }

    #[test]
    fn test_state_learning_elevated() {
        let state_low = NeuromodulationState {
            dopamine: DopamineLevel::default(),
            serotonin: SerotoninLevel::default(),
            noradrenaline: NoradrenalineLevel::default(),
            acetylcholine: 0.001,
            timestamp: Instant::now(),
        };
        assert!(!state_low.is_learning_elevated());

        let state_high = NeuromodulationState {
            dopamine: DopamineLevel::default(),
            serotonin: SerotoninLevel::default(),
            noradrenaline: NoradrenalineLevel::default(),
            acetylcholine: 0.002,
            timestamp: Instant::now(),
        };
        assert!(state_high.is_learning_elevated());
    }
}
