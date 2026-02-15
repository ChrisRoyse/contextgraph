//! NodeMetadata container for MemoryNode supplementary information.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::Modality;

/// Enhanced deletion tracking with provenance (Phase 4, item 5.9).
///
/// Captures who deleted a memory, why, and when recovery expires.
/// Stored alongside NodeMetadata when a soft-delete occurs.
///
/// # Constitution Compliance
/// - SEC-06: recovery_deadline = deleted_at + 30 days
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletionMetadata {
    /// Who requested the deletion
    pub deleted_by: Option<String>,
    /// Why the memory was deleted
    pub deletion_reason: Option<String>,
    /// When the deletion occurred
    pub deleted_at: DateTime<Utc>,
    /// When recovery expires (30 days per SEC-06)
    pub recovery_deadline: DateTime<Utc>,
}

/// Helper function for serde default version value
fn default_version() -> u32 {
    1
}

/// Metadata container for MemoryNode supplementary information.
///
/// # Fields
/// - Source tracking for provenance
/// - Tagging for categorization
/// - Versioning for change tracking
/// - Soft-delete support per SEC-06
/// - Hierarchical relationships (parent/child)
/// - Custom attributes for extensibility
///
/// # Constitution Compliance
/// - SEC-06: Soft delete with 30-day recovery
/// - Naming: snake_case fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Source identifier (e.g., file path, URL, session ID)
    pub source: Option<String>,

    /// Natural language code (ISO 639-1, e.g., "en", "es")
    pub language: Option<String>,

    /// Content modality type
    pub modality: Modality,

    /// User-defined tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Cached UTL learning score [0.0, 1.0]
    pub utl_score: Option<f32>,

    /// Whether this node has been consolidated
    #[serde(default)]
    pub consolidated: bool,

    /// Timestamp when consolidation occurred
    pub consolidated_at: Option<DateTime<Utc>>,

    /// Version counter (incremented on updates)
    #[serde(default = "default_version")]
    pub version: u32,

    /// Soft delete flag (SEC-06 compliance)
    #[serde(default)]
    pub deleted: bool,

    /// Timestamp when soft deletion occurred
    pub deleted_at: Option<DateTime<Utc>>,

    /// Parent node ID for hierarchical relationships
    pub parent_id: Option<Uuid>,

    /// Child node IDs for hierarchical relationships
    #[serde(default)]
    pub child_ids: Vec<Uuid>,

    /// Custom user-defined attributes (JSON-compatible values)
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,

    /// Rationale for storing this memory (required per AP-010)
    pub rationale: Option<String>,

    /// Enhanced deletion tracking metadata (Phase 4, item 5.9).
    /// Populated when a memory is soft-deleted via mark_deleted_with_metadata().
    #[serde(default)]
    pub deletion_metadata: Option<DeletionMetadata>,
}

impl NodeMetadata {
    /// Create new metadata with default values.
    pub fn new() -> Self {
        Self {
            source: None,
            language: None,
            modality: Modality::default(),
            tags: Vec::new(),
            utl_score: None,
            consolidated: false,
            consolidated_at: None,
            version: 1,
            deleted: false,
            deleted_at: None,
            parent_id: None,
            child_ids: Vec::new(),
            custom: HashMap::new(),
            rationale: None,
            deletion_metadata: None,
        }
    }

    /// Set the source identifier. Returns self for builder chaining.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the language code. Returns self for builder chaining.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the modality. Returns self for builder chaining.
    pub fn with_modality(mut self, modality: Modality) -> Self {
        self.modality = modality;
        self
    }

