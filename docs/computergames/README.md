# Synapse — Real-Time Computer-Use & Game-Control MCP for AI Agents

**Project codename:** Synapse (rename freely; "the nerve connection between AI brain and computer body").
**Language:** Rust, edition 2024, MSRV 1.83. All Rust end-to-end; no Python, no C/C++ glue beyond unavoidable FFI to Windows SDK / RocksDB / WASAPI / RP2040 SDK.
**Target host:** Windows 11 x64 (primary), Windows 10 x64 (best-effort). Linux/macOS deferred to v2.
**License:** MIT or Apache-2.0 dual.
**Repo:** fresh, independent. No upstream vendor dependencies; clean-room.

---

## What this is

Synapse is an **MCP (Model Context Protocol) server** that gives any MCP-aware AI agent (Claude, Codex, Cursor, custom runners) a fast, structured, low-token interface to **see**, **hear**, **act on**, and **react inside** any Windows desktop application — covering two equally important use modes:

| Mode | Examples | Primary perception path |
|---|---|---|
| **Computer-use** | VS Code, Excel, Outlook, Slack, browsers, file explorer, terminals, design tools | Accessibility tree (UIA), DOM (CDP), app-specific APIs, OS event hooks |
| **Game-control** | Single-player games, modded multiplayer, browser/Roblox games, real-time titles | GPU frame capture + small detection CNN, HUD OCR, spatial audio, game-specific RAM hooks where ethically allowed |

Both modes share the same Rust workspace, the same MCP tool surface, the same action subsystem, and the same sub-frame reflex runtime. The perception subsystem auto-selects the cheapest path that yields fidelity. When an app has a rich a11y tree, we use it — sub-millisecond latency, zero tokens spent on pixels. When an app renders only to GPU (most games, some Electron apps, all canvas-heavy tools), we capture + infer.

The agent doesn't decide which path to use — it asks `observe()` and gets a structured response. The body picks the best sensor.

**Synapse does NOT include:**

- Any goal-planning, MCTS, GOAP, skill libraries, or hierarchical decomposition (the agent does this through its tool-use loop)
- Any large prediction model, reward model, or learning loop (the agent does this in-context)
- Any inner LLM (model lives outside; we are pure infrastructure)
- Any anti-cheat-evasion features for unsanctioned online competitive play (see `08_anti_cheat_policy.md`)

---

## Why one system for both

Computer-use and game-control look different on the surface but share the load-bearing primitives:

| Primitive | Used by computer-use? | Used by game-control? |
|---|---|---|
| Zero-copy GPU frame capture | Yes (for canvas/video/Electron a11y holes) | Yes (primary perception) |
| Accessibility tree (UIA) walk + event hook | Yes (primary perception) | Sometimes (UI overlays, menus) |
| Structured semantic event stream | Yes (focus / mutation / file change) | Yes (entity appeared / HUD change / audio cue) |
| Keyboard / mouse SendInput | Yes | Yes |
| Virtual controller (ViGEm) | Rarely | Yes |
| Hardware HID (RP2040 gateway) | Rarely | Yes for AC-protected games |
| Aim curves / human-modeled cursor | Optional | Critical for games |
| Sub-frame reflex runtime | Helpful (auto-dismiss popups) | Critical (frame-perfect inputs) |
| OCR fallback | Yes (when a11y is sparse) | Yes (HUD readouts) |
| Per-app/per-game profile system | Yes | Yes |
| Token-efficient `observe()` JSON | Yes | Yes |

Shipping these as two separate products would duplicate ~90% of the engineering. **Synapse ships them once.**

---

## Read order

| # | Doc | Read when |
|---|---|---|
| README | this file | Always start here |
| 00 | [`00_vision_and_scope.md`](00_vision_and_scope.md) | First. Mission, users, non-goals. |
| 01 | [`01_architecture.md`](01_architecture.md) | Understanding the moving parts. Processes, threads, Rust workspace. |
| 02 | [`02_perception.md`](02_perception.md) | Building the eyes/ears. Capture, detection, a11y, OCR, audio, events. |
| 03 | [`03_action.md`](03_action.md) | Building the hands. Mouse/kbd/controller/HID. |
| 04 | [`04_reflex_runtime.md`](04_reflex_runtime.md) | Sub-frame reactive controllers. Event bus. |
| 05 | [`05_mcp_tool_surface.md`](05_mcp_tool_surface.md) | The public API. Every tool, parameter, return, error. |
| 06 | [`06_data_schemas.md`](06_data_schemas.md) | Rust structs, JSON envelopes, event types, error codes. |
| 07 | [`07_storage_and_profiles.md`](07_storage_and_profiles.md) | RocksDB schema, runtime files, per-app/per-game profile system. |
| 08 | [`08_anti_cheat_policy.md`](08_anti_cheat_policy.md) | What we will/won't do, scope, kernel-AC notes. |
| 09 | [`09_hardware_hid_gateway.md`](09_hardware_hid_gateway.md) | Pi Pico HID firmware + serial protocol + Rust host driver. |
| 10 | [`10_performance_budget.md`](10_performance_budget.md) | Latency targets, profiling, optimization rules. |
| 11 | [`11_security_and_safety.md`](11_security_and_safety.md) | Threat model, permissions, redaction, kill switches. |
| 12 | [`12_observability.md`](12_observability.md) | Logging, tracing, metrics, debug overlay, replay tool. |
| 13 | [`13_testing_strategy.md`](13_testing_strategy.md) | Unit/integration/E2E, fixtures, CI, perf regression. |
| 14 | [`14_build_and_packaging.md`](14_build_and_packaging.md) | Workspace, deps, profiles, installer, signing. |
| 15 | [`15_roadmap_and_milestones.md`](15_roadmap_and_milestones.md) | M0-M5 phases, scope per milestone, demo criteria. |
| 16 | [`16_open_questions.md`](16_open_questions.md) | Unresolved decisions, ADRs needed. |
| 17 | [`17_research_appendix.md`](17_research_appendix.md) | Web research, comparable projects, references with URLs. |

