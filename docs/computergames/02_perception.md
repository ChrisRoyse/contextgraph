# 02 — Perception Subsystem

## 1. What "perception" means here

The perception subsystem is everything that turns the live state of the user's machine into a structured `Observation` value that the agent can read with low latency and low token cost.

It runs in two paths that can be active simultaneously:

| Path | Source of truth | When primary |
|---|---|---|
| **A11y path** | Windows UI Automation tree, WinEvent push, Chromium DevTools Protocol | The foreground app exposes a usable accessibility tree (most native apps, browsers, IDEs, Office) |
| **Pixel path** | GPU frame capture + small CNN + targeted OCR + audio loopback | The foreground app is a game, a canvas-based application, or any window where a11y is empty/insufficient |

`observe()` is the unified read. Both paths feed the same `Observation` struct; the agent does not pick a path, it picks a query, and the subsystem decides which sensors are cheapest and richest for the current foreground.

This doc covers: capture, a11y, detection, OCR, audio, and event derivation. Output schemas live in `06_data_schemas.md`.

---

## 2. Frame capture (`synapse-capture` crate)

### Strategy

Primary capture API: **Windows Graphics Capture API** via the `windows-capture` crate v2.x. Fallback for older Windows / unsupported configurations: **DXGI Output Duplication** via raw `windows-rs` bindings.

Both APIs expose captured frames as `ID3D11Texture2D` in GPU memory. **We do not copy the texture to system memory** unless an explicit caller (e.g. OCR, JPEG export, replay log) requires it. Detection inference runs on the texture directly via DirectML / CUDA.

### Capture targets

| Target | Use |
|---|---|
| Primary monitor | Default; whole-desktop capture |
| Specific monitor by index | Multi-monitor setups; profile-selectable |
| Specific window by HWND | When agent calls `set_capture_target(window=...)`; reduces VRAM and CPU |

Per-target settings:

- `cursor_visible: bool` — render the cursor into the frame (default true)
- `secondary_windows: bool` — include child/popup windows (default true for window targets)
- `min_update_interval_ms` — frame-rate cap (default 16ms = 60fps; set 33ms for 30fps when GPU-bound)
- `dirty_region_only: bool` — when true, perception skips processing if no dirty pixels (default true)

### Frame lifecycle

```rust
pub struct CapturedFrame {
    pub texture: SendablePtr<ID3D11Texture2D>,   // unsafe Send wrapper; consumer must scope correctly
    pub width: u32,
    pub height: u32,
    pub format: DxgiFormat,                      // typically B8G8R8A8_UNORM
    pub captured_at: Instant,
    pub frame_seq: u64,
    pub dirty_region: Option<Rect>,
}
```

Frames flow through a bounded channel (capacity 2). If the consumer can't keep up, the oldest frame is dropped — never block the capture thread. A `frames_dropped` metric is incremented.

### Capture thread

One OS thread per active capture target. Runs at `THREAD_PRIORITY_TIME_CRITICAL`. Communicates with tokio via `crossbeam::channel::bounded(2)`.

The thread loop:

```rust
loop {
    let frame = graphics_capture.wait_for_next_frame()?;       // blocks until vsync or 16ms tick
    if !frame.has_changes() && config.dirty_region_only {
        continue;
    }
    if tx.try_send(frame).is_err() {
        metrics.frames_dropped.inc();
    }
}
```

### Fallback path

If Graphics Capture API initialization fails (unsupported Windows build, no DWM, etc.), the capture crate falls back to DXGI Output Duplication. Same `CapturedFrame` output shape; capture rate may degrade to ~30fps on heavy desktops.

### VRAM and lifetime

- Maximum two textures resident per capture target (current frame + previous, for dirty diff).
- D3D resources released on shutdown via RAII (`Drop` impl on `CaptureHandle`).
- Capture continues running while no one consumes, but at the min update interval; no VRAM growth.

---

## 3. Accessibility tree (`synapse-a11y` crate)

### Three subsystems

**3a. UIA tree walker.** Wraps the `uiautomation` crate. Provides:

