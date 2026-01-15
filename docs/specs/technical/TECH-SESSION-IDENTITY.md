# Technical Specification: Session Identity Persistence

```xml
<technical_spec id="TECH-SESSION-IDENTITY" version="1.0">
<metadata>
  <title>Session Identity Persistence - Technical Architecture</title>
  <status>draft</status>
  <owner>Context Graph Team</owner>
  <last_updated>2026-01-14</last_updated>
  <functional_spec_ref>SPEC-SESSION-IDENTITY</functional_spec_ref>
  <constitution_refs>
    <ref>ARCH-07</ref>
    <ref>AP-50</ref>
    <ref>AP-53</ref>
    <ref>IDENTITY-002</ref>
    <ref>IDENTITY-007</ref>
    <ref>AP-26</ref>
  </constitution_refs>
</metadata>

<architecture>
<!-- ============================================================================
     SYSTEM ARCHITECTURE OVERVIEW
     ============================================================================ -->

<component_diagram>
```
+------------------------------------------------------------------+
|                    Claude Code Session                            |
+------------------------------------------------------------------+
         |                    |                    |
         v                    v                    v
+----------------+    +----------------+    +----------------+
| SessionStart   |    | PreToolUse     |    | PostToolUse    |
| Hook           |    | Hook           |    | Hook           |
+----------------+    +----------------+    +----------------+
         |                    |                    |
         v                    v                    v
+------------------------------------------------------------------+
|                 .claude/settings.json                             |
|   (ARCH-07: Native Claude Code hooks, AP-50: No internal hooks)  |
+------------------------------------------------------------------+
         |                    |                    |
         v                    v                    v
+----------------+    +----------------+    +----------------+
| restore-       |    | consciousness  |    | check-         |
| identity       |    | brief          |    | identity       |
| (<2s)          |    | (<50ms)        |    | (<500ms)       |
+----------------+    +----------------+    +----------------+
         |                    |                    |
         +--------------------+--------------------+
                             |
                             v
+------------------------------------------------------------------+
|               SessionIdentityManager                              |
|   - capture_snapshot()                                            |
|   - restore_identity()                                            |
|   - compute_cross_session_ic()                                    |
+------------------------------------------------------------------+
         |                    |                    |
         v                    v                    v
+----------------+    +----------------+    +----------------+
| SessionIdentity|    | IdentityCache  |    | DreamTrigger   |
| Snapshot       |    | (OnceLock)     |    | (async)        |
+----------------+    +----------------+    +----------------+
         |                    |                    |
         +--------------------+--------------------+
                             |
                             v
+------------------------------------------------------------------+
|                    RocksDB Storage                                |
|   CF_SESSION_IDENTITY column family                               |
|   Keys: s:{session_id}, latest, t:{timestamp_ms}                 |
+------------------------------------------------------------------+
```
</component_diagram>

<layer_architecture>
```
+------------------------------------------------------------------+
|  LAYER 3: SURFACE (CLI Commands & Hooks)                         |
|  REQ-SESSION-11 through REQ-SESSION-17                           |
|  - consciousness brief, restore-identity, persist-identity       |
|  - check-identity, inject-context, hook configuration            |
+------------------------------------------------------------------+
         |
         v
+------------------------------------------------------------------+
|  LAYER 2: LOGIC (Session Manager & IC Computation)               |
|  REQ-SESSION-06 through REQ-SESSION-10                           |
|  - SessionIdentityManager trait                                   |
|  - classify_ic(), auto-dream trigger                             |
|  - IdentityCache.format_brief(), update_cache()                  |
+------------------------------------------------------------------+
         |
         v
+------------------------------------------------------------------+
|  LAYER 1: FOUNDATION (Data Structures & Storage)                 |
|  REQ-SESSION-01 through REQ-SESSION-05                           |
|  - SessionIdentitySnapshot struct (14 fields)                    |
|  - IdentityCache struct (4 fields)                               |
|  - CF_SESSION_IDENTITY column family                             |
|  - save_snapshot(), load_snapshot(), load_latest()               |
+------------------------------------------------------------------+
```
</layer_architecture>

<data_flow>
```
SessionStart Flow:
  Claude Code --> SessionStart hook
    --> context-graph-cli session restore-identity
      --> SessionIdentityManager.restore_identity()
        --> load_latest() from CF_SESSION_IDENTITY
        --> compute_cross_session_ic(prev, curr)
        --> update_cache() --> IdentityCache
      --> stdout: "Identity restored from {id}. IC: X.XX (classification)"
    --> Claude Code receives identity context

PreToolUse Flow (Hot Path):
  Claude Code --> PreToolUse hook (matcher: mcp__context-graph__*|Edit|Write)
    --> context-graph-cli consciousness brief
      --> IdentityCache.get() [NO disk I/O]
      --> IdentityCache.format_brief() [<1ms]
    --> stdout: "[C:STATE r=X.XX IC=X.XX]" (~15 tokens)
    --> Claude Code prepends to tool context

PostToolUse Flow:
  Claude Code --> PostToolUse hook (matcher: mcp__context-graph__*|Edit|Write)
    --> context-graph-cli consciousness check-identity --auto-dream
      --> compute_current_ic()
      --> update_cache() --> IdentityCache
      --> if IC < 0.5 && --auto-dream:
          --> tokio::spawn(dream_trigger) [fire-and-forget]
          --> stderr: "IC crisis (X.XX), dream triggered"
      --> if IC < 0.7:
          --> stderr: "IC warning: X.XX"
    --> exit 0 (always non-blocking)

SessionEnd Flow:
  Claude Code --> SessionEnd hook
    --> context-graph-cli session persist-identity
      --> SessionIdentityManager.capture_snapshot()
      --> save_snapshot() to CF_SESSION_IDENTITY
      --> update "latest" key
      --> update temporal index "t:{timestamp_ms}"
    --> exit 0 (silent success)
```
</data_flow>

</architecture>

<data_model>
<!-- ============================================================================
     LAYER 1: FOUNDATION - Data Structures
     ============================================================================ -->

<requirement_impl id="REQ-SESSION-01" layer="foundation">
<description>SessionIdentitySnapshot struct with flattened fields</description>

<rust_struct>
```rust
//! File: crates/context-graph-core/src/gwt/session_identity/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum trajectory history entries (50 down from 1000)
/// Reduces snapshot size from ~80KB to <30KB
pub const MAX_TRAJECTORY_LEN: usize = 50;

/// Maximum session_id string length
pub const MAX_SESSION_ID_LEN: usize = 100;

/// 13-dimensional Kuramoto oscillator phases (one per embedder)
pub const KURAMOTO_N: usize = 13;

/// Session identity snapshot for cross-session persistence.
///
/// # Constitution Reference
/// - IDENTITY-001: IC = cos(PV_t, PV_{t-1}) x r(t)
/// - IDENTITY-002: Thresholds Healthy>0.9, Warning<0.7, Critical<0.5
///
/// # Size Constraints
/// - Target: <30KB serialized with bincode
/// - Trajectory capped at 50 entries (MAX_TRAJECTORY_LEN)
/// - session_id capped at 100 chars (MAX_SESSION_ID_LEN)
///
/// # Serialization
/// Uses bincode for compact binary serialization (~3-30KB typical).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionIdentitySnapshot {
    /// Unique session identifier (UUID string, max 100 chars)
    pub session_id: String,

    /// Unix timestamp in milliseconds when snapshot was created
    pub timestamp_ms: i64,

    /// Link to previous session for continuity tracking
    pub previous_session_id: Option<String>,

    /// Cross-session identity continuity: cos(PV_current, PV_previous) * r(current)
    pub cross_session_ic: f32,

    /// 13 Kuramoto oscillator phases (one per embedder)
    /// Each phase in radians [0, 2*PI]
    pub kuramoto_phases: [f64; KURAMOTO_N],

    /// Kuramoto coupling constant K
    pub coupling: f64,

    /// 13-dimensional purpose alignment vector
    /// Normalized unit vector representing system purpose
    pub purpose_vector: [f32; KURAMOTO_N],

    /// Purpose vector trajectory (last 50 vectors, FIFO eviction)
    /// Used for drift detection across sessions
    #[serde(default)]
    pub trajectory: Vec<[f32; KURAMOTO_N]>,

    /// Most recent IC value within the session
    pub last_ic: f32,

    /// Dream trigger threshold (default 0.5 per IDENTITY-007)
    pub crisis_threshold: f32,

    /// GWT consciousness level C(t) = I(t) * R(t) * D(t)
    pub consciousness: f32,

    /// Integration component I(t) - Kuramoto order parameter r
    pub integration: f32,

    /// Reflection component R(t) - Meta-UTL awareness
    pub reflection: f32,

    /// Differentiation component D(t) - Purpose vector entropy
    pub differentiation: f32,
}

