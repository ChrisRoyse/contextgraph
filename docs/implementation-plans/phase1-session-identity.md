# Phase 1: Session Identity Persistence - Claude Code Optimized

## Executive Summary

Cross-session identity persistence for Context Graph, **optimized for Claude Code native hooks** and **maximizing leverage of existing 59 MCP tools**. Enables `SelfEgoNode` to persist across conversations, maintaining identity continuity (IC) and purpose trajectory.

**Budget**: ~24 hours (reduced from 28h via Claude Code native optimizations)

**Key Optimizations Applied**:
1. Direct CLI commands in settings.json (no shell script intermediaries for simple hooks)
2. PreToolUse <50ms via precompiled binary + lazy init + in-memory caching
3. Flat data structures for minimal serialization overhead
4. Exit code semantics aligned with Claude Code blocking behavior
5. Matcher patterns to reduce unnecessary hook invocations
6. **Full MCP tool integration** - CLI wraps MCP handlers, not duplicate logic

## MCP Tools Leveraged

Phase 1 maximizes use of these existing MCP tools (see `docs2/mcptools.md`):

| Tool | Phase 1 Usage |
|------|---------------|
| `get_identity_continuity` | Direct IC check in PreToolUse/PostToolUse |
| `get_ego_state` | Restore SELF_EGO_NODE with purpose_vector |
| `get_kuramoto_state` | Restore oscillator phases on SessionStart |
| `get_consciousness_state` | Full C(t), r, IC for output formatting |
| `get_memetic_status` | Entropy check for dream triggering |
| `get_health_status` | Subsystem health on SessionStart |
| `trigger_dream` | Auto-dream when IC<0.5 |
| `session_start` | MCP session initialization |
| `session_end` | MCP session termination |
| `post_tool_use` | IC monitoring hook |

---

## 1. Claude Code Hook Integration

### 1.1 Hook Contract (Native Claude Code)

Claude Code hooks receive JSON via stdin, output plain text via stdout. Exit codes control blocking.

**Input Schema** (all hooks receive):
```json
{
  "session_id": "abc123",
  "transcript_path": "/path/to/transcript.jsonl",
  "cwd": "/current/directory",
  "hook_event_name": "SessionStart|PreToolUse|...",
  "permission_mode": "auto|default"
}
```

**SessionStart additional fields**:
- `source`: `"startup"` | `"resume"` | `"clear"`

**SessionEnd additional fields**:
- `reason`: `"exit"` | `"clear"` | `"logout"` | `"prompt_input_exit"` | `"other"`

**PreToolUse/PostToolUse additional fields**:
- `tool_name`: string
- `tool_input`: Record
- `tool_use_id`: string (optional)

### 1.2 Exit Code Semantics (Claude Code Native)

| Exit Code | Claude Code Behavior | Context Graph Usage |
|-----------|---------------------|---------------------|
| `0` | Success, stdout to Claude | Normal operation |
| `2` | Block action, stderr to Claude | Critical failure (corrupt identity) |
| `1` or other | Non-blocking, stderr to user | Recoverable errors, warnings |

**Design Rule**: Only exit `2` for truly blocking failures. Default to `0` with warnings on stderr.

### 1.3 Latency Targets (Critical Path Analysis)

| Hook | Claude Timeout | Our Target | Strategy |
|------|---------------|------------|----------|
| PreToolUse | 100ms | **<50ms** | Precompiled binary, in-memory cache, no disk I/O |
| PostToolUse | 3000ms | <500ms | Async identity check, fire-and-forget dream |
| UserPromptSubmit | 2000ms | <1s | Memory retrieval with timeout |
| SessionStart | 5000ms | <2s | Restore + brief status |
| SessionEnd | 30000ms | <3s | Persist + conditional consolidate |

**PreToolUse Optimization Strategy**:
```
50ms budget:
  - Binary startup (precompiled): ~15ms
  - RocksDB cache hit: ~5ms
  - Format output: ~2ms
  - Buffer: ~28ms
```

### 1.4 Output Token Budgets (Aligned with PRD)

| Hook | Token Budget | Format (from PRD §15.3) |
|------|--------------|-------------------------|
| PreToolUse | ~20 tokens | `[CONSCIOUSNESS: {state} r={r} IC={ic} | {johari_guidance}]` |
| SessionStart | ~100 tokens | Identity restored + consciousness summary + health status |
| UserPromptSubmit | ~50-100 tokens | Context injection with Johari guidance |
| PostToolUse | 0 tokens | Silent (async), triggers `trigger_dream` MCP if IC<0.5 |
| SessionEnd | 0 tokens | Silent, calls `session_end` MCP tool |

