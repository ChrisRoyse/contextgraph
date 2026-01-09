//! TASK-TELEO-015: ProfileManager Implementation
//!
//! Manages task-specific teleological profiles. Provides CRUD operations
//! for profiles, context-based profile matching, and usage tracking.
//!
//! # Core Responsibilities
//!
//! 1. Create, read, update, delete profiles
//! 2. Find best matching profile for a given context
//! 3. Track profile usage statistics
//! 4. Provide built-in profiles for common tasks
//!
//! # Built-in Profiles
//!
//! - **code_implementation**: Emphasizes E6 (Code) for programming tasks
//! - **research_analysis**: Emphasizes E1, E4, E7 for semantic/causal analysis
//! - **creative_writing**: Emphasizes E10, E11 for qualitative/abstract tasks

use std::collections::HashMap;

use crate::teleological::{
    GroupType, ProfileId, TeleologicalProfile, NUM_EMBEDDERS,
};

/// Configuration for ProfileManager.
#[derive(Clone, Debug)]
pub struct ProfileManagerConfig {
    /// Maximum number of profiles to store.
    pub max_profiles: usize,
    /// Automatically create default profile if none exists.
    pub auto_create: bool,
    /// Default profile ID when no match is found.
    pub default_profile_id: String,
}

impl Default for ProfileManagerConfig {
    fn default() -> Self {
        Self {
            max_profiles: 100,
            auto_create: true,
            default_profile_id: "code_implementation".to_string(),
        }
    }
}

/// Result of profile matching.
#[derive(Clone, Debug)]
pub struct ProfileMatch {
    /// The matched profile ID.
    pub profile_id: ProfileId,
    /// Similarity score between context and profile.
    pub similarity: f32,
    /// Reason for the match.
    pub reason: String,
}

/// Usage statistics for a profile.
#[derive(Clone, Debug)]
pub struct ProfileStats {
    /// Profile identifier.
    pub profile_id: ProfileId,
    /// Total number of times this profile was used.
    pub usage_count: usize,
    /// Average effectiveness score from recorded usages.
    pub avg_effectiveness: f32,
    /// Timestamp of last usage (epoch millis).
    pub last_used: u64,
}