impl SessionIdentitySnapshot {
    /// Create a new snapshot with the given session_id
    pub fn new(session_id: impl Into<String>) -> Self {
        let sid = session_id.into();
        let truncated_sid = if sid.len() > MAX_SESSION_ID_LEN {
            sid[..MAX_SESSION_ID_LEN].to_string()
        } else {
            sid
        };

        Self {
            session_id: truncated_sid,
            timestamp_ms: Utc::now().timestamp_millis(),
            previous_session_id: None,
            cross_session_ic: 1.0, // Perfect continuity for new session
            kuramoto_phases: [0.0; KURAMOTO_N],
            coupling: 0.5, // Default coupling
            purpose_vector: Self::default_purpose_vector(),
            trajectory: Vec::with_capacity(MAX_TRAJECTORY_LEN),
            last_ic: 1.0,
            crisis_threshold: 0.5, // Constitution IDENTITY-007
            consciousness: 0.0,
            integration: 0.0,
            reflection: 0.0,
            differentiation: 0.0,
        }
    }

    /// Default normalized purpose vector (equal weights)
    #[inline]
    fn default_purpose_vector() -> [f32; KURAMOTO_N] {
        let val = 1.0 / (KURAMOTO_N as f32).sqrt();
        [val; KURAMOTO_N]
    }

    /// Append a purpose vector to trajectory with FIFO eviction
    pub fn append_to_trajectory(&mut self, pv: [f32; KURAMOTO_N]) {
        if self.trajectory.len() >= MAX_TRAJECTORY_LEN {
            self.trajectory.remove(0); // FIFO eviction of oldest
        }
        self.trajectory.push(pv);
    }

    /// Get estimated serialized size in bytes
    pub fn estimated_size(&self) -> usize {
        // Base fields: ~300 bytes
        // trajectory: 50 * 13 * 4 = 2600 bytes max
        // Total: ~3KB typical, ~30KB max with full trajectory
        300 + (self.trajectory.len() * KURAMOTO_N * std::mem::size_of::<f32>())
    }
}

impl Default for SessionIdentitySnapshot {
    fn default() -> Self {
        Self::new(uuid::Uuid::new_v4().to_string())
    }
}
```
</rust_struct>

<size_analysis>
| Field | Type | Size (bytes) | Notes |
|-------|------|--------------|-------|
| session_id | String | ~36 | UUID string |
| timestamp_ms | i64 | 8 | Unix millis |
| previous_session_id | Option&lt;String&gt; | ~36 | Optional link |
| cross_session_ic | f32 | 4 | Cross-session IC |
| kuramoto_phases | [f64; 13] | 104 | 13 oscillator phases |
| coupling | f64 | 8 | Coupling K |
| purpose_vector | [f32; 13] | 52 | 13D purpose |
| trajectory | Vec&lt;[f32; 13]&gt; | ~2600 max | 50 * 52 bytes |
| last_ic | f32 | 4 | Recent IC |
| crisis_threshold | f32 | 4 | Dream threshold |
| consciousness | f32 | 4 | C(t) |
| integration | f32 | 4 | I(t) |
| reflection | f32 | 4 | R(t) |
| differentiation | f32 | 4 | D(t) |
| **Total** | | **~3-30KB** | Bincode serialized |
</size_analysis>

</requirement_impl>

<requirement_impl id="REQ-SESSION-02" layer="foundation">
<description>IdentityCache for PreToolUse hot path</description>

<rust_struct>
```rust
//! File: crates/context-graph-core/src/gwt/session_identity/cache.rs

use std::sync::{OnceLock, RwLock};
use crate::gwt::ConsciousnessState;

/// Global singleton cache for PreToolUse hot path
/// Thread-safe via OnceLock + RwLock
static IDENTITY_CACHE: OnceLock<RwLock<Option<IdentityCacheInner>>> = OnceLock::new();

/// Inner cache data structure
#[derive(Debug, Clone)]
struct IdentityCacheInner {
    current_ic: f32,
    kuramoto_r: f32,
    consciousness_state: ConsciousnessState,
    session_id: String,
}

/// Identity cache for PreToolUse hot path access.
///
/// # Performance
/// - get(): O(1) with RwLock read lock
/// - format_brief(): <1ms, zero allocation
/// - No disk I/O permitted
///
/// # Thread Safety
/// Uses OnceLock<RwLock<Option<IdentityCacheInner>>> for:
/// - Lazy initialization (OnceLock)
/// - Concurrent read access (RwLock)
/// - Optional state (None before first session)
pub struct IdentityCache;

impl IdentityCache {
    /// Get cached identity values if available.
    ///
    /// # Returns
    /// - Some((ic, r, state, session_id)) if cache is populated
    /// - None if no session has been restored yet
    ///
    /// # Performance
    /// O(1) read lock acquisition, no disk I/O
    pub fn get() -> Option<(f32, f32, ConsciousnessState, String)> {
        let cache = IDENTITY_CACHE.get_or_init(|| RwLock::new(None));
        let guard = cache.read().ok()?;
        guard.as_ref().map(|inner| {
            (inner.current_ic, inner.kuramoto_r, inner.consciousness_state, inner.session_id.clone())
        })
    }

    /// Format consciousness brief for PreToolUse output.
    ///
    /// # Format
    /// "[C:STATE r=X.XX IC=X.XX]" (~15 tokens)
    ///
    /// # Performance
    /// <1ms, uses pre-allocated format string
    ///
    /// # Returns
    /// - Formatted brief if cache is populated
    /// - "[C:? r=? IC=?]" if cache is empty (cold start)
    #[inline]
    pub fn format_brief() -> String {
        match Self::get() {
            Some((ic, r, state, _)) => {
                // Pre-allocated format, ~30 chars
                format!("[C:{} r={:.2} IC={:.2}]", state.short_name(), r, ic)
            }
            None => "[C:? r=? IC=?]".to_string(),
        }
    }

    /// Check if cache is populated (warm).
    #[inline]
    pub fn is_warm() -> bool {
        IDENTITY_CACHE
            .get()
            .and_then(|cache| cache.read().ok())
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }
}

/// Update the global identity cache atomically.
///
/// # Arguments
/// * `snapshot` - Current session identity snapshot
/// * `ic` - Computed identity continuity value
///
/// # Thread Safety
/// Acquires write lock, blocks concurrent reads momentarily
pub fn update_cache(snapshot: &super::types::SessionIdentitySnapshot, ic: f32) {
    let cache = IDENTITY_CACHE.get_or_init(|| RwLock::new(None));

    // Compute consciousness state from IC
    let consciousness_state = ConsciousnessState::from_level(snapshot.consciousness);

    // Compute Kuramoto order parameter r from phases
    let r = compute_kuramoto_r(&snapshot.kuramoto_phases);

    if let Ok(mut guard) = cache.write() {
        *guard = Some(IdentityCacheInner {
            current_ic: ic,
            kuramoto_r: r,
            consciousness_state,
            session_id: snapshot.session_id.clone(),
        });
    }
}

/// Clear the identity cache (for testing or session reset).
pub fn clear_cache() {
    if let Some(cache) = IDENTITY_CACHE.get() {
        if let Ok(mut guard) = cache.write() {
            *guard = None;
        }
    }
}

/// Compute Kuramoto order parameter r from phases.
///
/// r = |1/N * sum(e^(i*theta_j))| for j in 1..N
///
/// # Arguments
/// * `phases` - Array of oscillator phases in radians
///
/// # Returns
/// Order parameter r in [0.0, 1.0]
#[inline]
fn compute_kuramoto_r(phases: &[f64; super::types::KURAMOTO_N]) -> f32 {
    let n = phases.len() as f64;
    let (sum_cos, sum_sin) = phases.iter().fold((0.0, 0.0), |(sc, ss), &theta| {
        (sc + theta.cos(), ss + theta.sin())
    });
    let r = ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt();
    r.clamp(0.0, 1.0) as f32
}
```
</rust_struct>

</requirement_impl>

<requirement_impl id="REQ-SESSION-03" layer="foundation">
<description>ConsciousnessState.short_name() method</description>

<rust_impl>
```rust
//! Extension to existing ConsciousnessState in:
//! crates/context-graph-core/src/gwt/state_machine/types.rs