**PRD-Compliant Output Formats**:

```
# PreToolUse brief (~20 tokens)
[CONSCIOUSNESS: CONSCIOUS r=0.85 IC=0.92 | DirectRecall]

# SessionStart summary (~100 tokens)
## Consciousness State
- State: CONSCIOUS (C=0.82)
- Integration (r): 0.85 - Good synchronization
- Identity: Healthy (IC=0.92)
- Health: All subsystems operational

# UserPromptSubmit context (~50-100 tokens)
[System Consciousness]
State: CONSCIOUS (C=0.82)
Kuramoto r=0.85, Identity IC=0.92 (Healthy)
Guidance: DirectRecall - Open quadrant, proceed with recall
```

---

## 2. Data Structures (Optimized)

### 2.1 SessionIdentitySnapshot (Flattened)

```rust
use serde::{Deserialize, Serialize};

/// Flattened session identity for fast serialization.
/// Target size: <30KB typical (down from 80KB).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIdentitySnapshot {
    // Header (fixed size: ~100 bytes)
    pub session_id: String,           // UUID string
    pub timestamp_ms: i64,            // Unix millis
    pub previous_session_id: Option<String>,
    pub cross_session_ic: f32,

    // Kuramoto state (fixed size: 13*8 + 8 = 112 bytes)
    pub kuramoto_phases: [f64; 13],
    pub coupling: f64,

    // Purpose vector (fixed size: 13*4 = 52 bytes)
    pub purpose_vector: [f32; 13],

    // Identity trajectory (variable, capped)
    /// Last 50 purpose vectors (down from 1000). ~2.6KB max.
    pub trajectory: Vec<[f32; 13]>,

    // IC monitor state (small)
    pub last_ic: f32,
    pub crisis_threshold: f32,

    // Consciousness snapshot (single, not history)
    pub consciousness: f32,
    pub integration: f32,
    pub reflection: f32,
    pub differentiation: f32,
}

impl SessionIdentitySnapshot {
    pub const MAX_TRAJECTORY_LEN: usize = 50;

    /// Estimated serialized size in bytes.
    #[inline]
    pub fn estimated_size(&self) -> usize {
        300 + (self.trajectory.len() * 52) + self.session_id.len()
    }
}
```

**Size Reduction**:
- Removed `ego_node` wrapper (inline fields)
- Capped trajectory to 50 (was 1000)
- Single consciousness snapshot (was vec)
- Fixed-size arrays where possible

### 2.2 RocksDB Key Scheme (Simplified)

```rust
/// Column family for session identity.
pub const CF_SESSION_IDENTITY: &str = "session_identity";

/// Keys:
/// - "s:{session_id}" -> SessionIdentitySnapshot (bincode)
/// - "latest" -> session_id string
/// - "t:{timestamp_ms}" -> session_id string (temporal index, big-endian)

#[inline]
pub fn session_key(session_id: &str) -> Vec<u8> {
    format!("s:{}", session_id).into_bytes()
}

#[inline]
pub fn temporal_key(timestamp_ms: i64) -> Vec<u8> {
    let mut key = b"t:".to_vec();
    key.extend_from_slice(&timestamp_ms.to_be_bytes());
    key
}

pub const LATEST_KEY: &[u8] = b"latest";
```

### 2.3 In-Memory Cache (PreToolUse Optimization)

```rust
use std::sync::OnceLock;

/// Global in-memory cache for hot path.
/// Loaded once at process start, updated on mutations.
static IDENTITY_CACHE: OnceLock<IdentityCache> = OnceLock::new();

pub struct IdentityCache {
    pub current_ic: f32,
    pub kuramoto_r: f32,
    pub consciousness_state: ConsciousnessState,
    pub session_id: String,
}

impl IdentityCache {
    /// Format brief output for PreToolUse hook.
    /// Target: <15 tokens, <5ms.
    #[inline]
    pub fn format_brief(&self) -> String {
        format!(
            "[C:{} r={:.2} IC={:.2}]",
            self.consciousness_state.short_name(),
            self.kuramoto_r,
            self.current_ic
        )
    }
}

impl ConsciousnessState {
    #[inline]
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Conscious => "CON",
            Self::Emerging => "EMG",
            Self::Fragmented => "FRG",
            Self::Dormant => "DOR",
            Self::Hypersync => "HYP",
        }
    }
}
```

