# 07 — Storage and Profiles

## 1. Storage philosophy

Synapse is a single-machine, single-tenant service. The storage layer exists for three reasons:

1. **Replay debugging.** Every event, every action, every reflex firing is persisted so a session can be replayed deterministically when something breaks.
2. **Caches.** OCR results, downloaded models, profile loads — anything expensive to compute or fetch lives in RocksDB after the first access.
3. **Session continuity.** A session-id keyed view that lets a re-connecting client resume subscriptions and rediscover its registered reflexes.

We deliberately do **not** persist:

- Captured frames (huge; only replay log keeps small diff hashes)
- Audio waveforms (huge; only summarized event metadata)
- Raw UIA tree snapshots beyond the 1-Hz `CF_OBSERVATIONS` sample
- The agent's MCP request payloads beyond minimal trace metadata

Storage is **wipe-friendly**. Schema changes = wipe and rebuild. No migrations. Pre-v1; once v1 ships, schema changes require ADR + tooling.

---

## 2. Backend choice: RocksDB (with sled fallback)

RocksDB via the `rocksdb` crate. Pinned version with explicit feature flags (`features = ["multi-threaded-cf"]`).

Why RocksDB:

- Mature on Windows (despite some warts; `bzip2`/`zlib` C dep is unavoidable but stable)
- Column families let us scope reads/writes precisely
- TTL / compaction filters give us cheap rolling retention
- Snapshot reads for the replay tool

Why a sled fallback:

- RocksDB has had Windows reliability hiccups in the past
- Some operator environments forbid native C deps
- `sled` is pure Rust, simpler footprint, slower but adequate

Feature flag `synapse-storage/sled-backend` swaps the implementation. Same API surface; both implement `trait Db`.

Default: RocksDB. Sled is `--feature sled-backend` opt-in for v1; promoted to first-class if RocksDB causes issues in production.

---

## 3. Database location

```
default:    %LOCALAPPDATA%\synapse\db\
override:   --db <path>     (CLI flag)
            SYNAPSE_DB_PATH (env)
            config.toml: db.path
```

Subdirectory layout:

```
%LOCALAPPDATA%\synapse\
├── db\                            # RocksDB instance
├── models\                        # ONNX model cache (separate from CF_MODEL_CACHE for very large files)
├── profiles\                      # User-installed profiles override bundled
├── replay\                        # Replay session exports (manual)
├── logs\                          # tracing logs
└── config.toml                    # operator-level config
```

---

## 4. Column families

| CF | Key | Value | Encoding | TTL | Notes |
|---|---|---|---|---|---|
| `CF_EVENTS` | `[seq u64 BE]` | `StoredEvent` | bincode | 7d | Append-only ring; the replay log |
| `CF_OBSERVATIONS` | `[seq u64 BE]` | `StoredObservation` | bincode | 24h | 1Hz sample + reason-triggered snapshots |
| `CF_PROFILES` | `[profile_id utf8]` | TOML bytes | raw bytes | none | Cached load; source of truth is on-disk TOML in `profiles/` dirs |
| `CF_MODEL_CACHE` | `[model_sha256 32 bytes]` | model bytes | raw bytes | none | Downloaded ONNX models, sha-verified |
| `CF_SESSIONS` | `[session_id utf8]` | `StoredSession` | json | 90d | One row per session |
| `CF_REFLEX_AUDIT` | `[reflex_id 16 bytes][at_ns u64 BE]` | `StoredReflexAudit` | bincode | 7d | Per-reflex audit; multiple rows per reflex |
| `CF_OCR_CACHE` | `[image_sha256 32 bytes]` | `OcrResult` | bincode | 1h | Memoization of OCR on stable regions |
| `CF_TELEMETRY` | `[metric_name utf8][at_ns u64 BE]` | `f64 LE` | raw 8 bytes | 24h | Local metric ringbuffer when OTLP isn't configured |
| `CF_ACTION_LOG` | `[at_ns u64 BE][seq u32 BE]` | `StoredActionRecord` | bincode | 7d | Every action emitted, with backend and outcome |
| `CF_PROCESS_HISTORY` | `[at_ns u64 BE][pid u32]` | json | json | 24h | Process started/exited events, for context |
| `CF_KV` | `[utf8]` | bytes | raw | none | Generic key-value for operator/profile extensions |

