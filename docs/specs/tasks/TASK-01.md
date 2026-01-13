# TASK-ARCH-001: Create context-graph-cuda crate skeleton

```xml
<task_spec id="TASK-ARCH-001" version="1.0">
<metadata>
  <title>Create context-graph-cuda crate skeleton</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements><requirement_ref>REQ-ARCH-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
CUDA FFI code is currently scattered across multiple crates (context-graph-embeddings,
context-graph-graph) creating security audit complexity and duplicate unsafe code.
This task creates the new consolidated crate that will house all GPU FFI bindings.
Constitution: AP-26 requires fail-fast error handling, centralized unsafe code enables focused auditing.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-embeddings/Cargo.toml (reference structure)
- /home/cabdru/contextgraph/Cargo.toml (workspace members)
</input_context_files>

<scope>
<in_scope>
- Create crate directory structure: crates/context-graph-cuda/
- Create Cargo.toml with appropriate dependencies (cuda-sys, nvml-wrapper, libc)
- Create src/lib.rs with module declarations (ffi, safe)
- Create src/ffi/mod.rs placeholder
- Create src/safe/mod.rs placeholder
- Add crate to workspace Cargo.toml members
</in_scope>
<out_of_scope>
- Actual FFI implementations (TASK-ARCH-002, TASK-ARCH-003)
- Safe wrapper implementations (TASK-ARCH-004)
- CI gate script (TASK-ARCH-005)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-cuda/src/lib.rs
pub mod ffi;
pub mod safe;
pub mod error;

pub use safe::*;
pub use error::GpuError;
```
</signatures>
<constraints>
- Crate name MUST be "context-graph-cuda"
- Must follow workspace conventions (edition 2021, rust-version 1.75+)
- ffi module must be private by default (not re-exported)
- safe module must be public (re-exported via pub use)
</constraints>
<verification>
```bash
cd /home/cabdru/contextgraph
cargo check -p context-graph-cuda
cargo metadata --format-version=1 | jq '.packages[] | select(.name=="context-graph-cuda")'
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-cuda/Cargo.toml
- crates/context-graph-cuda/src/lib.rs
- crates/context-graph-cuda/src/ffi/mod.rs
- crates/context-graph-cuda/src/safe/mod.rs
- crates/context-graph-cuda/src/error.rs
</files_to_create>

<files_to_modify>
- Cargo.toml (add "crates/context-graph-cuda" to workspace members)
</files_to_modify>

<test_commands>
```bash
# Verify crate compiles
cargo check -p context-graph-cuda

# Verify workspace membership
grep -q "context-graph-cuda" Cargo.toml
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Cargo.toml Template

```toml
[package]
name = "context-graph-cuda"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
description = "Consolidated CUDA/GPU FFI bindings for context-graph"
license = "MIT"

[dependencies]
libc = "0.2"
nvml-wrapper = "0.10"
thiserror = "2.0"

[dev-dependencies]
# Tests will mock GPU access

[features]
default = []
cuda = []  # Enable CUDA support
```

### Error Type Template

```rust
// src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("CUDA driver error: {0}")]
    CudaDriver(String),

    #[error("FAISS error: {0}")]
    Faiss(String),

    #[error("Device not available: {0}")]
    DeviceNotAvailable(String),

    #[error("NVML error: {0}")]
    Nvml(String),

    #[error("Green Contexts initialization failed: {0}")]
    GreenContexts(String),
}
```