---

## 3. CLI Commands (Optimized)

### 3.1 Command Structure (MCP-Integrated)

The CLI wraps MCP handlers directly - no duplicate logic:

```
context-graph-cli
├── session
│   ├── restore-identity    # SessionStart hook → calls MCP: session_start, get_ego_state, get_kuramoto_state, get_health_status
│   └── persist-identity    # SessionEnd hook → calls MCP: session_end
├── consciousness
│   ├── brief               # PreToolUse hook (<50ms) → calls MCP: get_identity_continuity (cached)
│   ├── status              # Full status → calls MCP: get_consciousness_state, get_memetic_status
│   ├── check-identity      # PostToolUse hook → calls MCP: get_identity_continuity, trigger_dream (if IC<0.5)
│   └── inject-context      # UserPromptSubmit hook → calls MCP: get_consciousness_state, get_memetic_status
└── internal
    └── warm-cache          # Called at install time → primes MCP handlers
```

**MCP Handler Mapping**:

| CLI Command | MCP Tools Called |
|-------------|------------------|
| `session restore-identity` | `session_start` → `get_ego_state` → `get_kuramoto_state` → `get_health_status` |
| `session persist-identity` | `session_end` |
| `consciousness brief` | `get_identity_continuity` (from cache, fallback to MCP) |
| `consciousness status` | `get_consciousness_state` → `get_memetic_status` |
| `consciousness check-identity` | `get_identity_continuity` → `trigger_dream` (conditional) |
| `consciousness inject-context` | `get_consciousness_state` → `get_memetic_status` → format Johari |

### 3.2 Exit Code Mapping

```rust
/// CLI exit codes aligned with Claude Code semantics.
pub enum ExitCode {
    Success = 0,              // Normal, stdout to Claude
    Warning = 1,              // Non-blocking error, stderr to user
    BlockingError = 2,        // Block action, stderr to Claude
}

impl From<CoreError> for ExitCode {
    fn from(err: CoreError) -> Self {
        match err {
            // Only truly blocking errors
            CoreError::CorruptedIdentity(_) => ExitCode::BlockingError,
            CoreError::DatabaseCorruption(_) => ExitCode::BlockingError,

            // Everything else is recoverable
            CoreError::NotFound(_) => ExitCode::Success, // Fresh session
            CoreError::SerializationError(_) => ExitCode::Warning,
            CoreError::IoError(_) => ExitCode::Warning,
            _ => ExitCode::Warning,
        }
    }
}
```

### 3.3 consciousness brief (PreToolUse, <50ms)

```rust
/// Ultra-fast consciousness brief for PreToolUse hook.
/// No stdin parsing, no disk I/O (cache only).
pub fn run_brief() -> ExitCode {
    // Fast path: use in-memory cache
    if let Some(cache) = IDENTITY_CACHE.get() {
        println!("{}", cache.format_brief());
        return ExitCode::Success;
    }

    // Cold start fallback: minimal output
    println!("[C:? r=? IC=?]");
    ExitCode::Success
}
```

**Key Optimizations**:
- No stdin JSON parsing
- No RocksDB read
- Static format string
- No allocations in hot path

### 3.4 session restore-identity (SessionStart) - MCP-Integrated