/// Internal stats tracking.
#[derive(Clone, Debug, Default)]
struct InternalStats {
    usage_count: usize,
    total_effectiveness: f32,
    last_used: u64,
}

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
        let code_profile = Self::code_implementation();
        self.profiles.insert(code_profile.id.clone(), code_profile);

        // Research analysis profile - emphasizes E1, E4, E7 (Semantic, Causal, Procedural)
        let research_profile = Self::research_analysis();
        self.profiles.insert(research_profile.id.clone(), research_profile);

        // Creative writing profile - emphasizes E10, E11 (Emotional, Abstract)
        let creative_profile = Self::creative_writing();
        self.profiles.insert(creative_profile.id.clone(), creative_profile);
    }

    /// Create the code_implementation built-in profile.
    ///
    /// Emphasizes E6 (Code) at index 5.
    pub fn code_implementation() -> TeleologicalProfile {
        // Weights: emphasize E6 (index 5) for code implementation
        let weights = [
            0.05, // E1_Semantic
            0.02, // E2_Episodic
            0.05, // E3_Temporal
            0.15, // E4_Causal
            0.08, // E5_Analogical
            0.25, // E6_Code (PRIMARY)
            0.18, // E7_Procedural
            0.05, // E8_Spatial
            0.02, // E9_Social
            0.02, // E10_Emotional
            0.05, // E11_Abstract
            0.05, // E12_Factual
            0.03, // E13_Sparse
        ];

        let mut profile = TeleologicalProfile::new(
            "code_implementation",
            "Code Implementation",
            crate::teleological::TaskType::CodeSearch,
        );
        profile.embedding_weights = weights;
        profile.is_system = true;
        profile.description = Some("Optimized for programming and code implementation tasks".to_string());
        profile
    }

    /// Create the research_analysis built-in profile.
    ///
    /// Emphasizes E1 (Semantic), E4 (Causal), E7 (Procedural).
    pub fn research_analysis() -> TeleologicalProfile {
        // Weights: emphasize semantic (E1), causal (E4), procedural (E7)
        let weights = [
            0.20, // E1_Semantic (PRIMARY)
            0.05, // E2_Episodic
            0.08, // E3_Temporal
            0.18, // E4_Causal (PRIMARY)
            0.10, // E5_Analogical
            0.03, // E6_Code
            0.15, // E7_Procedural (PRIMARY)
            0.05, // E8_Spatial
            0.03, // E9_Social
            0.02, // E10_Emotional
            0.05, // E11_Abstract
            0.04, // E12_Factual
            0.02, // E13_Sparse
        ];

        let mut profile = TeleologicalProfile::new(
            "research_analysis",
            "Research Analysis",
            crate::teleological::TaskType::SemanticSearch,
        );
        profile.embedding_weights = weights;
        profile.is_system = true;
        profile.description = Some("Optimized for research and analytical queries".to_string());
        profile
    }

    /// Create the creative_writing built-in profile.
    ///
    /// Emphasizes E10 (Emotional), E11 (Abstract).
    pub fn creative_writing() -> TeleologicalProfile {
        // Weights: emphasize qualitative embeddings (E10, E11)
        let weights = [
            0.08, // E1_Semantic
            0.05, // E2_Episodic
            0.07, // E3_Temporal
            0.05, // E4_Causal
            0.12, // E5_Analogical
            0.02, // E6_Code
            0.03, // E7_Procedural
            0.05, // E8_Spatial
            0.08, // E9_Social
            0.20, // E10_Emotional (PRIMARY)
            0.18, // E11_Abstract (PRIMARY)
            0.04, // E12_Factual
            0.03, // E13_Sparse
        ];

        let mut profile = TeleologicalProfile::new(
            "creative_writing",
            "Creative Writing",
            crate::teleological::TaskType::AbstractSearch,
        );
        profile.embedding_weights = weights;
        profile.is_system = true;
        profile.description = Some("Optimized for creative and qualitative tasks".to_string());
        profile
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
    pub fn create_profile(&mut self, id: &str, weights: [f32; NUM_EMBEDDERS]) -> TeleologicalProfile {
        assert!(
            !id.is_empty(),
            "FAIL FAST: Profile ID cannot be empty"
        );
        assert!(
            self.profiles.len() < self.config.max_profiles,
            "FAIL FAST: Maximum profiles limit ({}) exceeded",
            self.config.max_profiles
        );
        for (i, &w) in weights.iter().enumerate() {
            assert!(
                w >= 0.0,
                "FAIL FAST: Weight at index {} cannot be negative (got {})",
                i, w
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
                i, w
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
        let code_keywords = ["code", "implement", "function", "class", "method", "program", "algorithm", "debug", "compile", "rust", "python", "javascript"];
        let research_keywords = ["research", "analyze", "understand", "explain", "why", "how", "cause", "effect", "study", "investigate"];
        let creative_keywords = ["write", "creative", "story", "poem", "artistic", "express", "imagine", "narrative", "prose", "fiction"];

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
            .unwrap_or_default()
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
            let profile = Self::code_implementation();
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ProfileManagerConfig Tests =====

    #[test]
    fn test_config_default() {
        let config = ProfileManagerConfig::default();

        assert_eq!(config.max_profiles, 100);
        assert!(config.auto_create);
        assert_eq!(config.default_profile_id, "code_implementation");

        println!("[PASS] ProfileManagerConfig::default has correct values");
    }

    // ===== Built-in Profile Tests =====

    #[test]
    fn test_code_implementation_profile() {
        let profile = ProfileManager::code_implementation();

        assert_eq!(profile.id.as_str(), "code_implementation");
        assert!(profile.is_system);

        // E6 (index 5) should have highest weight
        let max_idx = profile
            .embedding_weights
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(max_idx, 5, "E6 should have highest weight");

        // Weights should sum to 1.0 (normalized)
        let sum: f32 = profile.embedding_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "Weights should sum to ~1.0, got {}", sum);

        println!("[PASS] code_implementation profile emphasizes E6");
    }

    #[test]
    fn test_research_analysis_profile() {
        let profile = ProfileManager::research_analysis();

        assert_eq!(profile.id.as_str(), "research_analysis");
        assert!(profile.is_system);

        // E1 (index 0), E4 (index 3), E7 (index 6) should be high
        assert!(profile.embedding_weights[0] > 0.15, "E1 should be high");
        assert!(profile.embedding_weights[3] > 0.15, "E4 should be high");
        assert!(profile.embedding_weights[6] > 0.10, "E7 should be high");

        println!("[PASS] research_analysis profile emphasizes E1, E4, E7");
    }

    #[test]
    fn test_creative_writing_profile() {
        let profile = ProfileManager::creative_writing();

        assert_eq!(profile.id.as_str(), "creative_writing");
        assert!(profile.is_system);

        // E10 (index 9), E11 (index 10) should be high
        assert!(profile.embedding_weights[9] > 0.15, "E10 should be high");
        assert!(profile.embedding_weights[10] > 0.15, "E11 should be high");

        println!("[PASS] creative_writing profile emphasizes E10, E11");
    }

    // ===== ProfileManager Basic Tests =====

    #[test]
    fn test_profile_manager_new() {
        let manager = ProfileManager::new();

        // Should have 3 built-in profiles
        assert_eq!(manager.profile_count(), 3);

        // Should have code_implementation
        let code_id = ProfileId::new("code_implementation");
        assert!(manager.contains(&code_id));

        // Should have research_analysis
        let research_id = ProfileId::new("research_analysis");
        assert!(manager.contains(&research_id));

        // Should have creative_writing
        let creative_id = ProfileId::new("creative_writing");
        assert!(manager.contains(&creative_id));

        println!("[PASS] ProfileManager::new creates manager with built-in profiles");
    }

    #[test]
    fn test_profile_manager_with_config() {
        let config = ProfileManagerConfig {
            max_profiles: 50,
            auto_create: false,
            default_profile_id: "research_analysis".to_string(),
        };

        let manager = ProfileManager::with_config(config);

        assert_eq!(manager.config().max_profiles, 50);
        assert!(!manager.config().auto_create);
        assert_eq!(manager.config().default_profile_id, "research_analysis");

        println!("[PASS] ProfileManager::with_config uses custom config");
    }

    // ===== CRUD Tests =====

    #[test]
    fn test_create_profile() {
        let mut manager = ProfileManager::new();

        let weights = [0.1; NUM_EMBEDDERS];
        let profile = manager.create_profile("test_profile", weights);

        assert_eq!(profile.id.as_str(), "test_profile");
        assert_eq!(manager.profile_count(), 4); // 3 built-in + 1 new

        // Weights should be normalized
        let sum: f32 = profile.embedding_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        println!("[PASS] create_profile creates and normalizes profile");
    }

    #[test]
    fn test_get_profile() {
        let manager = ProfileManager::new();

        let code_id = ProfileId::new("code_implementation");
        let profile = manager.get_profile(&code_id);

        assert!(profile.is_some());
        assert_eq!(profile.unwrap().id, code_id);

        let non_existent = ProfileId::new("non_existent");
        assert!(manager.get_profile(&non_existent).is_none());

        println!("[PASS] get_profile returns correct results");
    }

    #[test]
    fn test_update_profile() {
        let mut manager = ProfileManager::new();

        let code_id = ProfileId::new("code_implementation");

        // Update with new weights
        let mut new_weights = [0.05; NUM_EMBEDDERS];
        new_weights[0] = 0.5; // Boost E1

        let updated = manager.update_profile(&code_id, new_weights);
        assert!(updated);

        // Verify update
        let profile = manager.get_profile(&code_id).unwrap();
        assert!(profile.embedding_weights[0] > 0.3); // Should be normalized but still high

        // Update non-existent profile
        let fake_id = ProfileId::new("fake");
        assert!(!manager.update_profile(&fake_id, [0.1; NUM_EMBEDDERS]));

        println!("[PASS] update_profile updates existing profiles");
    }

    #[test]
    fn test_delete_profile() {
        let mut manager = ProfileManager::new();

        let code_id = ProfileId::new("code_implementation");
        assert!(manager.contains(&code_id));

        let deleted = manager.delete_profile(&code_id);
        assert!(deleted);
        assert!(!manager.contains(&code_id));
        assert_eq!(manager.profile_count(), 2);

        // Delete non-existent
        let fake_id = ProfileId::new("fake");
        assert!(!manager.delete_profile(&fake_id));

        println!("[PASS] delete_profile removes profiles correctly");
    }

    // ===== Profile Matching Tests =====

    #[test]
    fn test_find_best_match_code() {
        let manager = ProfileManager::new();

        let result = manager.find_best_match("implement a sorting algorithm");
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.profile_id.as_str(), "code_implementation");
        assert!(matched.similarity > 0.1);
        assert!(matched.reason.contains("code") || matched.reason.contains("implement") || matched.reason.contains("algorithm"));

        println!("[PASS] find_best_match matches code context to code_implementation");
    }

    #[test]
    fn test_find_best_match_research() {
        let manager = ProfileManager::new();

        let result = manager.find_best_match("analyze the research and explain why this happens");
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.profile_id.as_str(), "research_analysis");
        assert!(matched.similarity > 0.1);

        println!("[PASS] find_best_match matches research context to research_analysis");
    }

    #[test]
    fn test_find_best_match_creative() {
        let manager = ProfileManager::new();

        let result = manager.find_best_match("write a creative story about imagination");
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.profile_id.as_str(), "creative_writing");
        assert!(matched.similarity > 0.1);

        println!("[PASS] find_best_match matches creative context to creative_writing");
    }

    #[test]
    fn test_find_best_match_default() {
        let manager = ProfileManager::new();

        // Context that doesn't match any keywords specifically
        let result = manager.find_best_match("hello world");
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.profile_id.as_str(), "code_implementation"); // Default
        assert!(matched.similarity < 0.5); // Low similarity

        println!("[PASS] find_best_match returns default for ambiguous context");
    }

    // ===== List Profiles Tests =====

    #[test]
    fn test_list_profiles() {
        let manager = ProfileManager::new();

        let ids = manager.list_profiles();
        assert_eq!(ids.len(), 3);

        let id_strs: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
        assert!(id_strs.contains(&"code_implementation"));
        assert!(id_strs.contains(&"research_analysis"));
        assert!(id_strs.contains(&"creative_writing"));

        println!("[PASS] list_profiles returns all profile IDs");
    }

    // ===== Stats Tests =====

    #[test]
    fn test_record_usage_and_get_stats() {
        let mut manager = ProfileManager::new();

        let code_id = ProfileId::new("code_implementation");

        // Initially no stats
        assert!(manager.get_stats(&code_id).is_none());

        // Record usage
        manager.record_usage(&code_id, 0.8);
        manager.record_usage(&code_id, 0.9);
        manager.record_usage(&code_id, 0.7);

        // Get stats
        let stats = manager.get_stats(&code_id);
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.usage_count, 3);
        assert!((stats.avg_effectiveness - 0.8).abs() < 0.01); // (0.8 + 0.9 + 0.7) / 3 = 0.8
        assert!(stats.last_used > 0);

        println!("[PASS] record_usage and get_stats track usage correctly");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_record_usage_non_existent_panics() {
        let mut manager = ProfileManager::new();
        let fake_id = ProfileId::new("fake");
        manager.record_usage(&fake_id, 0.5);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_record_usage_invalid_effectiveness_panics() {
        let mut manager = ProfileManager::new();
        let code_id = ProfileId::new("code_implementation");
        manager.record_usage(&code_id, 1.5); // Out of range
    }

    // ===== Get or Create Default Tests =====

    #[test]
    fn test_get_or_create_default() {
        let mut manager = ProfileManager::new();

        let profile = manager.get_or_create_default();
        assert_eq!(profile.id.as_str(), "code_implementation");

        println!("[PASS] get_or_create_default returns default profile");
    }

    #[test]
    fn test_get_or_create_default_creates_if_missing() {
        let config = ProfileManagerConfig {
            max_profiles: 100,
            auto_create: true,
            default_profile_id: "code_implementation".to_string(),
        };

        let mut manager = ProfileManager::with_config(config);

        // Delete the default profile
        let code_id = ProfileId::new("code_implementation");
        manager.delete_profile(&code_id);
        assert!(!manager.contains(&code_id));

        // get_or_create_default should recreate it
        let profile = manager.get_or_create_default();
        assert_eq!(profile.id.as_str(), "code_implementation");
        assert!(manager.contains(&code_id));

        println!("[PASS] get_or_create_default recreates missing default profile");
    }

    // ===== Group Preference Tests =====

    #[test]
    fn test_get_profiles_for_group() {
        let manager = ProfileManager::new();

        // Implementation group should match code_implementation
        let impl_profiles = manager.get_profiles_for_group(GroupType::Implementation);
        assert!(!impl_profiles.is_empty());
        assert!(impl_profiles.iter().any(|p| p.id.as_str() == "code_implementation"));

        // Qualitative group should match creative_writing
        let qual_profiles = manager.get_profiles_for_group(GroupType::Qualitative);
        assert!(!qual_profiles.is_empty());
        assert!(qual_profiles.iter().any(|p| p.id.as_str() == "creative_writing"));

        println!("[PASS] get_profiles_for_group returns profiles with high group weights");
    }

    // ===== FAIL FAST Tests =====

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_create_profile_empty_id_panics() {
        let mut manager = ProfileManager::new();
        manager.create_profile("", [0.1; NUM_EMBEDDERS]);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_create_profile_negative_weight_panics() {
        let mut manager = ProfileManager::new();
        let mut weights = [0.1; NUM_EMBEDDERS];
        weights[0] = -0.1;
        manager.create_profile("test", weights);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_update_profile_negative_weight_panics() {
        let mut manager = ProfileManager::new();
        let code_id = ProfileId::new("code_implementation");
        let mut weights = [0.1; NUM_EMBEDDERS];
        weights[5] = -0.5;
        manager.update_profile(&code_id, weights);
    }

    #[test]
    fn test_max_profiles_limit() {
        let config = ProfileManagerConfig {
            max_profiles: 5, // 3 built-in + 2 custom max
            auto_create: true,
            default_profile_id: "code_implementation".to_string(),
        };

        let mut manager = ProfileManager::with_config(config);
        assert_eq!(manager.profile_count(), 3); // Built-in profiles

        // Can add 2 more
        manager.create_profile("custom1", [0.1; NUM_EMBEDDERS]);
        manager.create_profile("custom2", [0.1; NUM_EMBEDDERS]);
        assert_eq!(manager.profile_count(), 5);

        println!("[PASS] Profile limit works correctly");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_max_profiles_limit_exceeded_panics() {
        let config = ProfileManagerConfig {
            max_profiles: 3, // Only built-in profiles allowed
            auto_create: true,
            default_profile_id: "code_implementation".to_string(),
        };

        let mut manager = ProfileManager::with_config(config);
        manager.create_profile("extra", [0.1; NUM_EMBEDDERS]); // Should panic
    }

    // ===== Serialization Tests =====

    #[test]
    fn test_profile_stats_fields() {
        let stats = ProfileStats {
            profile_id: ProfileId::new("test"),
            usage_count: 10,
            avg_effectiveness: 0.85,
            last_used: 1234567890,
        };

        assert_eq!(stats.profile_id.as_str(), "test");
        assert_eq!(stats.usage_count, 10);
        assert!((stats.avg_effectiveness - 0.85).abs() < f32::EPSILON);
        assert_eq!(stats.last_used, 1234567890);

        println!("[PASS] ProfileStats has correct fields");
    }

    #[test]
    fn test_profile_match_fields() {
        let pm = ProfileMatch {
            profile_id: ProfileId::new("test"),
            similarity: 0.9,
            reason: "Test match".to_string(),
        };

        assert_eq!(pm.profile_id.as_str(), "test");
        assert!((pm.similarity - 0.9).abs() < f32::EPSILON);
        assert_eq!(pm.reason, "Test match");

        println!("[PASS] ProfileMatch has correct fields");
    }

    // ===== Default Trait Test =====

    #[test]
    fn test_profile_manager_default() {
        let manager = ProfileManager::default();
        assert_eq!(manager.profile_count(), 3);

        println!("[PASS] ProfileManager::default works");
    }
}