```rust
pub fn focused_window() -> Result<UIElement>;
pub fn focused_element() -> Result<UIElement>;
pub fn element_from_point(x: i32, y: i32) -> Result<UIElement>;
pub fn snapshot(element: &UIElement, depth: u32) -> Result<AccessibleSubtree>;
pub fn find(element: &UIElement, query: AccessibleQuery) -> Result<Vec<UIElement>>;
```

`AccessibleSubtree` is a serializable tree of `AccessibleNode`s with name, role/control-type, AutomationId, bounding rect, enabled, focused, pattern support (Invoke, Toggle, Value, Selection, ExpandCollapse, Scroll, Text), and a stable element id (Synapse-assigned; see `06_data_schemas.md`).

**Depth cap.** Default depth 2 around the focused element + 3 for a window snapshot. Cap is profile-overridable. UIA trees can be enormous (50K+ elements in Outlook); never walk the whole tree without bound.

**Cache request batching.** When snapshotting >100 elements, we use `IUIAutomationCacheRequest` to fetch all needed properties in a single cross-process COM round-trip. Per-element property fetch is ~1ms; cached batch fetch is ~10ms for hundreds of elements.

**3b. WinEvent hook.** Win32 `SetWinEventHook` with the COM apartment model. Subscribes to:

| Event | Fired when |
|---|---|
| `EVENT_SYSTEM_FOREGROUND` | Foreground window changed |
| `EVENT_OBJECT_FOCUS` | Focus moved (within or between windows) |
| `EVENT_OBJECT_VALUECHANGE` | Editable element value changed |
| `EVENT_OBJECT_NAMECHANGE` | Element name (title) changed |
| `EVENT_OBJECT_CREATE`, `EVENT_OBJECT_DESTROY` | Element appeared / disappeared |
| `EVENT_OBJECT_SELECTION`, `EVENT_OBJECT_SELECTIONADD`, `EVENT_OBJECT_SELECTIONREMOVE` | Selection changed |
| `EVENT_SYSTEM_MENUSTART`, `EVENT_SYSTEM_MENUEND` | Menu opened/closed |
| `EVENT_SYSTEM_ALERT` | Alert/error popup |

Hook callbacks run on the COM apartment thread. They marshal events into a `tokio::sync::mpsc::Sender<AccessibleEvent>` that the perception subsystem drains.

**3c. CDP client.** Chrome DevTools Protocol via `chromiumoxide`. Activated when:

- Foreground window's process is a Chromium-based browser (Chrome, Edge, Brave, Electron apps with debug port)
- Browser was launched with `--remote-debugging-port=<N>` OR `chrome.devtools_protocol` is reachable

Exposes:

- `DOMSnapshot.captureSnapshot` — full DOM + computed styles + bounds
- `Accessibility.getFullAXTree` — semantic AX tree
- `DOM.querySelector` — CSS selector query
- `Page.captureScreenshot` — fallback when DOM is sparse (canvas-heavy pages)
- `Network.requestWillBeSent` — request stream
- `Console.messageAdded` — console messages

CDP overrides UIA for the affected browser tab when available; richer data, no AX-tree-flattening of Chromium's internal hierarchy.

### Event filter

Synapse coalesces high-frequency a11y events to avoid spamming the agent and the storage layer:

- Events sharing `(window_id, element_id, kind)` within a 50ms window are merged into one `AccessibleEvent` with the latest values
- `EVENT_OBJECT_VALUECHANGE` on a text field that's actively being typed in is debounced to one event per 200ms (or on focus loss, whichever first)

---

## 4. Detection / inference (`synapse-perception::detect`)

When pixel-path perception is needed, we run a small object detection model on each captured frame.

### Model selection (default v1)

| Model | Size | Latency on RTX 3060 (DirectML) | Latency on RTX 5090 (CUDA) | Use |
|---|---|---|---|---|
| YOLOv8n / YOLOv10n | ~6 MB | ~6 ms @ 640px | ~2 ms @ 640px | Default. Object detection. |
| YOLOv8s | ~22 MB | ~10 ms | ~3 ms | Higher accuracy when latency budget permits |
| RT-DETR-s | ~80 MB | ~25 ms | ~5 ms | Stable-jitter alternative; selectable per profile |
| Florence-2-base (or similar) | ~480 MB | ~120 ms | ~25 ms | Slow loop only, ≤1 Hz, for unknown-game labeling |