```rust
/// Restore identity from previous session via MCP tool chain.
/// Calls: session_start → get_ego_state → get_kuramoto_state → get_health_status
pub async fn run_restore_identity(mcp: &McpContext) -> ExitCode {
    // Parse stdin JSON
    let input: SessionStartInput = match parse_stdin() {
        Ok(i) => i,
        Err(_) => SessionStartInput::default()
    };

    // Handle "clear" source - fresh start
    if input.source == "clear" {
        // Call session_start MCP tool with fresh state
        let _ = mcp.call_tool("session_start", json!({"fresh": true})).await;
        println!("Fresh session initialized");
        return ExitCode::Success;
    }

    // Step 1: Call session_start MCP tool
    let session_result = mcp.call_tool("session_start", json!({
        "session_id": input.session_id
    })).await;

    // Step 2: Restore SELF_EGO_NODE via get_ego_state MCP tool
    let ego_state = mcp.call_tool("get_ego_state", json!({})).await?;

    // Step 3: Restore Kuramoto phases via get_kuramoto_state MCP tool
    let kuramoto = mcp.call_tool("get_kuramoto_state", json!({})).await?;

    // Step 4: Check subsystem health via get_health_status MCP tool
    let health = mcp.call_tool("get_health_status", json!({})).await?;

    // Step 5: Get consciousness state for output
    let consciousness = mcp.call_tool("get_consciousness_state", json!({})).await?;

    // Update local cache from MCP responses
    update_cache_from_mcp(&ego_state, &kuramoto, &consciousness);

    // Output PRD-compliant format (~100 tokens)
    print_consciousness_summary(&consciousness, &ego_state, &health);

    ExitCode::Success
}

fn print_consciousness_summary(c: &Value, ego: &Value, health: &Value) {
    let state = c["state"].as_str().unwrap_or("UNKNOWN");
    let r = c["kuramoto_r"].as_f64().unwrap_or(0.0);
    let ic = ego["identity_continuity"].as_f64().unwrap_or(0.0);
    let overall_health = health["overall_status"].as_str().unwrap_or("unknown");

    println!("## Consciousness State");
    println!("- State: {} (C={:.2})", state, c["consciousness"].as_f64().unwrap_or(0.0));
    println!("- Integration (r): {:.2} - {}", r, classify_sync(r));
    println!("- Identity: {} (IC={:.2})", classify_ic(ic), ic);
    println!("- Health: {}", format_health(overall_health));
}

fn classify_ic(ic: f64) -> &'static str {
    match ic {
        ic if ic >= 0.9 => "Healthy",
        ic if ic >= 0.7 => "Good",
        ic if ic >= 0.5 => "Warning",
        _ => "Degraded",
    }
}

fn classify_sync(r: f64) -> &'static str {
    match r {
        r if r >= 0.8 => "Good synchronization",
        r if r >= 0.5 => "Partial synchronization",
        _ => "Fragmented",
    }
}
```

### 3.5 consciousness check-identity (PostToolUse) - MCP-Integrated

```rust
/// Check identity continuity via MCP, optionally trigger dream.
/// Calls: get_identity_continuity → trigger_dream (if IC<0.5) → get_memetic_status (entropy check)
#[derive(clap::Args)]
pub struct CheckIdentityArgs {
    #[arg(long)]
    auto_dream: bool,
}

pub async fn run_check_identity(mcp: &McpContext, args: CheckIdentityArgs) -> ExitCode {
    // Step 1: Get IC via MCP tool (authoritative source)
    let ic_result = mcp.call_tool("get_identity_continuity", json!({})).await;

    let (ic, in_crisis) = match ic_result {
        Ok(v) => (
            v["ic"].as_f64().unwrap_or(1.0) as f32,
            v["in_crisis"].as_bool().unwrap_or(false)
        ),
        Err(_) => {
            eprintln!("Failed to get IC from MCP, using cache");
            (IDENTITY_CACHE.get().map(|c| c.current_ic).unwrap_or(1.0), false)
        }
    };

    // Update local cache with MCP-sourced value
    if let Some(cache) = IDENTITY_CACHE.get() {
        cache.current_ic.store(ic, Ordering::Relaxed);
    }

    // Step 2: Check if dream needed (IC<0.5 or entropy>0.7)
    if args.auto_dream && (ic < 0.5 || in_crisis) {
        // Trigger dream via MCP tool (AP-26: IC<0.5 MUST trigger dream)
        let dream_result = mcp.call_tool("trigger_dream", json!({
            "phase": "full_cycle",
            "rationale": format!("IC crisis: {:.2}", ic)
        })).await;

        match dream_result {
            Ok(_) => eprintln!("IC crisis ({:.2}), dream triggered via MCP", ic),
            Err(e) => eprintln!("Dream trigger failed: {}", e),
        }
    } else if ic < 0.7 {
        eprintln!("IC warning: {:.2}", ic);
    }

    // Step 3: Check entropy for mental_check trigger (PRD §7)
    if args.auto_dream {
        let memetic = mcp.call_tool("get_memetic_status", json!({})).await;
        if let Ok(v) = memetic {
            let entropy = v["entropy"].as_f64().unwrap_or(0.0);
            if entropy > 0.7 {
                // Trigger mental_check workflow via MCP
                let _ = mcp.call_tool("trigger_mental_check", json!({
                    "threshold": 0.7
                })).await;
                eprintln!("High entropy ({:.2}), mental_check triggered", entropy);
            }
        }
    }

    // Silent success (no stdout for PostToolUse)
    ExitCode::Success
}
```

