# 17 — Research Appendix

External research consulted during PRD authoring. Citations carry direct URLs. Group is informational; URLs may rot.

---

## 1. MCP protocol and SDKs

| Reference | URL |
|---|---|
| Official Rust MCP SDK (`rmcp`) repo + docs | https://github.com/modelcontextprotocol/rust-sdk, https://docs.rs/rmcp/latest/rmcp/ |
| `rmcp` crate on crates.io (v1.7+) | https://crates.io/crates/rmcp |
| Alternative Rust SDK (`rust-mcp-sdk`) — Hyper/Axum-based | https://github.com/rust-mcp-stack/rust-mcp-sdk |
| MCP Streamable HTTP transport (March 2025 spec; default for remote servers) | https://mcpindotnet.github.io/docs/concepts/architecture-overview/layers/transport-layer/streamable-http/ |
| MCP streaming patterns guide (SSE / progress notifications / push streams) | https://chatforest.com/guides/mcp-real-time-streaming/ |
| MCP SSE → Streamable HTTP migration guide | https://www.channel.tel/blog/mcp-sse-to-streamable-http-migration |
| Hugging Face on building MCP servers | https://github.com/huggingface/blog/blob/main/building-hf-mcp.md |
| RapidDev — Add Streaming to MCP Server | https://www.rapidevelopers.com/mcp-tutorial/how-to-add-streaming-to-mcp-server |

Key takeaways for Synapse:

- Use Streamable HTTP (single endpoint, dynamic JSON↔SSE upgrade) for HTTP mode. SSE-only transport is deprecated.
- Sessions via `Mcp-Session-Id` header, not connection-coupled.
- `rmcp` crate is the official Rust SDK; uses tokio + `#[tool]` macro. Stable on crates.io.
- For long-running tools, support progress notifications via the SSE upgrade path.
- Server-initiated notifications (push events) are first-class; use a long-lived SSE stream for them.

---

## 2. Computer-use MCP servers (prior art)

| Reference | URL | Notes |
|---|---|---|
| `anaisbetts/mcp-computer-use` | https://github.com/anaisbetts/mcp-computer-use | OpenAI CUA spec, Rust, cross-platform, `enigo` + `windows-capture` |
| `Harusame64/desktop-touch-mcp` | https://github.com/Harusame64/desktop-touch-mcp | Windows, 57 tools, Rust native core via napi-rs, UIA + Chrome DevTools |
| `hugefiver/mcp-computer-use` | https://github.com/hugefiver/mcp-computer-use | Rust, browser via thirtyfour WebDriver |
| `lpmwfx/gui-mcp` | https://github.com/lpmwfx/gui-mcp | Rust single binary, Windows, 14 tools, template matching for UI |
| `JeenyJAI/mcp-test-utils` | https://github.com/JeenyJAI/mcp-test-utils | Windows, native Win32 APIs, UIA + WinRT OCR + ripgrep |
| `sh3ll3x3c/native-devtools-mcp` | https://github.com/sh3ll3x3c/native-devtools-mcp | macOS/Windows/Android, accessibility-first, CDP for browsers/Electron |
| `shimondoodkin/screenmcp` | https://github.com/shimondoodkin/screenmcp | Node MCP shim + Rust worker, mobile + desktop |
| `iannelsondev/symbiosis` | https://github.com/iannelsondev/symbiosis | 25 tools, OCR + input + clipboard, security-gated |

Common pattern across these:

- UIA tree as the primary perception for Windows native apps
- `windows-capture` (Graphics Capture API) for frame capture
- `enigo` for cross-platform input
- WinRT `Windows.Media.Ocr` for OCR when no Tesseract dep is wanted
- Per-app templates for elements that UIA misses

Synapse builds on this body of work but adds: pixel-CNN perception for games, hardware HID gateway, sub-frame reflex runtime, profile system, audio capture + STT, and a unified `observe()` that fuses paths.

---

## 3. Anthropic / OpenAI computer-use specs

