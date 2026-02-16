//! Session state stubs for hooks.
//!
//! This module provides minimal session state management for hooks.
//! The previous GWT-based implementation was removed per constitution/PRD alignment.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Number of embedder spaces (13 per constitution).
pub const NUM_EMBEDDERS: usize = 13;

/// Maximum trajectory size for tracking session evolution.
pub const MAX_TRAJECTORY_SIZE: usize = 100;

/// Global session cache (thread-safe singleton).
static SESSION_CACHE: Mutex<Option<SessionSnapshot>> = Mutex::new(None);

/// Simplified session snapshot for hook coherence tracking.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    /// Session identifier
    pub session_id: String,
    /// Topic profile across embedder spaces
    pub topic_profile: [f32; NUM_EMBEDDERS],
    /// Integration metric [0.0, 1.0]
    pub integration: f32,
    /// Reflection metric [0.0, 1.0]
    pub reflection: f32,
    /// Differentiation metric [0.0, 1.0]
    pub differentiation: f32,
    /// Trajectory of previous topic profiles
    pub trajectory: Vec<[f32; NUM_EMBEDDERS]>,
    /// Link to previous session
    pub previous_session_id: Option<String>,
    /// Timestamp in milliseconds
    pub timestamp_ms: u64,
}

impl SessionSnapshot {
    /// Create a new session snapshot with default values.
    pub fn new(session_id: &str) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            session_id: session_id.to_string(),
            topic_profile: [0.0; NUM_EMBEDDERS],
            integration: 0.5,
            reflection: 0.5,
            differentiation: 0.5,
            trajectory: Vec::new(),
            previous_session_id: None,
            timestamp_ms,
        }
    }

    /// Update timestamp to current time.
    pub fn touch(&mut self) {
        self.timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
    }

    /// Append a topic profile to the trajectory.
    pub fn append_to_trajectory(&mut self, profile: [f32; NUM_EMBEDDERS]) {
        if self.trajectory.len() >= MAX_TRAJECTORY_SIZE {
            self.trajectory.remove(0);
        }
        self.trajectory.push(profile);
    }
}

/// In-memory session cache for hook state management.
pub struct SessionCache;

impl SessionCache {
    /// Get the cached session snapshot (if any).
    pub fn get() -> Option<SessionSnapshot> {
        let guard = match SESSION_CACHE.lock() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("ERROR: Session cache mutex poisoned: {} — session state unavailable", e);
                return None;
            }
        };
        guard.clone()
    }

    /// Check if cache has a snapshot.
    #[allow(dead_code)] // Used in tests across the crate
    pub fn is_warm() -> bool {
        match SESSION_CACHE.lock() {
            Ok(g) => g.is_some(),
            Err(e) => {
                eprintln!("ERROR: Session cache mutex poisoned: {} — cannot check warmth", e);
                false
            }
        }
    }
}

/// Store a snapshot in the global cache.
pub fn store_in_cache(snapshot: &SessionSnapshot) {
    match SESSION_CACHE.lock() {
        Ok(mut guard) => {
            *guard = Some(snapshot.clone());
        }
        Err(e) => {
            eprintln!("ERROR: Session cache mutex poisoned: {} — cannot store snapshot", e);
        }
    }
}

/// Simplified coherence state for session tracking.
#[derive(Debug, Clone, Copy)]
pub enum CoherenceState {
    /// High coherence (>= 0.8)
    Active,
    /// Good coherence (>= 0.5)
    Aware,
    /// Low coherence (>= 0.2)
    DIM,
    /// Very low coherence (< 0.2)
    DOR,
}

impl CoherenceState {
    /// Create from coherence level [0.0, 1.0].
    pub fn from_level(level: f32) -> Self {
        match level {
            l if l >= 0.8 => Self::Active,
            l if l >= 0.5 => Self::Aware,
            l if l >= 0.2 => Self::DIM,
            _ => Self::DOR,
        }
    }

    /// Short name for output.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Aware => "Aware",
            Self::DIM => "DIM",
            Self::DOR => "DOR",
        }
    }
}