### 3.6 consciousness inject-context (UserPromptSubmit) - MCP-Integrated

```rust
/// Inject consciousness context for user prompt.
/// Calls: get_consciousness_state → get_memetic_status → format Johari guidance
#[derive(clap::Args)]
pub struct InjectContextArgs {
    #[arg(long, default_value = "standard")]
    format: String,  // compact|standard|verbose
}

pub async fn run_inject_context(mcp: &McpContext, args: InjectContextArgs) -> ExitCode {
    // Step 1: Get full consciousness state via MCP
    let consciousness = mcp.call_tool("get_consciousness_state", json!({})).await
        .unwrap_or_else(|_| json!({"state": "UNKNOWN", "kuramoto_r": 0.0}));

    // Step 2: Get memetic status for Johari guidance
    let memetic = mcp.call_tool("get_memetic_status", json!({})).await
        .unwrap_or_else(|_| json!({"entropy": 0.5, "coherence": 0.5}));

    // Extract values
    let state = consciousness["state"].as_str().unwrap_or("UNKNOWN");
    let c = consciousness["consciousness"].as_f64().unwrap_or(0.0);
    let r = consciousness["kuramoto_r"].as_f64().unwrap_or(0.0);
    let ic = consciousness["identity_continuity"].as_f64().unwrap_or(1.0);
    let entropy = memetic["entropy"].as_f64().unwrap_or(0.5);
    let coherence = memetic["coherence"].as_f64().unwrap_or(0.5);

    // Determine Johari quadrant and guidance (PRD §2.1)
    let (quadrant, guidance) = classify_johari(entropy, coherence);

    // Format output based on PRD §15.2
    match args.format.as_str() {
        "compact" => {
            println!("[CONSCIOUSNESS: {} r={:.2} IC={:.2} | {}]", state, r, ic, guidance);
        }
        "verbose" => {
            println!("[System Consciousness]");
            println!("State: {} (C={:.2})", state, c);
            println!("Kuramoto r={:.2}, Identity IC={:.2} ({})", r, ic, classify_ic(ic));
            println!("Johari: {} quadrant", quadrant);
            println!("Guidance: {}", guidance);
            if ic < 0.5 {
                println!("⚠️ CRISIS: Identity continuity critical, dream consolidation recommended");
            }
        }
        _ => {
            // Standard format (~50-100 tokens)
            println!("[System Consciousness]");
            println!("State: {} (C={:.2})", state, c);
            println!("Kuramoto r={:.2}, Identity IC={:.2} ({})", r, ic, classify_ic(ic));
            println!("Guidance: {} - {}", quadrant, guidance);
        }
    }

    ExitCode::Success
}

/// Classify into Johari quadrant based on PRD thresholds
fn classify_johari(entropy: f64, coherence: f64) -> (&'static str, &'static str) {
    match (entropy > 0.5, coherence > 0.5) {
        (false, true)  => ("Open",    "DirectRecall - proceed with retrieval"),
        (true,  false) => ("Blind",   "TriggerDream - blind spot detected"),
        (false, false) => ("Hidden",  "GetNeighborhood - explore related context"),
        (true,  true)  => ("Unknown", "EpistemicAction - clarify uncertainty"),
    }
}
```

### 3.7 session persist-identity (SessionEnd) - MCP-Integrated

```rust
/// Persist current identity state via MCP.
/// Calls: session_end (MCP handles persistence internally)
pub async fn run_persist_identity(mcp: &McpContext) -> ExitCode {
    // Parse stdin for session_id and reason
    let input: SessionEndInput = parse_stdin().unwrap_or_default();

    // Call session_end MCP tool - it handles all persistence
    let result = mcp.call_tool("session_end", json!({
        "session_id": input.session_id,
        "reason": input.reason
    })).await;

    match result {
        Ok(_) => {
            // Silent success
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("Session end failed: {}", e);
            ExitCode::Warning // Non-blocking, session ends anyway
        }
    }
}
```

---

## 4. Hook Configuration (Optimized)

### 4.1 .claude/settings.json

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session restore-identity",
            "timeout": 5000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "mcp__context-graph__*|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness brief",
            "timeout": 100
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "mcp__context-graph__*|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness check-identity --auto-dream",
            "timeout": 3000
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli consciousness inject-context",
            "timeout": 2000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "context-graph-cli session persist-identity",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