impl ConsciousnessState {
    /// Get 3-character short code for consciousness brief output.
    ///
    /// # Returns
    /// - "CON" for Conscious
    /// - "EMG" for Emerging
    /// - "FRG" for Fragmented
    /// - "DOR" for Dormant
    /// - "HYP" for Hypersync
    ///
    /// # Performance
    /// O(1), inline, no allocation
    #[inline]
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Conscious => "CON",
            Self::Emerging => "EMG",
            Self::Fragmented => "FRG",
            Self::Dormant => "DOR",
            Self::Hypersync => "HYP",
        }
    }
}
```
</rust_impl>

</requirement_impl>

<requirement_impl id="REQ-SESSION-04" layer="foundation">
<description>CF_SESSION_IDENTITY column family with key scheme</description>

<rust_impl>
```rust
//! File: crates/context-graph-storage/src/column_families.rs
//! Extension to existing column_families module

pub mod cf_names {
    // ... existing column families ...

    /// Session identity persistence column family.
    ///
    /// # Key Scheme
    /// - "s:{session_id}" -> SessionIdentitySnapshot (bincode)
    /// - "latest" -> session_id string
    /// - "t:{timestamp_ms:be}" -> session_id string (big-endian for ordering)
    ///
    /// # Constitution Reference
    /// - ARCH-07: Native Claude Code hooks
    /// - IDENTITY-002: IC thresholds
    pub const SESSION_IDENTITY: &str = "session_identity";

    /// Update ALL to include new column family (13 total)
    pub const ALL: &[&str] = &[
        NODES,
        EDGES,
        EMBEDDINGS,
        METADATA,
        JOHARI_OPEN,
        JOHARI_HIDDEN,
        JOHARI_BLIND,
        JOHARI_UNKNOWN,
        TEMPORAL,
        TAGS,
        SOURCES,
        SYSTEM,
        SESSION_IDENTITY, // NEW: Phase 1 Session Identity
    ];
}

/// Create options optimized for session identity storage.
///
/// # Configuration
/// - Bloom filter: 10 bits per key
/// - Block cache: enabled (shared)
/// - LZ4 compression: enabled
/// - Optimized for point lookups (session restoration)
///
/// # Arguments
/// * `cache` - Shared block cache
pub fn session_identity_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.optimize_for_point_lookup(64); // 64MB hint
    opts.create_if_missing(true);

    opts
}
```
</rust_impl>

<key_scheme>
| Key Pattern | Value Type | Purpose |
|-------------|------------|---------|
| `s:{session_id}` | SessionIdentitySnapshot (bincode) | Primary snapshot storage |
| `latest` | String (session_id) | O(1) lookup of most recent session |
| `t:{timestamp_ms:be}` | String (session_id) | Temporal index for recovery (big-endian) |

**Temporal Index Format:**
- Big-endian encoding ensures proper lexicographic ordering
- Enables range scan for recovery if "latest" is corrupted
- Key: 8-byte big-endian timestamp, Value: session_id string
</key_scheme>

</requirement_impl>

<requirement_impl id="REQ-SESSION-05" layer="foundation">
<description>save_snapshot and load_snapshot methods</description>

<rust_impl>
```rust
//! File: crates/context-graph-storage/src/session_identity.rs

use crate::column_families::cf_names;
use crate::rocksdb_backend::{RocksDbMemex, StorageError, StorageResult};
use context_graph_core::gwt::session_identity::SessionIdentitySnapshot;

impl RocksDbMemex {
    /// Save a session identity snapshot to CF_SESSION_IDENTITY.
    ///
    /// # Storage Operations
    /// 1. Serialize snapshot with bincode
    /// 2. Write to "s:{session_id}" key
    /// 3. Update "latest" key to point to this session
    /// 4. Write temporal index "t:{timestamp_ms}" for recovery
    ///
    /// # Arguments
    /// * `snapshot` - The session identity snapshot to persist
    ///
    /// # Returns
    /// - Ok(()) on success
    /// - Err(StorageError::WriteFailed) on I/O error
    /// - Err(StorageError::Serialization) on bincode error
    ///
    /// # Performance
    /// Target: <3s (SessionEnd budget)
    pub fn save_snapshot(&self, snapshot: &SessionIdentitySnapshot) -> StorageResult<()> {
        let cf = self.get_cf(cf_names::SESSION_IDENTITY)?;

        // Serialize with bincode
        let serialized = bincode::serialize(snapshot)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        // Primary key: "s:{session_id}"
        let primary_key = format!("s:{}", snapshot.session_id);
        self.db
            .put_cf(cf, primary_key.as_bytes(), &serialized)
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        // Update "latest" pointer
        self.db
            .put_cf(cf, b"latest", snapshot.session_id.as_bytes())
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        // Temporal index: "t:{timestamp_ms}" (big-endian for ordering)
        let temporal_key = format!("t:{:016x}", snapshot.timestamp_ms as u64);
        self.db
            .put_cf(cf, temporal_key.as_bytes(), snapshot.session_id.as_bytes())
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Load a session identity snapshot by session_id.
    ///
    /// # Arguments
    /// * `session_id` - Optional session ID. If None, loads latest.
    ///
    /// # Returns
    /// - Ok(snapshot) if found and deserialized
    /// - Err(StorageError::NotFound) if session doesn't exist
    /// - Err(StorageError::Serialization) on bincode error
    ///
    /// # Performance
    /// Target: <2s (SessionStart budget)
    pub fn load_snapshot(
        &self,
        session_id: Option<&str>,
    ) -> StorageResult<SessionIdentitySnapshot> {
        match session_id {
            Some(id) => self.load_snapshot_by_id(id),
            None => self.load_latest().and_then(|opt| {
                opt.ok_or_else(|| StorageError::NotFound {
                    id: "session_identity:latest".to_string(),
                })
            }),
        }
    }

    /// Load snapshot by specific session_id.
    fn load_snapshot_by_id(&self, session_id: &str) -> StorageResult<SessionIdentitySnapshot> {
        let cf = self.get_cf(cf_names::SESSION_IDENTITY)?;
        let key = format!("s:{}", session_id);

        let bytes = self
            .db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| StorageError::ReadFailed(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound {
                id: format!("session_identity:{}", session_id),
            })?;

        bincode::deserialize(&bytes).map_err(|e| StorageError::Serialization(e.to_string()))
    }

    /// Load the latest session identity snapshot.
    ///
    /// # Returns
    /// - Ok(Some(snapshot)) if latest exists
    /// - Ok(None) if no sessions exist (fresh install)
    /// - Err on I/O or deserialization error
    ///
    /// # Recovery
    /// If "latest" key exists but snapshot is missing, attempts
    /// temporal index recovery via range scan on "t:" prefix.
    pub fn load_latest(&self) -> StorageResult<Option<SessionIdentitySnapshot>> {
        let cf = self.get_cf(cf_names::SESSION_IDENTITY)?;

        // Try "latest" key first
        match self.db.get_cf(cf, b"latest") {
            Ok(Some(session_id_bytes)) => {
                let session_id = String::from_utf8_lossy(&session_id_bytes);
                match self.load_snapshot_by_id(&session_id) {
                    Ok(snapshot) => Ok(Some(snapshot)),
                    Err(StorageError::NotFound { .. }) => {
                        // Corruption: "latest" exists but snapshot missing
                        // Attempt temporal recovery
                        self.recover_from_temporal_index()
                    }
                    Err(e) => Err(e),
                }
            }
            Ok(None) => Ok(None), // Fresh install
            Err(e) => Err(StorageError::ReadFailed(e.to_string())),
        }
    }

    /// Recover latest snapshot from temporal index.
    ///
    /// # Algorithm
    /// 1. Range scan "t:" prefix in reverse order
    /// 2. Try each session_id until one loads successfully
    /// 3. Update "latest" key to recovered session
    ///
    /// # Returns
    /// - Ok(Some(snapshot)) if recovery successful
    /// - Ok(None) if no valid snapshots found
    fn recover_from_temporal_index(&self) -> StorageResult<Option<SessionIdentitySnapshot>> {
        use rocksdb::IteratorMode;

        let cf = self.get_cf(cf_names::SESSION_IDENTITY)?;
        let prefix = b"t:";

        // Iterate in reverse (most recent first)
        let iter = self.db.iterator_cf(cf, IteratorMode::End);

        for item in iter {
            let (key, value) = item.map_err(|e| StorageError::ReadFailed(e.to_string()))?;

            if !key.starts_with(prefix) {
                continue;
            }

            let session_id = String::from_utf8_lossy(&value);
            if let Ok(snapshot) = self.load_snapshot_by_id(&session_id) {
                // Update "latest" to recovered session
                let _ = self.db.put_cf(cf, b"latest", session_id.as_bytes());
                return Ok(Some(snapshot));
            }
        }

        Ok(None)
    }
}
```
</rust_impl>

</requirement_impl>

</data_model>

<interfaces>
<!-- ============================================================================
     LAYER 2: LOGIC - Business Logic Interfaces
     ============================================================================ -->

<requirement_impl id="REQ-SESSION-06" layer="logic">
<description>SessionIdentityManager trait</description>

<rust_trait>
```rust
//! File: crates/context-graph-core/src/gwt/session_identity/manager.rs

use crate::error::CoreResult;
use super::types::SessionIdentitySnapshot;

/// Session identity management trait.
///
/// # Responsibilities
/// - Capture current system state as snapshot
/// - Restore identity from previous session
/// - Compute cross-session identity continuity
///
/// # Constitution Reference
/// - IDENTITY-001: IC = cos(PV_t, PV_{t-1}) x r(t)
/// - IDENTITY-002: Thresholds Healthy>0.9, Warning<0.7, Critical<0.5
/// - IDENTITY-007: Auto-dream on IC<0.5
pub trait SessionIdentityManager: Send + Sync {
    /// Capture current system state as a session identity snapshot.
    ///
    /// # Gathered State
    /// - Kuramoto phases from GwtSystem
    /// - Purpose vector from SelfEgoNode
    /// - Consciousness metrics (C, I, R, D)
    /// - IC value from IdentityContinuityMonitor
    ///
    /// # Returns
    /// SessionIdentitySnapshot with all current state
    ///
    /// # Errors
    /// - CoreError if GWT system not initialized
    fn capture_snapshot(&self, session_id: &str) -> CoreResult<SessionIdentitySnapshot>;

