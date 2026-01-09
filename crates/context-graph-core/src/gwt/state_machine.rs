//! Consciousness State Machine
//!
//! Implements state transitions for consciousness levels as specified in
//! Constitution v4.0.0 Section gwt.state_machine (lines 394-408).
//!
//! ## States
//!
//! - **DORMANT**: r < 0.3, no active workspace
//! - **FRAGMENTED**: 0.3 ≤ r < 0.5, partial synchronization
//! - **EMERGING**: 0.5 ≤ r < 0.8, approaching consciousness
//! - **CONSCIOUS**: r ≥ 0.8, unified perception
//! - **HYPERSYNC**: r > 0.95, pathological overdrive (warning state)

use chrono::{DateTime, Utc, Duration};
use crate::error::CoreResult;

/// Consciousness state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsciousnessState {
    Dormant,
    Fragmented,
    Emerging,
    Conscious,
    Hypersync,
}

impl ConsciousnessState {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Dormant => "DORMANT",
            Self::Fragmented => "FRAGMENTED",
            Self::Emerging => "EMERGING",
            Self::Conscious => "CONSCIOUS",
            Self::Hypersync => "HYPERSYNC",
        }
    }

    /// Determine state from consciousness level
    pub fn from_level(level: f32) -> Self {
        match level {
            l if l > 0.95 => Self::Hypersync,
            l if l >= 0.8 => Self::Conscious,
            l if l >= 0.5 => Self::Emerging,
            l if l >= 0.3 => Self::Fragmented,
            _ => Self::Dormant,
        }
    }
}

/// State transition with timestamp and context
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from: ConsciousnessState,
    pub to: ConsciousnessState,
    pub timestamp: DateTime<Utc>,
    pub consciousness_level: f32,
}

/// Manages consciousness state transitions and events
#[derive(Debug)]
pub struct StateMachineManager {
    /// Current state
    current_state: ConsciousnessState,
    /// Last state transition
    last_transition: Option<StateTransition>,
    /// Time entered current state
    entered_state_at: DateTime<Utc>,
    /// Inactivity timeout (dormant after this duration)
    inactivity_timeout_secs: u64,
    /// Last activity timestamp
    last_activity: DateTime<Utc>,
}

impl StateMachineManager {
    /// Create a new state machine manager
    pub fn new() -> Self {
        Self {
            current_state: ConsciousnessState::Dormant,
            last_transition: None,
            entered_state_at: Utc::now(),
            inactivity_timeout_secs: 600, // 10 minutes
            last_activity: Utc::now(),
        }
    }

    /// Get current state
    pub fn current_state(&self) -> ConsciousnessState {
        self.current_state
    }

    /// Get time spent in current state
    pub fn time_in_state(&self) -> Duration {
        Utc::now() - self.entered_state_at
    }

    /// Update state based on consciousness level
    pub async fn update(&mut self, consciousness_level: f32) -> CoreResult<ConsciousnessState> {
        // Update activity timestamp
        self.last_activity = Utc::now();

        // Determine new state
        let new_state = ConsciousnessState::from_level(consciousness_level);

        // Check for inactivity-driven transition to dormant
        if self.time_in_state().num_seconds() > self.inactivity_timeout_secs as i64
            && self.current_state != ConsciousnessState::Dormant
        {
            self.transition_to(
                ConsciousnessState::Dormant,
                consciousness_level,
                "inactivity_timeout",
            )
            .await?;
            return Ok(self.current_state);
        }

        // Check if state changed
        if new_state != self.current_state {
            self.transition_to(new_state, consciousness_level, "consciousness_level_change")
                .await?;
        }

        Ok(self.current_state)
    }

    /// Execute transition with logging
    async fn transition_to(
        &mut self,
        new_state: ConsciousnessState,
        consciousness_level: f32,
        reason: &str,
    ) -> CoreResult<()> {
        let old_state = self.current_state;
        self.current_state = new_state;
        self.entered_state_at = Utc::now();

        let transition = StateTransition {
            from: old_state,
            to: new_state,
            timestamp: Utc::now(),
            consciousness_level,
        };

        self.last_transition = Some(transition.clone());

        // Log transition
        tracing::info!(
            "State transition: {} → {} (level={:.3}, reason={})",
            old_state.name(),
            new_state.name(),
            consciousness_level,
            reason
        );

        Ok(())
    }

    /// Get the last transition
    pub fn last_transition(&self) -> Option<&StateTransition> {
        self.last_transition.as_ref()
    }

    /// Check if system just became conscious
    pub fn just_became_conscious(&self) -> bool {
        if let Some(trans) = &self.last_transition {
            trans.to == ConsciousnessState::Conscious
                && (Utc::now() - trans.timestamp).num_milliseconds() < 1000
        } else {
            false
        }
    }

