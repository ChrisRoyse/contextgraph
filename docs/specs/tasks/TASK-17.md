# TASK-PERF-005: Add parking_lot::RwLock to wake_controller

```xml
<task_spec id="TASK-PERF-005" version="1.0">
<metadata>
  <title>Add parking_lot::RwLock to wake_controller</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements><requirement_ref>REQ-PERF-005</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>1</estimated_hours>
</metadata>

<context>
The wake_controller uses std::sync::RwLock which has OS-level overhead and can cause
priority inversion. parking_lot::RwLock is faster (spinlock-based for short holds)
and provides fair scheduling. Constitution: ISS-015 performance concern.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/wake_controller.rs
- /home/cabdru/contextgraph/crates/context-graph-core/Cargo.toml
</input_context_files>

<scope>
<in_scope>
- Add parking_lot dependency to context-graph-core
- Replace std::sync::RwLock with parking_lot::RwLock in wake_controller
- Update lock() calls (parking_lot has different API)
</in_scope>
<out_of_scope>
- Other RwLock usages in crate
- Async lock conversion (different issue)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/dream/wake_controller.rs
use parking_lot::RwLock;

pub struct WakeController {
    state: RwLock<WakeState>,
    // ...
}

impl WakeController {
    pub fn get_state(&self) -> WakeState {
        // parking_lot returns guard directly, no Result
        self.state.read().clone()
    }

    pub fn set_state(&self, new_state: WakeState) {
        *self.state.write() = new_state;
    }
}
```
</signatures>
<constraints>
- MUST use parking_lot::RwLock, not std::sync::RwLock
- Lock acquisitions MUST NOT use .unwrap() (parking_lot doesn't return Result)
- parking_lot version MUST be "0.12"
</constraints>
<verification>
```bash
cargo check -p context-graph-core
cargo tree -p context-graph-core | grep parking_lot
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-core/Cargo.toml (add parking_lot)
- crates/context-graph-core/src/dream/wake_controller.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core wake_controller
```
</test_commands>
</task_spec>
```

## Implementation Notes

### parking_lot vs std::sync

| Feature | std::sync | parking_lot |
|---------|-----------|-------------|
| Poisoning | Yes (Result) | No (direct) |
| Performance | OS mutex | Spinlock hybrid |
| Fairness | None | Fair queuing |
| Size | 40+ bytes | 8 bytes |

### API Differences

```rust
// std::sync::RwLock
let guard = lock.read().unwrap();  // Returns Result
let guard = lock.write().unwrap(); // Returns Result

// parking_lot::RwLock
let guard = lock.read();  // Returns guard directly
let guard = lock.write(); // Returns guard directly
```

### When NOT to use parking_lot

- When lock is held across await points (use tokio::sync instead)
- When poisoning semantics are required
