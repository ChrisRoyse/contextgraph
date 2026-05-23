# 05 — MCP Tool Surface

## 1. Design rules

These rules constrain every tool we ship.

1. **Tool count cap: 30 at v1.** Tools that overlap are merged. Profile and parameter knobs are the escape hatches.
2. **One tool, one verb.** No `do_everything(action_kind, ...)` mega-tools.
3. **Structured input, structured output.** Every tool defines a JSON Schema with `additionalProperties: false`. Every response carries explicit fields, no free-form text.
4. **No silent success.** If the tool did not do the work, it returns an MCP error with `code: SCREAMING_SNAKE_CASE`, never `success: true` with a partial result.
5. **All async; all cancellable.** Long-running tools support progress notifications via Streamable HTTP SSE upgrade.
6. **Idempotency tokens where it matters.** `act_run_shell`, `act_launch`, and similar accept an optional `idempotency_key` so the agent can retry safely.
7. **Stable identifiers.** `element_id`, `entity_id`, `track_id`, `reflex_id`, `session_id` are returned by tools and accepted unchanged by subsequent calls. The agent never invents these.

The tool list below is the contract. Schemas use abbreviated JSON Schema syntax; the canonical schema lives in `synapse-mcp/src/tools/definitions/*.rs` and is exported as part of the standard MCP `tools/list` response.

---

## 2. Tool registry summary

| # | Tool | Verb | Side effect |
|---|---|---|---|
| 1 | `observe` | read | none |
| 2 | `find` | read | none |
| 3 | `describe` | read (slow path) | none |
| 4 | `read_text` | read | none |
| 5 | `read_hud` | read | none |
| 6 | `audio_tail` | read | none |
| 7 | `audio_transcribe` | read | optional STT inference |
| 8 | `subscribe` | read | opens push stream |
| 9 | `set_capture_target` | config | reconfigures capture |
| 10 | `set_perception_mode` | config | reconfigures perception |
| 11 | `act_click` | write | mouse click |
| 12 | `act_type` | write | keyboard |
| 13 | `act_press` | write | keyboard |
| 14 | `act_aim` | write | mouse move |
| 15 | `act_drag` | write | mouse drag |
| 16 | `act_scroll` | write | mouse scroll |
| 17 | `act_pad` | write | gamepad |
| 18 | `act_combo` | write | scheduled sequence |
| 19 | `act_clipboard` | write/read | clipboard |
| 20 | `act_run_shell` | write (gated) | spawns process |
| 21 | `act_launch` | write (gated) | launches process |
| 22 | `reflex_register` | write | adds reflex |
| 23 | `reflex_cancel` | write | removes reflex |
| 24 | `reflex_list` | read | none |
| 25 | `reflex_history` | read | none |
| 26 | `release_all` | write | releases all held inputs |
| 27 | `profile_list` | read | none |
| 28 | `profile_activate` | config | loads profile |
| 29 | `health` | read | none |
| 30 | `replay_record` | config | enables/disables replay log |

30 tools. Hard cap until ADR-approved exception.

---

## 3. Tool detail

### 3.1 `observe`

Returns the current unified perception observation.

```json
{
  "name": "observe",
  "description": "Returns structured state of the focused window and surrounding context. Replaces screenshots for most use cases.",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "include": {
        "type": "array",
        "items": {"enum": ["focused", "elements", "entities", "hud", "audio", "events", "clipboard", "fs", "diagnostics"]},
        "description": "Which slots to populate. Defaults to ['focused','elements','entities','hud','events']"
      },
      "depth": {"type": "integer", "minimum": 0, "maximum": 6, "default": 2},
      "max_elements": {"type": "integer", "minimum": 1, "maximum": 500, "default": 60},
      "since_event_seq": {"type": "integer", "description": "If set, recent_events filtered to events with seq > this"}
    }
  }
}
```

Returns `Observation` (see `06_data_schemas.md`). Typical size 1–6 KB.

Errors: `OBSERVE_NO_PERCEPTION_AVAILABLE` (all sensors down), `OBSERVE_INTERNAL` (unexpected).

### 3.2 `find`

Semantic search over visible elements and entities.

