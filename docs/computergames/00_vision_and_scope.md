# 00 — Vision and Scope

## 1. Mission

**Give AI agents a body.** Modern frontier models (Claude, GPT, Gemini class) are excellent reasoners but blind, deaf, and limbless on a real computer. Synapse fixes that by exposing the operating system, every visible window, and every running game as a structured, queryable, controllable surface over the Model Context Protocol.

The agent thinks. Synapse perceives, acts, and reflexes.

## 2. Problem statement

Today, an AI agent that wants to operate a Windows machine has three bad choices:

1. **Screenshot loops** — capture a PNG, pay 1500-2500 vision tokens, wait for the model to find a button, send a coordinate, screenshot again. Cost: ~$0.05-$0.20 per step. Latency: 500ms-3000ms per action. Cannot react to anything in real time. Fails on games entirely.
2. **Browser-only tools** — Playwright/Puppeteer give rich structured DOM but cover ~5% of the desktop. Useless for native apps and games.
3. **Custom OS automation libraries** (PyAutoGUI, AutoIt) — coordinate-based, brittle, no semantic state, no event push, no game support.

None of these meet the bar a competent human operator hits: **see structure, react instantly, act precisely, work everywhere**.

Synapse is the missing primitive.

## 3. Target users

| Persona | Need | How they use Synapse |
|---|---|---|
| **AI engineer building a desktop agent** | Reliable, fast, semantic perception + action across any app | Embed `synapse-mcp` as a tool in Claude Code / Codex / custom runner; build agent behaviors in prompts |
| **Game-AI researcher** | Low-latency observation + control loop for arbitrary games | Use Synapse as the I/O layer; bring their own perception model when defaults aren't enough |
| **Accessibility tooling builder** | Programmatic operation of inaccessible apps | Use UIA path for accessible apps, capture+OCR fallback for the rest |
| **QA / automation engineer** | Replace flaky pixel-matching with structured action | Use a11y path for stable element references; replay test logs from RocksDB |
| **Hobbyist letting Claude play their game** | Set it up once, let the agent loop on a game while they watch | Install via `cargo install`, point Claude Desktop at it, give a goal |
| **Speedrunning / TAS-adjacent research** | Frame-perfect inputs with structured state | Hardware HID gateway + reflex runtime |

**Not** target users:
- Cheaters on competitive PvP ladders. We are not building this for them.
- Bot farmers running massive parallel accounts. Single-machine system.
- Mobile or console operators. Desktop only.

## 4. Why now

Three things changed in the last 18 months that make this newly tractable:

1. **MCP became a real standard.** Streamable HTTP transport (March 2025) supports both fast tool calls and long-lived event streams from one endpoint. Every major agent client (Claude Desktop, Cursor, Codex, VS Code, ChatGPT Desktop) now speaks it natively.
2. **Frontier models gained tool-use loops.** Claude 3.5+, GPT-4o+, Gemini 2+ can plan and reflect across hundreds of tool calls. The "agent" is now a credible outer loop — we don't need to build planning, we just need to feed it good observations and act on its commands.
3. **Consumer GPUs got absurdly capable.** RTX 4090 / 5090 class hardware runs YOLO-class detectors at 200+ FPS, ConvNeXt-tiny at 500+ FPS, small VLMs at 5-10 FPS. Real-time game perception on a single workstation is now trivially fast.

## 5. Concrete capabilities at v1

When the agent connects to `synapse-mcp`, it gains these capabilities for any foreground Windows app or game:

### See (perception)

- Read the structured tree of every accessible window (UIA), with names, roles, AutomationIds, bounding boxes, patterns, focus state, enabled state
- Read the DOM and accessibility tree of any Chromium-based browser (CDP)
- Read the structured state of any app exposing a known automation API (Office COM, terminal PTY, VS Code LSP via extension, Slack/Discord APIs)
- Capture any window or the full desktop as a GPU texture at 60+ fps with zero CPU copy
- Run a small object-detection model (YOLO-class, ~20MB ONNX) on captured frames at 50+ fps
- OCR the screen or any subregion via WinRT `Windows.Media.Ocr` (no Tesseract dep) or a fine-tuned CRNN
- Capture and transcribe system audio (WASAPI loopback + a small STT model)
- Detect spatial direction of stereo audio events (FPS footstep direction)
- Watch the filesystem for changes (`ReadDirectoryChangesW`)
- Watch processes, sockets, and clipboard
- Subscribe to all of the above as a push event stream over MCP notifications