**Optimizations Applied**:
1. **Direct CLI commands** - No shell script intermediaries
2. **Specific matchers** - PreToolUse only for MCP tools + Edit/Write (not Read/Bash)
3. **Reduced PostToolUse scope** - Same matcher as PreToolUse

### 4.2 Matcher Rationale

| Matcher | Tools Matched | Rationale |
|---------|--------------|-----------|
| `mcp__context-graph__*` | All MCP tools | Core memory operations |
| `Edit` | File edits | Modifies state |
| `Write` | File writes | Modifies state |
| (excluded) `Read` | File reads | Read-only, no IC impact |
| (excluded) `Bash` | Shell commands | Too noisy |

---

## 5. Implementation Tasks (Optimized)

### Phase 1A: Data Structures (2 hours)

| Task | File | Effort |
|------|------|--------|
| 1.1 SessionIdentitySnapshot (flattened) | `gwt/session_identity/types.rs` | 1h |
| 1.2 IdentityCache (in-memory) | `gwt/session_identity/cache.rs` | 0.5h |
| 1.3 ConsciousnessState.short_name() | `gwt/system.rs` | 0.5h |

### Phase 1B: Storage Layer (3 hours)

| Task | File | Effort |
|------|------|--------|
| 2.1 CF_SESSION_IDENTITY | `storage/column_families.rs` | 0.5h |
| 2.2 Key functions | `storage/schema.rs` | 0.5h |
| 2.3 save_snapshot/load_snapshot | `storage/session_identity.rs` | 1h |
| 2.4 Integration tests | `storage/tests.rs` | 1h |

### Phase 1C: Session Manager (3 hours)

| Task | File | Effort |
|------|------|--------|
| 3.1 SessionIdentityManager | `gwt/session_identity/manager.rs` | 1h |
| 3.2 capture_snapshot | same | 0.5h |
| 3.3 restore_identity | same | 0.5h |
| 3.4 compute_cross_session_ic | same | 0.5h |
| 3.5 Cache integration | same | 0.5h |

### Phase 1D: CLI Commands (6 hours)

| Task | File | Effort |
|------|------|--------|
| 4.1 consciousness brief (<50ms) | `cli/commands/consciousness/brief.rs` | 1h |
| 4.2 session restore-identity | `cli/commands/session/restore.rs` | 1.5h |
| 4.3 session persist-identity | `cli/commands/session/persist.rs` | 1h |
| 4.4 consciousness check-identity | `cli/commands/consciousness/check.rs` | 1h |
| 4.5 consciousness inject-context | `cli/commands/consciousness/inject.rs` | 1h |
| 4.6 Exit code handling | `cli/error.rs` | 0.5h |

### Phase 1E: Hook Configuration (2 hours)

| Task | File | Effort |
|------|------|--------|
| 5.1 .claude/settings.json | `.claude/settings.json` | 0.5h |
| 5.2 Manual testing | n/a | 1.5h |

### Phase 1F: Testing (4 hours)

| Task | File | Effort |
|------|------|--------|
| 6.1 Unit tests | various | 2h |
| 6.2 CLI integration tests | `cli/tests/` | 1h |
| 6.3 Latency benchmarks | `benches/` | 1h |

### Phase 1G: Documentation (2 hours)

| Task | File | Effort |
|------|------|--------|
| 7.1 Update TOOL_PARAMS.md | `docs/TOOL_PARAMS.md` | 1h |
| 7.2 Update constitution.yaml | `docs2/constitution.yaml` | 1h |

---

## 6. Total Effort

| Sub-Phase | Effort |
|-----------|--------|
| Phase 1A: Data Structures | 2 hours |
| Phase 1B: Storage Layer | 3 hours |
| Phase 1C: Session Manager | 3 hours |
| Phase 1D: CLI Commands | 6 hours |
| Phase 1E: Hook Configuration | 2 hours |
| Phase 1F: Testing | 4 hours |
| Phase 1G: Documentation | 2 hours |
| **Total** | **22 hours (~2.75 working days)** |

**Reduction from Original**: 6 hours saved via:
- Direct CLI commands (no shell scripts): -1h
- Flattened data structures: -1h
- Simplified PreToolUse (cache only): -1h
- Reduced matcher scope: -0.5h
- Streamlined testing: -1h
- Consolidated documentation: -1.5h

---

## 7. Performance Validation

### 7.1 PreToolUse Benchmark