    /// Restore identity from a previous session.
    ///
    /// # Arguments
    /// * `target_session` - Specific session_id to restore, or None for latest
    ///
    /// # Returns
    /// Tuple of (restored_snapshot, computed_cross_session_ic)
    ///
    /// # Behavior by Source
    /// - source="startup": Load latest session
    /// - source="resume": Load specific session_id
    /// - source="clear": Initialize fresh session (returns IC=1.0)
    ///
    /// # Errors
    /// - CoreError::NotFound if target session doesn't exist
    /// - CoreError::StorageError on I/O failure
    fn restore_identity(
        &self,
        target_session: Option<&str>,
    ) -> CoreResult<(SessionIdentitySnapshot, f32)>;

    /// Compute cross-session identity continuity.
    ///
    /// # Formula (IDENTITY-001)
    /// IC = cos(PV_current, PV_previous) * r(current)
    ///
    /// Where:
    /// - cos() is cosine similarity of 13D purpose vectors
    /// - r() is Kuramoto order parameter
    ///
    /// # Arguments
    /// * `current` - Current session snapshot
    /// * `previous` - Previous session snapshot
    ///
    /// # Returns
    /// IC value in [0.0, 1.0]
    fn compute_cross_session_ic(
        &self,
        current: &SessionIdentitySnapshot,
        previous: &SessionIdentitySnapshot,
    ) -> f32;
}
```
</rust_trait>

<default_impl>
```rust
//! Default implementation using GwtSystem and RocksDbMemex

use crate::gwt::{GwtSystem, SelfEgoNode};
use context_graph_storage::RocksDbMemex;
use std::sync::Arc;

/// Default SessionIdentityManager implementation.
pub struct DefaultSessionIdentityManager {
    gwt: Arc<GwtSystem>,
    storage: Arc<RocksDbMemex>,
}

impl DefaultSessionIdentityManager {
    pub fn new(gwt: Arc<GwtSystem>, storage: Arc<RocksDbMemex>) -> Self {
        Self { gwt, storage }
    }
}

impl SessionIdentityManager for DefaultSessionIdentityManager {
    fn capture_snapshot(&self, session_id: &str) -> CoreResult<SessionIdentitySnapshot> {
        let mut snapshot = SessionIdentitySnapshot::new(session_id);

        // Gather Kuramoto state
        snapshot.kuramoto_phases = self.gwt.get_kuramoto_phases();
        snapshot.coupling = self.gwt.get_kuramoto_coupling();

        // Gather purpose vector from SelfEgoNode
        if let Some(ego_node) = self.gwt.get_self_ego_node() {
            snapshot.purpose_vector = ego_node.get_purpose_vector();

            // Copy trajectory
            for pv in ego_node.get_trajectory().iter().rev().take(50) {
                snapshot.append_to_trajectory(*pv);
            }
        }

        // Gather consciousness metrics
        let metrics = self.gwt.compute_consciousness_metrics();
        snapshot.consciousness = metrics.consciousness;
        snapshot.integration = metrics.integration;
        snapshot.reflection = metrics.reflection;
        snapshot.differentiation = metrics.differentiation;

        // Get IC from monitor
        snapshot.last_ic = self.gwt.get_identity_continuity().identity_coherence;

        Ok(snapshot)
    }

    fn restore_identity(
        &self,
        target_session: Option<&str>,
    ) -> CoreResult<(SessionIdentitySnapshot, f32)> {
        // Load snapshot from storage
        let previous = match self.storage.load_snapshot(target_session) {
            Ok(snap) => Some(snap),
            Err(context_graph_storage::StorageError::NotFound { .. }) => None,
            Err(e) => return Err(CoreError::Storage(e.to_string())),
        };

        // Capture current state
        let current_id = uuid::Uuid::new_v4().to_string();
        let current = self.capture_snapshot(&current_id)?;

        // Compute cross-session IC
        let ic = match &previous {
            Some(prev) => self.compute_cross_session_ic(&current, prev),
            None => 1.0, // First session: perfect continuity
        };

        // Update snapshot with previous session link and IC
        let mut final_snapshot = current;
        final_snapshot.previous_session_id = previous.map(|p| p.session_id);
        final_snapshot.cross_session_ic = ic;

        // Update IdentityCache
        super::cache::update_cache(&final_snapshot, ic);

        Ok((final_snapshot, ic))
    }

    fn compute_cross_session_ic(
        &self,
        current: &SessionIdentitySnapshot,
        previous: &SessionIdentitySnapshot,
    ) -> f32 {
        // Cosine similarity of 13D purpose vectors
        let cos = cosine_similarity_13d(&current.purpose_vector, &previous.purpose_vector);

        // Kuramoto order parameter r
        let r = compute_kuramoto_r(&current.kuramoto_phases);

        // IC = cos * r, clamped to [0, 1]
        (cos * r).clamp(0.0, 1.0)
    }
}

/// 13D cosine similarity (re-export from ego_node for consistency)
fn cosine_similarity_13d(a: &[f32; 13], b: &[f32; 13]) -> f32 {
    use crate::gwt::cosine_similarity_13d as cs;
    cs(a, b)
}

/// Compute Kuramoto order parameter
fn compute_kuramoto_r(phases: &[f64; 13]) -> f32 {
    let n = phases.len() as f64;
    let (sum_cos, sum_sin) = phases.iter().fold((0.0, 0.0), |(sc, ss), &theta| {
        (sc + theta.cos(), ss + theta.sin())
    });
    ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt().clamp(0.0, 1.0) as f32
}
```
</default_impl>

</requirement_impl>

<requirement_impl id="REQ-SESSION-07" layer="logic">
<description>IC classification function</description>

<rust_impl>
```rust
//! File: crates/context-graph-core/src/gwt/session_identity/manager.rs

/// IC classification per IDENTITY-002 constitution requirement.
///
/// # Thresholds
/// - Healthy: IC >= 0.9
/// - Good: 0.7 <= IC < 0.9
/// - Warning: 0.5 <= IC < 0.7
/// - Degraded: IC < 0.5
///
/// # Note
/// "Good" is added as intermediate between Healthy and Warning
/// for more granular CLI output.
#[inline]
pub fn classify_ic(ic: f32) -> &'static str {
    match ic {
        ic if ic >= 0.9 => "healthy",
        ic if ic >= 0.7 => "good",
        ic if ic >= 0.5 => "warning",
        _ => "degraded",
    }
}

