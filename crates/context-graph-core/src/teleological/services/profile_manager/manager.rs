//! ProfileManager implementation
//!
//! Core implementation of the ProfileManager service for managing
//! task-specific teleological profiles.

use std::collections::HashMap;

use tracing::warn;

use crate::teleological::{GroupType, ProfileId, TeleologicalProfile, NUM_EMBEDDERS};

use super::builtin;
use super::types::{InternalStats, ProfileManagerConfig, ProfileMatch, ProfileStats};

/// TELEO-015: Service for managing task-specific teleological profiles.
///
/// # Example
///
/// ```
/// use context_graph_core::teleological::services::ProfileManager;
///
/// let mut manager = ProfileManager::new();
///
/// // Get built-in profile
/// let profile = manager.get_or_create_default();
/// assert_eq!(profile.id.as_str(), "code_implementation");
///
/// // Create custom profile
/// let custom = manager.create_profile("my_profile", [0.1; 13]);
/// assert_eq!(custom.id.as_str(), "my_profile");
///
/// // Find best match for context
/// let match_result = manager.find_best_match("implement a sorting algorithm");
/// assert!(match_result.is_some());
/// ```
pub struct ProfileManager {
    /// Stored profiles.
    profiles: HashMap<ProfileId, TeleologicalProfile>,
    /// Per-profile usage statistics.
    stats: HashMap<ProfileId, InternalStats>,
    /// Configuration.
    config: ProfileManagerConfig,
}

impl ProfileManager {
    /// Create a new ProfileManager with default configuration and built-in profiles.
    pub fn new() -> Self {
        let config = ProfileManagerConfig::default();
        let mut manager = Self {
            profiles: HashMap::new(),
            stats: HashMap::new(),
            config,
        };

        // Initialize with built-in profiles
        manager.init_builtin_profiles();

        manager
    }

    /// Create a ProfileManager with custom configuration.
    pub fn with_config(config: ProfileManagerConfig) -> Self {
        let mut manager = Self {
            profiles: HashMap::new(),
            stats: HashMap::new(),
            config,
        };

        // Initialize with built-in profiles
        manager.init_builtin_profiles();

        manager
    }

    /// Initialize built-in profiles.
    fn init_builtin_profiles(&mut self) {
        // Code implementation profile - emphasizes E6 (Code)
        let code_profile = builtin::code_implementation();
        self.profiles.insert(code_profile.id.clone(), code_profile);

        // Research analysis profile - emphasizes E1, E4, E7 (Semantic, Causal, Procedural)
        let research_profile = builtin::research_analysis();
        self.profiles
            .insert(research_profile.id.clone(), research_profile);

        // Creative writing profile - emphasizes E10, E11 (Emotional, Abstract)
        let creative_profile = builtin::creative_writing();
        self.profiles
            .insert(creative_profile.id.clone(), creative_profile);
    }

    /// Create the code_implementation built-in profile.
    ///
    /// Emphasizes E6 (Code) at index 5.
    pub fn code_implementation() -> TeleologicalProfile {
        builtin::code_implementation()
    }

    /// Create the research_analysis built-in profile.
    ///
    /// Emphasizes E1 (Semantic), E4 (Causal), E7 (Procedural).
    pub fn research_analysis() -> TeleologicalProfile {
        builtin::research_analysis()
    }

    /// Create the creative_writing built-in profile.
    ///
    /// Emphasizes E10 (Emotional), E11 (Abstract).
    pub fn creative_writing() -> TeleologicalProfile {
        builtin::creative_writing()
    }

