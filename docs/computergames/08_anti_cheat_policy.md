# 08 — Anti-Cheat Policy

## 1. Purpose of this doc

This is a **policy** doc, not a technical doc. It defines what Synapse will and will not do with respect to anti-cheat (AC) systems, what categories of games are supported and at what risk level to the operator, and what the operator must affirmatively configure before AC-risky paths become available.

This policy is binding on contributors. PRs that violate it are rejected. Operators who explicitly want AC-risky behavior can opt in through configuration; default settings are conservative.

---

## 2. The single rule

> **Synapse exists for single-player, PvE, modded, custom-server, accessibility, automation, research, and AI-tournament use. It is not built to cheat in competitive PvP. The hardware HID path exists because it has many legitimate uses; using it to violate a game's ToS is the operator's choice and the operator's responsibility, not ours.**

We do not maintain anti-cheat-evasion features. We do not catalog which AC catches what. We do not optimize for undetectability beyond what naturally falls out of human-modeled input curves (which exist for accessibility and authenticity reasons unrelated to AC).

---

## 3. What anti-cheat systems detect (informational summary)

Synthesized from public research (see `17_research_appendix.md` §8). This is **what we know AC systems do**, not what we help operators evade.

| Vector | Detected by | Synapse default |
|---|---|---|
| DLL injection into game process | All major AC | We never do this. Forbidden in policy. |
| Process memory reads/writes of game | All major AC | Out of scope at v1; explicit "memory hook" feature would require ADR + AC-aware gating. |
| `SendInput` / `keybd_event` / `mouse_event` from third-party process | BattlEye (heuristic), EAC (heuristic), Vanguard (heuristic + via LBR-tracking on `MouseClassServiceCallback`) | This is our default action backend. Works for almost everything single-player. Flagged in competitive titles. |
| Virtual HID drivers (ViGEm, similar) | Some AC flag; many allow | Optional, off-by-default in `pixel_only` profiles for AC-protected titles |
| Kernel driver hooks (Interception, custom drivers) | All major AC catch most of these | Not used by Synapse |
| DXGI Present hooks / D3D injection | All major AC | Not used; we use Graphics Capture API (non-injection) |
| Hypervisor / TSC anomalies | Some AC (timing attacks) | Synapse doesn't run a hypervisor; n/a |
| LBR (Last Branch Record) anomalies near `MouseClassServiceCallback` | Vanguard | We don't hook `MouseClassServiceCallback`; n/a |
| Statistical anomalies in input timing (linear curves, robotic timing) | Some AC + game-side heuristics + manual review | `Natural` aim curves and `Natural` keystroke dynamics naturally reduce this; they exist for accessibility, not AC. |
| Cursor-snap pattern (instant pixel-perfect jumps) | Game-side anti-aim heuristics | Use `Bezier` / `Natural` curves; never use `Instant` curve when authenticity matters |
| Hardware HID with identifiable VID/PID | Manual review only; not technical detection | Operator can mirror VID/PID of a real device they own; default firmware reports generic VID/PID |

What anti-cheat **cannot** detect (when configured correctly):