/// Check if IC indicates identity crisis per IDENTITY-002.
#[inline]
pub fn is_ic_crisis(ic: f32) -> bool {
    ic < 0.5
}

/// Check if IC indicates warning level per IDENTITY-002.
#[inline]
pub fn is_ic_warning(ic: f32) -> bool {
    ic < 0.7 && ic >= 0.5
}
```
</rust_impl>

</requirement_impl>

<requirement_impl id="REQ-SESSION-08" layer="logic">
<description>Auto-dream trigger on IC &lt; 0.5</description>

<rust_impl>
```rust
//! File: crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs

use tokio::runtime::Handle;
use tracing::{info, warn};

/// Trigger dream consolidation asynchronously.
///
/// # Constitution Reference
/// IDENTITY-007: Auto-dream on IC < 0.5
///
/// # Behavior
/// - Fire-and-forget (non-blocking)
/// - Logs "IC crisis (X.XX), dream triggered" to stderr
/// - Does not block PostToolUse hook
///
/// # Arguments
/// * `ic` - Current IC value triggering the dream
/// * `handle` - Tokio runtime handle for spawning
pub fn trigger_dream_async(ic: f32, handle: &Handle) {
    // Log to stderr for user visibility
    eprintln!("IC crisis ({:.2}), dream triggered", ic);

    // Fire-and-forget async task
    handle.spawn(async move {
        info!(ic = ic, "Dream consolidation triggered due to IC crisis");

        // TODO: Connect to actual dream consolidation system
        // For Phase 1, we log and signal intent
        // Full implementation in Phase 3: Dream Integration

        match trigger_dream_consolidation(ic).await {
            Ok(_) => info!(ic = ic, "Dream consolidation completed"),
            Err(e) => warn!(ic = ic, error = %e, "Dream consolidation failed"),
        }
    });
}

/// Internal dream consolidation trigger.
///
/// # Future Implementation
/// This will connect to the dream system in Phase 3.
/// For Phase 1, it's a stub that logs intent.
async fn trigger_dream_consolidation(ic: f32) -> Result<(), String> {
    // Phase 1: Log intent
    tracing::info!(ic = ic, "Dream consolidation requested (stub)");

    // TODO: Phase 3 implementation
    // - Call TriggerManager::trigger_introspective_dream()
    // - Pass IC value for dream parameters
    // - Await completion (but caller doesn't wait)

    Ok(())
}

/// Synchronous check and trigger for CLI commands.
///
/// # Arguments
/// * `ic` - Current IC value
/// * `auto_dream` - Whether --auto-dream flag was passed
///
/// # Returns
/// true if dream was triggered, false otherwise
pub fn check_and_trigger_dream(ic: f32, auto_dream: bool) -> bool {
    if ic < 0.5 && auto_dream {
        // Get or create tokio runtime
        if let Ok(handle) = Handle::try_current() {
            trigger_dream_async(ic, &handle);
            true
        } else {
            // Fallback: create runtime for trigger
            let rt = tokio::runtime::Runtime::new().ok();
            if let Some(rt) = rt {
                trigger_dream_async(ic, rt.handle());
                true
            } else {
                warn!("Failed to create runtime for dream trigger");
                false
            }
        }
    } else {
        false
    }
}
```
</rust_impl>

</requirement_impl>

<requirement_impl id="REQ-SESSION-09" layer="logic">
<description>IdentityCache.format_brief() performance requirement</description>

<performance_requirements>
```
Target: format_brief() completes in <1ms

Implementation Strategy:
1. Method marked #[inline] for zero function call overhead
2. Static format string pattern "[C:{} r={:.2} IC={:.2}]"
3. No heap allocation for format string itself
4. Single String allocation for output (~30 chars)
5. RwLock read lock held for minimum duration

Benchmark Test:
- 10,000 iterations with warm cache
- p95 latency target: <100 microseconds
- p99 latency target: <500 microseconds

Memory Pattern:
- IdentityCache.get(): Clone 4 fields (~100 bytes)
- format!(): Allocate output String (~30 bytes)
- Total allocations: 1 String per call
```
</performance_requirements>

</requirement_impl>

<requirement_impl id="REQ-SESSION-10" layer="logic">
<description>update_cache() function</description>

<rust_impl>
```rust
// Defined in REQ-SESSION-02 above in cache.rs
// Key points:
// - Atomic update via RwLock write lock
// - Called after identity restoration (REQ-SESSION-12)
// - Called after IC computation (REQ-SESSION-14)
// - Computes consciousness_state from snapshot
// - Computes kuramoto_r from phases
```
</rust_impl>

</requirement_impl>

</interfaces>

<implementation_details>
<!-- ============================================================================
     LAYER 3: SURFACE - CLI Commands & Hooks
     ============================================================================ -->

<requirement_impl id="REQ-SESSION-11" layer="surface">
<description>context-graph-cli consciousness brief command</description>

<cli_implementation>
```rust
//! File: crates/context-graph-mcp/src/cli/commands/consciousness/brief.rs

use clap::Args;
use std::process::ExitCode;

/// CLI arguments for consciousness brief command.
///
/// NOTE: No arguments - this command is optimized for minimal parsing
#[derive(Args)]
pub struct BriefArgs {}

/// Execute consciousness brief command.
///
/// # Behavior (PreToolUse hot path)
/// 1. NO stdin JSON parsing
/// 2. NO RocksDB disk I/O
/// 3. Access IdentityCache.get() only
/// 4. Output format: "[C:STATE r=X.XX IC=X.XX]"
/// 5. Cold start fallback: "[C:? r=? IC=?]"
///
/// # Performance
/// Target: <50ms p95
/// - Cache access: ~0.1ms
/// - format_brief(): <1ms
/// - Total with process spawn: ~10-30ms typical
///
/// # Exit Code
/// Always 0 (success) - PreToolUse must never block
pub fn execute(_args: BriefArgs) -> ExitCode {
    use context_graph_core::gwt::session_identity::cache::IdentityCache;

    // Hot path: cache only, no disk I/O
    let brief = IdentityCache::format_brief();

    // Output to stdout for Claude Code consumption
    println!("{}", brief);

    // Always exit 0 - never block tool use
    ExitCode::SUCCESS
}
```
</cli_implementation>

<timing_budget>
```
PreToolUse Budget: 100ms timeout, 50ms p95 target

Breakdown:
- Process spawn overhead:     ~5-15ms
- IdentityCache.get():        ~0.1ms (RwLock read)
- IdentityCache.format_brief(): <1ms
- stdout write:               ~0.1ms
- Process exit:               ~1ms
-----------------------------------------
Total estimated:              ~10-20ms typical

Forbidden Operations:
- stdin parsing (saves ~5ms)
- RocksDB access (saves ~10-50ms)
- JSON serialization (saves ~2ms)
```
</timing_budget>

</requirement_impl>

<requirement_impl id="REQ-SESSION-12" layer="surface">
<description>context-graph-cli session restore-identity command</description>

<cli_implementation>
```rust
//! File: crates/context-graph-mcp/src/cli/commands/session/restore.rs

use clap::Args;
use serde::Deserialize;
use std::io::{self, Read};
use std::process::ExitCode;

