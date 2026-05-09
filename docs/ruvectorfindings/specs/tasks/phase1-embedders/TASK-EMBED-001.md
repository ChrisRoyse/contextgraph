# TASK-EMBED-001 — Crate skeleton + Instrument trait extension

```xml
<task_spec id="TASK-EMBED-001" version="1.0">
<metadata>
  <title>Create crate skeleton context-graph-mejepa-embedders + extend Instrument trait with embedder_id / version / weight_sha</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-EMBED-01">Uniform Instrument::encode interface across 21 embedders</item>
    <item ref="TECH-EMBEDDERS::crate_layout">crates/context-graph-mejepa-embedders skeleton</item>
  </implements>
  <depends_on><!-- empty: first task in domain --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>5 files</estimated_files>
</metadata>

<context>
Foundation task. Creates the new clean-room crate that wraps the shipped 21-embedder ensemble and extends the existing `Instrument` trait in `context-graph-mejepa-instruments` with three new methods (with default impls so existing E_Oracle / E_Witness / E_StaticAnalysis impls keep compiling). All subsequent EMBEDDERS tasks live in this crate or extend the trait wired here.
</context>

<input_context_files>
  <file purpose="trait_definition_to_extend">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="constitution">docs/ruvectorfindings/specs/constitution.md</file>
  <file purpose="technical_spec_crate_layout">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md</file>
  <file purpose="cargo_toml_reference">crates/context-graph-mejepa-instruments/Cargo.toml</file>
  <file purpose="workspace_root">Cargo.toml</file>
</input_context_files>

<prerequisites>
  <check>Workspace `Cargo.toml` resolver=2 with member glob covering crates/*</check>
  <check>context-graph-mejepa-instruments crate compiles cleanly</check>
  <check>Constitution `&lt;tech_stack&gt;` library versions noted (candle 0.9.x, cudarc, tokenizers 0.21.x, sha2, ed25519-dalek 2.x)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-embedders/Cargo.toml` with workspace deps + `candle-core`, `candle-nn`, `candle-transformers`, `cudarc`, `tokenizers`, `sha2`, `ed25519-dalek`, `tokio`, `thiserror`, `serde`, `serde_json`, `uuid`, `chrono`
    - Create `crates/context-graph-mejepa-embedders/src/lib.rs` with module re-exports skeleton
    - Add new crate to workspace `[workspace.members]` if explicit listing is used
    - Extend `crates/context-graph-mejepa-instruments/src/lib.rs::Instrument` trait with 3 new methods having default impls
    - Add provenance comment header on every created file
  </in_scope>
  <out_of_scope>
    - EmbedderId enum (TASK-EMBED-002)
    - Any concrete EmbedderInstrument wrapper (TASK-EMBED-010..019)
    - Routing table (TASK-EMBED-006)
    - VRAM budget (TASK-EMBED-007)
    - Cargo.lock change auditing (handled at workspace build time)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/lib.rs">
      // Extended Instrument trait — 3 new methods with default impls so existing
      // EOracleInstrument / EWitnessInstrument / EStaticAnalysisInstrument
      // implementations keep compiling unchanged.
      pub trait Instrument {
          type Input;
          fn slot(&self) -> InstrumentSlot;
          fn output_dim(&self) -> usize { self.slot().dim() }
          fn encode(&self, input: &Self::Input) -> InstrumentResult&lt;Vec&lt;f32&gt;&gt;;

          /// Identifier of the underlying ensemble embedder (E1-E21) when this
          /// instrument is backed by a single embedder. Defaults to `None` for
          /// deterministic typed encoders that do not consume an ensemble.
          fn embedder_id(&amp;self) -&gt; Option&lt;crate::EmbedderIdRef&gt; { None }

          /// Frozen version tag (e.g. `"e5-large-v2-2024-11-15"`). Defaults to
          /// the static string `"deterministic"` for non-embedder-backed
          /// encoders.
          fn version(&amp;self) -&gt; &amp;str { "deterministic" }

          /// SHA-256 of the loaded weight file(s); 64 hex chars. Defaults to
          /// `None` for encoders that have no neural weights.
          fn weight_sha(&amp;self) -&gt; Option&lt;&amp;str&gt; { None }
      }

      // Type alias resolved by downstream `context-graph-mejepa-embedders` to
      // its concrete `EmbedderId` enum; defined here as an open transparent
      // newtype so the upstream crate need not depend on the embedders crate.
      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
      pub struct EmbedderIdRef(pub u8);
    </signature>
    <signature file="crates/context-graph-mejepa-embedders/src/lib.rs">
      //! `context-graph-mejepa-embedders` — wraps the shipped 21-embedder
      //! ensemble (E1-E21) behind the `Instrument` trait. Per
      //! `docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md`.
      //!
      //! Inspired by `crates/context-graph-embeddings/src/types/model_id/core.rs`
      //! HEAD ef5274c2. Clean-room reimplementation; no upstream code copied.

      pub mod cache;        // forward-pass cache (filled in TASK-EMBED-009)
      pub mod diagnostics;  // coherence + collapse + agent_state (TASK-EMBED-022, 021)
      pub mod digest;       // streaming SHA-256 verifier (TASK-EMBED-008)
      pub mod ensemble;     // Ensemble manager (TASK-EMBED-009)
      pub mod error;        // EmbedError taxonomy (TASK-EMBED-005)
      pub mod instrument;   // EmbedderInstrument&lt;E&gt; wrapper (TASK-EMBED-010..019)
      pub mod lora_manager; // LoraAdapterManager (TASK-EMBED-020)
      pub mod routing;      // RoutingTable (TASK-EMBED-006)
      pub mod vram_budget;  // VramBudget + cuMemGetInfo (TASK-EMBED-007)

      // EmbedderId / EmbedderInput / EmbedderOutput re-exports land in
      // TASK-EMBED-002 and TASK-EMBED-003.
    </signature>
    <signature file="crates/context-graph-mejepa-embedders/Cargo.toml">
      [package]
      name = "context-graph-mejepa-embedders"
      version = "0.1.0"
      edition = "2021"
      description = "ME-JEPA-Code Phase 1b: 21-embedder ensemble wrapped behind the Instrument trait"
      license = "MIT OR Apache-2.0"

      [features]
      default = []
      gpu = ["cudarc/cuda-12000"]
      strict-fp32-replay = []

      [dependencies]
      context-graph-mejepa-instruments = { path = "../context-graph-mejepa-instruments" }
      context-graph-embeddings        = { path = "../context-graph-embeddings" }
      context-graph-core              = { path = "../context-graph-core" }
      candle-core                     = { workspace = true }
      candle-nn                       = { workspace = true }
      candle-transformers             = { workspace = true }
      cudarc                          = { workspace = true, optional = false }
      tokenizers                      = { workspace = true }
      sha2                            = { workspace = true }
      ed25519-dalek                   = { workspace = true }
      tokio                           = { workspace = true, features = ["full"] }
      thiserror                       = { workspace = true }
      serde                           = { workspace = true, features = ["derive"] }
      serde_json                      = { workspace = true }
      uuid                            = { workspace = true, features = ["v4", "serde"] }
      chrono                          = { workspace = true, features = ["serde"] }

      [dev-dependencies]
      tempfile = { workspace = true }
    </signature>
  </signatures>

  <constraints>
    - File `lib.rs` &lt; 500 lines (constitution `&lt;file_organization&gt;`).
    - The 3 new trait methods MUST have default impls so existing impls in `e_oracle.rs`, `e_witness.rs`, `e_static_analysis.rs`, `e_text.rs`, `e_code.rs`, `e_trace.rs`, `e_runtime.rs`, `e_reasoning.rs`, `e_scalars.rs` compile unchanged.
    - The `EmbedderIdRef` newtype is a transparent `u8` so downstream `EmbedderId` enum (TASK-EMBED-002) can implement `Into&lt;EmbedderIdRef&gt;` without circular dep.
    - Every created file carries the provenance header per constitution `&lt;coding_standards&gt;` provenance:
      `// Inspired by crates/context-graph-embeddings/src/types/model_id/core.rs HEAD ef5274c2.`
      `// Clean-room reimplementation; no upstream code copied.`
    - No `unwrap` / `expect` in lib.rs module skeleton (just module declarations).
    - No `mod tests` block yet — tests land in their respective tasks.
  </constraints>

  <verification>
    - `cargo check -p context-graph-mejepa-embedders` succeeds.
    - `cargo build --release -p context-graph-mejepa-instruments` still succeeds (default trait methods don't break existing impls).
    - `cargo test -p context-graph-mejepa-instruments` still passes (existing tests unaffected).
    - `cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings` clean (skeleton has no logic).
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa-embedders/Cargo.toml:
  - Standard package metadata; workspace = true on dep versions.
  - `[features]` gpu enables cudarc/cuda-12000; strict-fp32-replay is a build flag for FP32 verification path.

crates/context-graph-mejepa-embedders/src/lib.rs:
  - Module declarations only; concrete types added in subsequent tasks.
  - Provenance comment header.

crates/context-graph-mejepa-instruments/src/lib.rs:
  - Add `EmbedderIdRef(pub u8)` transparent newtype.
  - Extend `Instrument` trait with 3 default-impl methods.
  - Re-export `EmbedderIdRef` at crate root.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/Cargo.toml">Crate manifest with workspace deps</file>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Module skeleton + provenance header</file>
  <file path="crates/context-graph-mejepa-embedders/src/error.rs">Empty placeholder; populated by TASK-EMBED-005</file>
  <file path="crates/context-graph-mejepa-embedders/src/instrument.rs">Empty placeholder; populated by TASK-EMBED-010+</file>
  <file path="crates/context-graph-mejepa-embedders/src/cache.rs">Empty placeholder; populated by TASK-EMBED-009</file>
  <file path="crates/context-graph-mejepa-embedders/src/digest.rs">Empty placeholder; populated by TASK-EMBED-008</file>
  <file path="crates/context-graph-mejepa-embedders/src/diagnostics.rs">Empty placeholder; populated by TASK-EMBED-022</file>
  <file path="crates/context-graph-mejepa-embedders/src/ensemble.rs">Empty placeholder; populated by TASK-EMBED-009</file>
  <file path="crates/context-graph-mejepa-embedders/src/lora_manager.rs">Empty placeholder; populated by TASK-EMBED-020</file>
  <file path="crates/context-graph-mejepa-embedders/src/routing.rs">Empty placeholder; populated by TASK-EMBED-006</file>
  <file path="crates/context-graph-mejepa-embedders/src/vram_budget.rs">Empty placeholder; populated by TASK-EMBED-007</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Extend Instrument trait with embedder_id / version / weight_sha (default impls); add EmbedderIdRef newtype</file>
  <file path="Cargo.toml">Add `crates/context-graph-mejepa-embedders` to workspace members if explicit listing is used (otherwise glob already catches it)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo check -p context-graph-mejepa-embedders succeeds with empty placeholders</criterion>
  <criterion>cargo build --release -p context-graph-mejepa-instruments unchanged behaviour</criterion>
  <criterion>Existing context-graph-mejepa-instruments tests pass unchanged</criterion>
  <criterion>cargo clippy --all-features -- -D warnings passes for both crates</criterion>
  <criterion>Provenance header present on every created .rs and Cargo.toml file</criterion>
  <criterion>No `unwrap` or `expect` in skeleton files</criterion>
</validation_criteria>

<test_commands>
  <command>cargo check -p context-graph-mejepa-embedders</command>
  <command>cargo build --release -p context-graph-mejepa-instruments -p context-graph-mejepa-embedders</command>
  <command>cargo test -p context-graph-mejepa-instruments</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