| Reference | URL |
|---|---|
| Anthropic computer-use tool docs (`computer_20251124`) | https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool |
| Anthropic quickstarts computer.py | https://github.com/anthropics/anthropic-quickstarts/blob/main/computer-use-demo/computer_use_demo/tools/computer.py |
| AI SDK Anthropic computer_20250124 / 20251124 schemas | https://cdn.jsdelivr.net/npm/@ai-sdk/anthropic@3.0.64/src/tool/computer_20250124.ts, https://cdn.jsdelivr.net/npm/@ai-sdk/anthropic@3.0.64/src/tool/computer_20251124.ts |
| Rust crate wrapper `llm-kit-anthropic` computer tool | https://docs.rs/llm-kit-anthropic/latest/llm_kit_anthropic/provider_tool/computer_20250124/fn.computer_20250124.html |

Action set (Anthropic computer_20251124):
`key`, `hold_key`, `type`, `cursor_position`, `mouse_move`, `left_mouse_down`, `left_mouse_up`, `left_click`, `left_click_drag`, `right_click`, `middle_click`, `double_click`, `triple_click`, `scroll`, `wait`, `screenshot`, `zoom`.

Synapse supports all of these as a subset of its action surface, plus richer game-specific actions.

---

## 4. Windows GPU frame capture

| Reference | URL |
|---|---|
| `windows-capture` Rust crate v2 | https://docs.rs/windows-capture, https://docs.rs/crate/windows-capture/latest/source/src/lib.rs |
| Windows.Graphics.Capture namespace docs | https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture |
| Desktop Duplication API | https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api |
| OBS Studio DXGI present hook implementation (reference) | https://github.com/obsproject/obs-studio/blob/master/plugins/win-capture/graphics-hook/dxgi-capture.cpp |
| C# Direct3D hook screen capture (Justin Stenning's blog) | https://spazzarama.com/2011/03/14/c-screen-capture-and-overlays-for-direct3d-9-10-and-11-using-api-hooks/ |
| Stack Overflow: fastest Windows screen capture | https://stackoverflow.com/questions/5069104/fastest-method-of-screen-capturing-on-windows |
| DXGI IDXGISwapChain::Present | https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-present |
| Simon Mourier — DXGI Output Duplication + WIC | https://www.simonmourier.com/blog/Capturing-desktop-using-DXGI-s-Output-Duplication-and-saving-it-to-a-jpeg-file-u/ |
| Capture method comparison (BitBlt vs Duplication vs Graphics Capture vs DwmThumbnail) | https://github.com/mika-f/dotnet-window-capture |

Decision: Synapse uses **Windows Graphics Capture API** via `windows-capture` crate as primary, with **DXGI Output Duplication** as fallback. Both expose zero-copy `ID3D11Texture2D` to perception. DXGI Present hooking is avoided (triggers most kernel anti-cheats).

---

## 5. UI Automation (UIA) on Windows

| Reference | URL |
|---|---|
| Rust `uiautomation` crate v0.24+ | https://docs.rs/uiautomation/latest/uiautomation/, https://docs.rs/crate/uiautomation/latest |
| `uiautomation::core::UITreeWalker` | https://docs.rs/uiautomation/latest/uiautomation/core/struct.UITreeWalker.html |
| `uiautomation::core::UIElement` | https://docs.rs/uiautomation/latest/uiautomation/core/struct.UIElement.html |
| `windows-rs` IUIAutomation | https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomation.html |
| `windows-rs` IUIAutomationElement / IUIAutomationElement3 | https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomationElement.html, https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomationElement3.html |
| `windows-rs` IUIAutomationTreeWalker | https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomationTreeWalker.html |
| UIProperty enum | https://docs.rs/uiautomation/latest/uiautomation/types/enum.UIProperty.html |

Synapse uses `uiautomation` crate for high-level tree walking; falls back to raw `windows-rs` UIA bindings for advanced patterns (cached property fetch, event handler registration with custom marshalling).

---

## 6. Input simulation