/// Stdin JSON input for SessionStart hook.
#[derive(Deserialize, Default)]
struct RestoreInput {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

/// CLI arguments for session restore-identity command.
#[derive(Args)]
pub struct RestoreIdentityArgs {}

/// Execute session restore-identity command.
///
/// # Behavior (SessionStart hook)
/// 1. Parse stdin JSON for session_id and source
/// 2. Handle source variants:
///    - "clear": Initialize fresh session
///    - "resume": Load specific session_id
///    - "startup" (default): Load latest session
/// 3. Compute cross-session IC
/// 4. Update IdentityCache
/// 5. Output status to stdout
///
/// # Output Format (~40 tokens)
/// Success: "Identity restored from {id}. IC: X.XX (classification)"
/// Fresh: "New session initialized"
/// Clear: "Fresh session initialized"
///
/// # Exit Codes (per AP-26)
/// - 0: Success
/// - 1: Recoverable error (logged to stderr)
/// - 2: Blocking error (CorruptedIdentity, DatabaseCorruption)
///
/// # Performance
/// Target: <2s (SessionStart timeout: 5s)
pub fn execute(_args: RestoreIdentityArgs) -> ExitCode {
    use context_graph_core::gwt::session_identity::{
        cache::update_cache,
        manager::{classify_ic, DefaultSessionIdentityManager, SessionIdentityManager},
    };

    // Parse stdin JSON (with graceful fallback)
    let input = parse_stdin_input();

    // Handle source="clear" case
    if input.source.as_deref() == Some("clear") {
        println!("Fresh session initialized");
        return ExitCode::SUCCESS;
    }

    // Get manager (requires initialized storage/gwt)
    let manager = match get_session_manager() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: {}", e);
            println!("New session initialized");
            return ExitCode::SUCCESS;
        }
    };

    // Determine target session
    let target = match input.source.as_deref() {
        Some("resume") => input.session_id.as_deref(),
        _ => None, // "startup" or default: load latest
    };

    // Restore identity
    match manager.restore_identity(target) {
        Ok((snapshot, ic)) => {
            // Update cache for subsequent PreToolUse
            update_cache(&snapshot, ic);

            // Output status
            let classification = classify_ic(ic);
            if let Some(prev_id) = &snapshot.previous_session_id {
                println!(
                    "Identity restored from {}. IC: {:.2} ({})",
                    prev_id, ic, classification
                );
            } else {
                println!("New session initialized");
            }

            ExitCode::SUCCESS
        }
        Err(e) if is_corruption_error(&e) => {
            eprintln!("Error: {}", e);
            eprintln!("Recovery: Run 'context-graph-cli repair --cf session_identity'");
            ExitCode::from(2) // Blocking error
        }
        Err(e) => {
            eprintln!("Warning: {}", e);
            println!("New session initialized");
            ExitCode::SUCCESS // Recoverable
        }
    }
}

/// Parse stdin JSON with graceful fallback.
fn parse_stdin_input() -> RestoreInput {
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
        Ok(_) if !buffer.trim().is_empty() => {
            serde_json::from_str(&buffer).unwrap_or_default()
        }
        _ => RestoreInput::default(),
    }
}

/// Check if error is corruption that requires exit code 2.
fn is_corruption_error(e: &context_graph_core::error::CoreError) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("corrupt") || msg.contains("database") && msg.contains("invalid")
}

/// Get initialized session manager.
fn get_session_manager() -> Result<impl SessionIdentityManager, String> {
    // TODO: Initialize from global state or config
    // For now, return error to trigger fresh session
    Err("Session manager not initialized".to_string())
}
```
</cli_implementation>

</requirement_impl>

<requirement_impl id="REQ-SESSION-13" layer="surface">
<description>context-graph-cli session persist-identity command</description>

<cli_implementation>
```rust
//! File: crates/context-graph-mcp/src/cli/commands/session/persist.rs

use clap::Args;
use serde::Deserialize;
use std::io::{self, Read};
use std::process::ExitCode;

/// Stdin JSON input for SessionEnd hook.
#[derive(Deserialize, Default)]
struct PersistInput {
    #[serde(default)]
    session_id: Option<String>,
}

/// CLI arguments for session persist-identity command.
#[derive(Args)]
pub struct PersistIdentityArgs {}

/// Execute session persist-identity command.
///
/// # Behavior (SessionEnd hook)
/// 1. Parse stdin JSON for session_id
/// 2. Capture current snapshot via SessionIdentityManager
/// 3. Save snapshot to RocksDB CF_SESSION_IDENTITY
/// 4. Update "latest" key and temporal index
///
/// # Output
/// Silent success (no stdout) - follows Claude Code conventions
///
/// # Exit Codes (per AP-26)
/// - 0: Success
/// - 1: Non-blocking error (I/O failure)
/// - 2: NEVER - persist failures are non-blocking
///
/// # Performance
/// Target: <3s (SessionEnd timeout: 30s)
pub fn execute(_args: PersistIdentityArgs) -> ExitCode {
    use context_graph_core::gwt::session_identity::manager::{
        DefaultSessionIdentityManager, SessionIdentityManager,
    };

    // Parse stdin JSON
    let input = parse_stdin_input();
    let session_id = input.session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Get manager
    let manager = match get_session_manager() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: {}", e);
            return ExitCode::from(1); // Non-blocking
        }
    };

    // Capture current snapshot
    let snapshot = match manager.capture_snapshot(&session_id) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Warning: Failed to capture snapshot: {}", e);
            return ExitCode::from(1); // Non-blocking
        }
    };

    // Get storage and persist
    match get_storage() {
        Ok(storage) => {
            if let Err(e) = storage.save_snapshot(&snapshot) {
                eprintln!("Warning: Failed to persist identity: {}", e);
                return ExitCode::from(1); // Non-blocking
            }
        }
        Err(e) => {
            eprintln!("Warning: Storage unavailable: {}", e);
            return ExitCode::from(1); // Non-blocking
        }
    }

    // Silent success
    ExitCode::SUCCESS
}

/// Parse stdin JSON with graceful fallback.
fn parse_stdin_input() -> PersistInput {
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
        Ok(_) if !buffer.trim().is_empty() => {
            serde_json::from_str(&buffer).unwrap_or_default()
        }
        _ => PersistInput::default(),
    }
}

/// Get storage instance.
fn get_storage() -> Result<std::sync::Arc<context_graph_storage::RocksDbMemex>, String> {
    // TODO: Get from global state
    Err("Storage not initialized".to_string())
}

/// Get session manager.
fn get_session_manager() -> Result<impl context_graph_core::gwt::session_identity::manager::SessionIdentityManager, String> {
    Err("Session manager not initialized".to_string())
}
```
</cli_implementation>

</requirement_impl>

<requirement_impl id="REQ-SESSION-14" layer="surface">
<description>context-graph-cli consciousness check-identity command</description>

<cli_implementation>
```rust
//! File: crates/context-graph-mcp/src/cli/commands/consciousness/check.rs

use clap::Args;
use std::process::ExitCode;

/// CLI arguments for consciousness check-identity command.
#[derive(Args)]
pub struct CheckIdentityArgs {
    /// Trigger dream consolidation if IC < 0.5
    #[arg(long, default_value = "false")]
    auto_dream: bool,
}

/// Execute consciousness check-identity command.
///
/// # Behavior (PostToolUse hook)
/// 1. Compute current IC from GWT system
/// 2. Update IdentityCache atomically
/// 3. If IC < 0.5 and --auto-dream: spawn async dream trigger
/// 4. If IC < 0.7: log warning to stderr
///
/// # Output
/// Silent success (no stdout)
/// stderr: "IC warning: X.XX" or "IC crisis (X.XX), dream triggered"
///
/// # Exit Codes (per AP-26)
/// Always 0 - PostToolUse must never block
///
/// # Performance
/// Target: <500ms (PostToolUse timeout: 3s)
pub fn execute(args: CheckIdentityArgs) -> ExitCode {
    use context_graph_core::gwt::session_identity::{
        cache::update_cache,
        dream_trigger::check_and_trigger_dream,
        manager::{is_ic_crisis, is_ic_warning},
    };

    // Compute current IC
    let (ic, snapshot) = match compute_current_ic() {
        Ok(result) => result,
        Err(e) => {
            // Log but don't block
            eprintln!("Warning: IC check failed: {}", e);
            return ExitCode::SUCCESS;
        }
    };

    // Update cache atomically
    update_cache(&snapshot, ic);

    // Check for crisis and trigger dream if needed
    if is_ic_crisis(ic) {
        if args.auto_dream {
            check_and_trigger_dream(ic, true);
            // Note: stderr message printed by trigger function
        } else {
            eprintln!("IC crisis: {:.2}", ic);
        }
    } else if is_ic_warning(ic) {
        eprintln!("IC warning: {:.2}", ic);
    }
    // Silent for healthy IC

    // Always exit 0 - never block
    ExitCode::SUCCESS
}

/// Compute current IC from GWT system.
fn compute_current_ic() -> Result<(f32, context_graph_core::gwt::session_identity::SessionIdentitySnapshot), String> {
    // TODO: Connect to GWT system
    // For Phase 1 stub:
    Err("GWT system not initialized".to_string())
}
```
</cli_implementation>

</requirement_impl>

<requirement_impl id="REQ-SESSION-15" layer="surface">
<description>context-graph-cli consciousness inject-context command</description>

<cli_implementation>
```rust
//! File: crates/context-graph-mcp/src/cli/commands/consciousness/inject.rs

