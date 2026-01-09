//! Global Workspace Selection - Winner-Take-All Algorithm
//!
//! Implements conscious memory selection via winner-take-all (WTA) competition
//! as specified in Constitution v4.0.0 Section gwt.global_workspace (lines 352-369).
//!
//! ## Algorithm
//!
//! 1. Compute Kuramoto order parameter r for all candidate memories
//! 2. Filter: candidates where r ≥ coherence_threshold (0.8)
//! 3. Rank: score = r × importance × north_star_alignment
//! 4. Select: top-1 becomes active_memory
//! 5. Broadcast: active_memory visible to all subsystems (100ms window)
//! 6. Inhibit: losing candidates receive dopamine reduction

use crate::error::{CoreError, CoreResult};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

/// A memory candidate competing for workspace entry
#[derive(Debug, Clone)]
pub struct WorkspaceCandidate {
    /// Memory unique identifier
    pub id: Uuid,
    /// Kuramoto order parameter r (coherence measure)
    pub order_parameter: f32,
    /// Memory importance score [0,1]
    pub importance: f32,
    /// North star alignment score [0,1]
    pub alignment: f32,
    /// Computed competition score
    pub score: f32,
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
}

impl WorkspaceCandidate {
    /// Create a new workspace candidate
    pub fn new(
        id: Uuid,
        order_parameter: f32,
        importance: f32,
        alignment: f32,
    ) -> CoreResult<Self> {
        if !(0.0..=1.0).contains(&order_parameter) {
            return Err(CoreError::ValidationError {
                field: "order_parameter".to_string(),
                message: format!("out of [0,1]: {}", order_parameter),
            });
        }
        if !(0.0..=1.0).contains(&importance) {
            return Err(CoreError::ValidationError {
                field: "importance".to_string(),
                message: format!("out of [0,1]: {}", importance),
            });
        }
        if !(0.0..=1.0).contains(&alignment) {
            return Err(CoreError::ValidationError {
                field: "alignment".to_string(),
                message: format!("out of [0,1]: {}", alignment),
            });
        }

        let score = order_parameter * importance * alignment;

        Ok(Self {
            id,
            order_parameter,
            importance,
            alignment,
            score,
            timestamp: Utc::now(),
        })
    }
}

/// Global workspace for consciousness broadcasting
#[derive(Debug)]
pub struct GlobalWorkspace {
    /// Currently active (conscious) memory
    pub active_memory: Option<Uuid>,
    /// Candidates in competition
    pub candidates: Vec<WorkspaceCandidate>,
    /// Coherence threshold for entry (default 0.8)
    pub coherence_threshold: f32,
    /// Broadcast duration in milliseconds
    pub broadcast_duration_ms: u64,
    /// Last broadcast time
    pub last_broadcast: Option<DateTime<Utc>>,
    /// History of previous winners (for dream replay)
    pub winner_history: Vec<(Uuid, DateTime<Utc>, f32)>, // (id, time, score)
}

impl GlobalWorkspace {
    /// Create a new global workspace
    pub fn new() -> Self {
        Self {
            active_memory: None,
            candidates: Vec::new(),
            coherence_threshold: 0.8,
            broadcast_duration_ms: 100,
            last_broadcast: None,
            winner_history: Vec::new(),
        }
    }

    /// Add a candidate memory to the workspace competition
    pub async fn add_candidate(&mut self, candidate: WorkspaceCandidate) -> CoreResult<()> {
        // Check if memory should enter workspace based on coherence
        if candidate.order_parameter >= self.coherence_threshold {
            self.candidates.push(candidate);
        }
        Ok(())
    }

    /// Select winning memory via winner-take-all
    ///
    /// # Algorithm
    /// 1. Filter candidates with r ≥ coherence_threshold
    /// 2. Rank by score = r × importance × alignment
    /// 3. Select top-1
    pub async fn select_winning_memory(
        &mut self,
        candidates: Vec<(Uuid, f32, f32, f32)>, // (id, r, importance, alignment)
    ) -> CoreResult<Option<Uuid>> {
        // Clear previous candidates
        self.candidates.clear();

        // Build candidates
        for (id, r, importance, alignment) in candidates {
            if let Ok(candidate) = WorkspaceCandidate::new(id, r, importance, alignment) {
                if candidate.order_parameter >= self.coherence_threshold {
                    self.candidates.push(candidate);
                }
            }
        }

        // Select winner
        if self.candidates.is_empty() {
            self.active_memory = None;
            return Ok(None);
        }

        // Sort by score (descending)
        self.candidates
            .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        let winner = self.candidates[0].clone();
        let winner_id = winner.id;

        // Update workspace state
        self.active_memory = Some(winner_id);
        self.last_broadcast = Some(Utc::now());

        // Store in history (keep last 100)
        self.winner_history
            .push((winner_id, Utc::now(), winner.score));
        if self.winner_history.len() > 100 {
            self.winner_history.remove(0);
        }

        tracing::debug!(
            "Workspace selected memory: {:?} with score {:.3}",
            winner_id,
            winner.score
        );

        Ok(Some(winner_id))
    }

    /// Check if broadcast window is still active
    pub fn is_broadcasting(&self) -> bool {
        if let Some(last_time) = self.last_broadcast {
            let elapsed = Utc::now() - last_time;
            elapsed < Duration::milliseconds(self.broadcast_duration_ms as i64)
        } else {
            false
        }
    }