    /// Create a new profile with specified weights.
    ///
    /// # Arguments
    /// * `id` - Unique profile identifier
    /// * `weights` - 13-element weight array for each embedder
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `id` is empty (FAIL FAST)
    /// - Maximum profiles limit exceeded (FAIL FAST)
    /// - Any weight is negative (FAIL FAST)
    pub fn create_profile(
        &mut self,
        id: &str,
        weights: [f32; NUM_EMBEDDERS],
    ) -> TeleologicalProfile {
        assert!(!id.is_empty(), "FAIL FAST: Profile ID cannot be empty");
        assert!(
            self.profiles.len() < self.config.max_profiles,
            "FAIL FAST: Maximum profiles limit ({}) exceeded",
            self.config.max_profiles
        );
        for (i, &w) in weights.iter().enumerate() {
            assert!(
                w >= 0.0,
                "FAIL FAST: Weight at index {} cannot be negative (got {})",
                i,
                w
            );
        }

        let profile_id = ProfileId::new(id);

        let mut profile = TeleologicalProfile::new(
            id,
            id, // Use ID as name by default
            crate::teleological::TaskType::General,
        );
        profile.embedding_weights = weights;
        profile.normalize_weights();

        self.profiles.insert(profile_id, profile.clone());

        profile
    }

    /// Get a profile by ID.
    pub fn get_profile(&self, id: &ProfileId) -> Option<&TeleologicalProfile> {
        self.profiles.get(id)
    }