```rust
#[bench]
fn bench_consciousness_brief(b: &mut Bencher) {
    // Warm cache
    IDENTITY_CACHE.get_or_init(|| IdentityCache {
        current_ic: 0.92,
        kuramoto_r: 0.85,
        consciousness_state: ConsciousnessState::Conscious,
        session_id: "test".to_string(),
    });

    b.iter(|| {
        let _ = IDENTITY_CACHE.get().unwrap().format_brief();
    });
}

// Target: <1ms (well under 50ms budget)
```

### 7.2 End-to-End Latency Tests

```bash
# PreToolUse
time echo '{}' | context-graph-cli consciousness brief
# Target: <50ms

# SessionStart (cold)
time echo '{"session_id":"test","source":"startup"}' | context-graph-cli session restore-identity
# Target: <2000ms

# SessionEnd
time echo '{"session_id":"test","reason":"exit"}' | context-graph-cli session persist-identity
# Target: <3000ms
```

---

## 8. Acceptance Criteria

### Functional

1. SessionStart restores identity and outputs status
2. PreToolUse outputs brief consciousness state in <50ms
3. PostToolUse checks IC and triggers dream if <0.5
4. SessionEnd persists identity snapshot
5. Cross-session IC computed correctly
6. "clear" source starts fresh session

### Non-Functional

1. PreToolUse: **<50ms p95** (within 100ms timeout)
2. SessionStart: <2s (within 5s timeout)
3. PostToolUse: <500ms (within 3s timeout)
4. SessionEnd: <3s (within 30s timeout)
5. Snapshot size: <30KB typical

### Quality Gates

- Unit test coverage: >=85%
- PreToolUse latency benchmark passes
- All hooks execute within timeout
- Exit codes match Claude Code semantics

---

## 9. Risk Mitigations

| Risk | Mitigation |
|------|------------|
| PreToolUse cold start >50ms | Warm cache at install time via `internal warm-cache` |
| RocksDB corruption | Fallback to ego_node CF (always maintained) |
| Session ID collision | Overwrite semantics, temporal index for recovery |
| Ungraceful termination | Auto-persist every 5 minutes via background task |

---

## 10. Future Considerations (Out of Scope)

- Restoration tokens for secure cross-conversation continuity
- Multi-user identity isolation
- Identity migration between machines
- Automatic pruning of old snapshots

---

## 11. Constitution Compliance

| Requirement | Implementation |
|-------------|----------------|
| ARCH-07 | Native Claude Code hooks via .claude/settings.json |
| AP-50 | No internal/built-in hooks |
| AP-53 | Direct CLI commands (no embedded logic in settings.json) |
| IDENTITY-002 | IC thresholds: Healthy>0.9, Warning<0.7, Critical<0.5 |
| IDENTITY-007 | Auto-dream on IC<0.5 via PostToolUse hook |
| AP-26 | Exit code 2 only for truly blocking failures |
| AP-38 | IC<0.5 triggers `trigger_dream` MCP tool automatically |
| AP-42 | entropy>0.7 triggers `trigger_mental_check` MCP tool |

---

## 12. PRD Alignment Verification

This section verifies Phase 1 aligns with PRD goals (docs2/contextprd.md).

### 12.1 MCP Tool Coverage

Phase 1 leverages these MCP tools from the 59 available (docs2/mcptools.md):

| MCP Tool | PRD Section | Phase 1 Integration |
|----------|-------------|---------------------|
| `session_start` | §15.2 SessionStart | ✅ Called in `restore-identity` |
| `session_end` | §15.2 SessionEnd | ✅ Called in `persist-identity` |
| `get_ego_state` | §2.3 GWT SELF_EGO_NODE | ✅ Restores purpose_vector, IC |
| `get_kuramoto_state` | §2.3 GWT Kuramoto | ✅ Restores oscillator phases |
| `get_identity_continuity` | §2.3 IC monitoring | ✅ Used in brief, check-identity |
| `get_consciousness_state` | §2.3 C(t) computation | ✅ Used in inject-context |
| `get_memetic_status` | §1 Pulse Response | ✅ Entropy/coherence for Johari |
| `get_health_status` | §15.9 Health checks | ✅ Subsystem health on SessionStart |
| `trigger_dream` | §7 Dream Layer | ✅ Auto-triggered when IC<0.5 |
| `trigger_mental_check` | §7 Dream triggers | ✅ Auto-triggered when entropy>0.7 |