```json
{
  "name": "find",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["query"],
    "properties": {
      "query": {"type": "string", "description": "Free-text query, e.g., 'save button' or 'the red enemy'"},
      "scope": {"enum": ["elements", "entities", "both"], "default": "both"},
      "limit": {"type": "integer", "minimum": 1, "maximum": 20, "default": 5},
      "in_window": {"type": "string", "description": "Optional element_id of a window to restrict to"}
    }
  }
}
```

Returns:

```json
{
  "results": [
    {
      "kind": "element",
      "element_id": "...",
      "name": "Save",
      "role": "Button",
      "automation_id": "btnSave",
      "bbox": {"x": 100, "y": 200, "w": 80, "h": 32},
      "score": 0.93
    },
    {
      "kind": "entity",
      "entity_id": "track:42",
      "class_label": "enemy",
      "bbox": {"x": 820, "y": 340, "w": 60, "h": 130},
      "score": 0.87
    }
  ]
}
```

Implementation: combines string similarity against UIA names/automation IDs and detection class labels with a small bias for foreground-window scope.

### 3.3 `describe`

Slow-path natural-language description of the screen using a small VLM. Used for first-orientation on unknown games or when a11y + detection produce sparse results.

```json
{
  "name": "describe",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "region": {"type": "object", "properties": {"x":{}, "y":{}, "w":{}, "h":{}}, "description": "Region in screen coordinates; default full focused window"},
      "detail": {"enum": ["coarse", "standard", "detailed"], "default": "standard"}
    }
  }
}
```

Returns:

```json
{
  "description": "...",
  "model_id": "florence2-base",
  "latency_ms": 230
}
```

Latency is high (100–500 ms). Agents should use this sparingly; default to `observe` + `find` first.

### 3.4 `read_text`

OCR a region.

```json
{
  "name": "read_text",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "region": {"type": "object", "properties": {"x":{}, "y":{}, "w":{}, "h":{}}},
      "element_id": {"type": "string", "description": "Alternative to region: OCR an a11y element's bounding rect"},
      "backend": {"enum": ["winrt", "crnn", "auto"], "default": "auto"},
      "lang_hint": {"type": "string", "description": "BCP-47 language tag, e.g., 'en-US'"}
    }
  }
}
```

Returns:

```json
{
  "text": "...",
  "words": [{"text": "Save", "bbox": {}, "confidence": 0.99}],
  "language": "en"
}
```

### 3.5 `read_hud`

Read named HUD regions defined by the active profile.

```json
{
  "name": "read_hud",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "fields": {"type": "array", "items": {"type": "string"}, "description": "Subset of HUD field names; default all"}
    }
  }
}
```

Returns:

```json
{
  "readings": {
    "hp": {"value": 85, "raw_text": "85/100", "confidence": 0.98, "stale_ms": 16},
    "ammo": {"value": 12, "raw_text": "12", "confidence": 0.99, "stale_ms": 16}
  }
}
```

Errors: `HUD_NO_ACTIVE_PROFILE`, `HUD_FIELD_NOT_DEFINED`.

### 3.6 `audio_tail`

Returns a summary of recent audio events.

```json
{
  "name": "audio_tail",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "seconds": {"type": "number", "minimum": 0.1, "maximum": 30, "default": 5},
      "include_raw_waveform": {"type": "boolean", "default": false}
    }
  }
}
```

Returns:

```json
{
  "events": [
    {"at": "...", "kind": "loud_transient", "azimuth_deg": 47, "confidence": 0.8}
  ],
  "rms_db": -22.5,
  "vad_speech_pct": 0.0,
  "waveform_b64": null
}
```

### 3.7 `audio_transcribe`

Speech-to-text on the recent audio window.

```json
{
  "name": "audio_transcribe",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "seconds": {"type": "number", "minimum": 0.1, "maximum": 30, "default": 10},
      "lang_hint": {"type": "string", "default": "en"}
    }
  }
}
```

Returns:

```json
{"text": "...", "segments": [{"start_s": 0, "end_s": 1.2, "text": "..."}], "model_id": "whisper-tiny-int8"}
```

### 3.8 `subscribe`

Opens a push stream (SSE) of filtered events. Returns immediately with a `subscription_id`; events arrive as MCP notifications.

```json
{
  "name": "subscribe",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "filter": {"$ref": "#/$defs/EventFilter"},
      "snapshot_first": {"type": "boolean", "default": true, "description": "If true, push current Observation before live events"}
    }
  }
}
```

