# TASK-GWT-P1-001: SELF_EGO_NODE Persistence Layer

## Atomic Task Specification

```xml
<task_spec id="TASK-GWT-P1-001" version="1.0">
<metadata>
  <title>Implement SELF_EGO_NODE Persistence for Identity Continuity</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>1</sequence>
  <implements>
    <item>Constitution v4.0.0 Section gwt.self_ego_node (lines 371-392)</item>
    <item>Sherlock-03 Finding: System identity amnesia on restart</item>
    <item>Purpose evolution archival for identity trajectory</item>
  </implements>
  <depends_on>
    <task_ref>TASK-GWT-P0-003</task_ref>
    <note>Dependency on SelfEgoNode struct existence (already in ego_node.rs)</note>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
The SELF_EGO_NODE represents the system's persistent identity across restarts.
Currently, SelfEgoNode exists in `crates/context-graph-core/src/gwt/ego_node.rs`
but has NO persistence layer. Every system restart creates a fresh identity,
losing all purpose_evolution history and identity_trajectory snapshots.

This task adds:
1. A dedicated RocksDB column family `CF_EGO_NODE` for ego node storage
2. `save_ego_node()` and `load_ego_node()` methods on TeleologicalStore
3. Serialization/deserialization for SelfEgoNode, PurposeSnapshot, IdentityContinuity
4. Startup logic to load existing ego node or create fresh one

The ego node uses a deterministic UUID (`Uuid::nil()`) as its key, ensuring
there is exactly ONE system identity node across restarts.
</context>

<input_context_files>
  <file purpose="SelfEgoNode struct definition and fields">crates/context-graph-core/src/gwt/ego_node.rs</file>
  <file purpose="TeleologicalStore trait to extend">crates/context-graph-core/src/traits/teleological_memory_store.rs</file>
  <file purpose="Column family patterns">crates/context-graph-storage/src/teleological/column_families.rs</file>
  <file purpose="Serialization patterns">crates/context-graph-storage/src/teleological/serialization.rs</file>
  <file purpose="Key schema patterns">crates/context-graph-storage/src/teleological/schema.rs</file>
  <file purpose="RocksDB store implementation">crates/context-graph-storage/src/teleological/rocksdb_store.rs</file>
</input_context_files>

<prerequisites>
  <check>SelfEgoNode struct exists in ego_node.rs with all fields (id, fingerprint, purpose_vector, coherence_with_actions, identity_trajectory, last_updated)</check>
  <check>PurposeSnapshot struct exists with vector, timestamp, context fields</check>
  <check>IdentityContinuity struct exists with recent_continuity, kuramoto_order_parameter, identity_coherence, status fields</check>
  <check>TeleologicalFingerprint type is defined and serializable</check>
  <check>RocksDbTeleologicalStore is operational with existing column families</check>
</prerequisites>

<scope>
  <in_scope>
    - Add CF_EGO_NODE column family constant and option builder
    - Add ego_node_key() function to schema.rs (returns Uuid::nil() bytes)
    - Add serialize_ego_node() and deserialize_ego_node() functions
    - Add save_ego_node() and load_ego_node() methods to TeleologicalMemoryStore trait
    - Implement save_ego_node() and load_ego_node() in RocksDbTeleologicalStore
    - Implement save_ego_node() and load_ego_node() in InMemoryTeleologicalStore (stub)
    - Add #[derive(Serialize, Deserialize)] to SelfEgoNode, PurposeSnapshot, IdentityContinuity
    - Unit tests for serialization round-trip
    - Unit tests for persistence across store close/reopen
  </in_scope>
  <out_of_scope>
    - MCP handler for ego node operations (separate task)
    - Automatic ego node loading on server startup (separate task)
    - Identity migration from v1 to v2 (not applicable)
    - SelfAwarenessLoop integration (separate task)
    - Dream consolidation triggers (handled in TASK-GWT-P1-002)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/column_families.rs">
      /// Column family for SELF_EGO_NODE singleton storage.
      ///
      /// Stores the system's persistent identity node.
      /// Key: Uuid::nil() (16 bytes, fixed singleton key)
      /// Value: SelfEgoNode serialized via bincode (~2-10KB depending on trajectory size)
      pub const CF_EGO_NODE: &str = "ego_node";
    </signature>

    <signature file="crates/context-graph-storage/src/teleological/schema.rs">
      /// Key for ego_node CF: Uuid::nil() as 16 bytes.
      ///
      /// The SELF_EGO_NODE uses a fixed nil UUID as its key,
      /// ensuring there is exactly one system identity node.
      ///
      /// # Returns
      /// Exactly 16 bytes (Uuid::nil() in big-endian format)
      #[inline]
      pub fn ego_node_key() -> [u8; 16] {
          *Uuid::nil().as_bytes()
      }
    </signature>

    <signature file="crates/context-graph-storage/src/teleological/serialization.rs">
      /// Serialize SelfEgoNode to bytes with version prefix.
      ///
      /// # Panics
      /// - Panics if serialization fails (FAIL FAST)
      /// - Panics if serialized size exceeds 50KB (trajectory overflow protection)
      pub fn serialize_ego_node(ego: &SelfEgoNode) -> Vec&lt;u8&gt;;

      /// Deserialize SelfEgoNode from bytes.
      ///
      /// # Panics
      /// - Panics if version mismatch (FAIL FAST, no migration)
      /// - Panics if deserialization fails
      pub fn deserialize_ego_node(data: &[u8]) -> SelfEgoNode;
    </signature>

    <signature file="crates/context-graph-core/src/traits/teleological_memory_store.rs">
      /// Save the SELF_EGO_NODE to persistent storage.
      ///
      /// This is a singleton operation - only one ego node exists per system.
      /// Overwrites any existing ego node.
      ///
      /// # Arguments
      /// * `ego_node` - The ego node to persist
      ///
      /// # Errors
      /// - `CoreError::StorageError` - Write failure
      /// - `CoreError::SerializationError` - Serialization failure
      async fn save_ego_node(&self, ego_node: &SelfEgoNode) -> CoreResult&lt;()&gt;;

      /// Load the SELF_EGO_NODE from persistent storage.
      ///
      /// Returns None if no ego node has been persisted yet (fresh system).
      ///
      /// # Returns
      /// `Some(SelfEgoNode)` if exists, `None` if fresh system.
      ///
      /// # Errors
      /// - `CoreError::StorageError` - Read failure
      /// - `CoreError::SerializationError` - Deserialization failure
      async fn load_ego_node(&self) -> CoreResult&lt;Option&lt;SelfEgoNode&gt;&gt;;
    </signature>

    <signature file="crates/context-graph-core/src/gwt/ego_node.rs">
      // Add derives to existing structs:
      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct SelfEgoNode { ... }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct PurposeSnapshot { ... }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct IdentityContinuity { ... }

      #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
      pub enum IdentityStatus { ... }
    </signature>
  </signatures>

  <constraints>
    - SINGLETON: Only one ego node per system (key = Uuid::nil())
    - FAIL FAST: Panic on serialization/deserialization errors (no recovery)
    - VERSION PREFIX: Use EGO_NODE_VERSION = 1 for future migration support
    - SIZE LIMIT: Max serialized size 50KB (1000 snapshots * ~50 bytes each)
    - NO MIGRATION: Version mismatch panics (clean break for v1 systems)
    - ATOMIC WRITE: Use single put operation (no multi-key transactions)
    - CONSISTENT HASH: TeleologicalFingerprint field uses Option to handle initial nil state
  </constraints>

  <verification>
    - `cargo test -p context-graph-storage ego_node` passes
    - `cargo test -p context-graph-core ego_node` passes
    - Serialization round-trip preserves all fields including identity_trajectory
    - Store close/reopen preserves ego node state
    - Fresh store returns None from load_ego_node()
    - Second save_ego_node() overwrites first
    - InMemoryTeleologicalStore (stub) compiles and returns Ok(None)/Ok(())
  </verification>
</definition_of_done>

<pseudo_code>
1. ego_node.rs - Add Serde derives:
   - Add `use serde::{Serialize, Deserialize};` import
   - Add `#[derive(..., Serialize, Deserialize)]` to SelfEgoNode
   - Add `#[derive(..., Serialize, Deserialize)]` to PurposeSnapshot
   - Add `#[derive(..., Serialize, Deserialize)]` to IdentityContinuity
   - Add `#[derive(..., Serialize, Deserialize)]` to IdentityStatus
   - Mark TeleologicalFingerprint field with `#[serde(skip)]` or use Option

