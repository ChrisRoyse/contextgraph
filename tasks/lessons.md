# Lessons — Training Data Export Implementation

Running log of corrections, patterns, and rules. Append-only during this session.

## 2026-04-14 — Pre-flight

- **Lesson**: `subprocess.PIPE` on the server's `stderr` deadlocks when the consumer never reads it. The kernel fills the pipe buffer, the server blocks on `tracing::info!`, and the harness hangs mid-session.
  - Rule: use `stderr=subprocess.DEVNULL` or drain with a background thread; never PIPE without a reader.

- **Lesson**: `os.kill(pid, 0)` returns success for Linux zombie (`Z`) processes. "Still alive" checks need `/proc/{pid}/status` State or `Popen.poll()`.
  - Rule: liveness check = process is either missing from `/proc` OR state is `Z`. Anything else means "still alive in a meaningful way."

- **Lesson**: `prctl(PR_SET_PDEATHSIG, SIGTERM)` + an OS-thread doomsday watchdog calling `libc::_exit(0)` after 20 s is the bulletproof combination for preventing orphan MCP servers. Neither alone is sufficient.

- **Lesson**: `compute_topic_profile_from_fingerprint` using raw L2 norms saturates at ~1.0 for unit-normalized embeddings, destroying signal. The real per-memory topic profile is already stored in `CF_TOPIC_PROFILES` at store time — **read that CF** instead of re-deriving.

- **Lesson**: `get_causal_relationships_by_source(fp.id)` returns records whose `source_fingerprint_id == fp.id` by construction. Setting `related_memory_id = rel.source_fingerprint_id` means every "related peer" is self. Use `rel.id` (the relationship UUID) for joining; `related_memory_id` stays `Uuid::nil()` unless the peer end is resolvable.

- **Lesson**: `includeShape=true` over 1000 UUIDs with 1000 sequential `get_cf` calls is ~30× slower than one `multi_get_cf`. Always prefer batched RocksDB reads on hot paths.

- **Lesson**: `std::thread::spawn` CAN fail (EAGAIN under thread exhaustion). The doomsday watchdog that panics on spawn failure is its own worst enemy. Log-and-continue.

## Patterns adopted

- Every CF: const, options fn, descriptor list entry, counts bumped, tests updated — all in one commit.
- Every new MCP tool: names.rs + definitions/<category>.rs + handlers/tools/<category>_tools.rs + dispatch.rs + count test.
- Every new value format: version byte + bincode (dense) or version byte + JSON (provenance).
- Every new CF: MCP read-back tools (list/get/count) mandatory for FSV.