| Reference | URL |
|---|---|
| `enigo` crate v0.6+ | https://docs.rs/enigo/latest/enigo/, https://github.com/enigo-rs/enigo |
| `enigo` Windows backend source | https://github.com/enigo-rs/enigo/blob/main/src/win/win_impl.rs |
| `vigem-client` Rust crate (ViGEm in pure Rust) | https://docs.rs/vigem-client/latest/vigem_client/, https://github.com/CasualX/vigem-client |
| `vigem-rust` alternative high-level wrapper | https://docs.rs/vigem-rust/latest/vigem_rust/ |
| ViGEmBus driver releases | https://github.com/nefarius/ViGEmBus/releases (current stable: 1.22.0) |
| ViGEmBus repo | https://github.com/ViGEm/ViGEmBus/ |

Decision: `enigo` for keyboard/mouse software input. `vigem-client` for virtual Xbox 360 / DualShock 4. Hardware HID via custom firmware (`09_hardware_hid_gateway.md`).

---

## 7. Hardware HID gateways

| Reference | URL | Notes |
|---|---|---|
| `vynxc/VBox` — RP2040 HID forwarder + KMBox-compatible serial | https://github.com/vynxc/VBox | Reference firmware design; mirrors VID/PID, supports serial commands |
| `jfedor2/hid-forwarder` — Pi Pico receiver, wired + Bluetooth | https://github.com/jfedor2/hid-forwarder | Good protocol reference |
| `Foxtrott7/Foxbot-AI-Aimbot` — YOLO + Arduino HID bridge example | https://github.com/Foxtrott7/Foxbot-AI-Aimbot | Demonstrates the AC-bypass legal-gray pattern |
| UnknownCheats — HID over USB Host Shield writeup (Vanguard) | https://www.unknowncheats.me/forum/valorant/686973-undetected-mouse-movement-using-arduino-usb-host-shield-com-port.html | Detection vectors for hardware bridges |
| `jsonmeister/color-aimbot` — TCP socket → MCU, no COM port | https://github.com/jsonmeister/color-aimbot | Architecture reference |
| `Fenrified/Gordons-Sim-Controller` — RP2040 config-driven HID input | https://github.com/Fenrified/Gordons-Sim-Controller | Embedded HID best practices |

Decision: Synapse ships firmware for RP2040 (Pi Pico, $4, easy to source). Communication: serial @ 1 Mbaud over USB CDC. Spec details in `09_hardware_hid_gateway.md`.

---

## 8. Anti-cheat detection vectors (informational; we don't evade)

| Reference | URL |
|---|---|
| Adrian's security research — BattlEye BEDaisy reverse | https://s4dbrd.github.io/posts/reversing-bedaisy/ |
| Riot Vanguard vgk.sys analysis | https://gist.github.com/rhaym-tech/f636b76deeca15528e70304b5ee95980 |
| Archie — Vanguard syscall dispatch table hooks | https://archie-osu.github.io/2025/04/11/vanguard-research.html |
| Secret Club — BattlEye analysis 2019 | https://secret.club/2019/02/10/battleye-anticheat.html |
| Secret Club — anti-cheat hypervisor detection 2020 | https://secret.club/2020/04/13/how-anti-cheats-detect-system-emulation.html |

What anti-cheats catch (read-only summary so we know what NOT to do for competitive PvP):

- DLL injection / process memory writes — banned instantly
- Software input via `SendInput` from unsigned process — flagged, banned heuristically
- `mouse_event` / `keybd_event` — same
- Kernel driver hooks — banned (Vanguard) or scanned (BattlEye)
- ViGEm virtual controllers — flagged by some, allowed by others; depends on title
- Hardware HID with bezier-curve human-modeled aim — hard to detect statistically; flagged manually only via gameplay-pattern analysis
- DXGI Present hooks — flagged by all kernel-level AC
- ETW / LBR / PMC-based detection of LBR-stomping (Vanguard) — for advanced kernel-mode cheats

Synapse policy: **default-off for any AC-protected title**. Operator must explicitly enable. Detailed in `08_anti_cheat_policy.md`.

---

## 9. Aim curves / human input modeling

