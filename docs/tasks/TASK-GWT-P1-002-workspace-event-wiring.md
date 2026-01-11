# TASK-GWT-P1-002: Workspace Event Wiring to Subsystems

```xml
<task_spec id="TASK-GWT-P1-002" version="1.0">
<metadata>
  <title>Wire Workspace Events to Dream, Neuromodulation, and MetaCognitive Subsystems</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>4</sequence>
  <implements>
    <item>PRD: Workspace Events (lines 39-44 of gwt/mod.rs documentation)</item>
    <item>Constitution: neuromod.Dopamine.trigger = memory_enters_workspace</item>
    <item>Constitution: dream replay from memory_exits_workspace</item>
    <item>Constitution: workspace_empty triggers epistemic action</item>
    <item>Sherlock-01 GAP 4: Event listeners not connected to subsystems</item>
    <item>Sherlock-01 GAP 3: Dopamine feedback for losing WTA candidates</item>
  </implements>
  <depends_on>
    <task_ref>TASK-GWT-P0-001</task_ref>
    <task_ref>TASK-GWT-P0-002</task_ref>
    <task_ref>TASK-GWT-P0-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
The WorkspaceEventBroadcaster exists in the GWT system but has NO listeners registered.
Events like MemoryEnters, MemoryExits, WorkspaceConflict, and WorkspaceEmpty fire into
the void. This task wires these events to their designated handlers:
- DreamController queues MemoryExits for offline replay consolidation
- NeuromodulationManager boosts dopamine on MemoryEnters
- MetaCognitiveLoop triggers epistemic action on WorkspaceEmpty
- Losing WTA candidates receive dopamine reduction (completing WTA algorithm step 6)
</context>

<input_context_files>
  <file purpose="WorkspaceEventBroadcaster and WorkspaceEvent enum definitions">
    crates/context-graph-core/src/gwt/workspace.rs
  </file>
  <file purpose="GwtSystem struct that owns event_broadcaster">
    crates/context-graph-core/src/gwt/mod.rs
  </file>
  <file purpose="DreamController for MemoryExits handling">
    crates/context-graph-core/src/dream/controller.rs
  </file>
  <file purpose="NeuromodulationManager for dopamine boost on MemoryEnters">
    crates/context-graph-core/src/neuromod/state.rs
  </file>
  <file purpose="DopamineModulator for on_workspace_entry() and on_negative_event()">
    crates/context-graph-core/src/neuromod/dopamine.rs
  </file>
  <file purpose="MetaCognitiveLoop for WorkspaceEmpty handling">
    crates/context-graph-core/src/gwt/meta_cognitive.rs
  </file>
  <file purpose="WorkspaceEventListener trait definition">
    crates/context-graph-core/src/gwt/workspace.rs
  </file>
</input_context_files>

<prerequisites>
  <check>WorkspaceEventBroadcaster exists with listeners vector</check>
  <check>WorkspaceEvent enum has MemoryEnters, MemoryExits, WorkspaceConflict, WorkspaceEmpty variants</check>
  <check>WorkspaceEventListener trait exists with on_event() method</check>
  <check>DreamController has mechanism to queue memories (winner_history exists as reference)</check>
  <check>NeuromodulationManager has on_workspace_entry() method for dopamine</check>
  <check>DopamineModulator has on_negative_event() method for losers</check>
  <check>MetaCognitiveLoop can detect need for epistemic action</check>
</prerequisites>

<scope>
  <in_scope>
    - Add register_listener() method to WorkspaceEventBroadcaster
    - Create DreamEventListener implementing WorkspaceEventListener
    - Create NeuromodulationEventListener implementing WorkspaceEventListener
    - Create MetaCognitiveEventListener implementing WorkspaceEventListener
    - Wire MemoryExits -> DreamController::queue_for_replay()
    - Wire MemoryEnters -> NeuromodulationManager::on_workspace_entry()
    - Wire WorkspaceEmpty -> MetaCognitiveLoop::trigger_epistemic_action()
    - Implement dopamine reduction for losing WTA candidates
    - Register all listeners during GwtSystem initialization
    - Add integration tests for event flow
  </in_scope>
  <out_of_scope>
    - KuramotoNetwork integration (handled in TASK-GWT-P0-001)
    - Background Kuramoto stepping (handled in TASK-GWT-P0-002)
    - MCP handler implementation (handled in TASK-GWT-P0-003)
    - Dream cycle execution logic (already implemented)
    - Neuromodulator decay logic (already implemented)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/gwt/workspace.rs">
impl WorkspaceEventBroadcaster {
    pub async fn register_listener(&self, listener: Box&lt;dyn WorkspaceEventListener&gt;);
}
    </signature>

    <signature file="crates/context-graph-core/src/gwt/listeners.rs">
/// Listener that queues exiting memories for dream replay
pub struct DreamEventListener {
    dream_queue: Arc&lt;RwLock&lt;Vec&lt;Uuid&gt;&gt;&gt;,
}

impl WorkspaceEventListener for DreamEventListener {
    fn on_event(&amp;self, event: &amp;WorkspaceEvent);
}
    </signature>

    <signature file="crates/context-graph-core/src/gwt/listeners.rs">
/// Listener that boosts dopamine on memory entry
pub struct NeuromodulationEventListener {
    neuromod_manager: Arc&lt;RwLock&lt;NeuromodulationManager&gt;&gt;,
}

impl WorkspaceEventListener for NeuromodulationEventListener {
    fn on_event(&amp;self, event: &amp;WorkspaceEvent);
}
    </signature>

    <signature file="crates/context-graph-core/src/gwt/listeners.rs">
/// Listener that triggers epistemic action on workspace empty
pub struct MetaCognitiveEventListener {
    meta_cognitive: Arc&lt;RwLock&lt;MetaCognitiveLoop&gt;&gt;,
    epistemic_action_triggered: Arc&lt;AtomicBool&gt;,
}

impl WorkspaceEventListener for MetaCognitiveEventListener {
    fn on_event(&amp;self, event: &amp;WorkspaceEvent);
}
    </signature>

    <signature file="crates/context-graph-core/src/gwt/workspace.rs">
impl GlobalWorkspace {
    /// Apply dopamine reduction to losing WTA candidates
    pub async fn inhibit_losers(
        &amp;self,
        winner_id: Uuid,
        neuromod: &amp;mut NeuromodulationManager,
    ) -&gt; CoreResult&lt;usize&gt;;
}
    </signature>
  </signatures>

  <constraints>
    - Listeners must be Send + Sync to work with async broadcaster
    - DreamEventListener must NOT block - queue operation only
    - NeuromodulationEventListener dopamine boost must use on_workspace_entry()
    - Loser inhibition must use DopamineModulator::on_negative_event()
    - MetaCognitiveEventListener must set flag, not block on action
    - All listeners must handle all event variants (even if no-op)
    - Event handling must be non-panicking (log errors, don't crash)
    - Memory IDs from events must be propagated correctly
  </constraints>

  <verification>
    - Unit test: register_listener adds to internal vector
    - Unit test: DreamEventListener queues on MemoryExits, ignores other events
    - Unit test: NeuromodulationEventListener calls on_workspace_entry on MemoryEnters
    - Unit test: MetaCognitiveEventListener sets flag on WorkspaceEmpty
    - Unit test: inhibit_losers reduces dopamine for non-winners
    - Integration test: Full event flow from workspace selection to subsystem reaction
    - cargo test gwt::listeners passes
    - cargo test gwt::workspace::tests::test_listener passes
  </verification>
</definition_of_done>

<pseudo_code>
WorkspaceEventBroadcaster::register_listener (workspace.rs):
  Acquire write lock on listeners vector
  Push listener box into vector
  Log registration

DreamEventListener::on_event (listeners.rs):
  Match event variant:
    MemoryExits { id, .. } =>
      Spawn blocking task to acquire queue lock
      Push id to dream_queue
      Log: "Queued memory {} for dream replay"
    _ => no-op (ignore other events)

NeuromodulationEventListener::on_event (listeners.rs):
  Match event variant:
    MemoryEnters { id, order_parameter, .. } =>
      Spawn blocking task to acquire neuromod lock
      Call neuromod_manager.on_workspace_entry()
      Log: "Dopamine boosted for memory {} entering workspace (r={})"
    _ => no-op

MetaCognitiveEventListener::on_event (listeners.rs):
  Match event variant:
    WorkspaceEmpty { duration_ms, .. } =>
      Set epistemic_action_triggered flag to true
      Log: "Workspace empty for {}ms - epistemic action triggered"
    _ => no-op

GlobalWorkspace::inhibit_losers (workspace.rs):
  Count = 0
  For each candidate in self.candidates:
    If candidate.id != winner_id:
      Calculate inhibition_magnitude = 1.0 - candidate.score (higher score = less inhibition)
      Call neuromod.adjust(Dopamine, -inhibition_magnitude * DA_INHIBITION_FACTOR)
      Count += 1
  Return count of inhibited candidates

GwtSystem::new (mod.rs):
  Create event_broadcaster
  Create listeners with references to subsystems:
    - DreamEventListener with dream_queue Arc
    - NeuromodulationEventListener with neuromod_manager Arc
    - MetaCognitiveEventListener with meta_cognitive Arc
  Register all listeners with broadcaster
  Return initialized system

GwtSystem::select_workspace_memory (mod.rs):
  Call workspace.select_winning_memory(candidates)
  If winner selected:
    Broadcast MemoryEnters event for winner
    Call workspace.inhibit_losers(winner_id, &amp;mut neuromod)
    For each losing candidate:
      Broadcast MemoryExits event
  If no winner and workspace was previously active:
    Broadcast WorkspaceEmpty event
  Return winner
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/gwt/listeners.rs">
    New module containing:
    - DreamEventListener struct and impl
    - NeuromodulationEventListener struct and impl
    - MetaCognitiveEventListener struct and impl
    - DA_INHIBITION_FACTOR constant (0.1 per PRD)
    - Helper types for async listener operations
  </file>
  <file path="crates/context-graph-core/src/gwt/listeners/tests.rs">
    Unit tests for all three listeners
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/gwt/workspace.rs">
    - Add register_listener() method to WorkspaceEventBroadcaster
    - Add inhibit_losers() method to GlobalWorkspace
    - Add tests for new functionality
  </file>
  <file path="crates/context-graph-core/src/gwt/mod.rs">
    - Add pub mod listeners; declaration
    - Re-export listener types
    - Modify GwtSystem::new() to create and register listeners
    - Modify select_workspace_memory() to broadcast events and inhibit losers
    - Add neuromod_manager field to GwtSystem (or accept as parameter)
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>WorkspaceEventBroadcaster.register_listener() accepts and stores listeners</criterion>
  <criterion>DreamEventListener queues memory IDs on MemoryExits events</criterion>
  <criterion>NeuromodulationEventListener boosts dopamine on MemoryEnters events</criterion>
  <criterion>MetaCognitiveEventListener flags epistemic action on WorkspaceEmpty events</criterion>
  <criterion>Losing WTA candidates receive dopamine reduction via inhibit_losers()</criterion>
  <criterion>All three listeners registered during GwtSystem initialization</criterion>
  <criterion>Events broadcast during select_workspace_memory flow</criterion>
  <criterion>No panics on event handling - errors logged gracefully</criterion>
  <criterion>All existing tests continue to pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core gwt::listeners</command>
  <command>cargo test -p context-graph-core gwt::workspace::tests::test_register_listener</command>
  <command>cargo test -p context-graph-core gwt::workspace::tests::test_inhibit_losers</command>
  <command>cargo test -p context-graph-core gwt::tests::test_event_flow_integration</command>
  <command>cargo test -p context-graph-core neuromod::state::tests::test_manager_workspace_entry</command>
  <command>cargo clippy -p context-graph-core -- -D warnings</command>
</test_commands>

<notes>
## Event Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    GwtSystem::select_workspace_memory()          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │  GlobalWorkspace::select_winning_memory()          │
              │  - Filters by coherence (r >= 0.8)                 │
              │  - Ranks by score = r × importance × alignment     │
              │  - Selects top-1 winner                            │
              └───────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
              ▼                               ▼
       Winner Selected                  No Winner
              │                               │
              │                               ▼
              │                  ┌─────────────────────────┐
              │                  │ Broadcast: WorkspaceEmpty        │
              │                  └─────────────────────────┘
              │                               │
              │                               ▼
              │                  ┌─────────────────────────┐
              │                  │ MetaCognitiveEventListener       │
              │                  │ → Set epistemic_action flag      │
              │                  └─────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Broadcast: MemoryEnters (winner)                │
└─────────────────────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────────┐
│             NeuromodulationEventListener                         │
│             → NeuromodulationManager::on_workspace_entry()       │
│             → Dopamine boost for winner                          │
└─────────────────────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────────┐
│         GlobalWorkspace::inhibit_losers(winner_id, neuromod)     │
│         → For each non-winner candidate:                         │
│             - Calculate inhibition magnitude                     │
│             - DopamineModulator::on_negative_event()             │
│             - Broadcast: MemoryExits                             │
└─────────────────────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  DreamEventListener (for each loser)             │
│                  → Queue memory ID for dream replay              │
└─────────────────────────────────────────────────────────────────┘
```

