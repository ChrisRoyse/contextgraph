# TASK-INSTR-022 — Panel JSON serialization protocol + RocksDB CF_MEJEPA_PANELS / CF_MEJEPA_PANEL_META persistence

```xml
<task_spec id="TASK-INSTR-022" version="1.0">

<metadata>
  <title>Panel JSON serialization protocol with provenance metadata + RocksDB CF_MEJEPA_PANELS / CF_MEJEPA_PANEL_META persistence path</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>22</sequence>
  <implements>
    <item ref="REQ-INSTR-23">Provenance metadata in panel JSON (code_version, embedder_versions, frozen_at, corpus_sha)</item>
    <item ref="component:PanelJSONExternal">version, code_version, embedder_versions, time_step, attempt_id, slots, scalars, total_dim, sha256</item>
    <item ref="ERR-INSTR-10">missing provenance fields rejected fail-closed</item>
    <item ref="EC-INSTR-12">stale embedder_versions triggers panel rejection at read time</item>
    <item ref="constitution:provenance">Every persistent artifact carries provenance metadata</item>
    <item ref="constitution:SEC-MEJEPA-02">Embedder weight files load from pinned SHA-256 digests</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-020</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file created (panel_json), 2 files modified (lib.rs, context-graph-storage CF descriptors)</estimated_files>
</metadata>

<context>
Per REQ-INSTR-23 + constitution `<provenance>`, every persisted Panel must carry:

- `version` (schema version, "1.0.0")
- `code_version` (git SHA of producing codebase)
- `embedder_versions` (map E1..E21 → weights file SHA-256)
- `time_step` (0/1/2)
- `attempt_id` (UUID v4)
- `slots` (per-slot vectors keyed by slug)
- `scalars` (128-d Scalars vector)
- `total_dim` (== 5,120)
- `sha256` (digest of canonical concat of slots + scalars)

The Panel struct shipped today (`lib.rs`) only carries `data` + `filled_mask`; this task adds the
provenance wrapper `PanelJsonV1` (or v1.0.0) plus the validation gate (deserialization rejects
stale embedder_versions, missing fields, malformed SHA-256, etc.).

Storage path: write to RocksDB `CF_MEJEPA_PANELS` keyed by `(attempt_id_uuid_bytes, time_step_u8)`
= 17 bytes. Provenance metadata in companion CF `CF_MEJEPA_PANEL_META` keyed by `attempt_id`.
Both CFs MUST be registered in `context-graph-storage` (this task adds the descriptors).

Per constitution `<security_requirements>` SEC-MEJEPA-02, embedder_versions SHA-256 entries are
verified against `models_config.toml` pins on read; mismatch -&gt; reject with InstrumentError +
recommend rebuild.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-23, ERR-INSTR-10, EC-INSTR-12)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (panel_serialization_protocol; data_models PanelJSONExternal)</file>
  <file purpose="shipped_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel, PanelSerde)</file>
  <file purpose="materialize">crates/context-graph-mejepa-instruments/src/materialize.rs (TASK-INSTR-020)</file>
  <file purpose="storage_cf_pattern">crates/context-graph-storage/src/lib.rs (existing CF descriptor pattern)</file>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (provenance, SEC-MEJEPA-02; rocksdb_cfs naming)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-020 done (materialize_panel produces a Panel)</check>
  <check>context-graph-storage crate available; existing CF descriptor pattern documented</check>
  <check>models_config.toml exists with E1..E21 SHA-256 pins (cross-domain Phase 1b deliverable)</check>
</prerequisites>

<scope>
  <in_scope>
    - New module `crates/context-graph-mejepa-instruments/src/panel_json.rs`
    - `pub struct PanelJsonV1` matching PanelJSONExternal in TECH-INSTRUMENTS data_models
    - `pub fn panel_to_json(panel: &amp;Panel, meta: &amp;PanelProvenance) -&gt; InstrumentResult&lt;PanelJsonV1&gt;`
    - `pub fn panel_from_json(json: &amp;PanelJsonV1, embedder_pins: &amp;EmbedderPins) -&gt; InstrumentResult&lt;Panel&gt;`
    - Validation gates per panel_serialization_protocol: version="1.0.0", total_dim==5,120, slots[slug].len() == slot.dim(), scalars.len()==128, every f32 finite, SHA-256 of canonical concat matches `sha256` field, embedder_versions[E_k] matches pinned digest, code_version non-empty
    - `pub struct PanelProvenance { code_version, embedder_versions, frozen_at, corpus_sha }`
    - Re-export from lib.rs
    - Add CF descriptors `CF_MEJEPA_PANELS`, `CF_MEJEPA_PANEL_META` to `context-graph-storage` (modify existing CF registry)
    - Persistence helpers: `pub fn write_panel(db: &amp;DB, attempt_id: Uuid, time_step: TimeStep, panel: &amp;PanelJsonV1) -&gt; StorageResult&lt;()&gt;` and matching reader
    - Tier-compression integration hook: persistence helper accepts optional tier policy; routes to context-graph-tier-compression when tier policy is non-None (deferred neural tier policy goes here)
    - Round-trip tests: panel_to_json(panel_from_json(...)) is byte-equal
    - Stale-pin test: embedder_versions["E1"] != pinned -&gt; rejection with InstrumentError + remediation message
    - Missing-field test: removing code_version from JSON triggers ERR-INSTR-10
  </in_scope>
  <out_of_scope>
    - Tier compression policy decisions (Phase 6)
    - Phase 7 mcp__cgreality__mejepa_predict_latest (Phase 7 spec)
    - Migration of pre-1.0.0 panels (per memory/feedback_no_backwards_compat.md, wipe + regenerate is the default)
    - The Phase 2 predictor read path (separate spec)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/panel_json.rs">
pub const PANEL_JSON_SCHEMA_VERSION: &amp;str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelJsonV1 {
    pub version: String,                         // "1.0.0"
    pub code_version: String,                    // git SHA
    pub embedder_versions: HashMap&lt;String, String&gt;, // E1..E21 -&gt; SHA-256 hex
    pub time_step: u8,                           // 0|1|2
    pub attempt_id: String,                      // UUID v4
    pub slots: HashMap&lt;String, Vec&lt;f32&gt;&gt;,         // slug -&gt; vector
    pub scalars: Vec&lt;f32&gt;,                       // 128 f32
    pub total_dim: usize,                        // 5_120
    pub sha256: String,                          // digest of canonical concat
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelProvenance {
    pub code_version: String,
    pub embedder_versions: HashMap&lt;String, String&gt;,
    pub frozen_at: String,                       // ISO 8601
    pub corpus_sha: String,
}

#[derive(Debug, Clone)]
pub struct EmbedderPins {
    pinned: HashMap&lt;String, String&gt;,             // loaded from models_config.toml
}

impl EmbedderPins {
    pub fn from_config(path: &amp;Path) -&gt; InstrumentResult&lt;Self&gt;;
    pub fn verify(&amp;self, embedder_versions: &amp;HashMap&lt;String, String&gt;) -&gt; InstrumentResult&lt;()&gt;;
}

pub fn panel_to_json(panel: &amp;Panel, meta: &amp;PanelProvenance, time_step: u8, attempt_id: &amp;str) -&gt; InstrumentResult&lt;PanelJsonV1&gt;;
pub fn panel_from_json(json: &amp;PanelJsonV1, pins: &amp;EmbedderPins) -&gt; InstrumentResult&lt;Panel&gt;;
    </signature>
    <signature file="crates/context-graph-storage/src/cf.rs (or existing CF registry module)">
pub const CF_MEJEPA_PANELS: &amp;str = "CF_MEJEPA_PANELS";
pub const CF_MEJEPA_PANEL_META: &amp;str = "CF_MEJEPA_PANEL_META";

pub fn mejepa_panel_cf_descriptors() -&gt; Vec&lt;ColumnFamilyDescriptor&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-instruments/src/panel_json.rs">
pub fn write_panel(
    db: &amp;DB,
    attempt_id: Uuid,
    time_step: u8,
    panel_json: &amp;PanelJsonV1,
    meta: &amp;PanelProvenance,
) -&gt; InstrumentResult&lt;()&gt;;

pub fn read_panel(
    db: &amp;DB,
    attempt_id: Uuid,
    time_step: u8,
    pins: &amp;EmbedderPins,
) -&gt; InstrumentResult&lt;(Panel, PanelProvenance)&gt;;
    </signature>
  </signatures>
  <constraints>
    - Schema version exactly "1.0.0"
    - total_dim == 5,120 enforced on serialize + deserialize
    - SHA-256 of canonical concat (slot[0]..slot[14] in InstrumentSlot::all() order, then scalars, all little-endian f32 bytes) -&gt; matches sha256 field
    - Missing code_version / embedder_versions / sha256 -&gt; ERR-INSTR-10
    - Stale pin (`pins.verify(json.embedder_versions)` fails) -&gt; reject with remediation "rebuild panel with current weights"
    - Storage key for CF_MEJEPA_PANELS: 16-byte UUID + 1-byte time_step = 17 bytes; little-endian uniform encoding
    - Provenance written to CF_MEJEPA_PANEL_META keyed by 16-byte UUID
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments panel_json::
    - cargo test -p context-graph-storage cf_mejepa_panels
    - Round-trip: serialize -&gt; bytes -&gt; deserialize -&gt; assert PartialEq
    - Stale-pin test: embedder_versions modified after persist; read rejects
    - Missing-field test: stripped JSON rejected with ERR-INSTR-10
  </verification>
</definition_of_done>

<pseudo_code>
panel_json.rs:

  pub fn panel_to_json(panel, meta, time_step, attempt_id) -&gt; InstrumentResult&lt;PanelJsonV1&gt; {
      let mut slots = HashMap::new();
      for slot in InstrumentSlot::all() {
          if slot != InstrumentSlot::Scalars {
              slots.insert(slot.slug().to_string(), panel.slot(slot).to_vec());
          }
      }
      let scalars = panel.slot(InstrumentSlot::Scalars).to_vec();
      let sha = compute_canonical_sha256(panel);
      Ok(PanelJsonV1 {
          version: PANEL_JSON_SCHEMA_VERSION.to_string(),
          code_version: meta.code_version.clone(),
          embedder_versions: meta.embedder_versions.clone(),
          time_step,
          attempt_id: attempt_id.to_string(),
          slots,
          scalars,
          total_dim: PANEL_DIM,
          sha256: sha,
      })
  }

  pub fn panel_from_json(json, pins) -&gt; InstrumentResult&lt;Panel&gt; {
      // Required-field gates
      if json.version != PANEL_JSON_SCHEMA_VERSION {
          return Err(InstrumentError::invalid("version", &amp;json.version, "expected 1.0.0"));
      }
      if json.code_version.is_empty() { return Err(missing_field("code_version")); }
      if json.embedder_versions.is_empty() { return Err(missing_field("embedder_versions")); }
      if json.total_dim != PANEL_DIM { return Err(InstrumentError::invalid("total_dim", ...)); }
      // Pin verification
      pins.verify(&amp;json.embedder_versions)?;
      // Reassemble panel.data
      let mut data = vec![0.0_f32; PANEL_DIM];
      let mut filled_mask: u16 = 0;
      for (slug, vec) in &amp;json.slots {
          let slot = InstrumentSlot::from_slug(slug)
              .ok_or_else(|| InstrumentError::invalid("slot", slug, "unknown slot slug"))?;
          if vec.len() != slot.dim() { return Err(InstrumentError::invalid("slot.dim", ...)); }
          let (off, dim) = slot.extent();
          data[off..off + dim].copy_from_slice(vec);
          filled_mask |= slot_bit(slot);
      }
      if json.scalars.len() != 128 { return Err(InstrumentError::invalid("scalars.len", ...)); }
      let (scalars_off, scalars_dim) = InstrumentSlot::Scalars.extent();
      data[scalars_off..scalars_off + scalars_dim].copy_from_slice(&amp;json.scalars);
      filled_mask |= slot_bit(InstrumentSlot::Scalars);
      // SHA verify
      let recomputed = compute_canonical_sha256_from_data(&amp;data);
      if recomputed != json.sha256 {
          return Err(InstrumentError::invalid("sha256", "mismatch", "panel was modified after persistence"));
      }
      Panel::try_new(data, filled_mask)
  }

  pub fn write_panel(db, attempt_id, time_step, panel_json, meta) -&gt; InstrumentResult&lt;()&gt; {
      let key = panel_key(attempt_id, time_step);  // 17 bytes
      let value = serde_json::to_vec(panel_json)?;
      db.put_cf(&amp;db.cf_handle(CF_MEJEPA_PANELS).unwrap(), &amp;key, &amp;value)?;
      let meta_key = attempt_id.as_bytes();
      let meta_value = serde_json::to_vec(meta)?;
      db.put_cf(&amp;db.cf_handle(CF_MEJEPA_PANEL_META).unwrap(), meta_key, &amp;meta_value)?;
      Ok(())
  }

  pub fn read_panel(db, attempt_id, time_step, pins) -&gt; InstrumentResult&lt;(Panel, PanelProvenance)&gt; {
      let key = panel_key(attempt_id, time_step);
      let json_bytes = db.get_cf(&amp;db.cf_handle(CF_MEJEPA_PANELS).unwrap(), &amp;key)?
          .ok_or_else(|| InstrumentError::invalid("attempt_id", "panel not found", "..."))?;
      let json: PanelJsonV1 = serde_json::from_slice(&amp;json_bytes)?;
      let panel = panel_from_json(&amp;json, pins)?;
      let meta_bytes = db.get_cf(&amp;db.cf_handle(CF_MEJEPA_PANEL_META).unwrap(), attempt_id.as_bytes())?
          .ok_or_else(|| InstrumentError::invalid("attempt_id", "meta not found", "..."))?;
      let meta: PanelProvenance = serde_json::from_slice(&amp;meta_bytes)?;
      Ok((panel, meta))
  }

tests:
  - panel_to_json_round_trip
  - panel_from_json_missing_code_version_rejected
  - panel_from_json_missing_embedder_versions_rejected
  - panel_from_json_unknown_slot_rejected
  - panel_from_json_stale_pin_rejected
  - panel_from_json_sha_mismatch_rejected
  - panel_from_json_total_dim_wrong_rejected
  - cf_mejepa_panels_descriptors_registered
  - write_read_round_trip_byte_equal
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-instruments/src/panel_json.rs">PanelJsonV1, PanelProvenance, EmbedderPins, panel_to_json, panel_from_json, write_panel, read_panel; tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Add `pub mod panel_json;` re-export {PanelJsonV1, PanelProvenance, EmbedderPins, PANEL_JSON_SCHEMA_VERSION, panel_to_json, panel_from_json, write_panel, read_panel}</file>
  <file path="crates/context-graph-storage/src/lib.rs">Register CF_MEJEPA_PANELS + CF_MEJEPA_PANEL_META descriptors in the master CF list</file>
  <file path="crates/context-graph-mejepa-instruments/Cargo.toml">Add rocksdb (or storage crate) dep + uuid dep + sha2 dep if not already present</file>
</files_to_modify>

<validation_criteria>
  <criterion>panel_to_json + panel_from_json round-trip byte-equal</criterion>
  <criterion>Missing code_version/embedder_versions/sha256 rejected with ERR-INSTR-10</criterion>
  <criterion>Stale embedder pin rejected with remediation message</criterion>
  <criterion>SHA-256 mismatch rejected (tamper detection)</criterion>
  <criterion>total_dim != 5,120 rejected</criterion>
  <criterion>slots[slug].len() != slot.dim() rejected</criterion>
  <criterion>scalars.len() != 128 rejected</criterion>
  <criterion>CF_MEJEPA_PANELS + CF_MEJEPA_PANEL_META registered in context-graph-storage</criterion>
  <criterion>write_panel + read_panel round-trip via real RocksDB</criterion>
  <criterion>cargo build --release -p context-graph-mejepa-instruments clean</criterion>
  <criterion>cargo build --release -p context-graph-storage clean</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-instruments -- -D warnings clean</criterion>
  <criterion>file under 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo build --release -p context-graph-storage</command>
  <command>cargo test -p context-graph-mejepa-instruments panel_json::</command>
  <command>cargo test -p context-graph-storage cf_mejepa</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