    /// Get the currently active memory (if broadcasting)
    pub fn get_active_memory(&self) -> Option<Uuid> {
        if self.is_broadcasting() {
            self.active_memory
        } else {
            None
        }
    }

    /// Get all candidates that passed coherence threshold
    pub fn get_coherent_candidates(&self) -> Vec<&WorkspaceCandidate> {
        self.candidates
            .iter()
            .filter(|c| c.order_parameter >= self.coherence_threshold)
            .collect()
    }

    /// Check for workspace conflict (multiple memories with r > 0.8)
    pub fn has_conflict(&self) -> bool {
        self.candidates
            .iter()
            .filter(|c| c.order_parameter > 0.8)
            .count()
            > 1
    }

    /// Get conflict details if present
    pub fn get_conflict_details(&self) -> Option<Vec<Uuid>> {
        if self.has_conflict() {
            Some(
                self.candidates
                    .iter()
                    .filter(|c| c.order_parameter > 0.8)
                    .map(|c| c.id)
                    .collect(),
            )
        } else {
            None
        }
    }
}

impl Default for GlobalWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

/// Events fired by workspace state changes
#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    /// Memory entered workspace (r crossed 0.8 upward)
    MemoryEnters {
        id: Uuid,
        order_parameter: f32,
        timestamp: DateTime<Utc>,
    },
    /// Memory exited workspace (r dropped below 0.7)
    MemoryExits {
        id: Uuid,
        order_parameter: f32,
        timestamp: DateTime<Utc>,
    },
    /// Multiple memories competing for workspace (conflict)
    WorkspaceConflict {
        memories: Vec<Uuid>,
        timestamp: DateTime<Utc>,
    },
    /// No memory in workspace for extended time
    WorkspaceEmpty {
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
}

pub trait WorkspaceEventListener: Send + Sync {
    fn on_event(&self, event: &WorkspaceEvent);
}

/// Broadcasts workspace events to subsystems
pub struct WorkspaceEventBroadcaster {
    listeners: std::sync::Arc<tokio::sync::RwLock<Vec<Box<dyn WorkspaceEventListener>>>>,
}

impl std::fmt::Debug for WorkspaceEventBroadcaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceEventBroadcaster").finish()
    }
}

impl WorkspaceEventBroadcaster {
    pub fn new() -> Self {
        Self {
            listeners: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    pub async fn broadcast(&self, event: WorkspaceEvent) {
        let listeners = self.listeners.read().await;
        for listener in listeners.iter() {
            listener.on_event(&event);
        }
    }
}

impl Default for WorkspaceEventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_candidate_creation() {
        let id = Uuid::new_v4();
        let candidate = WorkspaceCandidate::new(id, 0.85, 0.9, 0.88).unwrap();

        assert_eq!(candidate.id, id);
        assert_eq!(candidate.order_parameter, 0.85);
        assert!(candidate.score > 0.65);
    }

    #[test]
    fn test_workspace_candidate_invalid_bounds() {
        let id = Uuid::new_v4();

        // Test invalid order parameter
        assert!(WorkspaceCandidate::new(id, 1.5, 0.9, 0.88).is_err());

        // Test invalid importance
        assert!(WorkspaceCandidate::new(id, 0.85, 1.5, 0.88).is_err());

        // Test invalid alignment
        assert!(WorkspaceCandidate::new(id, 0.85, 0.9, 1.5).is_err());
    }

    #[tokio::test]
    async fn test_workspace_selection_wta() {
        let mut workspace = GlobalWorkspace::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let candidates = vec![
            (id1, 0.85, 0.5, 0.8),  // score ≈ 0.34
            (id2, 0.88, 0.9, 0.88), // score ≈ 0.7 (winner)
            (id3, 0.92, 0.6, 0.7),  // score ≈ 0.387
        ];

        let winner = workspace.select_winning_memory(candidates).await.unwrap();
        assert_eq!(winner, Some(id2));
        assert_eq!(workspace.active_memory, Some(id2));
    }

    #[tokio::test]
    async fn test_workspace_selection_filters_low_coherence() {
        let mut workspace = GlobalWorkspace::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let candidates = vec![
            (id1, 0.7, 0.9, 0.88), // Below coherence threshold
            (id2, 0.85, 0.8, 0.8), // Above threshold (winner)
        ];

        let winner = workspace.select_winning_memory(candidates).await.unwrap();
        assert_eq!(winner, Some(id2));
    }

    #[tokio::test]
    async fn test_workspace_no_coherent_candidates() {
        let mut workspace = GlobalWorkspace::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let candidates = vec![(id1, 0.5, 0.9, 0.88), (id2, 0.6, 0.8, 0.8)];

        let winner = workspace.select_winning_memory(candidates).await.unwrap();
        assert_eq!(winner, None);
    }

    #[test]
    fn test_workspace_conflict_detection() {
        let mut workspace = GlobalWorkspace::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        workspace.candidates = vec![
            WorkspaceCandidate::new(id1, 0.85, 0.9, 0.88).unwrap(),
            WorkspaceCandidate::new(id2, 0.82, 0.85, 0.85).unwrap(),
        ];

        assert!(workspace.has_conflict());
        let conflict = workspace.get_conflict_details();
        assert!(conflict.is_some());
        assert_eq!(conflict.unwrap().len(), 2);
    }

    #[test]
    fn test_workspace_broadcast_duration() {
        let workspace = GlobalWorkspace::new();
        assert!(!workspace.is_broadcasting()); // Not yet broadcasting
    }
}
