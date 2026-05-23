# 03 — Action Subsystem

## 1. The hands

`synapse-action` is the only crate that emits input to the OS or to a virtual / hardware device. The contract is short:

> **Anything the agent or the reflex runtime decides to do flows through one mpsc actor that serializes by device and emits at the requested back-end. Nothing else touches the input APIs.**

This serialization guarantee is non-negotiable. It prevents stuck modifiers, double-clicks merging, and ordering bugs in combos.

---

## 2. Back-ends

Three back-ends ship at v1. Per call, the caller (or the active profile) picks one.

| Back-end | Path | Use |
|---|---|---|
| `software` | Win32 `SendInput` via the `enigo` crate (or direct `windows-rs` if `enigo` is limiting) | Default. Cheapest. Works for most apps and many single-player games. |
| `vigem` | Virtual Xbox 360 / DualShock 4 controller via `vigem-client` crate | Games that require a gamepad (analog movement, controller-only menus). Many games accept ViGEm even when they reject software input. |
| `hardware` | Serial to RP2040 HID gateway over USB-CDC | Games where ViGEm is detected. Single-player only. See `09_hardware_hid_gateway.md`. |

Selection rules:

1. If the caller explicitly names a back-end, use it.
2. Else, if the active profile names a default back-end, use it.
3. Else, use `software`.

ViGEm requires the ViGEmBus driver to be installed (one-time, signed). Hardware requires a flashed RP2040 board and a `--hardware-hid <COM_port>` argument or `SYNAPSE_HARDWARE_HID_PORT` env.

---

## 3. The action types

All actions emit through one `Action` enum (full schema in `06_data_schemas.md`):

```rust
pub enum Action {
    // Keyboard
    KeyPress { key: Key, hold: Duration, backend: Backend },
    KeyDown { key: Key, backend: Backend },
    KeyUp { key: Key, backend: Backend },
    KeyChord { keys: Vec<Key>, hold: Duration, backend: Backend },
    TypeText { text: String, dynamics: KeystrokeDynamics, backend: Backend },

    // Mouse
    MouseMove { to: MouseTarget, curve: AimCurve, duration: Duration, backend: Backend },
    MouseMoveRelative { dx: f32, dy: f32, backend: Backend },
    MouseButton { button: MouseButton, action: ButtonAction, hold: Duration, backend: Backend },
    MouseDrag { from: Point, to: Point, button: MouseButton, curve: AimCurve, duration: Duration, backend: Backend },
    MouseScroll { dy: i32, dx: i32, at: Option<Point>, backend: Backend },

    // Controller
    PadButton { pad: PadId, button: PadButton, action: ButtonAction, hold: Duration },
    PadStick { pad: PadId, stick: Stick, x: f32, y: f32 },      // -1.0 .. 1.0
    PadTrigger { pad: PadId, trigger: Trigger, value: f32 },    // 0.0 .. 1.0
    PadReport { pad: PadId, report: GamepadReport },            // full report

    // High-level intents (compiled internally)
    AimAt { target: AimTarget, style: AimStyle, deadline: Duration, backend: Backend },
    Combo { steps: Vec<ComboStep>, backend: Backend },

    // Lifecycle / safety
    ReleaseAll,
}
```

`KeyChord { ctrl+s }` is the cleanest way to express a hotkey. `Combo` is for frame-accurate sequences (e.g., fighting-game motions: `↓ → A` within 3 frames).

---

## 4. The serialization actor

```rust
pub struct ActionEmitter {
    rx: mpsc::Receiver<(Action, Sender<Result<()>>)>,
    software: SoftwareBackend,
    vigem: Option<VigemBackend>,
    hardware: Option<HidGateway>,
    held_keys: BitSet,            // for ReleaseAll safety
    held_buttons: BitSet,
    pad_state: HashMap<PadId, GamepadReport>,
}

impl ActionEmitter {
    pub async fn run(mut self, cancel: CancellationToken) {
        loop {
            tokio::select! {
                Some((action, ack)) = self.rx.recv() => {
                    let result = self.execute(action).await;
                    let _ = ack.send(result);
                },
                _ = cancel.cancelled() => {
                    self.release_all();
                    return;
                }
            }
        }
    }
}
```

Public API:

```rust
pub struct ActionHandle {
    tx: mpsc::Sender<(Action, Sender<Result<()>>)>,
}
impl ActionHandle {
    pub async fn execute(&self, action: Action) -> Result<()>;
    pub fn try_execute(&self, action: Action) -> Result<()>;  // for hot paths from reflex runtime; uses try_send
}
```