Returns:

```json
{"subscription_id": "...", "snapshot_observation_seq": 12345}
```

Push events arrive as JSON-RPC notifications with method `synapse/event` and params containing the `Event` value.

To cancel: `mcp/cancelled` JSON-RPC notification with the original request id. Synapse also exposes `subscribe_cancel(subscription_id)` for explicit teardown.

### 3.9 `set_capture_target`

Reconfigures the active capture target.

```json
{
  "name": "set_capture_target",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "target": {
        "oneOf": [
          {"type": "object", "required": ["kind"], "properties": {"kind": {"const": "primary"}}},
          {"type": "object", "required": ["kind", "monitor_index"], "properties": {"kind": {"const": "monitor"}, "monitor_index": {"type": "integer"}}},
          {"type": "object", "required": ["kind", "window_hwnd"], "properties": {"kind": {"const": "window"}, "window_hwnd": {"type": "integer"}}},
          {"type": "object", "required": ["kind", "element_id"], "properties": {"kind": {"const": "element_window"}, "element_id": {"type": "string"}}}
        ]
      },
      "min_update_interval_ms": {"type": "integer", "default": 16},
      "cursor_visible": {"type": "boolean", "default": true},
      "dirty_region_only": {"type": "boolean", "default": true}
    }
  }
}
```

### 3.10 `set_perception_mode`

```json
{
  "name": "set_perception_mode",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["mode"],
    "properties": {
      "mode": {"enum": ["a11y_only", "pixel_only", "hybrid", "auto"]}
    }
  }
}
```

### 3.11 `act_click`

```json
{
  "name": "act_click",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "target": {
        "oneOf": [
          {"type": "object", "required": ["element_id"], "properties": {"element_id": {"type": "string"}}},
          {"type": "object", "required": ["x","y"], "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}}}
        ]
      },
      "button": {"enum": ["left", "right", "middle", "x1", "x2"], "default": "left"},
      "clicks": {"type": "integer", "minimum": 1, "maximum": 3, "default": 1},
      "modifiers": {"type": "array", "items": {"enum": ["ctrl","shift","alt","super"]}},
      "curve": {"$ref": "#/$defs/AimCurve", "default": "EaseInOut"},
      "duration_ms": {"type": "integer", "default": 80, "description": "Cursor travel time when moving to target"},
      "backend": {"enum": ["software","vigem","hardware","auto"], "default": "auto"},
      "use_invoke_pattern": {"type": "boolean", "default": true, "description": "When target is an element_id with Invoke support, use semantic invoke (no cursor move)"}
    },
    "required": ["target"]
  }
}
```

### 3.12 `act_type`

```json
{
  "name": "act_type",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["text"],
    "properties": {
      "text": {"type": "string"},
      "into_element": {"type": "string", "description": "Optional element_id; focuses + clears + types"},
      "dynamics": {"enum": ["burst","linear","natural"], "default": "burst"},
      "linear_ms_per_char": {"type": "integer", "default": 30},
      "use_scancodes": {"type": "boolean", "default": false},
      "press_enter_after": {"type": "boolean", "default": false},
      "backend": {"enum": ["software","hardware","auto"], "default": "auto"}
    }
  }
}
```

### 3.13 `act_press`

```json
{
  "name": "act_press",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["keys"],
    "properties": {
      "keys": {"type": "array", "items": {"type": "string"}, "description": "Key names; multiple = chord (e.g., ['ctrl','s'])"},
      "hold_ms": {"type": "integer", "minimum": 1, "maximum": 30000, "default": 33},
      "backend": {"enum": ["software","hardware","auto"], "default": "auto"}
    }
  }
}
```

Key name vocabulary: standard symbolic names (`a`..`z`, `0`..`9`, `f1`..`f24`, `up`, `down`, `enter`, `space`, `tab`, `esc`, `ctrl`, `shift`, `alt`, `super`, `lmb`, `rmb`, `mmb`, etc.). Per-game profile may extend (e.g., `medkit` → bound to whatever key is configured in that game).

### 3.14 `act_aim`