## Constitution References

- **neuromod.Dopamine.trigger**: "memory_enters_workspace" - satisfied by NeuromodulationEventListener
- **dream replay**: "memory_exits_workspace" - satisfied by DreamEventListener
- **workspace_empty**: "epistemic action trigger" - satisfied by MetaCognitiveEventListener
- **WTA step 6**: "Inhibit: losing candidates receive dopamine reduction" - satisfied by inhibit_losers()

## Thread Safety Considerations

All listeners use Arc+RwLock patterns for thread-safe access to subsystems.
The WorkspaceEventListener::on_event() method is synchronous but listeners
should spawn tokio tasks for async operations to avoid blocking the broadcaster.

## Error Handling

Listeners must not panic. Use try-lock patterns and log errors:
```rust
if let Ok(mut guard) = self.neuromod_manager.try_write() {
    guard.on_workspace_entry();
} else {
    tracing::warn!("Could not acquire neuromod lock for workspace entry event");
}
```
</notes>
</task_spec>
```

---

## Appendix: Existing Code Analysis

### WorkspaceEventBroadcaster (Current State)

Location: `crates/context-graph-core/src/gwt/workspace.rs`

```rust
pub trait WorkspaceEventListener: Send + Sync {
    fn on_event(&self, event: &WorkspaceEvent);
}