### Act (action)

- Click, double-click, right-click, drag, scroll (mouse — software, virtual driver, or hardware HID)
- Type text, press keys, hold modifiers, send chord combos (keyboard — same three paths)
- Drive a virtual Xbox 360 / DualShock 4 controller (ViGEm)
- Send analog stick deltas, trigger pressures, button presses
- Move the cursor with human-modeled aim curves (Bezier with micro-tremor + variable timing)
- Type with human-modeled keystroke dynamics (Gaussian inter-arrival)
- Send frame-perfect input sequences (fighting-game motion inputs)
- Read/write clipboard
- Launch / focus / close windows
- Run shell commands (gated, optional)

### React (reflex runtime)

- Continuous aim-tracking: "lock onto target X until told to stop"
- Hold patterns: WASD strafe, bunny-hop, sprint-jump-crouch chains
- Frame-accurate combo sequencers
- `on_event` reactive bindings: "when low_hp event fires, press the medkit slot"
- Auto-dismiss popups by class+text matching
- Watchdog timers: "if no progress in 5s, abort and notify model"

### Persist + observe

- All events recorded to RocksDB with timestamps for replay debugging
- All MCP requests/responses traced with `tracing` + OTLP export
- Per-app/per-game profiles (HUD layout, keymap, capture region) loaded on-demand
- Replay tool to play back any session deterministically

## 6. Non-goals (what we explicitly do NOT build)

These are out of scope, full stop. Don't accept feature requests for them without an ADR.

1. **No goal planning, no MCTS, no GOAP, no skill libraries.** The agent is the planner. We do not invent a per-game skill ontology, do not maintain a skill graph, do not run search over plan space. If the agent wants to compose actions into a multi-step plan, that's in its tokens, not ours.
2. **No inner LLM.** Synapse loads no large model. Optional vision models stay small (≤100M params). The agent connecting over MCP is the only "intelligence."
3. **No prediction / world model / learning head.** We do not predict future states, do not maintain a reward signal, do not adapt weights at runtime. Optional model inference is for perception (object detection / OCR), not for prediction or RL.
4. **No anti-cheat-evasion for competitive PvP.** See `08_anti_cheat_policy.md`. We support hardware HID for legitimate purposes; we do not maintain a list of "what each anti-cheat detects."
5. **No general-purpose RPA / web scraping framework.** A browser CDP integration ships because games and apps often have web subviews; it is not a Playwright competitor.
6. **No mobile, console, embedded.** Windows desktop only at v1. Linux/macOS v2.
7. **No multiplayer / multi-machine orchestration.** One agent, one machine, one Synapse server.
8. **No cloud service.** Synapse runs entirely on the operator's machine. No telemetry leaves the box without explicit opt-in.

## 7. Success criteria

Synapse v1 is successful when:

1. **An agent driving Claude via stdio MCP can open Notepad, type a paragraph, save the file to a specified path, and verify the file exists — using a total of ≤8 tool calls and ≤2500 tokens.** Today this takes ~30+ screenshot-based steps and ~30K tokens.
2. **An agent can complete a 30-minute single-player game session** (e.g., play Minecraft from spawn → build a small shelter → kill a mob) using only Synapse, with no human intervention, and ≤200 tool calls.
3. **An agent can react to in-game events at frame rate** for at least one supported FPS — i.e., agent says "track that enemy and shoot if visible," and the reflex runtime delivers a clicked shot within 33ms of the enemy becoming visible.
4. **Steady-state token cost is ≤ 800 tokens per agent turn** for a structured observation, vs. ~1800 for a screenshot of the same scene.
5. **Detection inference + capture stays under 16ms p99** on a 5090.
6. **No silent failures.** Every MCP tool that fails to do its work returns a structured error code, not a `success: true` shell.
7. **One-command install** on a fresh Windows 11 machine: `winget install Nefarius.ViGEmBus; cargo install --git ... synapse-mcp`.