```json
{
  "name": "act_aim",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["target"],
    "properties": {
      "target": {
        "oneOf": [
          {"type": "object", "required": ["x","y"], "properties": {"x":{"type":"integer"}, "y":{"type":"integer"}}},
          {"type": "object", "required": ["element_id"], "properties": {"element_id":{"type":"string"}}},
          {"type": "object", "required": ["track_id"], "properties": {"track_id":{"type":"integer"}}}
        ]
      },
      "style": {"enum": ["snap","flick","natural","track"], "default": "snap"},
      "deadline_ms": {"type": "integer", "default": 80},
      "backend": {"enum": ["software","hardware","auto"], "default": "auto"}
    }
  }
}
```

`style: "track"` registers an aim_track reflex and returns its `reflex_id` instead of completing immediately. Cancel via `reflex_cancel`.

### 3.15 `act_drag`

```json
{
  "name": "act_drag",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["from","to"],
    "properties": {
      "from": {"type": "object", "properties": {"x":{}, "y":{}, "element_id":{}}},
      "to":   {"type": "object", "properties": {"x":{}, "y":{}, "element_id":{}}},
      "button": {"enum": ["left","right","middle"], "default": "left"},
      "curve": {"$ref": "#/$defs/AimCurve", "default": "EaseInOut"},
      "duration_ms": {"type": "integer", "default": 200},
      "backend": {"enum": ["software","hardware","auto"], "default": "auto"}
    }
  }
}
```

### 3.16 `act_scroll`

```json
{
  "name": "act_scroll",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "dy": {"type": "integer", "default": 0, "description": "Positive = down, negative = up; units are wheel ticks"},
      "dx": {"type": "integer", "default": 0},
      "at": {"type": "object", "properties": {"x":{}, "y":{}}, "description": "Optional cursor target before scrolling"},
      "smooth": {"type": "boolean", "default": false, "description": "Split into multiple small wheel events for animation"}
    }
  }
}
```

### 3.17 `act_pad`

Drives a virtual or hardware gamepad.

```json
{
  "name": "act_pad",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["pad_id"],
    "properties": {
      "pad_id": {"type": "integer", "default": 0},
      "report": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "buttons": {"type": "array", "items": {"enum": ["a","b","x","y","lb","rb","ls","rs","back","start","up","down","left","right"]}},
          "thumb_l": {"type": "array", "items": {"type":"number"}, "minItems":2, "maxItems":2},
          "thumb_r": {"type": "array", "items": {"type":"number"}, "minItems":2, "maxItems":2},
          "lt": {"type": "number", "minimum": 0, "maximum": 1},
          "rt": {"type": "number", "minimum": 0, "maximum": 1}
        }
      },
      "backend": {"enum": ["vigem","hardware"], "default": "vigem"},
      "hold_ms": {"type": "integer", "description": "If set, applies report for this duration then returns to neutral"}
    }
  }
}
```

### 3.18 `act_combo`

```json
{
  "name": "act_combo",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["steps"],
    "properties": {
      "steps": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["at_ms", "input"],
          "properties": {
            "at_ms": {"type": "integer", "minimum": 0},
            "input": {"type": "object", "description": "Sub-action: key/mouse/pad press/release"}
          }
        }
      },
      "backend": {"enum": ["software","hardware","vigem","auto"], "default": "auto"}
    }
  }
}
```

### 3.19 `act_clipboard`

```json
{
  "name": "act_clipboard",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "verb": {"enum": ["read","write","clear"]},
      "text": {"type": "string", "description": "Required for write"},
      "format": {"enum": ["text","unicode"], "default": "unicode"}
    },
    "required": ["verb"]
  }
}
```

### 3.20 `act_run_shell`

Gated. Disabled unless `--allow-shell <pattern>` was passed at startup.

```json
{
  "name": "act_run_shell",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["argv"],
    "properties": {
      "argv": {"type": "array", "items": {"type": "string"}},
      "cwd": {"type": "string"},
      "env": {"type": "object", "additionalProperties": {"type": "string"}},
      "timeout_ms": {"type": "integer", "default": 30000},
      "idempotency_key": {"type": "string"}
    }
  }
}
```

Returns:

```json
{"exit_code": 0, "stdout": "...", "stderr": "...", "duration_ms": 152}
```

### 3.21 `act_launch`

Gated.

```json
{
  "name": "act_launch",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["target"],
    "properties": {
      "target": {"type": "string", "description": "Executable name (e.g., 'notepad.exe') or Steam appid (e.g., 'steam://run/440')"},
      "args": {"type": "array", "items": {"type": "string"}},
      "wait_for_window_title_regex": {"type": "string"},
      "wait_timeout_ms": {"type": "integer", "default": 10000}
    }
  }
}
```

