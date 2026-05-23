# 06 — Data Schemas

Canonical type definitions live in `synapse-core`. JSON serialization via `serde` (`#[serde(rename_all = "snake_case")]` everywhere). Bincode for RocksDB hot paths only.

This doc is the spec; the Rust code in `synapse-core/src/types.rs` is the implementation. Drift between them is a CI failure.

---

## 1. Core enums and primitives

### 1.1 Backend selection

```rust
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Software,    // Win32 SendInput
    Vigem,       // Virtual Xbox/DS4 via ViGEm
    Hardware,    // RP2040 HID gateway
    Auto,        // Resolve per call from profile + caller hint
}
```

### 1.2 Perception mode

```rust
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionMode {
    A11yOnly,
    PixelOnly,
    Hybrid,
    Auto,
}
```

### 1.3 Geometry

```rust
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Point { pub x: i32, pub y: i32 }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Size { pub w: u32, pub h: u32 }
```

All coordinates are physical pixels in the virtual desktop coordinate system unless tagged otherwise (per-window rects carry an explicit field).

### 1.4 IDs

All IDs are `String` (UUID-v7 or composite namespaced) for cross-version stability:

```rust
pub type SessionId = String;     // UUID-v7
pub type ElementId = String;     // "<hwnd>:<uia_runtime_id_hex>"
pub type EntityId = String;      // "track:<u64>"
pub type ReflexId = String;      // UUID-v7
pub type SubscriptionId = String; // UUID-v7
pub type ProfileId = String;     // "namespace.name", e.g., "minecraft.java"
```

`ElementId` is composite: the window HWND plus the UIA `RuntimeId` hex. This is stable across snapshots within a session but NOT across sessions; a re-launched window gets a new RuntimeId.

---

## 2. Observation

The unified perception result returned by `observe()`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub seq: u64,
    pub at: chrono::DateTime<chrono::Utc>,
    pub mode: PerceptionMode,
    pub foreground: ForegroundContext,
    pub focused: Option<FocusedElement>,
    pub elements: Vec<AccessibleNode>,
    pub entities: Vec<DetectedEntity>,
    pub hud: HudReadings,
    pub audio: AudioContext,
    pub recent_events: Vec<EventSummary>,
    pub clipboard_summary: Option<ClipboardSummary>,
    pub fs_recent: Vec<FsEvent>,
    pub diagnostics: ObservationDiagnostics,
}
```

### 2.1 ForegroundContext

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForegroundContext {
    pub hwnd: i64,
    pub pid: u32,
    pub process_name: String,         // basename, e.g., "notepad.exe"
    pub process_path: String,         // full path
    pub window_title: String,
    pub window_bounds: Rect,
    pub monitor_index: u32,
    pub dpi_scale: f32,
    pub profile_id: Option<ProfileId>,
    pub steam_appid: Option<u32>,     // resolved from Steam if applicable
    pub is_fullscreen: bool,
    pub is_dwm_composed: bool,
}
```