pub struct WorkspaceEventBroadcaster {
    listeners: std::sync::Arc<tokio::sync::RwLock<Vec<Box<dyn WorkspaceEventListener>>>>,
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
```

**Missing**: `register_listener()` method.

### WorkspaceEvent (Current State)

```rust
pub enum WorkspaceEvent {
    MemoryEnters { id: Uuid, order_parameter: f32, timestamp: DateTime<Utc> },
    MemoryExits { id: Uuid, order_parameter: f32, timestamp: DateTime<Utc> },
    WorkspaceConflict { memories: Vec<Uuid>, timestamp: DateTime<Utc> },
    WorkspaceEmpty { duration_ms: u64, timestamp: DateTime<Utc> },
}
```

### NeuromodulationManager (Current State)

Location: `crates/context-graph-core/src/neuromod/state.rs`

```rust
impl NeuromodulationManager {
    pub fn on_workspace_entry(&mut self) {
        self.dopamine.on_workspace_entry();
    }
}
```

**Exists and ready for wiring.**

### DopamineModulator (Current State)

Location: `crates/context-graph-core/src/neuromod/dopamine.rs`

```rust
impl DopamineModulator {
    pub fn on_workspace_entry(&mut self) {
        self.level.value = (self.level.value + DA_WORKSPACE_INCREMENT).clamp(DA_MIN, DA_MAX);
        // ...
    }