Returns:

```json
{"pid": 12345, "hwnd": 67890, "launched_at": "..."}
```

### 3.22 `reflex_register`

```json
{
  "name": "reflex_register",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["kind","params"],
    "properties": {
      "kind": {"enum": ["aim_track","hold_move","hold_button","combo","on_event"]},
      "params": {"type": "object"},
      "priority": {"type": "integer", "minimum": 0, "maximum": 1000, "default": 100},
      "lifetime": {"$ref": "#/$defs/ReflexLifetime"},
      "exclusive": {"type": "boolean", "default": false}
    }
  }
}
```

Returns `{ "reflex_id": "..." }`.

### 3.23 `reflex_cancel`

```json
{
  "name": "reflex_cancel",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["reflex_id"],
    "properties": {"reflex_id": {"type": "string"}}
  }
}
```

### 3.24 `reflex_list`

Returns all currently-active reflexes for this session with their state.

### 3.25 `reflex_history`

Returns past reflex events (fires, cancellations, lifetime expiries, conflicts) within a time window.

```json
{
  "name": "reflex_history",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "since_ms_ago": {"type": "integer", "default": 60000},
      "reflex_id": {"type": "string"},
      "kinds": {"type": "array", "items": {"type": "string"}}
    }
  }
}
```

### 3.26 `release_all`

```json
{"name": "release_all", "input_schema": {"type": "object", "additionalProperties": false}}
```

Returns the count of released inputs by class.

### 3.27 `profile_list`

```json
{"name": "profile_list", "input_schema": {"type": "object", "additionalProperties": false, "properties": {"filter": {"type":"string"}}}}
```

Returns:

```json
{
  "profiles": [
    {"id": "minecraft.java", "label": "Minecraft Java Edition", "match": {"exe": "javaw.exe", "title_regex": "Minecraft"}},
    {"id": "vscode", "label": "Visual Studio Code", "match": {"exe": "Code.exe"}}
  ],
  "active": "vscode"
}
```

### 3.28 `profile_activate`

```json
{
  "name": "profile_activate",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["profile_id"],
    "properties": {"profile_id": {"type": "string"}}
  }
}
```

### 3.29 `health`

```json
{"name": "health", "input_schema": {"type": "object", "additionalProperties": false}}
```

Returns:

```json
{
  "ok": true,
  "subsystems": {
    "capture": {"status": "healthy", "fps": 60, "frames_dropped_60s": 0},
    "a11y": {"status": "healthy", "events_60s": 412},
    "audio": {"status": "healthy", "device": "Speakers (Realtek...)"},
    "perception": {"status": "healthy", "detection_p99_ms": 4.2, "ocr_p99_ms": 7.8},
    "action": {"status": "healthy", "queue_depth": 0},
    "reflex": {"status": "healthy", "active_count": 2, "tick_jitter_us_p99": 180},
    "storage": {"status": "healthy", "db_size_mb": 234},
    "models": {"loaded": ["yolov10n", "whisper-tiny"]}
  },
  "version": "0.1.0",
  "uptime_s": 1245
}
```

### 3.30 `replay_record`

```json
{
  "name": "replay_record",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["verb"],
    "properties": {
      "verb": {"enum": ["start","stop","status"]},
      "session_id": {"type": "string"}
    }
  }
}
```

---

## 4. Errors

All errors follow JSON-RPC `error` shape:

```json
{
  "jsonrpc": "2.0",
  "id": <request_id>,
  "error": {
    "code": -32099,
    "message": "Human-readable summary",
    "data": {
      "code": "ACTION_QUEUE_FULL",
      "tool": "act_click",
      "details": {"queue_depth": 256, "limit": 256},
      "retry_after_ms": 50
    }
  }
}
```

`code: SCREAMING_SNAKE_CASE` in `data.code` is the stable identifier; the numeric JSON-RPC `code` is always `-32099` (server-defined error) or `-32602` (invalid params).

Full error code catalog in `06_data_schemas.md` §Error codes.

---

## 5. Transports