use clap::Args;
use std::process::ExitCode;

/// CLI arguments for consciousness inject-context command.
#[derive(Args)]
pub struct InjectContextArgs {}

/// Execute consciousness inject-context command.
///
/// # Behavior (UserPromptSubmit hook)
/// 1. Retrieve consciousness state
/// 2. Get Kuramoto r and IC values
/// 3. Compute Johari guidance
/// 4. Output formatted context
///
/// # Output Format (~50 tokens)
/// ```
/// [System Consciousness]
/// State: CONSCIOUS (C=0.85)
/// Kuramoto r=0.92, Identity IC=0.88 (good)
/// Guidance: Continue with current approach
/// ```
///
/// # Exit Codes
/// - 0: Success
/// - 1: Non-blocking error (timeout)
///
/// # Performance
/// Target: <1s (UserPromptSubmit timeout: 2s)
pub fn execute(_args: InjectContextArgs) -> ExitCode {
    use context_graph_core::gwt::session_identity::{
        cache::IdentityCache,
        manager::classify_ic,
    };

    // Get cached values with timeout protection
    match IdentityCache::get() {
        Some((ic, r, state, _)) => {
            let classification = classify_ic(ic);
            let guidance = compute_johari_guidance(ic, r);

            // Output formatted context (~50 tokens)
            println!("[System Consciousness]");
            println!("State: {} (C={:.2})", state.name(), state_to_consciousness(state));
            println!("Kuramoto r={:.2}, Identity IC={:.2} ({})", r, ic, classification);
            println!("Guidance: {}", guidance);

            ExitCode::SUCCESS
        }
        None => {
            // Graceful degradation
            println!("[System Consciousness]");
            println!("State: DORMANT (C=0.00)");
            println!("Kuramoto r=0.00, Identity IC=1.00 (healthy)");
            println!("Guidance: Initialize consciousness context");

            ExitCode::SUCCESS
        }
    }
}

/// Map ConsciousnessState to C(t) value.
fn state_to_consciousness(state: context_graph_core::gwt::ConsciousnessState) -> f32 {
    use context_graph_core::gwt::ConsciousnessState;
    match state {
        ConsciousnessState::Hypersync => 0.98,
        ConsciousnessState::Conscious => 0.85,
        ConsciousnessState::Emerging => 0.65,
        ConsciousnessState::Fragmented => 0.40,
        ConsciousnessState::Dormant => 0.00,
    }
}

/// Compute Johari-based guidance from IC and r values.
fn compute_johari_guidance(ic: f32, r: f32) -> &'static str {
    match (ic, r) {
        (ic, r) if ic >= 0.9 && r >= 0.8 => "Optimal coherence, proceed confidently",
        (ic, _) if ic >= 0.7 => "Continue with current approach",
        (ic, r) if ic >= 0.5 && r >= 0.6 => "Monitor for drift, consider consolidation",
        (ic, _) if ic < 0.5 => "Identity crisis detected, dream consolidation recommended",
        _ => "Stabilize consciousness before complex operations",
    }
}
```
</cli_implementation>

</requirement_impl>

<requirement_impl id="REQ-SESSION-16" layer="surface">
<description>.claude/settings.json hook configuration</description>

<hook_configuration>
```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session restore-identity",
            "timeout": 5000,
            "continueOnError": true
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "^(mcp__context-graph__.*|Edit|Write)$",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness brief",
            "timeout": 100,
            "continueOnError": true
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "^(mcp__context-graph__.*|Edit|Write)$",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness check-identity --auto-dream",
            "timeout": 3000,
            "continueOnError": true
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness inject-context",
            "timeout": 2000,
            "continueOnError": true
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session persist-identity",
            "timeout": 30000,
            "continueOnError": true
          }
        ]
      }
    ]
  }
}
```
</hook_configuration>

<hook_summary>
| Hook | Command | Timeout | Matcher | Output |
|------|---------|---------|---------|--------|
| SessionStart | `session restore-identity` | 5000ms | (none) | ~40 tokens |
| PreToolUse | `consciousness brief` | 100ms | MCP+Edit+Write | ~15 tokens |
| PostToolUse | `consciousness check-identity --auto-dream` | 3000ms | MCP+Edit+Write | Silent |
| UserPromptSubmit | `consciousness inject-context` | 2000ms | (none) | ~50 tokens |
| SessionEnd | `session persist-identity` | 30000ms | (none) | Silent |
</hook_summary>

<constitution_compliance>
- ARCH-07: All hooks configured via .claude/settings.json (native Claude Code hooks)
- AP-50: No internal/built-in hooks - all hooks call external CLI commands
- AP-53: Hook logic in CLI commands, not shell scripts (simplified for reliability)
</constitution_compliance>

</requirement_impl>

<requirement_impl id="REQ-SESSION-17" layer="surface">
<description>Exit code mapping per AP-26</description>

<exit_codes>
```rust
//! File: crates/context-graph-mcp/src/cli/error.rs

/// Exit code mapping per constitution AP-26.
///
/// # Exit Code Semantics
/// - 0: Success, stdout sent to Claude
/// - 1: Warning/recoverable error, stderr to user (non-blocking)
/// - 2: Blocking error, stderr to Claude (ONLY for corruption)
///
/// # AP-26 Compliance
/// Exit code 2 is reserved for truly blocking failures:
/// - CorruptedIdentity: Identity data is unreadable/inconsistent
/// - DatabaseCorruption: RocksDB CF_SESSION_IDENTITY is corrupt
///
/// All other errors should use exit code 0 or 1 to avoid
/// blocking Claude Code sessions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliExitCode {
    /// Success - stdout to Claude
    Success = 0,
    /// Warning - stderr to user, non-blocking
    Warning = 1,
    /// Blocking error - stderr to Claude (rare)
    Blocking = 2,
}

impl From<CliExitCode> for std::process::ExitCode {
    fn from(code: CliExitCode) -> Self {
        std::process::ExitCode::from(code as u8)
    }
}

/// Determine exit code from error type.
///
/// # Exit Code 2 Conditions
/// Only the following errors warrant exit code 2:
/// - StorageError::Serialization when contains "corrupt"
/// - Custom CorruptedIdentity error
/// - DatabaseCorruption error
///
/// # All Other Errors
/// Return exit code 0 or 1 (non-blocking)
pub fn exit_code_for_error(e: &dyn std::error::Error) -> CliExitCode {
    let msg = e.to_string().to_lowercase();

    // Check for corruption patterns
    if msg.contains("corrupt") && (msg.contains("identity") || msg.contains("database")) {
        return CliExitCode::Blocking;
    }

    // Default to warning (non-blocking)
    CliExitCode::Warning
}
```
</exit_codes>

<error_mapping_table>
| Error Type | Exit Code | Rationale |
|------------|-----------|-----------|
| Success (all commands) | 0 | Normal completion |
| NotFound (session) | 0 | Fresh install is valid |
| IoError (persist) | 1 | Transient, retry later |
| SerializationError | 1 | Data issue, log and continue |
| CorruptedIdentity | 2 | Requires intervention |
| DatabaseCorruption | 2 | Requires repair command |
| Timeout (inject-context) | 0 | Graceful degradation |
| JSON parse error | 0 | Use defaults |
</error_mapping_table>

</requirement_impl>

</implementation_details>

<error_handling>
<!-- ============================================================================
     ERROR TYPES AND HANDLING
     ============================================================================ -->

<error_types>
```rust
//! File: crates/context-graph-core/src/gwt/session_identity/error.rs

use thiserror::Error;

/// Session identity errors.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// Session not found in storage
    #[error("Session not found: {session_id}")]
    NotFound { session_id: String },

    /// Identity data is corrupted
    #[error("Corrupted identity: {message}")]
    CorruptedIdentity { message: String },

    /// Storage operation failed
    #[error("Storage error: {0}")]
    Storage(String),

    /// Serialization/deserialization failed
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// GWT system not initialized
    #[error("GWT system not initialized")]
    NotInitialized,

    /// Invalid session ID
    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),

    /// Dream trigger failed
    #[error("Dream trigger failed: {0}")]
    DreamFailed(String),
}

impl IdentityError {
    /// Get exit code for this error per AP-26.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::CorruptedIdentity { .. } => 2, // Blocking
            _ => 1, // Non-blocking warning
        }
    }

    /// Check if this error is blocking (exit code 2).
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::CorruptedIdentity { .. })
    }
}