    pub fn on_negative_event(&mut self, magnitude: f32) {
        let delta = magnitude.abs() * 0.1;
        self.level.value = (self.level.value - delta).clamp(DA_MIN, DA_MAX);
        // ...
    }
}
```

**Both methods exist and ready for wiring.**

### MetaCognitiveLoop (Current State)

Location: `crates/context-graph-core/src/gwt/meta_cognitive.rs`

The `MetaCognitiveLoop` handles Acetylcholine and dream triggering internally.
For workspace empty, we need to add a flag or method for epistemic action.

**May need minor extension for epistemic action flag.**

### DreamController (Current State)

Location: `crates/context-graph-core/src/dream/controller.rs`

Has `winner_history` tracking but no explicit "queue for replay" mechanism.
The `NremPhase` performs Hebbian replay but needs connection to exited memories.

**May need addition of replay queue or connection to existing history.**

---

## Traceability

| Requirement | Source | Implementation |
|-------------|--------|----------------|
| Dopamine boost on workspace entry | Constitution neuromod.Dopamine.trigger | NeuromodulationEventListener |
| Dream replay on workspace exit | PRD gwt.workspace_events | DreamEventListener |
| Epistemic action on workspace empty | PRD gwt.workspace_events | MetaCognitiveEventListener |
| Dopamine reduction for WTA losers | PRD gwt.global_workspace step 6 | GlobalWorkspace::inhibit_losers |
| Event broadcaster has listeners | Sherlock-01 GAP 4 fix | register_listener() + GwtSystem wiring |
