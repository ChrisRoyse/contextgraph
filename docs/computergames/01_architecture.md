# 01 — Architecture

## 1. Single-binary, multi-crate

Synapse ships as **one binary**: `synapse-mcp`. It is a workspace of focused crates so the boundaries are clear, tests are scoped, and any subsystem can be swapped without touching its peers.

```
crates/
├── synapse-mcp            (binary)
├── synapse-core           (shared types + error codes + constants)
├── synapse-capture        (frame capture)
├── synapse-a11y           (UIA + CDP + WinEvent)
├── synapse-perception     (detection + OCR + HUD + event derivation)
├── synapse-audio          (WASAPI loopback + STT + direction)
├── synapse-action         (input emit)
├── synapse-reflex         (sub-frame reactive runtime)
├── synapse-storage        (RocksDB)
├── synapse-profiles       (per-app/per-game profile loader)
├── synapse-hid-host       (serial driver for hardware HID gateway)
├── synapse-models         (ONNX runtime wrappers)
├── synapse-telemetry      (tracing + metrics + replay log)
└── synapse-test-utils
```

No crate depends on the binary. Each crate has its own `Cargo.toml`, its own tests, its own `Error` enum. The binary `synapse-mcp` is the only thing that wires them together.

## 2. Process model

**One process.** No spawned subprocesses (except optional shell-out for the agent's `act_run_shell` tool). All concurrency is via Tokio tasks.

```
synapse-mcp process
├── Tokio runtime (multi-threaded scheduler, 4 worker threads default)
│   ├── MCP transport task
│   │   ├── stdio reader (when --mode stdio)
│   │   └── Streamable HTTP server (when --mode http; axum)
│   ├── Capture loop task (one per active capture target)
│   │   └── On each frame: hand texture to perception
│   ├── A11y subscriber task
│   │   ├── UIA event handler (COM apartment thread)
│   │   ├── WinEvent hook (Win32 thread, message pump)
│   │   └── CDP WebSocket client (one per browser)
│   ├── Perception worker pool (CPU-bound; rayon or async tokio)
│   ├── Audio loopback task (real-time priority)
│   ├── Reflex runtime task (1ms tick, dedicated thread, parked when idle)
│   ├── Action emitter task (Win32 SendInput; per-target serialized)
│   ├── HID host serial task (when hardware gateway connected)
│   ├── Storage write task (batches RocksDB writes)
│   └── Telemetry exporter task (OTLP push every 10s)
```

### Threading rules

- **Main task = MCP transport.** All other tasks are spawned by it; it owns shutdown.
- **Capture and reflex use dedicated OS threads,** not the tokio pool, because their latency requirements don't tolerate scheduler jitter. They communicate with tokio via crossbeam channels.
- **COM apartment thread** for UIA event handlers (required by Windows COM model). Marshal results across the channel into the async world.
- **Real-time thread priority** (`SetThreadPriority(THREAD_PRIORITY_TIME_CRITICAL)`) on capture and reflex threads. The audio loopback thread also runs at MMCSS "Pro Audio".

### Concurrency invariants

- Action emission is **serialized per device**. We never have two tokio tasks calling `SendInput` simultaneously. `synapse-action` owns a single emitter task that drains a bounded `mpsc::Sender<Action>` channel.
- Perception results are **single-producer, multi-consumer**: the perception worker writes to a `tokio::sync::watch` channel; both the MCP `observe()` handler and the reflex runtime read from it.
- The reflex runtime is the **only writer to the action channel that isn't an MCP tool handler**. This means actions originate from exactly two places: explicit MCP tool calls, and registered reflexes. There is no third path.

## 3. Data flow

```
┌─────────────────────────────────────────────────────────────────┐
│  Sources                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐               │
│  │ GPU         │  │ UIA + Win   │  │ WASAPI      │               │
│  │ frame       │  │ event hook  │  │ loopback    │               │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘               │
│         │                 │                │                       │
│         ▼                 ▼                ▼                       │
│  ┌──────────────────────────────────────────────────────┐         │
│  │ synapse-perception → unified Observation             │         │
│  │  fields: focused_app, focused_element, entities,    │         │
│  │  hud, audio_events, recent_events, screen_summary   │         │
│  └────────────────────────┬─────────────────────────────┘         │
└────────────────────────────┼────────────────────────────────────┘
                             │
                ┌────────────┴────────────┐
                ▼                          ▼
        ┌───────────────┐         ┌──────────────────┐
        │ MCP observe() │         │ Reflex runtime   │
        │ JSON-RPC reply│         │ pattern matching │
        └───────┬───────┘         └────────┬─────────┘
                │                          │
                ▼                          ▼
         agent receives           reflex fires action
                                            │
                                            ▼
                                  ┌──────────────────┐
                                  │ synapse-action   │
                                  │ device-serialized│
                                  └────────┬─────────┘
                                           │
                                           ▼
                                    keyboard / mouse /
                                    controller / HID

         agent → MCP request → synapse-action (same path) → device
```

Two write paths to the device, both serialized through `synapse-action`. Two read paths into the agent, both routed through `synapse-perception`.

## 4. Persistent state

Single RocksDB instance, configurable path (default `%LOCALAPPDATA%\synapse\db`). Schema is wipe-and-rebuild on version change.

Column families at v1:

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_EVENTS` | `(monotonic_ts_ns, seq)` | bincode `Event` | Append-only replay log; TTL 7 days |
| `CF_OBSERVATIONS` | `(monotonic_ts_ns)` | bincode `Observation` | Sampled snapshots (1 Hz) for replay |
| `CF_PROFILES` | `profile_id` | toml bytes | Per-app/per-game profiles, cached after first load |
| `CF_MODEL_CACHE` | `model_sha256` | binary | Downloaded ONNX models, sha-verified |
| `CF_SESSIONS` | `session_id` | json | MCP session metadata |
| `CF_REFLEX_AUDIT` | `(reflex_id, fired_at_ns)` | json | Audit log of every reflex firing |
| `CF_OCR_CACHE` | `image_sha256` | bincode `OcrResult` | OCR memoization, TTL 1h |
| `CF_TELEMETRY` | `(metric_name, ts_ns)` | f64 | Local metric ringbuffer if no OTLP |

Detail in `07_storage_and_profiles.md`.

## 5. Workspace dependency graph

```
synapse-mcp ────────────────────────────────────┐
   │                                             │
   ├─► synapse-perception ───┐                   │
   │      ├─► synapse-capture                    │
   │      ├─► synapse-a11y                       │
   │      ├─► synapse-audio                      │
   │      ├─► synapse-models                     │
   │      └─► synapse-core                       │
   │                                             │
   ├─► synapse-reflex ───────► synapse-core      │
   │      └─► synapse-action                     │
   │                                             │
   ├─► synapse-action ───────┬─► synapse-core    │
   │      └─► synapse-hid-host                   │
   │                                             │
   ├─► synapse-storage ──────► synapse-core      │
   ├─► synapse-profiles ─────► synapse-core      │
   └─► synapse-telemetry ────► synapse-core      │
                                                  │
   synapse-core (zero internal deps) ◄────────────┘

   synapse-test-utils (zero internal deps; dev-only)
```

**Acyclic.** `synapse-core` has zero internal dependencies; it's the type/error/constant root. Everything else depends on it but nothing else depends back. This is enforced by `cargo build --workspace --check` in CI.

## 6. Crate responsibilities

### synapse-core

Shared domain types, error codes (`#[error("...")] enum SynapseError { ... }`), constants (CF names, slot dims, performance budgets), and the `Observation` / `Event` / `Action` enum hierarchy. Zero internal deps. Pure types + small helpers.

### synapse-capture

Windows GPU frame capture. Wraps `windows-capture` 2.x. Exposes:

```rust
pub struct CaptureTarget { /* monitor index or HWND */ }
pub trait FrameSink: Send + 'static {
    fn on_frame(&mut self, frame: CapturedFrame);
}
pub fn start_capture(target: CaptureTarget, sink: Box<dyn FrameSink>) -> Result<CaptureHandle>;
```

`CapturedFrame` carries an `ID3D11Texture2D` handle + monotonic timestamp + dirty region. **No CPU copy** unless the sink asks for one. Capture thread runs at `THREAD_PRIORITY_TIME_CRITICAL`.

Fallback path: DXGI Output Duplication for systems where Graphics Capture API is unavailable (rare on 10/11). Selectable via env var or per-target config.

### synapse-a11y

UIA tree walker + WinEvent hook + Chrome DevTools Protocol client. Exposes:

```rust
pub fn focused_window() -> Result<AccessibleWindow>;
pub fn snapshot(window: HWND, depth: u32) -> Result<AccessibleSubtree>;
pub fn subscribe_events(filter: EventFilter) -> impl Stream<Item = AccessibleEvent>;
pub fn cdp_attach(browser_endpoint: &str) -> Result<CdpClient>;
```

Uses `uiautomation` crate for UIA. Custom `windows-rs`-based WinEvent hook (no third-party crate is sufficient). CDP via `chromiumoxide` for Chromium browsers.

### synapse-perception

Receives frames and a11y events, runs detection / OCR / HUD extraction, and emits unified `Observation` + `Event`. This is the only crate that knows how to fuse pixel-derived and a11y-derived state into one structured view.

Sub-modules:

- `detect` — ONNX object detection (YOLO-nano, RT-DETR-s as alternates)
- `ocr` — WinRT `Windows.Media.Ocr` wrapper + optional CRNN for HUD-specific text
- `hud` — per-game HUD region extractors, driven by profiles
- `screen_summary` — coarse scene classification (which game / which app)
- `events` — derives semantic events from frame diffs and a11y mutations

Uses `synapse-models` for ONNX runtime.

### synapse-audio

WASAPI loopback capture, small STT model (Whisper-tiny ONNX), naive spatial-direction estimator (L/R channel energy + cross-correlation lag).

```rust
pub fn start_loopback(sink: impl AudioSink) -> Result<AudioHandle>;
pub fn transcribe_last(seconds: f32) -> Result<TranscriptResult>;
pub fn direction_estimate(seconds: f32) -> Result<Option<DirectionEstimate>>;
```

### synapse-action

The hands. Three back-ends, selected per call:

- `software` — Win32 `SendInput` (default)
- `vigem` — virtual Xbox/DS4 via `vigem-client`
- `hardware` — serial-over-USB to `synapse-hid-host`

```rust
pub fn click(target: Target, button: Button, opts: ClickOpts) -> Result<()>;
pub fn aim_to(target: Coord, curve: AimCurve, duration: Duration) -> Result<()>;
pub fn press(key: Key, frames: u8) -> Result<()>;
pub fn type_text(text: &str, dynamics: KeystrokeDynamics) -> Result<()>;
pub fn pad_update(report: GamepadReport) -> Result<()>;
```

Serialization invariant: at most one action in flight per device at a time, enforced by an mpsc actor pattern.

### synapse-reflex

Sub-frame reactive runtime. Owns the event bus and a small set of named controllers:

- `aim_track { target_id, axis, gain }` — runs at 1000 Hz until target lost or cancelled
- `hold_move { keys, until }` — keeps WASD pressed until condition
- `combo_sequence { steps }` — frame-accurate input chain
- `on_event { match, action }` — registered binding

Runs on a dedicated OS thread at `THREAD_PRIORITY_TIME_CRITICAL`. Pulls events from the bus, dispatches matched reflexes' actions through `synapse-action`.

### synapse-storage

RocksDB wrapper. Opens fixed CF list. Provides:

```rust
pub fn open(path: &Path) -> Result<Db>;
impl Db {
    pub fn put_event(&self, event: &Event) -> Result<()>;
    pub fn iter_events(&self, range: TimeRange) -> impl Iterator<Item = Result<Event>>;
    pub fn get_profile(&self, id: &str) -> Result<Option<Profile>>;
    /* ... */
}
```

Pinned `rocksdb` crate version. Feature flag `storage-sled` provides a sled-backed fallback for systems where RocksDB is fragile.

### synapse-profiles

Per-app and per-game profile loader. Profiles are TOML. The crate watches a profile directory and exposes:

```rust
pub fn load_profile(id: &str) -> Result<Profile>;
pub fn detect_profile_from_window(hwnd: HWND) -> Result<Option<ProfileId>>;
pub fn list_profiles() -> Vec<ProfileSummary>;
```

Detection logic: process exe basename + window title regex + optional Steam appid lookup. First match wins.

### synapse-hid-host

Serial driver for the RP2040 HID gateway (see `09_hardware_hid_gateway.md`). Opens a COM port at fixed baud (1 Mbaud), implements the line protocol, exposes:

```rust
pub fn connect(port: &str) -> Result<HidGateway>;
impl HidGateway {
    pub fn mouse_move(&self, dx: i16, dy: i16) -> Result<()>;
    pub fn mouse_click(&self, button: MouseButton, hold_ms: u16) -> Result<()>;
    pub fn key_press(&self, hid_code: u8, hold_ms: u16) -> Result<()>;
    pub fn pad_update(&self, report: GamepadReport) -> Result<()>;
}
```

Uses `serialport` crate.

### synapse-models

Thin ONNX runtime wrapper. Uses `ort` crate (ONNX Runtime bindings). Loads detection / OCR / STT models from `CF_MODEL_CACHE` or downloads on first use with sha-verification. Exposes typed `infer(input) -> output` functions per model.

### synapse-telemetry

`tracing-subscriber` setup, OTLP exporter (`opentelemetry-otlp`), JSON file logger, in-memory metrics ringbuffer for the debug overlay, replay log writer.

### synapse-test-utils

Dev-only. Provides:

- Deterministic mock frame source (PNG sequences)
- Mock UIA tree fixture loader
- Synthetic event generators
- Latency assertion helpers (`assert_p99_lt!`)
- Test RocksDB with `tempdir`-based path

## 7. Binary entry

`synapse-mcp/src/main.rs` is small. It:

1. Parses CLI args (`clap` derive) — `--mode {stdio|http}`, `--bind`, `--db`, `--profile-dir`, `--log-level`, `--reflex-disabled`, `--vigem-disabled`, `--hardware-hid {port}`.
2. Initializes `tracing` via `synapse-telemetry`.
3. Opens RocksDB via `synapse-storage`.
4. Loads profile dir via `synapse-profiles`.
5. Starts capture / a11y / audio / perception / reflex / action tasks.
6. Builds the MCP server (`rmcp` 1.x) with the tool registry.
7. Serves stdio or HTTP transport until SIGINT.

Total LoC for `main.rs`: target ≤ 300.

## 8. Configuration sources (precedence high → low)

1. **CLI flags** — explicit overrides
2. **Environment variables** — `SYNAPSE_*` (e.g., `SYNAPSE_HARDWARE_HID_PORT=COM7`)
3. **Config file** — `%APPDATA%\synapse\config.toml` if present
4. **Built-in defaults** — `synapse-core::defaults`

No config hot-reload at v1. Restart the daemon to change settings.

## 9. Error handling

Three classes of errors:

| Class | Where | Strategy |
|---|---|---|
| **Recoverable** (transient device, brief OS hiccup) | capture, a11y, audio, action | Log warn, retry with backoff, return structured error to caller if persistent |
| **User-facing** (invalid input, missing profile, unsupported game) | MCP tool handlers | Return MCP JSON-RPC error with `code: SCREAMING_SNAKE_CASE` + `data: {...}` |
| **Fatal** (storage corruption, panic in unsafe section) | storage, capture FFI | Crash with `panic!`; supervisor (operator) restarts |

`thiserror` for crate-local enums. `anyhow` only in the binary. Every error variant has a `.code()` method returning a `&'static str` SCREAMING_SNAKE_CASE identifier. Error codes are stable across versions.

## 10. Configuration of perception modes

Perception runs in one of three modes per active app/game, selectable explicitly or auto-detected:

| Mode | When | What runs |
|---|---|---|
| `a11y_only` | App exposes rich UIA tree, no need for pixels | UIA + WinEvent. CNN/OCR disabled. Lowest CPU/GPU. |
| `pixel_only` | Games with no useful a11y; pure detection + OCR + audio | Capture + CNN + HUD OCR + audio. UIA disabled. |
| `hybrid` (default) | Mixed apps (Electron, browser, complex IDEs) | Both paths run; `observe()` merges them with a11y preferred for elements UIA finds, pixel for the rest |

Auto-detection: profile sets the mode. If no profile matches, try `hybrid` and let the caller see what's populated.

## 11. Cancellation and shutdown

All long-running tasks accept a `CancellationToken` (`tokio-util::sync::CancellationToken`). On SIGINT:

1. MCP transport task signals cancellation
2. Reflex runtime drains in-flight actions, then exits
3. Action emitter releases held inputs (no stuck keys)
4. Capture stops; releases D3D resources
5. RocksDB flushes; closes cleanly
6. Process exits 0

Shutdown timeout: 5 seconds. After that, force-kill.

**Stuck-input guard:** on any error path, the action emitter sends a `release_all` (every key up, all mouse buttons up, gamepad neutral). This runs even on panic via a panic hook.

## 12. What runs on what hardware

Recommended host:

- CPU: 8+ cores, ≥3.5 GHz
- RAM: 16 GB
- GPU: any DX11-capable for capture; NVIDIA RTX 3060+ recommended if running detection models on GPU (ORT CUDA / DirectML)
- Disk: 5 GB free for RocksDB + model cache
- USB: one free port if using hardware HID

Minimum host (degraded modes only):

- CPU: 4 cores
- RAM: 8 GB
- GPU: any DX11
- Detection disabled, OCR via WinRT only

## 13. What this doc does NOT cover

- Exact JSON schemas for every MCP tool → `05_mcp_tool_surface.md`
- Exact Rust struct definitions → `06_data_schemas.md`
- Exact RocksDB key encodings → `07_storage_and_profiles.md`
- Performance budgets per call → `10_performance_budget.md`
- Specific dependency versions → `14_build_and_packaging.md`