`pub const`s for each CF name live in `synapse-core::cf`. Constants match the on-disk strings exactly. A test asserts the match — drift fails CI.

### 4.1 Key encoding rules

- Time-keyed CFs (`CF_EVENTS`, `CF_OBSERVATIONS`, `CF_REFLEX_AUDIT`, `CF_ACTION_LOG`, `CF_TELEMETRY`, `CF_PROCESS_HISTORY`): `u64` big-endian for natural sort order on `time-ascending` iteration. Add a `seq` or `pid` suffix as needed for uniqueness.
- ID-keyed CFs: UTF-8 strings (UUIDs as canonical hex). Easier to inspect with `ldb`.
- Hash-keyed CFs: raw 32-byte sha256.

### 4.2 TTL implementation

Per-CF compaction filter that drops records older than the CF's TTL. Implemented in `synapse-storage::compaction`. Runs on every compaction; effective expiry within ~1 hour of nominal TTL.

### 4.3 Write batches

All writes go through `Db::write_batch(Batch)` to minimize fsync cost. A `Batch` accumulates puts/deletes across multiple CFs. The storage task batches writes from a `mpsc::Receiver<WriteOp>` channel; flush triggers:

- Every 100 ms (idle batch flush)
- Every 64 KB accumulated
- Every explicit `Db::flush()` call (used at session close, after `act_run_shell`, etc.)

---

## 5. Replay log semantics

`CF_EVENTS` is the canonical replay source. Combined with `CF_OBSERVATIONS` it reconstructs any past 24h of session activity.

Replay tool: `synapse-mcp replay --session <id> [--speed 1.0] [--out <dir>]`:

1. Reads all `CF_SESSIONS` row for the id.
2. Reads `CF_EVENTS` and `CF_ACTION_LOG` for the session's time range.
3. Reads matching `CF_OBSERVATIONS` snapshots.
4. Produces a JSONL transcript + a Synapse Web Replay (SWR) bundle.
5. Optional `--simulate-actions` flag replays actions against the live machine (useful for repro).

SWR bundle is a single `.zip` containing the JSONL transcript + extracted observation snapshots + the active profile at the time. Self-contained; can be shipped for bug reports.

---

## 6. Cache management

| Cache | Eviction | Size cap |
|---|---|---|
| `CF_OCR_CACHE` | TTL 1h + LRU when > 50 MB | 50 MB |
| `CF_MODEL_CACHE` | LRU when > 1 GB; oldest least-recently-used first | 1 GB |
| `CF_PROFILES` | None (small) | Few MB |
| `CF_TELEMETRY` | TTL 24h | 200 MB hard cap (drop oldest) |

Eviction runs in the storage task on a 5-minute timer. Tracked via `cache_evictions_total{cf}` metric.

---

## 7. Operator-visible operations

| Command | Effect |
|---|---|
| `synapse-mcp db status` | DB path, total size, per-CF row count + bytes |
| `synapse-mcp db wipe` | Drops everything; confirms `--yes` flag |
| `synapse-mcp db backup <out>` | Hot backup via RocksDB checkpoint API |
| `synapse-mcp db restore <in>` | Stops daemon, restores from backup, restarts |
| `synapse-mcp db compact` | Force compaction across all CFs |
| `synapse-mcp models list` | Inventory of cached models |
| `synapse-mcp models import <path>` | Side-load a model file |
| `synapse-mcp models gc` | Drop unreferenced models |

---

## 8. Profile system

Profiles drive per-app and per-game behavior. Synapse ships a handful of bundled profiles; the operator can write more.

### 8.1 Profile directories (precedence high → low)