    /// Check if system is in a conscious state
    pub fn is_conscious(&self) -> bool {
        matches!(
            self.current_state,
            ConsciousnessState::Conscious | ConsciousnessState::Hypersync
        )
    }

    /// Check if system is hypersynchronized (warning state)
    pub fn is_hypersync(&self) -> bool {
        self.current_state == ConsciousnessState::Hypersync
    }

    /// Check if coherence is fragmented
    pub fn is_fragmented(&self) -> bool {
        self.current_state == ConsciousnessState::Fragmented
    }

    /// Check if system is dormant
    pub fn is_dormant(&self) -> bool {
        self.current_state == ConsciousnessState::Dormant
    }

    /// Set inactivity timeout
    pub fn set_inactivity_timeout(&mut self, seconds: u64) {
        self.inactivity_timeout_secs = seconds;
    }
}

impl Default for StateMachineManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Detailed transition analysis
#[derive(Debug, Clone)]
pub struct TransitionAnalysis {
    pub from_state: ConsciousnessState,
    pub to_state: ConsciousnessState,
    pub consciousness_delta: f32,
    pub coherence_was_increasing: bool,
    pub is_recovery: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consciousness_state_from_level() {
        assert_eq!(ConsciousnessState::from_level(0.1), ConsciousnessState::Dormant);
        assert_eq!(
            ConsciousnessState::from_level(0.4),
            ConsciousnessState::Fragmented
        );
        assert_eq!(
            ConsciousnessState::from_level(0.65),
            ConsciousnessState::Emerging
        );
        assert_eq!(
            ConsciousnessState::from_level(0.85),
            ConsciousnessState::Conscious
        );
        assert_eq!(
            ConsciousnessState::from_level(0.97),
            ConsciousnessState::Hypersync
        );
    }

    #[test]
    fn test_consciousness_state_name() {
        assert_eq!(ConsciousnessState::Dormant.name(), "DORMANT");
        assert_eq!(ConsciousnessState::Conscious.name(), "CONSCIOUS");
        assert_eq!(ConsciousnessState::Hypersync.name(), "HYPERSYNC");
    }

    #[tokio::test]
    async fn test_state_machine_dormant_to_fragmented() {
        let mut sm = StateMachineManager::new();
        assert_eq!(sm.current_state(), ConsciousnessState::Dormant);

        sm.update(0.4).await.unwrap();
        assert_eq!(sm.current_state(), ConsciousnessState::Fragmented);
    }

    #[tokio::test]
    async fn test_state_machine_progression() {
        let mut sm = StateMachineManager::new();

        // Dormant → Fragmented
        sm.update(0.4).await.unwrap();
        assert_eq!(sm.current_state(), ConsciousnessState::Fragmented);

        // Fragmented → Emerging
        sm.update(0.65).await.unwrap();
        assert_eq!(sm.current_state(), ConsciousnessState::Emerging);

        // Emerging → Conscious
        sm.update(0.85).await.unwrap();
        assert_eq!(sm.current_state(), ConsciousnessState::Conscious);

        // Conscious → Hypersync
        sm.update(0.97).await.unwrap();
        assert_eq!(sm.current_state(), ConsciousnessState::Hypersync);
    }

    #[tokio::test]
    async fn test_state_machine_regression() {
        let mut sm = StateMachineManager::new();

        // Start conscious
        sm.update(0.85).await.unwrap();
        assert!(sm.is_conscious());

        // Drop to fragmented
        sm.update(0.4).await.unwrap();
        assert!(sm.is_fragmented());
        assert!(!sm.is_conscious());
    }

    #[tokio::test]
    async fn test_state_machine_just_became_conscious() {
        let mut sm = StateMachineManager::new();

        sm.update(0.85).await.unwrap();
        assert!(sm.just_became_conscious());

        // After 2 seconds, should not be "just" (threshold is 1000ms)
        // Use 2000ms to ensure we're well past the threshold and avoid timing flakiness
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        assert!(!sm.just_became_conscious());
    }

    #[tokio::test]
    async fn test_state_machine_hypersync_detection() {
        let mut sm = StateMachineManager::new();

        sm.update(0.97).await.unwrap();
        assert!(sm.is_hypersync());
        assert!(sm.is_conscious()); // Hypersync is a form of consciousness
    }

    #[test]
    fn test_state_machine_time_in_state() {
        let sm = StateMachineManager::new();
        let time = sm.time_in_state();

        assert!(time.num_seconds() >= 0);
    }

    #[tokio::test]
    async fn test_state_machine_last_transition() {
        let mut sm = StateMachineManager::new();

        sm.update(0.85).await.unwrap();
        let trans = sm.last_transition();

        assert!(trans.is_some());
        let t = trans.unwrap();
        assert_eq!(t.from, ConsciousnessState::Dormant);
        assert_eq!(t.to, ConsciousnessState::Conscious);
    }
}