    /// Adds a tag to this node's metadata for categorization.
    ///
    /// Tags enable content discovery and filtering across the knowledge graph.
    /// Automatically deduplicates - adding an existing tag is a no-op.
    ///
    /// # Arguments
    ///
    /// * `tag` - The tag string to add (anything convertible to `String`)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::types::NodeMetadata;
    ///
    /// let mut meta = NodeMetadata::new();
    /// meta.add_tag("important");
    /// meta.add_tag("rust");
    /// meta.add_tag("important"); // Duplicate ignored
    ///
    /// assert_eq!(meta.tags.len(), 2);
    /// assert!(meta.has_tag("important"));
    /// assert!(meta.has_tag("rust"));
    /// ```
    ///
    /// `Constraint: O(n) deduplication check`
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Removes a tag from this node's metadata.
    ///
    /// # Arguments
    ///
    /// * `tag` - The tag string to remove
    ///
    /// # Returns
    ///
    /// `true` if the tag was present and removed, `false` if tag was not found.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::types::NodeMetadata;
    ///
    /// let mut meta = NodeMetadata::new();
    /// meta.add_tag("temporary");
    /// meta.add_tag("permanent");
    ///
    /// assert!(meta.remove_tag("temporary"));
    /// assert!(!meta.remove_tag("nonexistent"));
    /// assert_eq!(meta.tags.len(), 1);
    /// ```
    ///
    /// `Constraint: O(n) linear search`
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if a tag exists.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Set a custom attribute. Overwrites if key already exists.
    pub fn set_custom(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.custom.insert(key.into(), value);
    }

    /// Get a custom attribute by key.
    pub fn get_custom(&self, key: &str) -> Option<&serde_json::Value> {
        self.custom.get(key)
    }

    /// Remove a custom attribute. Returns the removed value if present.
    pub fn remove_custom(&mut self, key: &str) -> Option<serde_json::Value> {
        self.custom.remove(key)
    }

    /// Marks this node as consolidated during dream consolidation.
    ///
    /// Consolidation is the process of strengthening important memories
    /// during offline processing (inspired by sleep consolidation in
    /// biological memory systems).
    ///
    /// Sets `consolidated = true` and records the current timestamp
    /// in `consolidated_at`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::types::NodeMetadata;
    ///
    /// let mut meta = NodeMetadata::new();
    /// assert!(!meta.consolidated);
    /// assert!(meta.consolidated_at.is_none());
    ///
    /// meta.mark_consolidated();
    ///
    /// assert!(meta.consolidated);
    /// assert!(meta.consolidated_at.is_some());
    /// ```
    pub fn mark_consolidated(&mut self) {
        self.consolidated = true;
        self.consolidated_at = Some(Utc::now());
    }

    /// Marks this node as deleted (soft delete) with current timestamp.
    ///
    /// Implements soft deletion per SEC-06 constitution compliance:
    /// - Sets `deleted = true` and records timestamp in `deleted_at`
    /// - Node remains in storage for 30-day recovery window
    /// - Can be restored via [`restore()`](Self::restore)
    ///
    /// # Constitution Compliance
    ///
    /// SEC-06: Soft delete with 30-day recovery period before permanent removal.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use context_graph_core::types::NodeMetadata;
    ///
    /// let mut meta = NodeMetadata::new();
    /// assert!(!meta.deleted);
    ///
    /// meta.mark_deleted();
    ///
    /// assert!(meta.deleted);
    /// assert!(meta.deleted_at.is_some());
    ///
    /// // Can be restored within 30 days
    /// meta.restore();
    /// assert!(!meta.deleted);
    /// ```
    pub fn mark_deleted(&mut self) {
        self.deleted = true;
        self.deleted_at = Some(Utc::now());
    }

    /// Marks this node as deleted with enhanced provenance metadata (Phase 4, item 5.9).
    ///
    /// Records who deleted the memory, why, and computes a 30-day recovery deadline
    /// per SEC-06 constitution compliance.
    ///
    /// # Arguments
    ///
    /// * `operator_id` - Who requested the deletion (user/agent ID)
    /// * `reason` - Why the memory was deleted
    pub fn mark_deleted_with_metadata(&mut self, operator_id: Option<String>, reason: Option<String>) {
        let now = Utc::now();
        self.deleted = true;
        self.deleted_at = Some(now);
        self.deletion_metadata = Some(DeletionMetadata {
            deleted_by: operator_id,
            deletion_reason: reason,
            deleted_at: now,
            recovery_deadline: now + Duration::days(30),
        });
    }