1. **`--profile-dir <path>`** (CLI override)
2. **`%APPDATA%\synapse\profiles\`** (user-installed)
3. **`profiles/`** beside the executable (bundled)

Within a directory, files are `<id>.toml`. ID is the basename without extension and matches the `id` field in the file. Mismatch → `PROFILE_PARSE_ERROR`.

### 8.2 TOML schema

Concrete TOML mapped to the `Profile` Rust struct from `06_data_schemas.md`. Example below for a productivity app:

```toml
# profiles/vscode.toml
id = "vscode"
label = "Visual Studio Code"
version = "1.0.0"

[[matches]]
exe = "Code.exe"

[[matches]]
exe = "VSCode.exe"

mode = "a11y_only"

[capture]
target = { kind = "foreground_window" }
min_update_interval_ms = 100
cursor_visible = true

[detection]
model_id = "none"             # disable detection for this app
classes_of_interest = []
confidence_threshold = 0.0
max_detections = 0

[ocr]
default_backend = "winrt"

# no HUD fields for VS Code
hud = []

[keymap]
save = "ctrl+s"
quick_open = "ctrl+p"
command_palette = "ctrl+shift+p"

[backends]
default = "software"
keyboard_default = "software"
mouse_default = "software"
pad_default = "vigem"
```

Game profile example:

```toml
# profiles/minecraft.java.toml
id = "minecraft.java"
label = "Minecraft Java Edition"
version = "1.0.0"

[[matches]]
exe = "javaw.exe"
title_regex = "Minecraft\\* [0-9]"

mode = "pixel_only"

[capture]
target = { kind = "foreground_window" }
min_update_interval_ms = 16
cursor_visible = true

[detection]
model_id = "yolov10n_general"
classes_of_interest = ["player", "zombie", "skeleton", "creeper", "villager"]
confidence_threshold = 0.45
max_detections = 32

[ocr]
default_backend = "winrt"

[[hud]]
name = "hp_hearts"
extractor = { kind = "template_match", templates = ["hearts/full.png", "hearts/half.png", "hearts/empty.png"] }
parser = { kind = "number" }
region = { kind = "anchored_to_edge", edge = "bottom_left", x_offset = 220, y_offset = -50, w = 180, h = 18 }

[[hud]]
name = "hunger"
extractor = { kind = "template_match", templates = ["hunger/full.png", "hunger/half.png", "hunger/empty.png"] }
parser = { kind = "number" }
region = { kind = "anchored_to_edge", edge = "bottom_right", x_offset = -400, y_offset = -50, w = 180, h = 18 }

[keymap]
forward = "w"
back = "s"
left = "a"
right = "d"
jump = "space"
sneak = "shift"
sprint = "ctrl"
attack = "lmb"
place = "rmb"
inventory = "e"
drop = "q"
chat = "t"
hotbar1 = "1"
hotbar2 = "2"
hotbar3 = "3"

[backends]
default = "software"
keyboard_default = "software"
mouse_default = "software"
pad_default = "vigem"