## 8. Anti-success criteria (failure modes we want to avoid)

| Failure mode | How we avoid |
|---|---|
| Screenshot-loop fallback masquerading as "structured" perception | Hard rule: `observe()` returns structured data; if both a11y and detection fail, return `OBSERVE_NO_PERCEPTION_AVAILABLE` error with diagnostics, never silently include a screenshot |
| Slow path becoming the only path | Per-tool p99 latency budgets enforced in CI; perf regressions block merge |
| Anti-cheat detection landing operators in trouble | All AC-risky paths gated behind explicit env var + agent capability flag; default-off |
| Tool-bloat (200+ MCP tools, agent confused) | Hard cap: ≤ 30 tools at v1. Anything else is a profile, a parameter, or a sub-command of an existing tool |
| Token bloat per observation | Hard cap: `observe()` returns ≤ 1500 tokens by default; agent must `expand(slot)` for more |
| Per-game special-casing in core code | Per-game logic lives in declarative profiles (`profiles/<id>.toml`), not Rust code |
| Build complexity sprawl | Workspace ≤ 15 crates; one binary; no procmacro forests; no build.rs that hits the network |

## 9. Definition of "done" for the PRD itself

The PRD is done when:

1. All 18 docs in this directory exist and are internally consistent.
2. The architecture in `01` matches the structs in `06`, the tools in `05`, and the milestones in `15`.
3. Every external dep is named with a specific crate version range.
4. Every external service / OS API used is identified by exact name and minimum Windows version.
5. A reader who has not been in our conversations can sit down, read the PRD, and begin coding `synapse-core` without asking us a single clarifying question.

## 10. What ships at v1 vs deferred

| At v1 | Deferred |
|---|---|
| Windows 11 / Windows 10 21H2+ | Linux Wayland/X11, macOS |
| stdio + Streamable HTTP MCP transports | WebSocket, IPC pipes |
| UIA, CDP (Chromium), file watch, clipboard, processes | AT-SPI (Linux), AX (macOS), Wayland-specific |
| Software input, ViGEm virtual pad, RP2040 HID | Interception driver, kernel-level hooks |
| YOLO + ConvNeXt detection, WinRT OCR | Custom segmentation, depth-from-stereo, large VLM inference |
| WASAPI loopback + simple direction estimate | HRTF-accurate spatial audio, room acoustics |
| Per-app / per-game profile TOML | Profile auto-generation from one-shot bootstrap session |
| 5+ shipped profiles (Notepad, VS Code, Chrome, Minecraft, one FPS) | Marketplace of community profiles |
| Replay log, debug overlay | Visual session reviewer, timeline scrubber UI |
| MIT/Apache-2.0 dual license | Commercial OEM license tier |

## 11. Risk register (top items)

| Risk | Impact | Mitigation |
|---|---|---|
| Microsoft tightens GPU capture permissioning in a future Windows update | High — perception breaks | Maintain DXGI Output Duplication fallback in addition to Graphics Capture API |
| Game anti-cheat starts flagging ViGEm | Medium — game support narrows | Hardware HID is the contingency; document it as the supported path for AC-protected games (single-player only) |
| MCP transport spec changes again | Low — minor refactor | Stay on official `rmcp` crate; track spec releases |
| Vision-model dependency on bundled ONNX files | Medium — install size, licensing | Default-bundle only models with permissive licenses; download larger models on first run with explicit consent |
| RocksDB on Windows is sometimes finicky | Low | Pin a known-good `rocksdb` crate version; have a `sled` fallback feature flag |
| Hardware HID requires user to solder/buy a $4 board | Low | Make the gateway optional; document the use case clearly; ship pre-built firmware images |
| Claude/Codex/Cursor change MCP client behavior | Low | Compatibility tests against each client in CI; don't depend on undocumented behavior |

## 12. The single line that decides everything

When in doubt: **the model is the brain, Synapse is the body. If a feature requires the body to make strategic decisions, it doesn't belong here. If a feature requires the body to react faster than the model possibly can, it absolutely belongs here.**