---

## One-line system summary

A Rust MCP server that exposes structured desktop and game state as low-token JSON, accepts high-level action intents (click, type, aim, press, drag, combo), runs sub-frame reflexive controllers so model latency never costs a frame, and pipes everything through a single `synapse-mcp` binary the agent connects to over stdio or Streamable HTTP.

---

## Architecture sketch

```
┌─────────────────────────────────────────────────────────────────────┐
│  AI Agent (Claude / Codex / Cursor / custom runner) — the BRAIN     │
│  - Sets goals, plans, composes skills, adapts                       │
│  - Speaks MCP JSON-RPC over stdio or Streamable HTTP                │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ MCP request/response (slow loop ~1-3 Hz)
                               │ MCP notifications (push events, SSE)
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│  synapse-mcp (Rust binary) — the BODY                               │
│                                                                     │
│  ┌────────── Perception ──────────┐                                 │
│  │ ┌─ A11y path (UIA, CDP) ────┐  │ ──► observe() returns       │
│  │ │  tree walk + event hook  │  │       structured app state  │
│  │ └─ Pixel path (capture+CNN)┘  │                                 │
│  │   GPU frame → YOLO/ConvNeXt  │                                 │
│  │ ┌─ Audio (WASAPI loopback) ─┐  │                                 │
│  │ │  STT + spatial direction  │  │                                 │
│  │ └─ HUD OCR + template match ┘  │                                 │
│  └────────────────┬───────────────┘                                 │
│                   │                                                  │
│                   ▼ structured events to bus                         │
│  ┌────────── Reflex Runtime ──────┐                                 │
│  │  aim_track / on_event / combo │  ──► fast loop 60-1000 Hz       │
│  │  scheduler @ 1ms tick         │                                 │
│  └────────────────┬───────────────┘                                 │
│                   │                                                  │
│                   ▼                                                  │
│  ┌────────── Action ─────────────┐                                  │
│  │  SendInput  │  ViGEm  │  HID  │  ──► OS / virtual controller /   │
│  │  Interception driver  (opt)   │       USB serial → RP2040 board │
│  └────────────────┬───────────────┘                                 │
│                   │                                                  │
│  ┌────────────────▼─────────────────────────────────────────────┐   │
│  │  Event bus + RocksDB + profiles + tracing telemetry         │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
                Windows OS + GPU + foreground apps and games
```

Slow loop (model → MCP → response) runs at human-decision rate. Fast loop (reflex runtime) runs at frame rate. They are decoupled by the event bus.

---

## Performance targets (binding)

| Stage | Target p99 |
|---|---|
| Frame capture (zero-copy GPU surface) | ≤ 3 ms |
| Detection inference (small CNN on 5090-class GPU) | ≤ 8 ms |
| UIA tree snapshot for focused window | ≤ 10 ms |
| Full `observe()` response from request to reply | ≤ 30 ms |
| Event push from underlying frame/UIA event to subscriber | ≤ 50 ms |
| `act_aim_at` start-of-motion latency | ≤ 5 ms |
| `act_press` to electrical signal on USB | ≤ 2 ms (software) / ≤ 4 ms (hardware HID) |
| Reflex `on_event` action emission | ≤ 5 ms from event |
| MCP idle-tick CPU usage | ≤ 1% on one core |
| Steady-state VRAM when models loaded | ≤ 2 GB |

Detailed budget and profiling discipline: `10_performance_budget.md`.

---

## Quick start (target M3+ developer experience)