2. column_families.rs - Add CF_EGO_NODE:
   - Add `pub const CF_EGO_NODE: &str = "ego_node";`
   - Add CF_EGO_NODE to TELEOLOGICAL_CFS array
   - Update TELEOLOGICAL_CF_COUNT to 9
   - Add `ego_node_cf_options(cache: &Cache) -> Options` function
   - Add CF_EGO_NODE descriptor to get_teleological_cf_descriptors()

3. schema.rs - Add ego_node_key:
   - Add `pub fn ego_node_key() -> [u8; 16]` that returns Uuid::nil() bytes

4. serialization.rs - Add ego node serialization:
   - Add `pub const EGO_NODE_VERSION: u8 = 1;`
   - Add `const MAX_EGO_NODE_SIZE: usize = 50_000;`
   - Add `serialize_ego_node(ego: &SelfEgoNode) -> Vec<u8>`
   - Add `deserialize_ego_node(data: &[u8]) -> SelfEgoNode`

5. teleological_memory_store.rs - Add trait methods:
   - Add `async fn save_ego_node(&self, ego_node: &SelfEgoNode) -> CoreResult<()>;`
   - Add `async fn load_ego_node(&self) -> CoreResult<Option<SelfEgoNode>>;`

6. rocksdb_store.rs - Implement methods:
   - Add `cf_ego_node(&self) -> &ColumnFamily` helper
   - Implement `save_ego_node()`: serialize + put_cf
   - Implement `load_ego_node()`: get_cf + deserialize (return None if not found)