    /// Restore from soft deletion. Clears deleted flag, timestamp, and deletion metadata.
    pub fn restore(&mut self) {
        self.deleted = false;
        self.deleted_at = None;
        self.deletion_metadata = None;
    }

    /// Increment version counter. Saturates at u32::MAX (never wraps).
    pub fn increment_version(&mut self) {
        self.version = self.version.saturating_add(1);
    }

    /// Estimate memory size in bytes.
    pub fn estimated_size(&self) -> usize {
        let base = std::mem::size_of::<Self>();

        let source_size = self.source.as_ref().map_or(0, |s| s.len());
        let language_size = self.language.as_ref().map_or(0, |s| s.len());
        let rationale_size = self.rationale.as_ref().map_or(0, |s| s.len());

        let tags_size: usize = self.tags.iter().map(|t| t.len()).sum();
        let child_ids_size = self.child_ids.len() * 16; // UUID is 16 bytes

        // Rough estimate for HashMap: key lengths + value estimate (assume ~32 bytes avg)
        let custom_size: usize = self.custom.keys().map(|k| k.len() + 32).sum();

        base + source_size
            + language_size
            + rationale_size
            + tags_size
            + child_ids_size
            + custom_size
    }
}

impl Default for NodeMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_metadata_new_defaults() {
        let meta = NodeMetadata::new();

        assert!(meta.source.is_none());
        assert!(meta.language.is_none());
        assert_eq!(meta.modality, Modality::Text);
        assert!(meta.tags.is_empty());
        assert!(meta.utl_score.is_none());
        assert!(!meta.consolidated);
        assert!(meta.consolidated_at.is_none());
        assert_eq!(meta.version, 1);
        assert!(!meta.deleted);
        assert!(meta.deleted_at.is_none());
        assert!(meta.parent_id.is_none());
        assert!(meta.child_ids.is_empty());
        assert!(meta.custom.is_empty());
        assert!(meta.rationale.is_none());
        assert!(meta.deletion_metadata.is_none());
    }

    #[test]
    fn test_node_metadata_default_equals_new() {
        let from_new = NodeMetadata::new();
        let from_default = NodeMetadata::default();
        assert_eq!(from_new, from_default);
    }

    #[test]
    fn test_node_metadata_builder_chaining() {
        let meta = NodeMetadata::new()
            .with_source("file.rs")
            .with_language("rust")
            .with_modality(Modality::Code);

        assert_eq!(meta.source, Some("file.rs".to_string()));
        assert_eq!(meta.language, Some("rust".to_string()));
        assert_eq!(meta.modality, Modality::Code);
    }

    #[test]
    fn test_node_metadata_tag_operations() {
        let mut meta = NodeMetadata::new();
        meta.add_tag("important");
        meta.add_tag("important"); // duplicate
        meta.add_tag("beta");

        assert_eq!(meta.tags.len(), 2);
        assert!(meta.has_tag("important"));
        assert!(meta.has_tag("beta"));
        assert!(!meta.has_tag("missing"));

        assert!(meta.remove_tag("important"));
        assert!(!meta.remove_tag("nonexistent"));
        assert_eq!(meta.tags.len(), 1);
    }

    #[test]
    fn test_node_metadata_soft_delete_cycle() {
        let mut meta = NodeMetadata::new();

        assert!(!meta.deleted);
        meta.mark_deleted();
        assert!(meta.deleted);
        assert!(meta.deleted_at.is_some());

        meta.restore();
        assert!(!meta.deleted);
        assert!(meta.deleted_at.is_none());
    }

    #[test]
    fn test_node_metadata_version_saturates() {
        let mut meta = NodeMetadata::new();
        meta.version = u32::MAX;
        meta.increment_version();
        assert_eq!(meta.version, u32::MAX);
    }
}