```powershell
# One-time prerequisites
winget install Nefarius.ViGEmBus       # virtual controller driver (~30s GUI installer)

# Install Synapse
cargo install --git https://github.com/<your-org>/synapse synapse-mcp

# Stdio mode (for Claude Desktop / Codex CLI etc.)
synapse-mcp --mode stdio

# Streamable HTTP mode (for remote / multi-client agents)
synapse-mcp --mode http --bind 127.0.0.1:7700

# Optional: flash an RP2040 board for hardware HID
cargo run -p synapse-hid-host -- flash --device COM7
```

Configure your agent client to launch `synapse-mcp` as an MCP server. The agent immediately gains every tool defined in `05_mcp_tool_surface.md`.

---

## Repository layout (fresh repo, to be built)

```
synapse/
├── Cargo.toml                          # workspace root
├── README.md
├── LICENSE-MIT, LICENSE-APACHE
├── docs/                               # this PRD lives here
├── crates/
│   ├── synapse-mcp/                    # binary: MCP server entry
│   ├── synapse-core/                   # shared types, error codes, constants
│   ├── synapse-capture/                # GPU frame capture (windows-capture wrapper)
│   ├── synapse-a11y/                   # UIA tree walk + WinEvent hook + CDP client
│   ├── synapse-perception/             # detection, OCR, HUD, event derivation
│   ├── synapse-audio/                  # WASAPI loopback + STT + spatial direction
│   ├── synapse-action/                 # input emit (kbd/mouse/pad/HID)
│   ├── synapse-reflex/                 # sub-frame reactive runtime
│   ├── synapse-storage/                # RocksDB wrapper + CFs
│   ├── synapse-profiles/               # per-app/per-game profile loader
│   ├── synapse-hid-host/               # Rust serial driver for hardware HID gateway
│   ├── synapse-models/                 # ONNX runtime wrappers, model loader
│   ├── synapse-telemetry/              # tracing + metrics + replay log
│   └── synapse-test-utils/             # shared test helpers
├── firmware/
│   └── pico-hid/                       # RP2040 HID gateway (Rust, embassy-rs, no_std)
├── models/                             # bundled ONNX models (small set, downloaded on demand)
├── profiles/                           # bundled app + game profiles (community-extendable)
├── scripts/                            # build, signing, install (PowerShell + Rust)
└── tests/
    ├── e2e/
    └── fixtures/
```

Full crate boundaries and dep graph in `01_architecture.md`. Build details in `14_build_and_packaging.md`.

---

## Out of scope (explicit non-goals)

1. **Online competitive PvP cheating.** Synapse is for single-player, PvE, modded, dev-mode, custom-server, research, accessibility, and computer-use automation. It ships no anti-cheat-evasion logic for ladder/ranked play. The hardware HID path exists for legitimate accessibility, automation, and AI-tournament use — not unsanctioned competitive advantage. See `08_anti_cheat_policy.md`.
2. **Goal/planning/skill libraries.** The agent does this through its native tool-use loop.
3. **Inner LLM.** Optional small vision models (YOLO-nano, ConvNeXt-tiny) only.
4. **Cross-platform v1.** Windows first. Linux/macOS exist as v2.
5. **General-purpose RPA.** Web/SaaS form-filling is a side-effect, not a target.
6. **Reverse-engineering proprietary game protocols.** RAM reads / packet inspection only for games the operator owns and where ToS permits.

---

## Project status

| Phase | Status |
|---|---|
| PRD | This doc set. In active drafting. |
| M0 — bootstrap | Not started |
| M1 — perception MVP (a11y + capture) | Not started |
| M2 — action MVP (kbd/mouse/pad) | Not started |
| M3 — reflex + MCP surface | Not started |
| M4 — hardware HID + first game profile | Not started |
| M5 — production-ready, 5+ app/game profiles | Not started |

See `15_roadmap_and_milestones.md`.

---

## Authoring rules

- Files ≤ 500 lines, functions ≤ 30 lines, cyclomatic ≤ 10.
- `#![forbid(unsafe_code)]` everywhere except `synapse-capture` (DirectX FFI) and `synapse-hid-host` (serial / OS handle).
- All errors carry `SCREAMING_SNAKE_CASE` codes via `thiserror`. No `anyhow` in library crates.
- Public APIs and CF names are `pub const`s — no magic strings.
- `tracing` for everything. `println!` is a code-review rejection.
- No silent successes. If a tool didn't do the work, it returns an error code.
- No mocks in tests that gate completion. Real captures, real input, real RocksDB.
- Pre-production tree: schema changes = wipe-and-rebuild, no migration shims.

---

## Authoritative summary

**Synapse is the Rust MCP server that lets any AI agent see, hear, and act on a Windows machine — covering both productivity computer-use (Office, IDEs, browsers, terminals) and real-time video games — by auto-selecting between accessibility-tree perception and GPU-frame-capture perception, exposing structured state and semantic events as token-efficient JSON, accepting high-level action intents that the body compiles to frame-accurate input, and running sub-frame reflexive controllers so model latency never costs a frame.**