### 2.2 FocusedElement

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FocusedElement {
    pub element_id: ElementId,
    pub name: String,
    pub role: String,                 // UIA ControlType, e.g., "Button"
    pub automation_id: Option<String>,
    pub bbox: Rect,
    pub enabled: bool,
    pub patterns: Vec<UiaPattern>,
    pub value: Option<String>,        // if ValuePattern supported
    pub selected_text: Option<String>,// if TextPattern supports it
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiaPattern { Invoke, Toggle, Value, Selection, ExpandCollapse, Scroll, Text, Window, Transform, RangeValue }
```

### 2.3 AccessibleNode

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibleNode {
    pub element_id: ElementId,
    pub parent: Option<ElementId>,
    pub name: String,
    pub role: String,
    pub automation_id: Option<String>,
    pub bbox: Rect,
    pub enabled: bool,
    pub focused: bool,
    pub patterns: Vec<UiaPattern>,
    pub children_count: u32,          // not the children themselves
    pub depth: u32,
}
```

Tree is flattened — depth + parent enables reconstruction. Children are not nested to keep JSON small.

### 2.4 DetectedEntity

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectedEntity {
    pub entity_id: EntityId,
    pub track_id: u64,
    pub class_label: String,
    pub bbox: Rect,
    pub confidence: f32,
    pub first_seen_at: chrono::DateTime<chrono::Utc>,
    pub last_seen_at: chrono::DateTime<chrono::Utc>,
    pub velocity_px_per_s: Option<(f32, f32)>,
}
```

### 2.5 HudReadings

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HudReadings {
    pub by_name: std::collections::HashMap<String, HudReading>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HudReading {
    pub raw_text: String,
    pub parsed: HudValue,
    pub confidence: f32,
    pub stale_ms: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HudValue {
    Number(f64),
    Text(String),
    Enum(String),
    Null,
}
```

### 2.6 AudioContext

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioContext {
    pub rms_db: f32,
    pub vad_speech_recent: bool,
    pub recent_events: Vec<AudioEvent>,
    pub direction_estimate: Option<DirectionEstimate>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioEvent {
    pub at: chrono::DateTime<chrono::Utc>,
    pub kind: String,                 // "loud_transient" | "speech_started" | ...
    pub azimuth_deg: Option<f32>,
    pub confidence: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectionEstimate { pub azimuth_deg: f32, pub confidence: f32 }
```

### 2.7 ClipboardSummary

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardSummary {
    pub formats: Vec<String>,         // "text/plain", "image/png", ...
    pub text_len: Option<u32>,
    pub text_excerpt: Option<String>, // first ~120 chars, redacted if sensitive pattern matched
    pub redacted: bool,
}
```

### 2.8 FsEvent

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FsEvent {
    pub at: chrono::DateTime<chrono::Utc>,
    pub path: String,
    pub kind: FsEventKind,
    pub size_bytes: Option<u64>,
}
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FsEventKind { Created, Modified, Deleted, Renamed }
```

### 2.9 ObservationDiagnostics

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationDiagnostics {
    pub assembled_in_ms: f32,
    pub a11y_enabled: bool,
    pub pixel_enabled: bool,
    pub audio_enabled: bool,
    pub a11y_status: SensorStatus,
    pub capture_status: SensorStatus,
    pub detection_status: SensorStatus,
    pub audio_status: SensorStatus,
    pub elements_truncated: bool,
    pub entities_truncated: bool,
    pub size_bytes: u32,
    pub size_estimate_tokens: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorStatus {
    Healthy,
    DegradedLatency { last_p99_ms: f32 },
    DegradedSensorFailed { reason_code: String },
    Disabled,
    Unavailable,
}
```

---

## 3. Events

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,                     // monotonic across all sources
    pub at: chrono::DateTime<chrono::Utc>,
    pub source: EventSource,
    pub kind: String,                 // kebab-case, e.g., "entity-appeared"
    pub data: serde_json::Value,      // shape varies by kind
    pub correlations: Vec<EventRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    A11yUia,
    A11yWinEvent,
    A11yCdp,
    Perception,
    PerceptionDetection,
    PerceptionHud,
    PerceptionAudio,
    Filesystem,
    Process,
    Clipboard,
    ActionEmitter,
    Reflex,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventRef {
    pub seq: u64,
    pub relation: String,
}
```

`EventSummary` is the trimmed form returned in `Observation.recent_events`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventSummary {
    pub seq: u64,
    pub at: chrono::DateTime<chrono::Utc>,
    pub source: EventSource,
    pub kind: String,
    pub data_excerpt: serde_json::Value,    // capped to small size
}
```

### 3.1 Event kinds

| Kind | Source | `data` shape |
|---|---|---|
| `foreground-changed` | A11yWinEvent | `{ from_hwnd, to_hwnd, from_process, to_process }` |
| `focus-changed` | A11yWinEvent | `{ from_element_id, to_element_id }` |
| `element-appeared` | A11yUia | `{ element_id, parent, role, name }` |
| `element-disappeared` | A11yUia | `{ element_id }` |
| `value-changed` | A11yUia | `{ element_id, old, new }` |
| `name-changed` | A11yUia | `{ element_id, old, new }` |
| `selection-changed` | A11yUia | `{ container_id, selected_ids }` |
| `dom-mutation` | A11yCdp | `{ frame_id, mutation_summary }` |
| `navigation-committed` | A11yCdp | `{ frame_id, url, title }` |
| `entity-appeared` | PerceptionDetection | `{ entity_id, track_id, class_label, bbox, confidence }` |
| `entity-disappeared` | PerceptionDetection | `{ entity_id, track_id }` |
| `entity-class-changed` | PerceptionDetection | `{ entity_id, old, new }` |
| `hud-value-changed` | PerceptionHud | `{ field, old, new, confidence }` |
| `loud-transient` | PerceptionAudio | `{ azimuth_deg, rms_db, confidence }` |
| `speech-started` / `speech-ended` | PerceptionAudio | `{}` / `{ duration_ms }` |
| `file-created` / `file-changed` / `file-deleted` | Filesystem | `{ path, size_bytes? }` |
| `process-started` / `process-exited` | Process | `{ pid, name, cmdline?, exit_code? }` |
| `clipboard-changed` | Clipboard | `{ formats, text_len, redacted }` |
| `action-completed` | ActionEmitter | `{ action_kind, success, error_code?, duration_us }` |
| `reflex-registered` / `reflex-fired` / `reflex-cancelled` / `reflex-expired` | Reflex | `{ reflex_id, kind, ... }` |
| `system-shutdown` / `system-resume` | System | `{}` |

This is the v1 catalog. Additions go through ADR.

### 3.2 EventFilter

The mini-language used by `subscribe()` and `reflex_register(on_event)`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum EventFilter {
    All,
    None,
    Kind { kind: String },
    Source { source: EventSource },
    And { args: Vec<EventFilter> },
    Or  { args: Vec<EventFilter> },
    Not { arg: Box<EventFilter> },
    Data { path: String, predicate: DataPredicate },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DataPredicate {
    Eq { value: serde_json::Value },
    Ne { value: serde_json::Value },
    Lt { value: serde_json::Value },
    Le { value: serde_json::Value },
    Gt { value: serde_json::Value },
    Ge { value: serde_json::Value },
    Regex { pattern: String },
    InSet { values: Vec<serde_json::Value> },
    Exists,
}
```

`path` is JSON-Pointer style: `/track_id`, `/field`, `/bbox/x`. The evaluator is in `synapse-core::filter`.

Example: "low HP event":

```json
{
  "op": "and",
  "args": [
    {"op": "kind", "kind": "hud-value-changed"},
    {"op": "data", "path": "/field", "predicate": {"op": "eq", "value": "hp"}},
    {"op": "data", "path": "/new", "predicate": {"op": "lt", "value": 20}}
  ]
}
```

---

## 4. Action types

The full `Action` enum referenced in `03_action.md`. Each variant carries a `backend` field where applicable.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    KeyPress { key: Key, hold: Duration, backend: Backend },
    KeyDown  { key: Key, backend: Backend },
    KeyUp    { key: Key, backend: Backend },
    KeyChord { keys: Vec<Key>, hold: Duration, backend: Backend },
    TypeText { text: String, dynamics: KeystrokeDynamics, backend: Backend },

    MouseMove { to: MouseTarget, curve: AimCurve, duration: Duration, backend: Backend },
    MouseMoveRelative { dx: f32, dy: f32, backend: Backend },
    MouseButton { button: MouseButton, action: ButtonAction, hold: Duration, backend: Backend },
    MouseDrag { from: Point, to: Point, button: MouseButton, curve: AimCurve, duration: Duration, backend: Backend },
    MouseScroll { dy: i32, dx: i32, at: Option<Point>, backend: Backend },

    PadButton { pad: PadId, button: PadButton, action: ButtonAction, hold: Duration },
    PadStick  { pad: PadId, stick: Stick, x: f32, y: f32 },
    PadTrigger{ pad: PadId, trigger: Trigger, value: f32 },
    PadReport { pad: PadId, report: GamepadReport },

    AimAt    { target: AimTarget, style: AimStyle, deadline: Duration, backend: Backend },
    Combo    { steps: Vec<ComboStep>, backend: Backend },

    ReleaseAll,
}
```

### 4.1 Sub-types

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MouseTarget {
    Screen(Point),
    Element(ElementId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AimTarget {
    Screen(Point),
    Element(ElementId),
    Track(u64),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AimCurve {
    Instant,
    Linear,
    EaseInOut,
    Bezier { p1: (f32, f32), p2: (f32, f32) },
    Natural { params: AimNaturalParams },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AimNaturalParams {
    pub control_point_jitter: f32,
    pub tremor_stddev_px: f32,
    pub overshoot_prob: f32,
    pub overshoot_factor_range: (f32, f32),
    pub micro_correct_steps: u8,
    pub timing_stddev_ms: f32,
    pub seed: Option<u64>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AimStyle { Snap, Flick, Natural, Track }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeystrokeDynamics {
    Burst,
    Linear { ms_per_char: u32 },
    Natural { mean_iki_ms: f32, stddev_ms: f32, bigram_bias: bool },
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton { Left, Right, Middle, X1, X2 }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonAction { Press, Down, Up }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key {
    pub code: KeyCode,                // see vocab below
    pub use_scancode: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KeyCode {
    Named(String),                    // "a", "f1", "ctrl", "enter"
    Symbol(char),                     // single char
    HidCode(u8),                      // direct HID usage code (hardware backend only)
}

pub type PadId = u8;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PadButton { A,B,X,Y, Lb,Rb, Ls,Rs, Back,Start, Up,Down,Left,Right, Guide }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stick { Left, Right }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trigger { Left, Right }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GamepadReport {
    pub buttons: Vec<PadButton>,
    pub thumb_l: (f32, f32),          // -1.0..1.0
    pub thumb_r: (f32, f32),
    pub lt: f32,                      // 0.0..1.0
    pub rt: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComboStep {
    pub at_ms: u32,
    pub input: ComboInput,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComboInput {
    KeyDown { key: Key },
    KeyUp   { key: Key },
    KeyPress{ key: Key, hold_ms: u16 },
    MouseButton { button: MouseButton, action: ButtonAction },
    MouseMoveRel { dx: f32, dy: f32 },
    PadButton { pad: PadId, button: PadButton, action: ButtonAction },
    PadStick  { pad: PadId, stick: Stick, x: f32, y: f32 },
}
```

### 4.2 Key name vocabulary

Named keys (case-insensitive on input, normalized lowercase internally):

- Letters: `a` .. `z`
- Digits: `0` .. `9`
- Function: `f1` .. `f24`
- Arrows: `up`, `down`, `left`, `right`
- Modifiers: `ctrl`, `lctrl`, `rctrl`, `shift`, `lshift`, `rshift`, `alt`, `lalt`, `ralt`, `super`, `lsuper`, `rsuper`
- Whitespace / punctuation: `space`, `tab`, `enter`, `backspace`, `delete`, `escape`/`esc`, `home`, `end`, `pageup`, `pagedown`, `insert`
- Symbols: `minus`, `equals`, `lbracket`, `rbracket`, `backslash`, `semicolon`, `apostrophe`, `comma`, `period`, `slash`, `grave`/`backtick`
- Lock keys: `capslock`, `numlock`, `scrolllock`
- Numpad: `np_0`..`np_9`, `np_plus`, `np_minus`, `np_star`, `np_slash`, `np_dot`, `np_enter`
- Media: `volup`, `voldown`, `mute`, `play`, `next`, `prev`, `stop`
- Mouse pseudo-keys: `lmb`, `rmb`, `mmb`, `mbx1`, `mbx2`

Profile keymap aliases extend this set per-app (e.g., Minecraft profile maps `attack` → `lmb`, `place` → `rmb`, `inventory` → `e`).

---

## 5. Reflex types

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReflexRegistration {
    pub kind: ReflexKind,
    pub params: serde_json::Value,    // per-kind schema
    pub priority: u32,
    pub lifetime: ReflexLifetime,
    pub exclusive: bool,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflexKind { AimTrack, HoldMove, HoldButton, Combo, OnEvent }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReflexLifetime {
    OneShot,
    UntilCancelled,
    UntilEvent { filter: EventFilter },
    Duration { ms: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReflexState {
    pub reflex_id: ReflexId,
    pub kind: ReflexKind,
    pub registered_at: chrono::DateTime<chrono::Utc>,
    pub priority: u32,
    pub fired_count: u64,
    pub last_fired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: ReflexStatus,
    pub params: serde_json::Value,
    pub lifetime: ReflexLifetime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflexStatus {
    Active,
    Starved,
    AwaitingCondition,
    Terminated { reason: String, at: chrono::DateTime<chrono::Utc> },
}
```

---

## 6. Profile schema

Stored as TOML on disk under `profiles/<id>.toml`. Loaded into:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub label: String,
    pub version: String,                       // semver of this profile
    pub matches: Vec<ProfileMatch>,
    pub mode: PerceptionMode,
    pub capture: ProfileCapture,
    pub detection: ProfileDetection,
    pub ocr: ProfileOcr,
    pub hud: Vec<HudFieldSpec>,
    pub keymap: std::collections::HashMap<String, String>,    // alias -> key name
    pub backends: ProfileBackends,
    pub event_extensions: Vec<EventExtension>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileMatch {
    pub exe: Option<String>,                   // basename or full path regex
    pub title_regex: Option<String>,
    pub steam_appid: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileCapture {
    pub target: ProfileCaptureTarget,
    pub min_update_interval_ms: u32,
    pub cursor_visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileCaptureTarget {
    ForegroundWindow,
    PrimaryMonitor,
    MonitorIndex { index: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileDetection {
    pub model_id: Option<String>,              // None = disable detection
    pub classes_of_interest: Vec<String>,
    pub confidence_threshold: f32,
    pub max_detections: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileOcr {
    pub default_backend: OcrBackend,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrBackend { Winrt, Crnn, Auto }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HudFieldSpec {
    pub name: String,
    pub region: HudRegion,
    pub extractor: HudExtractor,
    pub parser: HudParser,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudRegion {
    Absolute { x: i32, y: i32, w: i32, h: i32 },
    FractionOfWindow { x: f32, y: f32, w: f32, h: f32 },
    AnchoredToEdge { edge: WindowEdge, x_offset: i32, y_offset: i32, w: i32, h: i32 },
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowEdge { TopLeft, TopRight, BottomLeft, BottomRight }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudExtractor {
    WinrtOcr,
    Crnn { model_id: String },
    TemplateMatch { templates: Vec<String> },   // sha or asset reference
    ColorRatio { sample_points: Vec<(i32, i32)>, mapping: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HudParser {
    Number,
    FractionNumerator,                          // "85/100" -> 85
    FractionDenominator,
    Regex { pattern: String, group: u32 },
    Enum { mapping: std::collections::HashMap<String, String> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileBackends {
    pub default: Backend,
    pub keyboard_default: Backend,
    pub mouse_default: Backend,
    pub pad_default: Backend,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventExtension {
    pub name: String,
    pub from_filter: EventFilter,
    pub emits_kind: String,
}
```

Profile TOML examples in `07_storage_and_profiles.md`.

---

## 7. Storage records (RocksDB values)

Most CF values are bincode-serialized for storage efficiency; some are JSON for human inspection. The choice is documented in `07_storage_and_profiles.md`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredEvent {
    pub event: Event,
    pub stored_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredObservation {
    pub obs: Observation,
    pub stored_at: chrono::DateTime<chrono::Utc>,
    pub reason: String,                         // "1hz_sample" | "before_action" | "user_requested"
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredReflexAudit {
    pub reflex_id: ReflexId,
    pub at: chrono::DateTime<chrono::Utc>,
    pub kind: String,                           // "registered" | "fired" | "cancelled" | "expired" | "starved"
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_id: SessionId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub client: String,                         // e.g., "claude-desktop/0.4.2"
    pub mode: PerceptionMode,
    pub active_profile: Option<ProfileId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub words: Vec<OcrWord>,
    pub language: Option<String>,
    pub backend: OcrBackend,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OcrWord {
    pub text: String,
    pub bbox: Rect,
    pub confidence: f32,
}
```

---

## 8. Error codes (full catalog)

Stable identifiers. Adding a code is a release-note item; renaming is forbidden until v2.

### 8.1 Perception

```
OBSERVE_NO_PERCEPTION_AVAILABLE
OBSERVE_INTERNAL
CAPTURE_GRAPHICS_API_UNSUPPORTED
CAPTURE_TARGET_LOST
CAPTURE_NO_DIRTY_REGIONS
A11Y_NOT_AVAILABLE
A11Y_ELEMENT_STALE
A11Y_NO_FOREGROUND
A11Y_CDP_UNREACHABLE
DETECTION_MODEL_NOT_LOADED
DETECTION_MODEL_INFER_FAILED
DETECTION_NO_FRAME
OCR_NO_TEXT
OCR_BACKEND_UNAVAILABLE
HUD_NO_ACTIVE_PROFILE
HUD_FIELD_NOT_DEFINED
HUD_EXTRACTION_FAILED
AUDIO_DEVICE_LOST
AUDIO_LOOPBACK_INIT_FAILED
AUDIO_STT_MODEL_NOT_LOADED
```

### 8.2 Action

```
ACTION_QUEUE_FULL
ACTION_RATE_LIMITED
ACTION_BACKEND_UNAVAILABLE
ACTION_TARGET_INVALID
ACTION_HOLD_EXCEEDED_MAX
ACTION_HID_PORT_DISCONNECTED
ACTION_VIGEM_NOT_INSTALLED
ACTION_VIGEM_PLUGIN_FAILED
ACTION_ELEMENT_NOT_RESOLVED
ACTION_FOREGROUND_LOST
ACTION_UNSUPPORTED_KEY
ACTION_DRAG_DISTANCE_EXCEEDS_LIMIT
STUCK_KEY_AUTO_RELEASED
SAFETY_RELEASE_ALL_FIRED
SAFETY_OPERATOR_HOTKEY_FIRED
```

### 8.3 Reflex

```
REFLEX_CAP_REACHED
REFLEX_KIND_INVALID
REFLEX_PARAMS_INVALID
REFLEX_TARGET_INVALID
REFLEX_FILTER_INVALID
REFLEX_PRIORITY_INVALID
REFLEX_TICK_LATE
REFLEX_TRACK_LOST
REFLEX_STARVED
REFLEX_DISABLED_BY_OPERATOR
REFLEX_LIFETIME_EXPIRED
```

### 8.4 Profile & config

```
PROFILE_NOT_FOUND
PROFILE_PARSE_ERROR
PROFILE_VERSION_INCOMPATIBLE
PROFILE_KEYMAP_INVALID
PROFILE_HUD_REGION_INVALID
CAPTURE_TARGET_INVALID
PERCEPTION_MODE_INVALID
```

### 8.5 MCP & session

```
SESSION_NOT_FOUND
SESSION_EXPIRED
SUBSCRIPTION_NOT_FOUND
SUBSCRIPTION_CAP_REACHED
TOOL_NOT_FOUND
TOOL_PARAMS_INVALID
TOOL_INTERNAL_ERROR
```

### 8.6 Storage

```
STORAGE_OPEN_FAILED
STORAGE_WRITE_FAILED
STORAGE_READ_FAILED
STORAGE_CORRUPTED
STORAGE_SCHEMA_MISMATCH
```

### 8.7 Models

```
MODEL_DOWNLOAD_FAILED
MODEL_HASH_MISMATCH
MODEL_LOAD_FAILED
MODEL_BACKEND_UNAVAILABLE
```

### 8.8 Hardware HID

```
HID_PORT_NOT_FOUND
HID_PORT_OPEN_FAILED
HID_PROTOCOL_HANDSHAKE_FAILED
HID_FIRMWARE_VERSION_MISMATCH
HID_COMMAND_REJECTED
HID_LINK_TIMEOUT
```

### 8.9 Safety

```
SAFETY_KILLSWITCH_ACTIVE
SAFETY_PROCESS_DENYLISTED
SAFETY_SHELL_DENIED_BY_POLICY
SAFETY_LAUNCH_DENIED_BY_POLICY
SAFETY_SECRET_REDACTED
```

All codes are exported as `pub const NAME: &str = "NAME";` in `synapse-core::error_codes`. Tests assert the constants match their literal string.

---

## 9. Versioning

`SCHEMA_VERSION` constant in `synapse-core`. Every persisted record carries this version. Reading a record with mismatched version returns `STORAGE_SCHEMA_MISMATCH`; the operator wipes the DB and restarts.

Pre-v1 versions: bump major freely. Post-v1: schema changes require ADR + migration plan or DB wipe with release-notes warning.

---

## 10. What this doc does NOT cover

- Storage layout, CF list, key encoding → `07_storage_and_profiles.md`
- Profile TOML examples → `07_storage_and_profiles.md`
- HID protocol record format → `09_hardware_hid_gateway.md`
- Tool API surface that consumes these types → `05_mcp_tool_surface.md`