**Selectable per profile.** A productivity profile (Notepad) disables detection entirely. A game profile pins YOLOv10n at 60 Hz.

### Inference path

```rust
trait Detector: Send + Sync {
    fn infer(&self, frame: &CapturedFrame, opts: DetectOpts) -> Result<DetectionBatch>;
}
```

Backend selection at startup, in priority order:
1. ONNX Runtime CUDA execution provider (NVIDIA only)
2. ONNX Runtime DirectML execution provider (any DX12 GPU)
3. ONNX Runtime CPU (degraded; only for small models in dev)

All via `ort` crate (Rust bindings to ONNX Runtime). One persistent inference session per loaded model. Reuse pinned input buffers.

### DetectionBatch

```rust
pub struct DetectionBatch {
    pub model_id: String,
    pub frame_seq: u64,
    pub inferred_at: Instant,
    pub items: Vec<Detection>,
}

pub struct Detection {
    pub class_label: String,       // from the model's class map
    pub bbox: Rect,                // pixel coords in capture frame
    pub confidence: f32,
    pub track_id: Option<u64>,     // assigned by simple IoU+name tracker
}
```

### Tracking

A lightweight tracker assigns persistent `track_id`s by IoU + class-name matching across consecutive frames. Track lifetime: a track that doesn't get a detection for 1000ms is retired. The agent can reference a stable `entity_id = track_id` in subsequent commands without re-querying every frame.

### Model cache

Models live in `%LOCALAPPDATA%\synapse\models\<sha256>.onnx`. On first need, Synapse downloads from a configured URL list (defaults to public GitHub releases). SHA-verified before load. No model is ever loaded without verification.

For offline use, the operator can side-load: `synapse-mcp models import <path>`.

---

## 5. OCR

Two OCR backends ship at v1:

| Backend | Latency p99 (1080p text region) | When to use |
|---|---|---|
| WinRT `Windows.Media.Ocr` | ~30 ms full screen, ~5 ms small region | Default. No external dep, ships with Windows. Excellent for printed text in most languages. |
| Fine-tuned CRNN (ONNX) | ~10 ms small region, ~80 ms full screen | Game HUD numbers (HP / ammo / score) where fonts are unusual; specifically trained per-profile. |

WinRT path uses `windows-rs` bindings to `Windows.Media.Ocr.OcrEngine`. Region cropping happens on the GPU texture; we only marshal the cropped region to system memory.

### Region targeting

The agent calls:

```
read_text(region={x,y,w,h}) -> { text, words: [{text, bbox, confidence}], language }
```

Default: read the focused element's bounds (UIA-resolved) or, if no a11y, the full screen.

### HUD extraction

Per-game profile defines named HUD regions:

```toml
[hud.hp]
region = { x = 50, y = 980, w = 120, h = 30 }
extractor = "crnn"           # or "winrt"
preprocessing = ["threshold:128", "invert"]
```

Profile-driven extraction runs in `synapse-perception::hud`. Returns:

```rust
pub struct HudReadings {
    pub by_name: HashMap<String, HudReading>,    // e.g. "hp" -> 85
}
pub struct HudReading {
    pub raw_text: String,
    pub parsed: HudValue,   // Number | String | Enum
    pub confidence: f32,
    pub stale_ms: u32,      // how old this reading is
}
```

### OCR cache

`CF_OCR_CACHE` keyed by `sha256(cropped_region_bytes)` → `OcrResult`. TTL 1h. Hit rate on stable HUDs (HP/ammo) is high; rebuilds dropped from 30 ms to ~0.1 ms.

---

## 6. Audio (`synapse-audio` crate)

### Loopback capture

Uses `wasapi` crate, system-default audio render device, loopback mode. Buffers last N seconds (default 5s) in a ring buffer.

```rust
pub fn start_loopback() -> Result<AudioHandle>;
pub fn tail_seconds(seconds: f32) -> Result<AudioWindow>;
pub fn subscribe_audio_events() -> impl Stream<Item = AudioEvent>;
```