The bounded channel capacity is 256 actions. Saturation returns `ACTION_QUEUE_FULL` to the caller; this is a sign of agent runaway or reflex misconfiguration.

---

## 5. Software back-end (`SendInput`)

Wraps `enigo` 0.6+ but with several behavior overrides:

- **Absolute mouse moves are sent as relative deltas in steps,** following an `AimCurve`. Single absolute jump is reserved for `MouseMove { curve: Instant, .. }`.
- **`SendInput` is called in batches** when emitting curve steps (e.g., 50 deltas in one `SendInput([..50])` call). Per-call overhead amortizes from ~2 µs to ~0.04 µs per delta.
- **Modifier state is tracked locally.** `held_keys` ensures `ReleaseAll` releases everything we pressed, even on panic.
- **Unicode text** uses `KEYEVENTF_UNICODE` for arbitrary text. Falls back to per-char scan-code when an active game ignores Unicode input (game profile flag).
- **Raw scan codes** can be requested for games that read keyboard via raw input (most FPS games). Profile flag `keyboard.use_scancodes = true`.

Latency p99 (idle Windows 11, RTX 3060, foreground app responsive):

| Action | p99 |
|---|---|
| `KeyPress` (single key, 16ms hold) | ~1 ms |
| `MouseButton` (left click, 16ms hold) | ~1 ms |
| `MouseMove` (absolute, instant curve, 100px) | ~1 ms |
| `MouseMove` (cubic Bezier, 200ms, 60 steps) | 200 ms (intended) |
| `TypeText` (10 chars, default dynamics) | ~50 ms |

---

## 6. Aim curves

`AimCurve` is the parameterized cursor-movement model. Five curves ship at v1:

| Curve | Shape | Use |
|---|---|---|
| `Instant` | Single jump to target | Productivity (click a menu); only when speed matters more than naturalness |
| `Linear` | Constant velocity | Simple lerp; for `mouse_move` requests with explicit duration |
| `EaseInOut` | Smoothstep velocity | Default for productivity UI clicks |
| `Bezier { p1, p2 }` | Cubic Bezier with two control points | Default for game aim; control points sampled stochastically from a bounded region |
| `Natural { tremor, micro_correct }` | Bezier with overshoot + micro-correction + sub-pixel tremor | Anti-detection profile; mimics human motor signature |

Each curve emits `N` steps over the requested `duration`. Default `N = max(8, duration_ms / 4)`. Steps are spaced by:

- `Linear`: uniform
- `EaseInOut`: smoothstep `t' = 3t² - 2t³`
- `Bezier`: cubic Bezier sampling
- `Natural`: Bezier + Gaussian sub-pixel jitter per step + a final 1-3 step micro-correction sequence

### Why `Natural` matters

Modern anti-cheat systems and anti-bot frameworks detect linear / perfectly-Bezier cursor motion via:

- Inter-arrival timing variance (humans have Gaussian-ish per-step delays)
- Velocity profile (humans accelerate-decelerate, not constant)
- Endpoint precision (humans overshoot then micro-correct)
- Sub-pixel positioning (humans don't land on pixel-aligned targets perfectly)

`Natural` curve injects all four characteristics. Parameters are profile-tunable; the curve is deterministic given the same seed (we expose a seed for replay determinism).

### Curve parameters

```rust
pub struct AimNaturalParams {
    pub control_point_jitter: f32,    // 0..1, stddev as fraction of distance
    pub tremor_stddev_px: f32,        // sub-pixel Gaussian, e.g. 0.3
    pub overshoot_prob: f32,          // 0..1, e.g. 0.4
    pub overshoot_factor_range: (f32, f32),  // e.g. (1.05, 1.20)
    pub micro_correct_steps: u8,      // e.g. 2-3
    pub timing_stddev_ms: f32,        // per-step jitter
}
```

Defaults are tuned to match published human-aim datasets. The agent rarely needs to touch these; profiles set them per game.

---

## 7. Keystroke dynamics

Typing has the same authenticity problem. `KeystrokeDynamics`:

```rust
pub enum KeystrokeDynamics {
    Burst,                       // Send all chars in one SendInput
    Linear { ms_per_char: u32 }, // Equal spacing
    Natural { mean_iki_ms: f32, stddev_ms: f32, bigram_bias: bool },
}
```

`Natural` samples inter-keystroke interval (IKI) from a Gaussian; if `bigram_bias`, common bigrams ("th", "he", "in") get reduced IKI to match human bigram statistics.

Default for productivity apps: `Burst`. Default for game profiles where typing in chat matters: `Natural`.

---

## 8. ViGEm back-end

Wraps `vigem-client` 0.1+.

```rust
pub struct VigemBackend {
    client: vigem_client::Client,
    pads: HashMap<PadId, VigemPad>,
}
enum VigemPad {
    X360(vigem_client::Xbox360Wired),
    Ds4(vigem_client::DualShock4Wired),
}
```

- One `Client` per process (singleton). Connection is opened lazily on first `PadButton` / `PadStick` / `PadTrigger` / `PadReport`.
- Pads are plugged in on first reference; unplugged on shutdown (RAII; also handled via panic hook).
- `wait_for_ready` is called on plug-in to avoid `TargetNotReady` errors.
- `GamepadReport` is updated by accumulating partial commands (a `PadButton(A, down)` flips a bit in the cached report and re-sends).

### Pad state model

```rust
pub struct GamepadReport {
    pub buttons: PadButtons,           // bitflags
    pub thumb_lx: i16,                 // -32768..32767
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
    pub left_trigger: u8,              // 0..255
    pub right_trigger: u8,
}
```

Compatible with both X360 and DS4 reports. The back-end translates fields per pad type.

### Pad analog handling

`PadStick { x, y }` accepts -1.0..1.0; multiplied by 32767 internally. `PadTrigger { value }` accepts 0.0..1.0; multiplied by 255.

Smoothing: by default, stick deltas of >0.5 in 16ms snap immediately (game-driven smoothing handles overshoot). For racing/sim profiles, an `AnalogCurve::Smooth { tau_ms }` can be configured to interpolate stick movement over time.

---

## 9. Hardware HID back-end

When `--hardware-hid <port>` is set, an additional back-end is available. Routes to a serial-protocol driver in `synapse-hid-host` that talks to an RP2040 board running our firmware (`firmware/pico-hid/`).

The board enumerates as a generic HID composite device (mouse + keyboard + gamepad). The PC sees it as a real USB peripheral. No `SendInput`, no virtual driver, no signal interception possible.

Routing:

```rust
match action {
    Action::MouseMoveRelative { dx, dy, backend: Backend::Hardware } => {
        hid_gateway.send_mouse_delta(dx, dy)?;
    }
    Action::KeyPress { key, hold, backend: Backend::Hardware } => {
        hid_gateway.send_key(hid_code_for(key), hold)?;
    }
    /* ... */
}
```

Protocol, latency, and firmware design are in `09_hardware_hid_gateway.md`. Host driver in `synapse-hid-host`.

---

## 10. High-level intents

`AimAt` and `Combo` are not transmitted to OS directly — they are compiled at the actor into a sequence of primitive actions.

### AimAt compilation

```rust
Action::AimAt { target: ScreenPoint(820, 340), style: Snap, deadline: 60ms, backend: software }
↓
[
  MouseMove { to: (820, 340), curve: EaseInOut, duration: 60ms, backend: software },
]

Action::AimAt { target: EntityTrack(track_id), style: Track, deadline: 0ms, backend: software }
↓
  registers an aim-track reflex; emits MouseMoveRelative on each frame following the track
```

`AimStyle`:

- `Snap` — fast, EaseInOut, ~50ms default
- `Flick` — very fast Bezier, ~30ms default, with overshoot
- `Natural` — Bezier with all human-modeling params, 100-300ms
- `Track` — registers a reflex; not a one-shot

### Combo compilation

```rust
Action::Combo {
    steps: vec![
        ComboStep { input: Down(↓), at_ms: 0 },
        ComboStep { input: Down(→), at_ms: 16 },
        ComboStep { input: Press(A), at_ms: 33 },
    ],
    backend: hardware,
}
↓
schedules each step on the reflex runtime's tick wheel at the exact ms offset
```

Combo execution runs on the reflex runtime thread to hit frame-accurate timing.

---

## 11. The release-all safety net

Three layers ensure no stuck inputs:

1. **Per-action timeout.** `KeyDown` without a paired `KeyUp` within `held_key_max_duration_ms` (default 30s) emits an automatic `KeyUp` and logs `STUCK_KEY_AUTO_RELEASED`.
2. **Shutdown handler.** `ReleaseAll` is sent on SIGINT / SIGTERM and on the tokio cancellation token's cancellation.
3. **Panic hook.** A process-wide panic hook (`std::panic::set_hook`) calls a static `RELEASE_ALL_HANDLE: OnceCell<ActionHandle>` to fire `ReleaseAll` even on unhandled panic before the process dies.

`ReleaseAll` does:

- All tracked held keys → `KeyUp` via active back-end
- All tracked held mouse buttons → up
- All ViGEm pads → neutral report (no buttons, sticks centered, triggers 0)
- All hardware HID inputs → neutral

This runs in ≤ 10 ms.

---

## 12. Authorization layer

Not every action is allowed by default. The MCP handler applies:

| Action class | Default | Override |
|---|---|---|
| Mouse / keyboard / pad | allowed | — |
| Hardware HID | requires `--hardware-hid <port>` flag | per-call `backend: hardware` |
| Launch process | gated behind `--allow-launch <exe>` allowlist | profile may extend |
| Run shell | gated behind `--allow-shell <pattern>` allowlist | profile may extend |
| Clipboard write of sensitive content | per-call `confirm_sensitive: true` | env var disables prompt |

See `11_security_and_safety.md` for the full permission model.

---

## 13. Click-on-element semantics

A common agent pattern is "click the Save button." Two layers of resolution:

1. **A11y-targeted click.** Caller passes an `element_id` from a recent `observe()` result. The actor:
   - Re-resolves the element via UIA (it may have moved).
   - Calls `IUIAutomationInvokePattern::Invoke` if the element supports Invoke — semantic click, no cursor movement.
   - Falls back to clicking the center of the element's bounding rect with the chosen curve.
2. **Coordinate click.** Caller passes raw `(x, y)`. Pure pixel click.

Semantic invoke is faster and more reliable than coordinate click for productivity apps; it's the default when an element_id is provided.

For games (no a11y), only coordinate clicks are possible.

---

## 14. Drag, scroll, multi-click

- **Drag.** `MouseDrag { from, to, ... }` = `MouseButton(down) + MouseMove(curve) + MouseButton(up)`. Curve drives the in-flight motion.
- **Scroll.** `MouseScroll { dy, dx }` uses `SendInput` with `MOUSEEVENTF_WHEEL` / `MOUSEEVENTF_HWHEEL`. Optional `at` point first moves the cursor.
- **Double-click.** Two `MouseButton` actions within `GetDoubleClickTime()` (default 500ms). The actor injects the gap; the agent doesn't manage it.
- **Triple-click.** Three within the same time window. For text-selection ops on plain edit fields.

---

## 15. Input rate limiting

Per back-end caps to prevent the OS or virtual device from being overwhelmed:

| Back-end | Per-second cap |
|---|---|
| Software | 5000 events/s |
| ViGEm | 1000 reports/s (Xbox 360 USB poll rate ~1ms anyway) |
| Hardware HID | depends on USB poll rate; default 1000 events/s |

Saturation returns `ACTION_RATE_LIMITED` and re-queues the action with a small backoff.

---

## 16. Determinism and replay

Every action sent through the actor is persisted to `CF_EVENTS` with:

- The originating call site (MCP tool / reflex id)
- The actual back-end used
- The exact parameters
- The success/failure result
- The completion timestamp

`synapse-mcp replay <session_id>` replays the actions deterministically (same seeds, same curves) against the same back-end. Used for debug and for regression testing.

---

## 17. Error codes

```rust
pub const ACTION_QUEUE_FULL: &str = "ACTION_QUEUE_FULL";
pub const ACTION_RATE_LIMITED: &str = "ACTION_RATE_LIMITED";
pub const ACTION_BACKEND_UNAVAILABLE: &str = "ACTION_BACKEND_UNAVAILABLE";
pub const ACTION_TARGET_INVALID: &str = "ACTION_TARGET_INVALID";
pub const ACTION_HOLD_EXCEEDED_MAX: &str = "ACTION_HOLD_EXCEEDED_MAX";
pub const ACTION_HID_PORT_DISCONNECTED: &str = "ACTION_HID_PORT_DISCONNECTED";
pub const ACTION_VIGEM_NOT_INSTALLED: &str = "ACTION_VIGEM_NOT_INSTALLED";
pub const ACTION_ELEMENT_NOT_RESOLVED: &str = "ACTION_ELEMENT_NOT_RESOLVED";
pub const STUCK_KEY_AUTO_RELEASED: &str = "STUCK_KEY_AUTO_RELEASED";
pub const SAFETY_RELEASE_ALL_FIRED: &str = "SAFETY_RELEASE_ALL_FIRED";
```

---

## 18. What this doc does NOT cover

- Reflex bindings (aim_track, on_event) → `04_reflex_runtime.md`
- MCP tool surface that wraps these actions → `05_mcp_tool_surface.md`
- Hardware HID firmware design → `09_hardware_hid_gateway.md`
- Anti-cheat policy and back-end gating → `08_anti_cheat_policy.md`