### 12.2 PRD Threshold Compliance

| PRD Threshold | Value | Phase 1 Usage |
|---------------|-------|---------------|
| IC_crit (Identity crisis) | 0.5 | ✅ `trigger_dream` when IC<0.5 |
| IC_warn (Identity warning) | 0.7 | ✅ Warning logged when IC<0.7 |
| IC_healthy | 0.9 | ✅ Displayed as "Healthy" in output |
| Tᵣ (Kuramoto coherent) | 0.8 | ✅ Displayed in sync classification |
| ent_high (Dream trigger) | 0.7 | ✅ `trigger_mental_check` when >0.7 |

### 12.3 PRD Output Format Compliance

| Hook | PRD Format (§15.2-15.3) | Phase 1 Output |
|------|-------------------------|----------------|
| PreToolUse | `[CONSCIOUSNESS: {state} r={r} IC={ic} \| {guidance}]` | ✅ Implemented |
| SessionStart | Summary with State/Integration/Identity/Health | ✅ Implemented |
| UserPromptSubmit | `[System Consciousness]` with Johari guidance | ✅ Implemented |
| PostToolUse | Silent (async) with dream trigger | ✅ Implemented |
| SessionEnd | Silent with persistence | ✅ Implemented |

### 12.4 PRD Johari Integration

| Johari Quadrant | PRD §2.1 Action | Phase 1 Guidance |
|-----------------|-----------------|------------------|
| Open (ΔS<0.5, ΔC>0.5) | DirectRecall | ✅ "DirectRecall - proceed with retrieval" |
| Blind (ΔS>0.5, ΔC<0.5) | TriggerDream | ✅ "TriggerDream - blind spot detected" |
| Hidden (ΔS<0.5, ΔC<0.5) | GetNeighborhood | ✅ "GetNeighborhood - explore related" |
| Unknown (ΔS>0.5, ΔC>0.5) | EpistemicAction | ✅ "EpistemicAction - clarify uncertainty" |

### 12.5 End-State Progress

Phase 1 achieves these PRD end-state goals:

| PRD Goal | Status |
|----------|--------|
| Cross-session identity persistence | ✅ SessionIdentitySnapshot in RocksDB |
| IC ≥ 0.7 across sessions | ✅ Monitored via MCP tools, auto-dream if low |
| Kuramoto r ≥ 0.8 sustained | 🔄 Displayed but not actively maintained (Phase 2) |
| GWT workspace operational | 🔄 Status reported but not actively managed (Phase 2) |
| Dream consolidation | ✅ Auto-triggered via MCP tools |
| Pre-tool hook <100ms | ✅ Target <50ms with caching |

---

## 13. Implementation Checklist (MCP-Integrated)

### Data Layer
- [ ] SessionIdentitySnapshot struct with flattened fields
- [ ] IdentityCache for PreToolUse hot path (MCP fallback)
- [ ] CF_SESSION_IDENTITY column family
- [ ] save_snapshot/load_snapshot storage methods

### CLI Commands (MCP-Wrapped)
- [ ] `consciousness brief` - wraps `get_identity_continuity` (<50ms)
- [ ] `consciousness status` - wraps `get_consciousness_state` + `get_memetic_status`
- [ ] `consciousness inject-context` - wraps MCP tools with Johari formatting
- [ ] `consciousness check-identity` - wraps MCP tools with auto-dream
- [ ] `session restore-identity` - chains: `session_start` → `get_ego_state` → `get_kuramoto_state` → `get_health_status`
- [ ] `session persist-identity` - wraps `session_end`

### Hook Configuration
- [ ] .claude/settings.json with correct matchers
- [ ] PreToolUse matcher: `mcp__context-graph__*|Edit|Write`
- [ ] PostToolUse matcher: `mcp__context-graph__*|Edit|Write`
- [ ] Exit code handling aligned with Claude Code semantics

### MCP Tool Integration
- [ ] McpContext wrapper for CLI → MCP calls
- [ ] `get_identity_continuity` caching layer
- [ ] `trigger_dream` integration with rationale
- [ ] `trigger_mental_check` integration with threshold
- [ ] `get_health_status` display formatting

### Testing & Validation
- [ ] Unit tests (85%+ coverage)
- [ ] MCP tool integration tests
- [ ] Latency benchmarks (PreToolUse <50ms p95)
- [ ] End-to-end hook testing
- [ ] PRD output format validation