`AudioWindow` is interleaved stereo f32 at the device's native sample rate (typically 48 kHz).

### STT

Whisper-tiny-int8 ONNX (~40 MB). Runs on demand when the agent calls `audio_transcribe()`. Latency ~150 ms for a 5-second window on CPU; ~50 ms with DirectML.

Not run continuously by default; only on call. Optional per-profile flag enables continuous transcription with rate limiting.

### Spatial direction estimate

Naive method: compute interleaved L/R energy ratio + GCC-PHAT-style cross-correlation lag. Returns:

```rust
pub struct DirectionEstimate {
    pub azimuth_deg: f32,          // 0 = front, +90 = right, -90 = left
    pub confidence: f32,
}
```

This is sufficient for FPS footstep direction at v1. Steam Audio (audionimbus crate) is the v2 upgrade for HRTF-accurate spatial localization.

### Audio events

Continuous detector for common audio events (running on the audio thread at low priority):

| Event | Trigger heuristic |
|---|---|
| `loud_transient` | RMS exceeds 5× moving average (gunshots, impacts) |
| `speech_started` | VAD pass through Silero VAD model (small, ~2 MB ONNX) |
| `speech_ended` | VAD passes silent for 500ms |
| `music_started` / `music_ended` | Spectral flatness threshold |

Events pushed to the event bus.

---

## 7. Event derivation

Both paths emit events:

| Event source | Examples |
|---|---|
| WinEvent | `focus_changed`, `window_opened`, `value_changed`, `selection_changed` |
| UIA structure | `element_appeared`, `element_disappeared` (computed by diffing snapshots) |
| CDP | `dom_mutation`, `navigation_committed`, `network_request_failed` |
| Detection | `entity_appeared` (new track), `entity_disappeared` (track timed out), `entity_class_changed` |
| HUD OCR | `hud_value_changed` (HP went from 100 → 85) |
| Audio | `loud_transient`, `speech_started`, `loud_transient_directional` |
| Filesystem | `file_created`, `file_changed`, `file_deleted` (via `notify` crate) |
| Process | `process_started`, `process_exited`, `socket_opened` |
| Clipboard | `clipboard_changed` |

All events normalized to `synapse-core::Event`:

```rust
pub struct Event {
    pub seq: u64,                    // monotonic across all sources
    pub at: SystemTime,
    pub source: EventSource,
    pub kind: EventKind,
    pub data: serde_json::Value,
    pub correlations: Vec<EventRef>, // e.g., "this hud_value_changed correlates with that entity_class_changed"
}
```

Full schema in `06_data_schemas.md`. Persistence in `CF_EVENTS` (`07_storage_and_profiles.md`).

---

## 8. The unified Observation

`observe()` returns this struct (JSON-serialized):

```rust
pub struct Observation {
    pub at: SystemTime,
    pub mode: PerceptionMode,                   // a11y_only | pixel_only | hybrid
    pub foreground: ForegroundContext,
    pub focused: Option<FocusedElement>,
    pub elements: Vec<AccessibleNode>,          // truncated by depth/N
    pub entities: Vec<DetectedEntity>,          // CNN detections with track ids
    pub hud: HudReadings,                       // game-only
    pub audio: AudioContext,                    // recent events + direction estimate if any
    pub recent_events: Vec<Event>,              // since last observe(), capped
    pub clipboard_summary: Option<ClipboardSummary>,
    pub fs_recent: Vec<FsEvent>,                // last 5 file changes
    pub diagnostics: ObservationDiagnostics,    // perception health (which sensors active, latencies)
}
```

Tokenization budget: aim for `serde_json::to_string(&observation)` to be ≤ 6 KB (≈ 1500 tokens) under typical conditions. Larger queries require explicit `expand(slot=...)` calls.

---

## 9. Perception mode auto-selection

```
on foreground change:
    profile = profiles::detect(hwnd)
    if profile.is_some():
        mode = profile.preferred_mode
    else:
        a11y_tree_size = a11y::snapshot(focused_window, depth=1).len()
        if a11y_tree_size > 5:
            mode = a11y_only
        else:
            mode = hybrid
    apply(mode):
        if a11y_only: stop pixel capture; keep UIA + WinEvent
        if pixel_only: stop UIA event handlers; keep capture + audio
        if hybrid: both
```