- A genuine USB HID device sending input bytes that look like a real human's
- Screen capture via the Graphics Capture API (it's a Windows feature)
- Audio capture via WASAPI loopback (it's a Windows feature)
- The agent's reasoning loop running in a separate process the AC doesn't see

This is why the hardware HID path is mentioned at all: it is the only output channel that survives kernel-AC scrutiny on AC-protected titles, and it has many legitimate uses (accessibility, AI research, single-player tournaments, demoscene work, hardware testing). It is also the only one whose use has real ToS implications in competitive games.

---

## 4. Three risk tiers for game support

Profiles declare their tier. The active tier gates which back-ends are available.

### 4.1 Tier 0 — No AC

Single-player and mod-friendly games. No anti-cheat in the binary at all.

Examples: Minecraft (Java), Factorio, Stardew Valley, Skyrim, most indie games, KSP, older titles, browser games, Roblox Studio, single-player AAA without online mode.

**All Synapse features available.** Software, ViGEm, hardware HID all allowed. Aim curves and keystroke dynamics tunable for authenticity but not for AC evasion (we have nothing to evade).

### 4.2 Tier 1 — Light AC, single-player or sanctioned bot use

Games with AC libraries that scan but don't aggressively ban for third-party tools when the game is single-player, in offline mode, in a developer/test mode, or playing on a community/private server.

Examples: many Valve games in single-player, GTA V Story Mode (NOT online), modded servers with permissive admins, dedicated server PvE.

**All Synapse features available**, with the following operator acknowledgments:

- Profile must explicitly declare `tier = "tier1_singleplayer"` and the matched window must be the single-player launcher / offline mode.
- If the game ever switches to online mode mid-session, Synapse's profile detector switches to a `tier2_blocked` profile and pauses input action emission until the operator confirms.

### 4.3 Tier 2 — Active competitive AC

Games with kernel-level AC and active enforcement against any third-party automation in competitive modes.

Examples: Valorant (Vanguard), CS2 (VAC), League of Legends (Vanguard on Korea, Riot AC elsewhere), Apex Legends (EAC), Fortnite (EAC), R6 Siege (BattlEye), most Battle Royales.

**Synapse profiles for these games are deliberately empty at v1.** No keymap, no HUD spec, no detection model preconfigured.

For Tier 2 games:

- Software input back-end is **disabled** in default profiles. Operator can re-enable via explicit profile flag if they have a legitimate use (e.g., recording bot footage on a private custom server). The flag's name is intentionally long: `backends.software_in_tier2_acknowledged = true`.
- ViGEm back-end is **disabled** by default. Same re-enable flag.
- Hardware HID back-end is **available but defaults to off**. Operator must pass `--allow-hardware-in-tier2` AND the profile must set `backends.hardware_in_tier2_acknowledged = true` AND the operator must answer an interactive prompt on first use ("This will use a hardware HID device against a competitive-AC-protected game. ToS may prohibit. Continue?").

The intent of the gating is not to make it impossible; experienced operators can flip every flag. The intent is to make it **impossible to do by accident** and to make the choice the operator's, recorded in their configuration, not ours.

---

## 5. What we will not ship (binding)

These are rejection-on-PR-sight features:

1. **DLL injection into any process.** Not Synapse's, not the game's, not anything's.
2. **Process memory read/write tooling** of any process other than Synapse itself. No Cheat-Engine-style RAM scanning. (Game-provided modding APIs that surface state through their own RAM-read mechanism are fine if the game documents them; e.g., Minecraft mods read game state through Minecraft's mod API, not raw memory.)
3. **Kernel driver hooks.** Synapse is user-mode only. No `.sys` files in the install.
4. **DXGI Present hooks** or any other graphics-pipeline injection. We use the Graphics Capture API, which is a Windows feature, not an injection.
5. **AC fingerprint database**, signature obfuscation, code virtualization, anti-debugger features. Synapse is open source and identifies itself plainly.
6. **HID firmware that hides itself.** The bundled RP2040 firmware reports a clearly-identifiable VID/PID combination. Operators can rebuild firmware with different IDs for their own devices; we don't ship pre-built firmware that mimics specific commercial peripherals.
7. **Automatic detection of which game is running for the purpose of opting into AC-risky modes.** Profile activation is operator-driven. Synapse never auto-flips into "stealthier" behavior because it noticed Vanguard is loaded.

---

## 6. What we will ship (binding)

These features are deliberately included even though they have implications for AC-protected games. They serve legitimate use cases outside the AC context.

1. **Human-modeled aim curves and keystroke dynamics.** Useful for accessibility (motor impairments simulating "natural" input), automation testing (RPA pipelines that must look human to web bot-detection), and game AI research. They are tunable; the default for productivity profiles is **off** (use `Instant` / `Burst`) because there's no reason to fake human authenticity for clicking the Save menu.
2. **Hardware HID gateway.** Useful for accessibility (eye-tracking, sip-and-puff input), demo recording, dedicated game-AI research rigs, hardware testing, and AI tournaments with sanctioned bot interfaces. It is gated as described in §4.3.
3. **Graphics Capture API**. Standard Windows screen capture used by OBS Studio and every screen recorder. No injection.
4. **WASAPI loopback audio capture**. Standard Windows audio loopback used by every recorder. No injection.
5. **WinEvent / UIA event subscribers**. Standard Windows accessibility APIs.
6. **Chrome DevTools Protocol attachment**. Public, documented browser API.
7. **Filesystem and process watchers**. Standard Windows APIs.

---

## 7. Operator responsibility

By installing and configuring Synapse, the operator acknowledges:

- They are responsible for compliance with the ToS of any software they automate.
- Synapse's defaults aim to make AC-risky modes opt-in, not opt-out.
- Enabling Tier 2 features against AC-protected games may result in account suspension or ban from the game in question.
- The Synapse project does not provide indemnification or support for ToS violations.

This acknowledgment is enforced via a first-run prompt:

```
Synapse is a powerful automation tool. By continuing you confirm:

1. You will not use Synapse to violate the Terms of Service of any third-party
   game or service.
2. AC-risky features are off by default and you will enable them only for
   sanctioned uses (single-player, custom servers, accessibility, research,
   AI tournaments).
3. The Synapse project provides no warranty or indemnity for ToS violations.

Type 'i agree' to continue. (Decline by closing this prompt.)
```

The acknowledgment is recorded in `%APPDATA%\synapse\agreement.json` with a hash of the prompt text and a timestamp. A new major version may invalidate the previous acknowledgment.

---

## 8. Detection responses

When Synapse detects that a Tier 2 AC is loaded and an action is about to fire through a back-end that's flagged for that tier:

| Situation | Default behavior | Operator override |
|---|---|---|
| Tier 2 AC loaded, software back-end requested | Refuse action, return `SAFETY_AC_TIER2_BACKEND_BLOCKED`, log event | `backends.software_in_tier2_acknowledged = true` in profile |
| Tier 2 AC loaded, ViGEm back-end requested | Refuse | profile flag |
| Tier 2 AC loaded, hardware HID requested | Refuse unless all three gates passed (§4.3) | Three explicit acknowledgments |
| AC drives detected via service enumeration (e.g., `BEService`, `EasyAntiCheat`, `vgc`) but no profile is loaded | Active profile is `tier_unknown`; software back-end allowed (likely productivity app on same machine) | n/a |

Detection of "is this AC active" is heuristic: list of service names + driver names + window title regexes. Documented in `synapse-core::ac_heuristics`. The list is informational, not adversarial — it's only used to gate Synapse's own behavior, not to alter input to evade detection.

---

## 9. Specific guidance for likely v1 game profiles

### 9.1 Minecraft (Java Edition)

Tier 0. No restrictions. Recommended.

### 9.2 Factorio

Tier 0. Headless support exists via Factorio's own API; Synapse's GUI-driving is supplementary. Recommended.

### 9.3 OpenTTD, Beam.NG, KSP, RimWorld, Stardew Valley

Tier 0. All fine.

### 9.4 GTA V Story Mode

Tier 1. Single-player only. Profile detects Story Mode by window title regex; switches off all action backends if GTA Online launches.

### 9.5 Skyrim, Witcher 3, Fallout 4

Tier 0 for unmodded; Tier 1 for major-mod environments where mod authors prefer no automation. Default Tier 0; operator can downgrade.

### 9.6 Valorant, CS2, Apex, Fortnite

Tier 2. Profiles ship empty. Documented warning in profile TOML comments.

### 9.7 League of Legends, Dota 2

Tier 2. Same.

### 9.8 Browser games and Roblox

Roblox Studio is Tier 0. Roblox player games depend on the experience; if the experience has Roblox's anti-cheat or competitive matchmaking, treat as Tier 2.

Browser games: same as Chrome. CDP is allowed; whether the game's own ToS allows automation is the operator's call.

---

## 10. Updating tiers

If a game's AC posture changes (developer adds aggressive AC where there was none), the tier in the bundled profile changes in the next Synapse release. Operators are responsible for keeping Synapse up to date. We do not auto-update profiles without consent.

The reverse — relaxing a tier — also goes through a release. We do not silently lower restrictions.

---

## 11. The honest assessment

This policy is conservative on purpose. The realistic worst case is an operator who installs Synapse, enables every flag, and gets their main Valorant account banned. The policy can't prevent that — the operator pressed every button. What it can do is make sure:

- No accident leads to a ban
- The path to the AC-risky configuration is explicit, multi-step, and logged
- The project's stated purpose isn't "automate competitive PvP"
- The features that exist for legitimate reasons (accessibility, research, demo) don't get blamed for being misused

That's the deal. Operators who want to do legitimate things with the tool can do them. Operators who want to cheat in ranked play have to actively bypass several layers and own the result.

---

## 12. What this doc does NOT cover

- Specific AC reverse-engineering (intentionally out of scope; see `17_research_appendix.md` §8 for public research that we read for context only)
- Per-game AC posture tracking (lives in the per-profile TOML comments)
- Hardware HID firmware design → `09_hardware_hid_gateway.md`
- The action back-end mechanics → `03_action.md`