[[event_extensions]]
name = "creeper_nearby"
from_filter = { op = "and", args = [
    { op = "kind", kind = "entity-appeared" },
    { op = "data", path = "/class_label", predicate = { op = "eq", value = "creeper" } },
    { op = "data", path = "/bbox/w", predicate = { op = "gt", value = 80 } }
] }
emits_kind = "creeper-imminent"
```

`event_extensions` rewrites/derives custom events the agent can subscribe to or use in `on_event` reflexes. They are evaluated by the perception subsystem.

### 8.3 Match precedence

When the foreground window changes, profile detection runs:

1. Walk all loaded profiles in stable order (user-installed first, then bundled).
2. For each profile, walk `matches` array in order.
3. First match wins. A match must satisfy all populated fields (`exe`, `title_regex`, `steam_appid` — fields not specified are wildcards).

The agent can override via `profile_activate(profile_id=...)` regardless of detection.

### 8.4 Bundled profiles at v1

| Profile | Use |
|---|---|
| `notepad` | Windows Notepad (smoke-test productivity app) |
| `vscode` | Visual Studio Code |
| `chrome` | Google Chrome (CDP-enabled when remote debugging port present) |
| `terminal` | Windows Terminal / PowerShell window |
| `file_explorer` | Windows File Explorer |
| `slack` | Slack desktop |
| `discord` | Discord desktop |
| `minecraft.java` | Minecraft Java Edition (single-player) |
| `factorio` | Factorio (no anti-cheat, mod-friendly) |
| `<one FPS>` | TBD — a single-player FPS for the M3 demo (likely a free game) |

Profiles for AC-protected titles ship empty by default (`pixel_only`, no keymap) so the agent can run them in sandbox modes but does nothing competitive-impacting without operator intent.

### 8.5 Profile hot reload

Synapse watches the profile directory via `notify` crate. File changes trigger re-parse and replace in memory. Existing observation streams switch profiles cleanly on the next event tick. Reflexes are not auto-restarted; if a reflex depends on a removed keymap alias, it fails with `REFLEX_PARAMS_INVALID` on next firing.

### 8.6 Versioning

`version` is semver. Profile loader rejects profiles whose major version > Synapse-supported major. Profile minor version is informational; profile authors version their own work.

The expected workflow is community-contributed profiles in a separate repo, installed via a small CLI helper (`synapse-mcp profiles install <repo>/<name>`).

### 8.7 Profile signing (post-v1)

Profiles are TOML data. They cannot execute arbitrary code (no scripts in v1). Even so, post-v1 we plan optional profile signing via a community-key model so the operator can choose to load only signed profiles. Tracked in `16_open_questions.md`.

---

## 9. Migrations

Pre-v1: none. DB wipe on schema change is acceptable. CI tests that wipe-and-rebuild from a sample data set succeeds.

Post-v1: migrations live in `synapse-storage::migrations` with explicit `from -> to` functions. Migration runs are idempotent and resumable. A migration failure halts the daemon with `STORAGE_SCHEMA_MISMATCH`; operator runs `synapse-mcp db migrate` manually.

---

## 10. Backups

`synapse-mcp db backup <out>` uses RocksDB's `CheckpointBuilder` for a hot, consistent snapshot. Output is a directory the operator can tar/zip.

Restore: `synapse-mcp db restore <in>` stops the daemon (via shared lock file), replaces the DB dir, starts the daemon.

Backup size is approximately the live DB size; with default retention it's typically 100–500 MB.

---

## 11. Disk pressure response

If the DB directory's free disk drops below 1 GB:

1. Log `STORAGE_DISK_LOW` and emit a `system-disk-low` event.
2. Aggressively expire `CF_OCR_CACHE`, `CF_TELEMETRY`, `CF_OBSERVATIONS`.
3. If still below 500 MB, halt writes to `CF_EVENTS` (replay log) and surface `STORAGE_WRITE_FAILED` to callers with that code.
4. Below 200 MB: refuse new MCP sessions; existing sessions get a one-line warning event.

The agent can poll `health` to see disk pressure state.

---

## 12. RocksDB tuning (initial)

```rust
let mut opts = Options::default();
opts.create_if_missing(true);
opts.create_missing_column_families(true);
opts.set_max_background_jobs(2);
opts.set_compression_type(DBCompressionType::Lz4);
opts.set_max_open_files(256);
opts.set_keep_log_file_num(8);
opts.set_write_buffer_size(64 * 1024 * 1024);   // 64 MB memtable
opts.set_max_write_buffer_number(3);
opts.set_target_file_size_base(64 * 1024 * 1024);
opts.set_level_zero_file_num_compaction_trigger(4);
```

Per-CF overrides:

- `CF_EVENTS`, `CF_ACTION_LOG`, `CF_REFLEX_AUDIT`: `LZ4` compression, prefix extractor on the time prefix for range scans.
- `CF_MODEL_CACHE`: `None` compression (already compressed binary), 256 MB write buffer.
- `CF_OBSERVATIONS`, `CF_SESSIONS`: `Zstd` (smaller, fewer writes).

Tuning is in `synapse-storage::tuning`. Easy to adjust without touching call sites.

---

## 13. What this doc does NOT cover

- Sled backend specifics → `synapse-storage::sled_impl` (code only at v1; no separate doc)
- Replay tool UI → none at v1; CLI-only output
- Operator config file schema → `14_build_and_packaging.md`
- Per-CF compaction filter implementation → code only