Profile-pinned mode wins over heuristics.

---

## 10. Health and degraded modes

Perception has structured health states:

| State | Meaning | Recovery |
|---|---|---|
| `healthy` | All requested sensors active and within latency budget | n/a |
| `degraded_latency` | One or more sensors exceeded latency budget recently | Auto-retry; surface in `diagnostics` |
| `degraded_sensor_failed` | A sensor crashed or returned errors persistently | Disable that sensor; surface in `diagnostics`; agent can `observe()` to see what's missing |
| `unavailable` | All sensors failed | Return `OBSERVE_NO_PERCEPTION_AVAILABLE` error from `observe()` |

Sensors recover independently. UIA failure does not stop pixel capture, and vice versa.

---

## 11. CPU / GPU budget

| Subsystem | Steady-state CPU | Steady-state GPU | VRAM |
|---|---|---|---|
| Capture (idle, no consumers) | ~0.1% | ~0% (texture shared with DWM) | ~10 MB |
| Capture (60 fps, consumer attached) | ~2% | ~2% | ~40 MB |
| Detection at 60 fps (YOLOv10n) | ~3% | ~15% on RTX 5090 | ~250 MB |
| UIA event subscriber | ~1% | 0 | ~10 MB |
| Audio loopback + ring buffer | ~0.5% | 0 | ~5 MB |
| OCR (on demand, region) | spike to ~5% for ~5 ms | spike to ~10% | ~50 MB |

Total ceiling at v1: 10% CPU, 25% GPU, 1.5 GB VRAM when fully active. Idle: <2% CPU, <1% GPU, <500 MB VRAM.

---

## 12. Coordinate systems

| System | Origin | Used in |
|---|---|---|
| Screen (virtual desktop) | Top-left of the union of all monitors | Mouse moves, top-level rect math |
| Monitor (per-monitor) | Top-left of one monitor | Per-target capture coords |
| Window client | Top-left of a specific window's client area | A11y bounding rects when window-relative |
| Capture frame | Top-left of captured texture | Detection bboxes |

DPI: all coordinates use **physical pixels** (per-monitor-aware v2 DPI scaling). Synapse calls `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at startup.

Transforms exposed:

```rust
pub fn screen_to_window(screen: Point, hwnd: HWND) -> Point;
pub fn window_to_screen(window: Point, hwnd: HWND) -> Point;
pub fn frame_to_screen(frame: Point, target: &CaptureTarget) -> Point;
```

All bounding rects in `Observation` are in screen coordinates unless explicitly tagged otherwise.

---

## 13. Failure surfaces

| Failure | Error code | Recovery |
|---|---|---|
| Graphics Capture unsupported | `CAPTURE_GRAPHICS_API_UNSUPPORTED` | Fall back to DXGI Output Duplication |
| Capture target HWND no longer valid | `CAPTURE_TARGET_LOST` | Surface to caller; agent picks a new target |
| UIA returned a stale element | `A11Y_ELEMENT_STALE` | Re-snapshot the parent |
| Detection model not loaded | `DETECTION_MODEL_NOT_LOADED` | Surface to caller; either operator imports model or perception runs without detection |
| OCR failed (no text found) | `OCR_NO_TEXT` | Not an error from agent's perspective; observation returns empty text |
| Audio device disconnected | `AUDIO_DEVICE_LOST` | Re-enumerate devices; resume on next default device |
| Per-app profile not found | `PROFILE_NOT_FOUND` | Use hybrid default |

Every error code is exported as a `pub const` in `synapse-core::error_codes`.

---

## 14. What this doc does NOT cover

- Per-tool MCP schemas → `05_mcp_tool_surface.md`
- Storage layout → `07_storage_and_profiles.md`
- Profile TOML format → `07_storage_and_profiles.md`
- Specific dependency versions → `14_build_and_packaging.md`
- Performance budgets per stage → `10_performance_budget.md`
- Event JSON shapes → `06_data_schemas.md`