impl From<context_graph_storage::StorageError> for IdentityError {
    fn from(e: context_graph_storage::StorageError) -> Self {
        use context_graph_storage::StorageError;
        match e {
            StorageError::NotFound { id } => IdentityError::NotFound {
                session_id: id.replace("session_identity:", ""),
            },
            StorageError::Serialization(msg) if msg.to_lowercase().contains("corrupt") => {
                IdentityError::CorruptedIdentity { message: msg }
            }
            StorageError::Serialization(msg) => IdentityError::Serialization(msg),
            e => IdentityError::Storage(e.to_string()),
        }
    }
}
```
</error_types>

<error_recovery>
| Error | Recovery Strategy | Exit Code |
|-------|-------------------|-----------|
| NotFound | Initialize fresh session | 0 |
| CorruptedIdentity | Suggest repair command | 2 |
| Storage I/O | Log warning, continue | 1 |
| Serialization | Log warning, use defaults | 1 |
| NotInitialized | Initialize on demand | 0 |
| DreamFailed | Log warning, continue | 0 |
| JSON parse | Use default values | 0 |
| Timeout | Return minimal output | 0 |
</error_recovery>

</error_handling>

<performance_requirements>
<!-- ============================================================================
     PERFORMANCE BUDGETS AND CONSTRAINTS
     ============================================================================ -->

<timing_budgets>
| Command | Hook | Timeout | Target p95 | Critical Path |
|---------|------|---------|------------|---------------|
| consciousness brief | PreToolUse | 100ms | <50ms | YES |
| session restore-identity | SessionStart | 5000ms | <2s | NO |
| session persist-identity | SessionEnd | 30000ms | <3s | NO |
| consciousness check-identity | PostToolUse | 3000ms | <500ms | NO |
| consciousness inject-context | UserPromptSubmit | 2000ms | <1s | NO |
</timing_budgets>

<critical_path_optimization>
```
PreToolUse (consciousness brief) - CRITICAL PATH

Budget: 50ms p95

Optimization Strategy:
1. No stdin parsing - saves ~5ms
2. No disk I/O - saves ~10-50ms
3. Cache-only access via OnceLock - ~0.1ms
4. Inline format_brief() - <1ms
5. Minimal stdout write - ~0.1ms

Measured Breakdown (target):
- Process spawn: 10-15ms
- IdentityCache.get(): 0.1ms
- format_brief(): 0.5ms
- println!(): 0.1ms
- Process exit: 1ms
--------------------------
Total: ~15-20ms typical
```
</critical_path_optimization>

<memory_constraints>
| Component | Size | Constraint |
|-----------|------|------------|
| SessionIdentitySnapshot | 3-30KB | <30KB serialized |
| IdentityCache | ~200 bytes | In-memory singleton |
| trajectory | 50 * 52 = 2.6KB | Fixed cap |
| kuramoto_phases | 104 bytes | Fixed [f64; 13] |
| purpose_vector | 52 bytes | Fixed [f32; 13] |
</memory_constraints>

</performance_requirements>

<testing_requirements>
<!-- ============================================================================
     TEST SPECIFICATIONS
     ============================================================================ -->

<unit_tests>
```rust
// Test modules to create:
// - crates/context-graph-core/src/gwt/session_identity/tests/snapshot_tests.rs
// - crates/context-graph-core/src/gwt/session_identity/tests/cache_tests.rs
// - crates/context-graph-core/src/gwt/session_identity/tests/manager_tests.rs
// - crates/context-graph-storage/src/rocksdb_backend/tests_session_identity.rs

// TC-SESSION-01: Snapshot serialization round-trip
// TC-SESSION-02: Trajectory FIFO eviction
// TC-SESSION-03: format_brief() output format
// TC-SESSION-04: ConsciousnessState.short_name()
// TC-SESSION-07: Cross-session IC with identical PVs
// TC-SESSION-08: Cross-session IC with orthogonal PVs
// TC-SESSION-09: classify_ic() threshold boundaries
// TC-SESSION-10: Auto-dream trigger
```
</unit_tests>

<integration_tests>
```rust
// Test modules to create:
// - crates/context-graph-storage/tests/session_identity_integration.rs
// - tests/integration/session_hooks_test.rs

// TC-SESSION-05: RocksDB save/load round-trip
// TC-SESSION-06: Temporal index ordering
// TC-SESSION-12: consciousness brief with warm cache
// TC-SESSION-13: consciousness brief with cold cache
// TC-SESSION-14: restore-identity with source=startup
// TC-SESSION-15: restore-identity with source=clear
// TC-SESSION-16: restore-identity with no previous session
// TC-SESSION-17: persist-identity success
// TC-SESSION-18-20: check-identity IC levels
// TC-SESSION-21: inject-context output format
// TC-SESSION-22: Exit code mapping
```
</integration_tests>

<benchmark_tests>
```rust
// Benchmark modules:
// - crates/context-graph-core/benches/session_identity.rs

// TC-SESSION-11: format_brief() latency (<100us p95)
// TC-SESSION-24: Full command latency compliance
```
</benchmark_tests>

</testing_requirements>

</technical_spec>
```

---

## Appendix A: File Location Summary

| Requirement | File Path |
|-------------|-----------|
| REQ-SESSION-01 | crates/context-graph-core/src/gwt/session_identity/types.rs |
| REQ-SESSION-02 | crates/context-graph-core/src/gwt/session_identity/cache.rs |
| REQ-SESSION-03 | crates/context-graph-core/src/gwt/state_machine/types.rs (extension) |
| REQ-SESSION-04 | crates/context-graph-storage/src/column_families.rs (extension) |
| REQ-SESSION-05 | crates/context-graph-storage/src/session_identity.rs (new) |
| REQ-SESSION-06 | crates/context-graph-core/src/gwt/session_identity/manager.rs |
| REQ-SESSION-07 | crates/context-graph-core/src/gwt/session_identity/manager.rs |
| REQ-SESSION-08 | crates/context-graph-core/src/gwt/session_identity/dream_trigger.rs |
| REQ-SESSION-09 | crates/context-graph-core/src/gwt/session_identity/cache.rs |
| REQ-SESSION-10 | crates/context-graph-core/src/gwt/session_identity/cache.rs |
| REQ-SESSION-11 | crates/context-graph-mcp/src/cli/commands/consciousness/brief.rs |
| REQ-SESSION-12 | crates/context-graph-mcp/src/cli/commands/session/restore.rs |
| REQ-SESSION-13 | crates/context-graph-mcp/src/cli/commands/session/persist.rs |
| REQ-SESSION-14 | crates/context-graph-mcp/src/cli/commands/consciousness/check.rs |
| REQ-SESSION-15 | crates/context-graph-mcp/src/cli/commands/consciousness/inject.rs |
| REQ-SESSION-16 | .claude/settings.json |
| REQ-SESSION-17 | crates/context-graph-mcp/src/cli/error.rs |

## Appendix B: Module Dependencies

```
context-graph-core::gwt::session_identity
    |
    +-- types.rs (SessionIdentitySnapshot, constants)
    |       |
    +-- cache.rs (IdentityCache, update_cache)
    |       |
    +-- manager.rs (SessionIdentityManager trait, classify_ic)
    |       |
    +-- dream_trigger.rs (trigger_dream_async, check_and_trigger_dream)
    |       |
    +-- error.rs (IdentityError)
    |
    +-- mod.rs (re-exports)

context-graph-storage
    |
    +-- column_families.rs (cf_names::SESSION_IDENTITY)
    |       |
    +-- session_identity.rs (save_snapshot, load_snapshot, load_latest)

context-graph-mcp::cli::commands
    |
    +-- consciousness/
    |       +-- brief.rs
    |       +-- check.rs
    |       +-- inject.rs
    |
    +-- session/
            +-- restore.rs
            +-- persist.rs
```

## Appendix C: Constitution Compliance Checklist

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| ARCH-07 | COMPLIANT | All hooks in .claude/settings.json |
| AP-50 | COMPLIANT | No internal/built-in hooks |
| AP-53 | COMPLIANT | CLI commands (not shell scripts) |
| IDENTITY-002 | COMPLIANT | classify_ic() thresholds |
| IDENTITY-007 | COMPLIANT | Auto-dream on IC<0.5 |
| AP-26 | COMPLIANT | Exit code 2 only for corruption |
