# 04 — Reflex Runtime

## 1. Why reflexes exist

The agent's tool-use loop is slow. Even a fast model on a local network takes 200–500 ms to round-trip a single MCP request. A 60 fps game runs at 16.7 ms per frame. A fighting-game frame window can be 1–3 frames.

If we required every action to round-trip the model, the agent could never:

- Click on a fast-moving target that's only on-screen for a few frames
- React to "low HP" by using a medkit within a survivable window
- Hit frame-perfect combos
- Track an enemy whose position is updating every frame
- Auto-dismiss popups that block input until clicked

The reflex runtime is the sub-frame body. The agent installs *intents* — "track this enemy", "press medkit if HP drops below 20", "hold W until I tell you to stop" — and the runtime executes them locally at frame rate.

Reflexes are NOT:

- Goal planning (that's the model's job)
- Policies (that's the model's job — these are not learned, they're declarative)
- Long-lived agents (every reflex is a small reactive primitive)
- Stateful skill libraries (the runtime forgets every reflex when the agent unregisters it)

---

## 2. Surface

The reflex runtime exposes five named controller families plus a generic event→action binding:

| Controller | What it does |
|---|---|
| `aim_track` | Continuously moves the mouse so a tracked entity stays under the crosshair; cancels on track loss or explicit stop |
| `hold_move` | Holds a set of keys until a condition is met (timeout, event, or stop) |
| `hold_button` | Holds a pad button or mouse button until a condition is met |
| `combo` | Fires a frame-accurate sequence of inputs at precise ms offsets |
| `on_event` | When an event matches the registered filter, emits an action and (optionally) unregisters itself |

All of these are first-class MCP tools (see `05_mcp_tool_surface.md`). They are also addressable internally for composition.

Each registered reflex has:

- A stable `reflex_id` (UUID, returned to the agent)
- A `kind` (one of the families above)
- A `parameters` blob
- A `lifetime` (`OneShot`, `UntilCancelled`, `UntilEvent(filter)`, `Duration(d)`)
- A `priority` (used when two reflexes contend for the same device)

---

## 3. Execution model

```
       Event bus
  ┌────────────┐
  │ Perception │──┐                    ┌─ aim_track    ──► action emitter
  │   events   │  │                    │
  └────────────┘  │   ┌────────────┐   ├─ hold_move    ──► action emitter
                  ├──►│ Reflex     │──►│
  ┌────────────┐  │   │  scheduler │   ├─ hold_button  ──► action emitter
  │  Frame     │──┘   └────────────┘   │
  │  ticks     │                       ├─ combo        ──► action emitter
  └────────────┘                       │
                                       └─ on_event     ──► action emitter
```

The scheduler runs on a **dedicated OS thread** at `THREAD_PRIORITY_TIME_CRITICAL` with a 1 ms tick (driven by a `multimedia timer` / `CreateWaitableTimer` with a high-resolution period). It does not use the tokio runtime; tokio's scheduler jitter would cost us frames.

Each tick the scheduler:

1. Drains the event bus (events from perception, frame-ticks, completion notifications)
2. For every active reflex, in priority order:
   - If it's a continuous controller (`aim_track`, `hold_move`, `hold_button`), invoke its `step()` and queue any actions
   - If it's an `on_event` reflex, check whether the most recent events match; if so, fire its action
   - If it's a `combo`, check whether the next step's `at_ms` is due (or past); if so, fire it
3. Push queued actions through the bounded `try_send` channel into the action emitter
4. Sleep until the next tick

Slept time is typically 0.5–0.9 ms per tick. The thread parks via `WaitForSingleObject` on the timer handle.

---

## 4. The event bus

A small, in-process broadcast bus:

```rust
pub struct EventBus {
    senders: ArcSwap<Vec<Subscriber>>,
}
pub struct Subscriber {
    id: SubscriberId,
    filter: EventFilter,
    sender: crossbeam::channel::Sender<Event>,
}
```

- Publishers: perception (capture/a11y/audio), action emitter (completion notifications), reflex scheduler (its own state changes — published for observability).
- Subscribers: reflex scheduler (filtered by reflex bindings), MCP push-event subscribers (per-agent filter), storage writer (writes every event to `CF_EVENTS`).
- Backpressure: each subscriber has a bounded channel. If a slow subscriber backs up, the bus drops the event for that subscriber and increments `events_dropped_for_subscriber{id}` metric. Never blocks the publisher.

Bus capacity per subscriber: 4096 events. With the perception event rate of typically 100–1000 events/s, this is many seconds of buffer.

---

## 5. The five controller families

### 5.1 `aim_track`

```rust
pub struct AimTrackParams {
    pub target: AimTarget,                   // ScreenPoint | EntityTrack(track_id) | NamedElement
    pub axis: AimAxis,                       // XY | XOnly | YOnly
    pub gain: f32,                           // 0.0..2.0; 1.0 = exact tracking
    pub deadzone_px: f32,                    // ignore micro-movements
    pub max_speed_px_per_ms: f32,            // cap; protects against teleport-on-track-spike
    pub curve_per_step: AimCurve,            // usually Linear or short-Bezier per-step
    pub backend: Backend,
    pub lifetime: ReflexLifetime,
}
```

On each tick:

1. Resolve target position. If `EntityTrack(track_id)`, look up the current detection track. If missing, increment `track_lost_ticks`; after 3 consecutive misses → tick fails this cycle (does not cancel; another detection may revive).
2. Compute screen delta from cursor to target.
3. Apply deadzone, gain, speed cap.
4. Emit `MouseMoveRelative { dx, dy }`.
5. If `track_lost_ticks > 60` (1 second), unregister with reason `track_lost`.

Cancellation: agent calls `reflex_cancel(reflex_id)`. Scheduler unregisters, emits a `reflex_terminated` event.

### 5.2 `hold_move`

```rust
pub struct HoldMoveParams {
    pub keys: Vec<Key>,                       // e.g. [W, Shift]
    pub backend: Backend,
    pub lifetime: ReflexLifetime,
}
```

On registration: emits `KeyDown` for each key. On cancellation or lifetime expiry: emits `KeyUp` for each.

Cooperates with the action emitter's "held key" tracking — if some OTHER caller releases W, the reflex re-presses it on next tick (configurable; default `re_assert = false`).

### 5.3 `hold_button`

Identical structure for mouse buttons or pad buttons.

### 5.4 `combo`

```rust
pub struct ComboParams {
    pub steps: Vec<ComboStep>,                // time-stamped inputs in ms offsets from start
    pub backend: Backend,
}
pub struct ComboStep {
    pub at_ms: u32,                           // offset from combo start
    pub input: ComboInput,                    // KeyDown/Up/Press, MouseButton, PadButton, PadStick
}
```

Frame-accurate execution. On registration, the scheduler records the start `Instant`. On each tick, fires every step whose `at_ms` has passed and not yet fired. After the last step fires, the reflex auto-terminates.

If the scheduler tick falls behind (extreme system load), late steps are still fired but tagged `late_by_ms` in the audit log. The agent can detect late combos.

### 5.5 `on_event`

```rust
pub struct OnEventParams {
    pub when: EventFilter,                    // e.g., kind="hud_value_changed" AND field="hp" AND new<20
    pub then: Action,                         // any Action enum variant
    pub debounce_ms: u32,                     // re-fire suppression
    pub lifetime: ReflexLifetime,             // OneShot | UntilCancelled
}
```

On each event matching `when`:

1. Check debounce — if `now < last_fire + debounce_ms`, skip.
2. Emit `then` action via action emitter.
3. Record `last_fire = now`.
4. If `lifetime == OneShot`, unregister.

`EventFilter` is a small declarative expression language (see `06_data_schemas.md` §EventFilter) — kind match + optional field comparisons + optional source filter.

---

## 6. Conflict resolution

Two reflexes that contend for the same device (e.g., two `aim_track` reflexes both trying to move the mouse) resolve by:

1. **Priority.** Higher priority wins this tick.
2. **Newer over older,** ties broken by `reflex_id` ordinal.

If a reflex consistently loses for >2 seconds, it logs a `reflex_starved` event so the agent can fix the conflict.

The agent can also register reflexes with `exclusive: true` — only one such reflex per device class can be active; the previous is auto-cancelled.

---

## 7. Lifetimes

```rust
pub enum ReflexLifetime {
    OneShot,                              // fires once, unregisters
    UntilCancelled,                       // active until agent cancels
    UntilEvent { filter: EventFilter },   // active until matching event
    Duration(Duration),                   // active for at most this long
}
```

`UntilEvent` is the most useful for game agents: "track this enemy until they die" → `lifetime: UntilEvent { kind: "entity_disappeared", track_id: target_id }`.

Default lifetime: `UntilCancelled`. Agents should explicitly cancel reflexes they no longer want; the runtime does not garbage-collect.

There is a per-session cap (`max_reflexes_per_session: 32`) to prevent runaway registration. Hitting the cap returns `REFLEX_CAP_REACHED`.

---

## 8. Reflex audit log

Every reflex registration, every firing, every cancellation, every lifetime expiry, every conflict, every starvation is written to `CF_REFLEX_AUDIT` and surfaced via `reflex_history` MCP tool. Keyed by `(reflex_id, fired_at_ns)` → JSON record.

This is critical for debugging — when an agent says "my aim-track didn't work," we need to know whether it never registered, never matched a target, or matched but lost the priority contest.

---

## 9. Composition examples

The agent composes reflexes by registering several at once. The runtime treats them independently. Three example patterns:

### Pattern A: Track-and-shoot

```
reflex_register(aim_track, target=entity_42, lifetime=UntilEvent(kind=entity_disappeared))
reflex_register(on_event,
    when=(kind=entity_visible AND track_id=42 AND inside_crosshair),
    then=mouse_button(left, hold=16ms),
    debounce_ms=200)
```

The agent has assembled tracking + auto-fire. Runtime handles both at frame rate. Agent makes ONE round-trip.

### Pattern B: Survival hotkey

```
reflex_register(on_event,
    when=(kind=hud_value_changed AND field=hp AND new<20),
    then=key_press("4"),                # "4" binds to medkit slot in this game
    debounce_ms=2000,                   # don't burn medkits in spasms
    lifetime=UntilCancelled)
```

### Pattern C: Long press until something happens

```
reflex_register(hold_move,
    keys=[W],
    lifetime=UntilEvent(kind=arrived_at_destination))
```

The agent has handed control of the W key to the runtime. The agent doesn't poll. When the perception subsystem fires the `arrived_at_destination` event, the reflex releases W.

---

## 10. Reflex stacking and the scheduler's two phases

A tick has two phases:

**Phase 1 — event drain.** Drain bus, fire on_event reflexes, update target positions for aim_track / combo state.

**Phase 2 — controller step.** For each active continuous reflex (aim_track / hold_move / hold_button / combo), call `.step()` and queue actions.

This guarantees that within one tick, event-driven actions fire BEFORE continuous-controller actions for that tick. If the agent has both an `on_event` that should cancel an `aim_track`, the cancellation lands first.

---

## 11. Latency targets

| Stage | Target p99 |
|---|---|
| Tick interval | 1 ms (with 100 µs jitter) |
| Event → matching reflex check | ≤ 200 µs |
| Reflex matched → action queued | ≤ 100 µs |
| Action queued → action emitter dequeues | ≤ 500 µs |
| Action emitter → SendInput call | ≤ 1 ms (software) |
| Total event → input on the wire | ≤ 5 ms |

Frame-accurate combos need the tick scheduler to fire at the right ms; jitter of 100-300 µs is acceptable. A 1 ms tick with high-resolution timer gives this on Windows 11 if MMCSS is active. We do not use `tokio::time::sleep` here (its accuracy is poor below 5 ms on Windows).

---

## 12. Safe defaults and limits

| Limit | Default | Why |
|---|---|---|
| Max reflexes per session | 32 | Prevent runaway |
| Max reflex lifetime | 1 hour | Agents should cancel; this catches lost cancellations |
| `aim_track` max speed | 5000 px/ms (5 px/µs, generous) | Avoid teleport-on-spike |
| `hold_move` re-assert | false | Don't fight with the user |
| `combo` total length | 5 seconds | Anything longer should be a sequence of combos |
| Tick deadline miss | logged as `reflex_tick_late` | Detect host overload |
| Event bus subscriber buffer | 4096 events | Allow >4s buffer at 1 kHz |

---

## 13. Reflex disable switch

Operator-controlled kill switch:

- CLI: `--reflex-disabled` flag at startup
- Runtime: `reflex_disable_all()` MCP tool (emits `ReleaseAll` first)
- Hotkey: a user-bindable global hotkey (`reflex_panic_hotkey` config) immediately disables ALL reflexes and emits `ReleaseAll`. Default: `Ctrl+Alt+Shift+P`.

This is non-negotiable safety machinery. If anything goes wrong, the operator hits the hotkey, all reflexes terminate, all held inputs release.

---

## 14. Error codes

```rust
pub const REFLEX_CAP_REACHED: &str = "REFLEX_CAP_REACHED";
pub const REFLEX_TARGET_INVALID: &str = "REFLEX_TARGET_INVALID";
pub const REFLEX_FILTER_INVALID: &str = "REFLEX_FILTER_INVALID";
pub const REFLEX_PRIORITY_INVALID: &str = "REFLEX_PRIORITY_INVALID";
pub const REFLEX_TICK_LATE: &str = "REFLEX_TICK_LATE";
pub const REFLEX_TRACK_LOST: &str = "REFLEX_TRACK_LOST";
pub const REFLEX_STARVED: &str = "REFLEX_STARVED";
pub const REFLEX_DISABLED_BY_OPERATOR: &str = "REFLEX_DISABLED_BY_OPERATOR";
```

---

## 15. What this doc does NOT cover

- Specific MCP tool schemas for reflex management → `05_mcp_tool_surface.md`
- Event filter grammar → `06_data_schemas.md`
- Audit log persistence → `07_storage_and_profiles.md`
- Action back-end details → `03_action.md`