| Reference | URL |
|---|---|
| `iisHong0w0/Axiom-AI-Aimbot` — PID + Bezier humanization | https://github.com/iisHong0w0/Axiom-AI_Aimbot |
| `NeedlessPage819/ShadowCursor` — humanized cursor lib | https://github.com/NeedlessPage819/ShadowCursor |
| HAWK gameplay-behavior cheating-detection paper | https://arxiv.org/pdf/2409.14830 |
| Synthetic Keystroke Dynamics & Bezier Mouse Emulation (Blue-team detection) | https://www.theauditveteran.com/bot-mechanics/synthetic-keystroke-dynamics-bezier-mouse-emulation/ |

Synapse implements **cubic Bezier mouse curves with Gaussian-jittered control points + Xorshift sub-pixel tremor**. Type-text uses Gaussian inter-keystroke timing with optional bigram-distance modulation. Both parameterizable; default off (linear) for productivity tasks where naturalness doesn't matter, on for game profiles.

---

## 10. Object detection models for real-time use

| Reference | URL | Latency on consumer GPU |
|---|---|---|
| `Shazy021/yolo-vs-rtdetr-benchmark` | https://github.com/Shazy021/yolo-vs-rtdetr-benchmark | YOLO+TensorRT 19.67ms; RT-DETR+TensorRT 24.15ms |
| Nature Sci Reports — large YOLOv8/RT-DETR edge benchmarks | https://www.nature.com/articles/s41598-026-46453-6 | NPU + TensorRT for edge |
| NHSJS — M2 benchmarks (small models) | https://nhsjs.com/2026/performance-analysis-of-modern-object-detection-models-for-edge-based-assistive-glasses/ | RT-DETR more stable across runs |
| Ultralytics — RTDETRv2 vs YOLOv6-3.0 | https://docs.ultralytics.com/compare/rtdetr-vs-yolov6 | RT-DETR-s 5.03ms on T4 |
| Ultralytics — RTDETRv2 vs YOLOv5 | https://docs.ultralytics.com/compare/rtdetr-vs-yolov5 | YOLOv5n 1.12ms on T4 |

Decision: Synapse defaults to **YOLOv8n / YOLOv10n** (anchor-free, small, ~3-6ms on RTX 30x0+ via DirectML or CUDA execution provider). RT-DETR-s available as alternate for stable-jitter use cases. Models ship via `synapse-models` crate; download on first use with sha verification.

ONNX Runtime via `ort` crate. DirectML execution provider for AMD/Intel GPUs; CUDA for NVIDIA.

---

## 11. Audio capture (WASAPI loopback) and spatial audio

| Reference | URL |
|---|---|
| `wasapi` Rust crate | https://docs.rs/wasapi/latest/wasapi/index.html |
| `wasapi::AudioClient::new_application_loopback_client` (per-process loopback) | https://docs.rs/wasapi/latest/wasapi/struct.AudioClient.html |
| `ratneshjain40/looback-audio-capture` (Rust example) | https://github.com/ratneshjain40/looback-audio-capture |
| `audionimbus` — Steam Audio Rust wrapper (HRTF, spatial) | https://docs.rs/audionimbus/latest/x86_64-pc-windows-msvc/audionimbus/, https://github.com/maxencemaire/audionimbus |

Decision: `wasapi` crate for capture. Custom STT via Whisper-tiny ONNX through `synapse-models`. Direction estimate via simple inter-channel L/R energy ratio + cross-correlation lag (no need for HRTF accuracy at v1; `audionimbus` is the v2 upgrade path).

---

## 12. UI grounding / set-of-marks (informational)

| Reference | URL |
|---|---|
| Microsoft OmniParser | https://github.com/microsoft/OmniParser |
| OmniParser arxiv paper | https://arxiv.org/pdf/2408.00203 |
| Microsoft Research — OmniParser blog | https://www.microsoft.com/en-us/research/articles/omniparser-for-pure-vision-based-gui-agent/ |
| OmniParser on HuggingFace | https://huggingface.co/microsoft/OmniParser |