    /// Update an existing profile's weights.
    ///
    /// # Returns
    ///
    /// `true` if the profile was updated, `false` if not found.
    ///
    /// # Panics
    ///
    /// Panics if any weight is negative (FAIL FAST).
    pub fn update_profile(&mut self, id: &ProfileId, weights: [f32; NUM_EMBEDDERS]) -> bool {
        for (i, &w) in weights.iter().enumerate() {
            assert!(
                w >= 0.0,
                "FAIL FAST: Weight at index {} cannot be negative (got {})",
                i,
                w
            );
        }

        if let Some(profile) = self.profiles.get_mut(id) {
            profile.embedding_weights = weights;
            profile.normalize_weights();
            profile.updated_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }

    /// Delete a profile by ID.
    ///
    /// # Returns
    ///
    /// `true` if the profile was deleted, `false` if not found.
    pub fn delete_profile(&mut self, id: &ProfileId) -> bool {
        let removed = self.profiles.remove(id).is_some();
        if removed {
            self.stats.remove(id);
        }
        removed
    }

    /// Find the best matching profile for a given context string.
    ///
    /// Uses keyword matching to determine which profile best fits the context.
    pub fn find_best_match(&self, context: &str) -> Option<ProfileMatch> {
        if self.profiles.is_empty() {
            return None;
        }

        let context_lower = context.to_lowercase();
        let mut best_match: Option<(ProfileId, f32, String)> = None;

        // Check for code-related keywords
        let code_keywords = [
            "code",
            "implement",
            "function",
            "class",
            "method",
            "program",
            "algorithm",
            "debug",
            "compile",
            "rust",
            "python",
            "javascript",
        ];
        let research_keywords = [
            "research",
            "analyze",
            "understand",
            "explain",
            "why",
            "how",
            "cause",
            "effect",
            "study",
            "investigate",
        ];
        let creative_keywords = [
            "write",
            "creative",
            "story",
            "poem",
            "artistic",
            "express",
            "imagine",
            "narrative",
            "prose",
            "fiction",
        ];

        // Score each profile
        for id in self.profiles.keys() {
            let id_str = id.as_str();
            let mut score = 0.0f32;
            let mut reason = String::new();

            if id_str == "code_implementation" {
                for kw in &code_keywords {
                    if context_lower.contains(kw) {
                        score += 0.2;
                        if reason.is_empty() {
                            reason = format!("Matched code keyword: {}", kw);
                        }
                    }
                }
            } else if id_str == "research_analysis" {
                for kw in &research_keywords {
                    if context_lower.contains(kw) {
                        score += 0.2;
                        if reason.is_empty() {
                            reason = format!("Matched research keyword: {}", kw);
                        }
                    }
                }
            } else if id_str == "creative_writing" {
                for kw in &creative_keywords {
                    if context_lower.contains(kw) {
                        score += 0.2;
                        if reason.is_empty() {
                            reason = format!("Matched creative keyword: {}", kw);
                        }
                    }
                }
            }

            // Clamp score to 1.0
            score = score.min(1.0);

            if score > 0.0 {
                match &best_match {
                    None => {
                        best_match = Some((id.clone(), score, reason));
                    }
                    Some((_, best_score, _)) if score > *best_score => {
                        best_match = Some((id.clone(), score, reason));
                    }
                    _ => {}
                }
            }
        }

        // If no match found, return default profile with low similarity
        if best_match.is_none() {
            let default_id = ProfileId::new(&self.config.default_profile_id);
            if self.profiles.contains_key(&default_id) {
                return Some(ProfileMatch {
                    profile_id: default_id,
                    similarity: 0.1,
                    reason: "Default profile (no specific match)".to_string(),
                });
            }
        }

        best_match.map(|(id, score, reason)| ProfileMatch {
            profile_id: id,
            similarity: score,
            reason,
        })
    }

    /// List all profile IDs.
    pub fn list_profiles(&self) -> Vec<ProfileId> {
        self.profiles.keys().cloned().collect()
    }

    /// Get usage statistics for a profile.
    pub fn get_stats(&self, id: &ProfileId) -> Option<ProfileStats> {
        self.stats.get(id).map(|internal| ProfileStats {
            profile_id: id.clone(),
            usage_count: internal.usage_count,
            avg_effectiveness: if internal.usage_count > 0 {
                internal.total_effectiveness / internal.usage_count as f32
            } else {
                0.0
            },
            last_used: internal.last_used,
        })
    }

    /// Record profile usage with effectiveness score.
    ///
    /// # Arguments
    /// * `id` - Profile ID
    /// * `effectiveness` - Effectiveness score [0.0, 1.0]
    ///
    /// # Panics
    ///
    /// Panics if profile does not exist (FAIL FAST).
    pub fn record_usage(&mut self, id: &ProfileId, effectiveness: f32) {
        assert!(
            self.profiles.contains_key(id),
            "FAIL FAST: Cannot record usage for non-existent profile: {}",
            id
        );
        assert!(
            (0.0..=1.0).contains(&effectiveness),
            "FAIL FAST: Effectiveness must be in [0.0, 1.0], got {}",
            effectiveness
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|e| {
                warn!("System clock before UNIX epoch: {} — using 0 timestamp for usage recording", e);
                std::time::Duration::ZERO
            })
            .as_millis() as u64;

        let stats = self.stats.entry(id.clone()).or_default();
        stats.usage_count += 1;
        stats.total_effectiveness += effectiveness;
        stats.last_used = now;
    }

    /// Get or create the default profile.
    ///
    /// If the default profile doesn't exist and auto_create is enabled,
    /// creates the code_implementation profile.
    pub fn get_or_create_default(&mut self) -> &TeleologicalProfile {
        let default_id = ProfileId::new(&self.config.default_profile_id);

        if !self.profiles.contains_key(&default_id) && self.config.auto_create {
            // Create the default profile
            let profile = builtin::code_implementation();
            self.profiles.insert(default_id.clone(), profile);
        }

        self.profiles
            .get(&default_id)
            .expect("FAIL FAST: Default profile must exist after get_or_create_default")
    }

    /// Get the number of stored profiles.
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Check if a profile exists.
    pub fn contains(&self, id: &ProfileId) -> bool {
        self.profiles.contains_key(id)
    }

    /// Get configuration.
    pub fn config(&self) -> &ProfileManagerConfig {
        &self.config
    }

    /// Get profiles by group preference.
    ///
    /// Returns profiles that have high weight in the specified group.
    pub fn get_profiles_for_group(&self, group: GroupType) -> Vec<&TeleologicalProfile> {
        let indices = group.embedding_indices();

        self.profiles
            .values()
            .filter(|p| {
                let group_weight: f32 = indices.iter().map(|&i| p.embedding_weights[i]).sum();
                group_weight > 0.2 // Threshold for "prefers this group"
            })
            .collect()
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}