7. teleological_store_stub.rs - Implement stub methods:
   - Add `ego_node: RwLock<Option<SelfEgoNode>>` field
   - Implement `save_ego_node()`: clone into RwLock
   - Implement `load_ego_node()`: clone from RwLock

8. Tests:
   - test_ego_node_serialization_roundtrip()
   - test_ego_node_persistence_across_reopen()
   - test_ego_node_fresh_store_returns_none()
   - test_ego_node_overwrite()
   - test_ego_node_with_trajectory()
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/ego_node_persistence.rs">
    Module containing ego node specific persistence logic (optional, can inline in rocksdb_store.rs)
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/gwt/ego_node.rs">
    Add Serialize, Deserialize derives to SelfEgoNode, PurposeSnapshot, IdentityContinuity, IdentityStatus.
    Handle TeleologicalFingerprint serialization (skip or Option).
  </file>
  <file path="crates/context-graph-storage/src/teleological/column_families.rs">
    Add CF_EGO_NODE constant, update TELEOLOGICAL_CFS array, add ego_node_cf_options(), update descriptor function.
  </file>
  <file path="crates/context-graph-storage/src/teleological/schema.rs">
    Add ego_node_key() function returning Uuid::nil() bytes.
  </file>
  <file path="crates/context-graph-storage/src/teleological/serialization.rs">
    Add EGO_NODE_VERSION, serialize_ego_node(), deserialize_ego_node() functions.
  </file>
  <file path="crates/context-graph-core/src/traits/teleological_memory_store.rs">
    Add save_ego_node() and load_ego_node() methods to TeleologicalMemoryStore trait.
  </file>
  <file path="crates/context-graph-storage/src/teleological/rocksdb_store.rs">
    Add cf_ego_node() helper, implement save_ego_node() and load_ego_node() methods.
  </file>
  <file path="crates/context-graph-core/src/stubs/teleological_store_stub.rs">
    Add ego_node field and implement save_ego_node()/load_ego_node() stub methods.
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build -p context-graph-core succeeds (Serde derives compile)</criterion>
  <criterion>cargo build -p context-graph-storage succeeds (new CF and methods compile)</criterion>
  <criterion>RocksDB opens with 9 column families (was 8, now includes ego_node)</criterion>
  <criterion>save_ego_node() followed by load_ego_node() returns identical data</criterion>
  <criterion>identity_trajectory with 100 snapshots serializes under 50KB</criterion>
  <criterion>Store close and reopen preserves ego node</criterion>
  <criterion>Fresh store load_ego_node() returns Ok(None)</criterion>
  <criterion>All existing tests still pass (no regression)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core --lib ego_node</command>
  <command>cargo test -p context-graph-storage --lib ego_node</command>
  <command>cargo test -p context-graph-storage test_ego_node_serialization</command>
  <command>cargo test -p context-graph-storage test_ego_node_persistence</command>
  <command>cargo test -p context-graph-storage teleological</command>
  <command>cargo clippy -p context-graph-core -p context-graph-storage -- -D warnings</command>