| Transport | When | Capabilities |
|---|---|---|
| stdio | Local agent client (Claude Desktop, Codex CLI) | Tool calls + push notifications via the same stream |
| Streamable HTTP | Remote / multi-agent | Tool calls (POST JSON), push events (SSE upgrade on GET), session via `Mcp-Session-Id` header |

The server binary selects via `--mode {stdio|http}`. `--bind 127.0.0.1:7700` for HTTP; default bind is localhost-only.

For long-running tools (e.g., `describe` with VLM inference, `audio_transcribe`), the server upgrades to SSE and emits progress notifications:

```json
{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"...","progress":50,"total":100,"message":"Inference 50% done"}}
```

The final response arrives as a normal JSON-RPC reply when complete.

---

## 6. Session model

`initialize` creates a session. The server returns `Mcp-Session-Id` (HTTP) or implicitly tracks the stdio process. Per-session state:

- Active capture target
- Active perception mode
- Active profile
- Open subscriptions
- Registered reflexes
- Allowed back-ends (`vigem`, `hardware`)
- Allow-listed shell patterns

Closing the session (or process exit) cancels all subscriptions and reflexes, releases all held inputs (via `release_all`), and persists session metadata to `CF_SESSIONS`.

A `delete` request to the MCP endpoint with the session id explicitly tears down the session.

---

## 7. Push event format

When a subscription fires, the server emits:

```json
{
  "jsonrpc": "2.0",
  "method": "synapse/event",
  "params": {
    "subscription_id": "...",
    "event": {
      "seq": 123456,
      "at": "2026-05-22T15:00:00Z",
      "source": "perception.detection",
      "kind": "entity_appeared",
      "data": {"track_id": 42, "class_label": "enemy", "bbox": {"x":820,"y":340,"w":60,"h":130}, "confidence": 0.87}
    }
  }
}
```

The agent's MCP client must implement notification handling (most do natively).

For resumability over HTTP, each SSE event carries `id: <seq>`. On reconnect, the client sends `Last-Event-ID: <seq>` and the server replays buffered events from there. Buffer depth is 4096 events per subscription.

---

## 8. Tool examples (end-to-end)

### Example A — open Notepad, type a paragraph, save

```
→ profile_activate(profile_id="notepad")
→ act_launch(target="notepad.exe", wait_for_window_title_regex="Untitled")
← {pid:..., hwnd:...}
→ observe(include=["focused","elements"])
← Observation with the editor element_id
→ act_type(into_element="<editor_element_id>", text="Hello world.\nThis is Synapse.")
→ act_press(keys=["ctrl","s"])
→ observe(include=["focused","elements"])           # waits implicitly until UIA settles
← Observation with the Save As dialog visible
→ find(query="filename edit field", scope=elements, limit=1)
← {results: [{element_id: "...", role: "Edit"}]}
→ act_type(into_element="<filename_edit_id>", text="C:\\tmp\\demo.txt", press_enter_after=true)
```

8 tool calls, ~2 KB token cost end-to-end.

### Example B — track and shoot in a game

```
→ profile_activate(profile_id="some_fps_singleplayer")
→ observe(include=["entities","hud"])
← entities: [{entity_id: "track:42", class_label: "enemy", ...}]
→ act_aim(target={track_id:42}, style="track")
← {reflex_id: "<r1>"}
→ reflex_register(
    kind="on_event",
    params={
      when:{kind:"detection",match:{track_id:42,inside_crosshair:true}},
      then:{action:"act_press", args:{keys:["lmb"], hold_ms:50}},
      debounce_ms:200
    },
    lifetime={UntilEvent:{kind:"entity_disappeared",track_id:42}}
)
← {reflex_id: "<r2>"}
(... agent waits, reflex runtime handles per-frame ...)
→ subscribe(filter={kind:"entity_disappeared",track_id:42})
← {subscription_id: "..."}
... entity_disappeared notification arrives ...
→ reflex_cancel(reflex_id="<r1>")
```

5 tool calls + 1 push event = under 1 KB tokens, with frame-accurate execution between calls.

---

## 9. What this doc does NOT cover

- Internal `Action` enum and back-end selection → `03_action.md`
- Reflex semantics in detail → `04_reflex_runtime.md`
- Observation struct fields → `06_data_schemas.md`
- Profile TOML fields → `07_storage_and_profiles.md`
- HTTP transport details → `01_architecture.md` and reference `rmcp` docs