Synapse v1 does NOT use OmniParser by default — UIA gives this for free on Windows. We do consider it for v2 cross-platform support when AT-SPI / AX trees are sparser.

---

## 13. Token-efficiency research

| Reference | URL |
|---|---|
| ReVision — temporal visual redundancy reduction | https://arxiv.org/html/2605.11212v2 |
| AQuaUI — quadtree visual token reduction | https://arxiv.org/html/2605.19260v1 |
| Token-pruning historical screenshots | https://arxiv.org/html/2603.26041v3 |
| GUI-KV — KV cache with spatio-temporal awareness | https://arxiv.org/html/2510.00536 |
| `ddavidgao/deltavision` | https://github.com/ddavidgao/deltavision |

Synapse's diff-driven event push (only send what changed) is consistent with these findings. Full structured-state JSON replaces the screenshot entirely for most observations, eliminating the visual-redundancy problem at the source.

---

## 14. Browser automation (CDP)

| Reference | URL |
|---|---|
| `chromiumoxide` crate | https://github.com/mattsse/chromiumoxide |
| Chrome DevTools Protocol docs | https://chromedevtools.github.io/devtools-protocol/ |

Synapse exposes Chromium browsers via CDP attach (when a browser is the foreground window and a CDP port is available). Provides DOM, accessibility tree, network, console without screenshots.

---

## 15. RP2040 firmware

| Reference | URL |
|---|---|
| Embassy async embedded Rust framework | https://embassy.dev/ |
| `embassy-rp` crate (RP2040 HAL via embassy) | https://docs.rs/embassy-rp |
| `usbd-hid` HID descriptors | https://docs.rs/usbd-hid |
| TinyUSB host stack (PIO-USB for RP2040) | https://github.com/sekigon-gonnoc/Pico-PIO-USB |

Synapse firmware uses `embassy-rp` for cooperative async on Cortex-M0+, USB CDC for serial, custom HID descriptor for mouse + keyboard + gamepad combined device.

---

## 16. Adjacent prior work

| Reference | URL | What we borrow |
|---|---|---|
| OpenAI's CUA (Operator) | https://openai.com/index/computer-using-agent/ | Action set shape; we make our actions a superset |
| AlphaStar (DeepMind, StarCraft II) | research.google papers | Hierarchical observation slots; per-game profile pattern |
| OpenAI Five (Dota 2) | https://openai.com/research/openai-five | Slot architecture; separate fast/slow loops |
| FAIR Diplomacy (Cicero) | research papers | Long-horizon planning is the model's job, not the body's |
| Tensorflow Agents / Stable-Baselines3 RL envs | Inspiration only; we are NOT building an RL env, but the observation/action API shape is informed by Gym conventions |
| Playwright (Microsoft) | https://playwright.dev/ | Stable element references > coordinates; auto-wait > polling |
| AutoHotkey | https://www.autohotkey.com/ | Reflex bindings pattern; hotkey + on-event paradigm |

---

## 17. Comparable commercial products

| Product | Notes |
|---|---|
| Anthropic Computer Use (Claude) | Screenshot-loop based. Slow. Token-expensive. Synapse is the structured replacement. |
| OpenAI Operator | Cloud-hosted browser only. Synapse is local + desktop + games. |
| Adept ACT-1 | Defunct; consumed by Amazon. |
| Cradle (multi-game agent research) | Closer to AlphaStar in spirit; not productized. |
| AutoIt / AutoHotkey | Powerful scripting but no semantic perception, no AI integration. |
| UI.Vision / UiPath / Power Automate | Enterprise RPA; expensive; not for games; not local-first. |

---

## 18. Bibliographic notes

All URLs in this appendix were valid at time of authoring. When a URL rots, the corresponding crate/repo can be located via `cargo search` or web search of the noted identifier. Direct API documentation links to `docs.rs` are version-pinned via `crates.io` and remain stable.

The reference set above is the foundation; the architecture decisions in `01_architecture.md` and the choices in `14_build_and_packaging.md` are downstream of these citations. When introducing a new dependency or technique not represented here, add a row.