</test_commands>
</task_spec>
```

## Problem Statement (Sherlock-03 Finding)

The SELF_EGO_NODE represents the system's persistent identity, but currently has **NO PERSISTENCE LAYER**:

1. **Identity Amnesia**: Every system restart creates a fresh `SelfEgoNode::new()`, losing all identity history
2. **Lost Purpose Evolution**: The `identity_trajectory` (purpose snapshots) is never archived
3. **No Continuity**: System cannot "remember" its previous purpose alignments or identity coherence state
4. **Fresh Identity Each Time**: `IdentityContinuity` starts at `Critical` status (IC=0.0) every restart

## Solution Architecture

```
                    SELF_EGO_NODE Persistence

    ┌─────────────────────────────────────────────────────────┐
    │                   SelfEgoNode                           │
    │  ├── id: Uuid::nil() (singleton key)                    │
    │  ├── fingerprint: Option<TeleologicalFingerprint>       │
    │  ├── purpose_vector: [f32; 13]                          │
    │  ├── coherence_with_actions: f32                        │
    │  ├── identity_trajectory: Vec<PurposeSnapshot>          │
    │  └── last_updated: DateTime<Utc>                        │
    └─────────────────────────────────────────────────────────┘
                              │
                              │ save_ego_node() / load_ego_node()
                              ▼
    ┌─────────────────────────────────────────────────────────┐
    │              TeleologicalMemoryStore                    │
    │  ├── save_ego_node(&SelfEgoNode) -> CoreResult<()>      │
    │  └── load_ego_node() -> CoreResult<Option<SelfEgoNode>> │
    └─────────────────────────────────────────────────────────┘
                              │
                              ▼
    ┌─────────────────────────────────────────────────────────┐
    │              RocksDB Column Family                      │
    │  CF_EGO_NODE: "ego_node"                                │
    │  Key: Uuid::nil() (16 bytes, singleton)                 │
    │  Value: bincode(SelfEgoNode) (~2-10KB)                  │
    └─────────────────────────────────────────────────────────┘
```

## Data Flow

### On System Startup
```
1. Open TeleologicalStore
2. Call load_ego_node()
3. If Some(ego) -> Use existing identity (continuity preserved!)
4. If None -> Create fresh SelfEgoNode::new() (first boot)
5. Initialize SelfAwarenessLoop with ego_node
```

### On Purpose Vector Update
```
1. SelfAwarenessLoop.cycle() updates ego_node
2. Call save_ego_node(&ego_node)
3. Identity trajectory persisted to RocksDB
4. Next restart will load full history
```

## Column Family Configuration

```rust
/// Column family for SELF_EGO_NODE singleton storage.
pub const CF_EGO_NODE: &str = "ego_node";

pub fn ego_node_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.optimize_for_point_lookup(16); // Small singleton
    opts.create_if_missing(true);
    opts
}
```

## Serialization Format

```
┌──────────┬───────────────────────────────────────────┐
│ Version  │           Bincode Payload                 │
│ (1 byte) │     (SelfEgoNode serialized)              │
└──────────┴───────────────────────────────────────────┘
```

- **Version**: `EGO_NODE_VERSION = 1`
- **Max Size**: 50KB (allows ~1000 trajectory snapshots)
- **Compression**: LZ4 at RocksDB level

## Dependencies

| Dependency | Type | Notes |
|------------|------|-------|
| TASK-GWT-P0-003 | Struct | SelfEgoNode already exists |
| bincode 1.3 | Crate | Already in use |
| serde | Crate | Already in use |
| uuid | Crate | Already in use |
| chrono | Crate | Already in use |

## Testing Strategy

1. **Unit Tests**: Serialization round-trip
2. **Integration Tests**: Persistence across store close/reopen
3. **Edge Cases**: Fresh store, overwrite, max trajectory size
4. **Regression**: Ensure existing tests pass

## Success Metrics

- [ ] SelfEgoNode survives system restart
- [ ] identity_trajectory preserved with all snapshots
- [ ] purpose_vector alignment history available after restart
- [ ] IdentityContinuity can resume from last known state
- [ ] No regression in existing TeleologicalStore functionality
