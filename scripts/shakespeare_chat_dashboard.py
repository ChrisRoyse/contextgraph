#!/usr/bin/env python3
"""Talk with Shakespeare — production chat dashboard.

Zero external dependencies beyond what's already installed.
Single Python file serving a modern glassmorphism chat UI.

Three-layer Shakespeare enforcement, fail-fast + self-healing:
    Layer 1 — LoRA weights (inference server on :8766)
    Layer 2 — Constrained logit decoder (inside the inference server)
    Layer 3 — 13-embedder constellation guard (context-graph daemon on :3100)

If ANY layer is broken, the dashboard returns 503 with a structured error.
No vocab-bypass, no BEST fallback, no silent passes. A background watchdog
probes both services every 15s and heals the daemon via ensure_daemon.sh.

    python3 scripts/shakespeare_chat_dashboard.py [--port 8765]
"""
from __future__ import annotations
import argparse, http.server, json, os, socket, subprocess, sys, threading, time, urllib.request
from pathlib import Path

SYSTEM = (
    "You are a knowledgeable assistant who writes EXCLUSIVELY in the style of "
    "William Shakespeare — early modern English using thee, thou, thy, thine, hast, "
    "dost, doth, ye, prithee, forsooth, hark, wherefore, methinks, 'tis, 'twas. "
    "You have full knowledge of ALL modern topics — science, technology, code, "
    "history, math, culture — and you explain them accurately and helpfully, but "
    "ALWAYS in Shakespearean prose or verse. Never refuse a topic. Never claim "
    "ignorance of modern things. Just answer everything in Shakespeare's voice."
)
INFERENCE_URL = "http://127.0.0.1:8766"          # LoRA'd Gemma inference server (Layer 1+2)

# Daemon + self-heal config (Layer 3 dependency)
DAEMON_HOST = "127.0.0.1"
DAEMON_PORT = 3100
INFERENCE_HOST = "127.0.0.1"
INFERENCE_PORT = 8766
SCRIPT_DIR = Path(__file__).resolve().parent
ENSURE_DAEMON_SH = SCRIPT_DIR / "ensure_daemon.sh"
DAEMON_PIDFILE = "/tmp/context-graph-daemon.pid"

# Shakespeare style anchor — stored constellation over ALL 313 sonnet memories,
# compiled 2026-04-15 and persisted in CF_CONSTELLATIONS. The guard scores
# every generated reply against this live centroid.
#
# Selected because Sonnets are the largest, purest-style Shakespeare corpus
# (no scene headers, no stage directions) and the centroid is stable with
# high coherence (0.9036).
SHAKESPEARE_CONSTELLATION_ID = "a8511ebf-80ad-4afb-a7d5-865b532ed4c5"  # Resurrection rebuild 2026-04-17, 161 sonnet passages

# STYLE-ONLY embedder weights for the guard verdict.
#
# Hard-won insight: style is HOW text is written, not WHAT it's about.
# E1 (semantic) and E5 (causal) capture topic, not style — they score modern
# English and Shakespeare similarly because neural-net explanations and sonnets
# can be about the same subjects. We ignore them.
#
# Calibrated against live scores (2026-04-17):
#   Pure Shakespeare  (score 0.551) — PASS
#   Shakespeare+modern mixed (0.383) — FAIL
#   Pure modern English (0.228) — FAIL
#
# E12 ColBERT and E13 SPLADE are the strongest style discriminators
# (ColBERT alone separates Shakespeare 0.68 vs Modern -0.02 = 0.70 delta).
# E7 (code/syntax patterns), E9 (HDC structural robustness), E10 (paraphrase)
# round out the panel with moderate discrimination.
STYLE_EMBEDDER_WEIGHTS = {
    # index-in-per_embedder_cosine (0-based) -> weight
    6:  0.15,   # E7  Code/syntax pattern
    8:  0.15,   # E9  HDC structural robustness
    9:  0.15,   # E10 Paraphrase (multimodal)
    11: 0.30,   # E12 ColBERT token-level phrase (STRONGEST)
    12: 0.25,   # E13 SPLADE term-expansion (STRONG)
}
assert abs(sum(STYLE_EMBEDDER_WEIGHTS.values()) - 1.0) < 1e-9, "style weights must sum to 1.0"
STYLE_SCORE_THRESHOLD = 0.45        # candidate must score >= this on style panel
VOCAB_DENSITY_THRESHOLD = 0.02      # candidate must also have >=2% archaic markers

# Retry/attempt policy
MAX_ATTEMPTS = 3

# Memory management constants
MAX_RECENT_EXCHANGES = 4   # Keep last 4 user+assistant pairs verbatim
SUMMARY_TRIGGER = 6        # Start summarizing after 6 messages (3 exchanges)


def _summarize_context(messages: list[dict]) -> str:
    """Build a compact summary of conversation topics from older messages."""
    topics = []
    for msg in messages:
        content = msg.get("content", "")
        role = msg.get("role", "")
        if role == "user":
            short = content[:80].replace("\n", " ").strip()
            topics.append(f"- The user asked: \"{short}\"")
        elif role == "assistant":
            first_line = content.split("\n")[0][:60].strip()
            if first_line:
                topics.append(f"  Shakespeare replied about: \"{first_line}...\"")
    return "\n".join(topics[-12:])


def build_context_window(full_history: list[dict]) -> list[dict]:
    """Build an optimized context window from the full conversation history."""
    if len(full_history) <= SUMMARY_TRIGGER:
        return [{"role": "system", "content": SYSTEM}] + full_history

    first_exchange = full_history[:2]
    recent_count = MAX_RECENT_EXCHANGES * 2
    recent = full_history[-recent_count:]
    middle = full_history[2:-recent_count] if len(full_history) > 2 + recent_count else []

    context_messages = [{"role": "system", "content": SYSTEM}]
    context_messages.extend(first_exchange)
    if middle:
        summary = _summarize_context(middle)
        context_messages.append({
            "role": "system",
            "content": f"[Prior conversation summary — the following topics were discussed between the first exchange and the recent messages:]\n{summary}"
        })
    context_messages.extend(recent)
    return context_messages


# Shakespeare vocabulary markers for fast local density check.
# This is a SANITY FLOOR, not the primary gate — the constellation guard is.
SHAKE_MARKERS = {
    "thee", "thou", "thy", "thine", "hast", "dost", "doth", "ye",
    "prithee", "forsooth", "hark", "wherefore", "methinks", "hath",
    "ere", "nay", "aye", "verily", "whence", "hence", "thence",
    "wilt", "shalt", "wouldst", "shouldst", "canst", "didst",
    "mayhap", "perchance", "alas", "betwixt", "anon", "yon",
    "yonder", "fie", "oft", "tarry", "beseech", "naught", "nought",
    "whilst", "unto", "thereof", "herein", "wherein", "hither",
    "thither", "whither", "morn", "quill", "mortal",
}


# =============================================================================
# Error types — every guard failure is one of these. No silent return-0 paths.
# =============================================================================
class GuardInfrastructureError(RuntimeError):
    """Guard could not run because a dependency (daemon/inference) is broken.
    The dashboard MUST return 503 and log the details. Do NOT retry per request —
    the watchdog handles recovery. Retrying a dead socket just delays the 503."""

class GuardVerdictFailed(RuntimeError):
    """Guard ran to completion but verdict was FAIL and all retries exhausted.
    The generated reply did NOT meet the Shakespeare style gate. Return 503 —
    we refuse to output off-style content."""

class GuardSchemaError(RuntimeError):
    """Daemon returned a response that does not match the expected schema.
    This is a PROTOCOL bug, not a data issue. Log the raw response and raise."""


# =============================================================================
# ServiceSupervisor — probes + heals the daemon; probes inference (never heals).
# =============================================================================
class ServiceSupervisor:
    """Monitors daemon + inference liveness. Self-heals daemon via ensure_daemon.sh.

    Flap protection: at most MAX_HEALS_PER_WINDOW heal attempts per HEAL_WINDOW_S
    seconds. Beyond that, heals are skipped and the watchdog logs CRITICAL until
    the window clears (operator must investigate root cause).
    """
    MAX_HEALS_PER_WINDOW = 3
    HEAL_WINDOW_S = 300
    PROBE_TIMEOUT_S = 5.0

    def __init__(self):
        self._heal_history: list[float] = []     # timestamps of recent heal attempts
        self._lock = threading.Lock()

    # ---------- probes (read-only, cheap) ----------
    def probe_daemon(self) -> tuple[bool, str]:
        """Return (ok, detail). MCP initialize handshake over TCP within timeout."""
        try:
            with socket.create_connection((DAEMON_HOST, DAEMON_PORT), timeout=self.PROBE_TIMEOUT_S) as s:
                s.settimeout(self.PROBE_TIMEOUT_S)
                req = json.dumps({
                    "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                               "clientInfo": {"name": "dashboard-supervisor", "version": "1"}},
                }) + "\n"
                s.sendall(req.encode())
                buf = b""
                deadline = time.time() + self.PROBE_TIMEOUT_S
                while time.time() < deadline:
                    chunk = s.recv(65536)
                    if not chunk:
                        return False, "daemon closed socket mid-initialize"
                    buf += chunk
                    if b"\n" in buf:
                        line, _, _ = buf.partition(b"\n")
                        try:
                            obj = json.loads(line)
                            if obj.get("id") == 1 and "result" in obj:
                                return True, "initialize OK"
                            return False, f"unexpected response: {line[:200]!r}"
                        except json.JSONDecodeError as e:
                            return False, f"bad json: {e} line={line[:200]!r}"
                return False, "daemon did not respond within timeout"
        except (ConnectionRefusedError, OSError) as e:
            return False, f"{type(e).__name__}: {e}"

    def probe_inference(self) -> tuple[bool, str]:
        """Return (ok, detail). HTTP GET /health within timeout."""
        try:
            with urllib.request.urlopen(f"{INFERENCE_URL}/health",
                                        timeout=self.PROBE_TIMEOUT_S) as r:
                if r.status != 200:
                    return False, f"HTTP {r.status}"
                body = json.loads(r.read())
                if not body.get("ok"):
                    return False, f"health body ok=False: {body}"
                return True, f"vram={body.get('vram_gb', '?')}GB gens={body.get('total_generations', '?')}"
        except Exception as e:
            return False, f"{type(e).__name__}: {e}"

    # ---------- heal (mutates state; invokes ensure_daemon.sh) ----------
    def _can_heal(self) -> bool:
        """Flap-protection check. Purges old history entries."""
        now = time.time()
        with self._lock:
            self._heal_history = [t for t in self._heal_history if now - t < self.HEAL_WINDOW_S]
            return len(self._heal_history) < self.MAX_HEALS_PER_WINDOW

    def _record_heal(self):
        with self._lock:
            self._heal_history.append(time.time())

    def heal_daemon(self, logger=print) -> bool:
        """Attempt to revive the daemon. Returns True if listening after heal."""
        if not self._can_heal():
            logger(f"[supervisor] REFUSED heal — {len(self._heal_history)} attempts in last "
                   f"{self.HEAL_WINDOW_S}s exceeds flap threshold {self.MAX_HEALS_PER_WINDOW}. "
                   f"Operator must investigate /tmp/context-graph-daemon.log.")
            return False
        self._record_heal()
        if not ENSURE_DAEMON_SH.is_file():
            logger(f"[supervisor] CRITICAL: {ENSURE_DAEMON_SH} missing — cannot auto-heal")
            return False
        t0 = time.time()
        logger(f"[supervisor] heal start: invoking {ENSURE_DAEMON_SH}")
        try:
            result = subprocess.run(
                [str(ENSURE_DAEMON_SH)],
                capture_output=True, text=True, timeout=360,
            )
        except subprocess.TimeoutExpired as e:
            logger(f"[supervisor] heal TIMEOUT after 360s: {e}")
            return False
        elapsed = round(time.time() - t0, 1)
        if result.returncode != 0:
            logger(f"[supervisor] heal FAILED rc={result.returncode} t={elapsed}s "
                   f"stderr={result.stderr.strip()!r}")
            return False
        ok, detail = self.probe_daemon()
        if ok:
            logger(f"[supervisor] heal OK in {elapsed}s — daemon responding: {detail}")
            return True
        logger(f"[supervisor] heal ran rc=0 but daemon still not responding: {detail}")
        return False

    # ---------- startup gate ----------
    def ensure_all_ready(self, logger=print) -> None:
        """Blocking startup check. Daemon: heal once if down. Inference: no auto-start.
        Raises RuntimeError if either cannot be reached after best effort."""
        logger("[supervisor] startup probe — daemon on 3100")
        ok, detail = self.probe_daemon()
        if ok:
            logger(f"[supervisor] daemon OK: {detail}")
        else:
            logger(f"[supervisor] daemon DOWN: {detail} — attempting heal")
            if not self.heal_daemon(logger):
                raise RuntimeError(
                    f"Daemon on {DAEMON_HOST}:{DAEMON_PORT} unreachable after heal attempt. "
                    f"Run: {ENSURE_DAEMON_SH}  (see /tmp/context-graph-daemon.log)"
                )
        logger("[supervisor] startup probe — inference server on 8766")
        ok, detail = self.probe_inference()
        if ok:
            logger(f"[supervisor] inference OK: {detail}")
        else:
            raise RuntimeError(
                f"Inference server on {INFERENCE_HOST}:{INFERENCE_PORT} unreachable: {detail}. "
                f"Start it: HF_TOKEN=hf_... python3 scripts/shakespeare_inference_server.py --port 8766"
            )


# =============================================================================
# ServiceWatchdog — background thread, probes both services every PERIOD_S.
# =============================================================================
class ServiceWatchdog(threading.Thread):
    """Periodic health monitor. Heals daemon if down. Never heals inference."""
    PERIOD_S = 15.0

    def __init__(self, supervisor: ServiceSupervisor):
        super().__init__(name="guard-watchdog", daemon=True)
        self._supervisor = supervisor
        self._stop = threading.Event()
        # Coalesce log noise: only log the state-CHANGE, not every probe.
        self._daemon_was_ok = True
        self._inference_was_ok = True

    def stop(self):
        self._stop.set()

    def run(self):
        print(f"[watchdog] running — period={self.PERIOD_S}s", file=sys.stderr)
        while not self._stop.is_set():
            try:
                self._tick()
            except Exception as e:
                print(f"[watchdog] uncaught {type(e).__name__}: {e}", file=sys.stderr)
            self._stop.wait(self.PERIOD_S)

    def _tick(self):
        d_ok, d_detail = self._supervisor.probe_daemon()
        if not d_ok:
            if self._daemon_was_ok:
                print(f"[watchdog] daemon TRANSITION ok→down: {d_detail}", file=sys.stderr)
            healed = self._supervisor.heal_daemon(
                logger=lambda m: print(m, file=sys.stderr))
            if healed:
                print(f"[watchdog] daemon restored", file=sys.stderr)
                self._daemon_was_ok = True
            else:
                self._daemon_was_ok = False
        elif not self._daemon_was_ok:
            print(f"[watchdog] daemon TRANSITION down→ok: {d_detail}", file=sys.stderr)
            self._daemon_was_ok = True

        i_ok, i_detail = self._supervisor.probe_inference()
        if not i_ok:
            if self._inference_was_ok:
                print(f"[watchdog] inference TRANSITION ok→down: {i_detail} "
                      f"(no auto-start — start it manually if you want chat)",
                      file=sys.stderr)
                self._inference_was_ok = False
        elif not self._inference_was_ok:
            print(f"[watchdog] inference TRANSITION down→ok: {i_detail}",
                  file=sys.stderr)
            self._inference_was_ok = True


# =============================================================================
# DaemonScorer — fail-fast 13-embedder guard.
# =============================================================================
class DaemonScorer:
    """Three-layer Shakespeare style verification.

    Layer 3a (local, ~instant): vocabulary density — % of archaic markers.
    Layer 3b (daemon, ~1s): constellation scoring — candidate text's per-embedder
        cosine against the Shakespeare-Sonnets constellation centroid
        (313 members, compiled 2026-04-15).

    STYLE-ONLY: we ignore semantic embedders (E1, E5) because they capture
    TOPIC, not STYLE. Shakespeare and modern English can be semantically
    identical ("about love") yet stylistically opposite. The style verdict
    uses only E7, E9, E10, E12, E13 with tuned weights.

    Every method that touches the daemon either returns usable data or raises
    a GuardError subclass. No silent fallback paths exist.
    """
    SOCKET_TIMEOUT_S = 30.0

    def __init__(self):
        self._sock: socket.socket | None = None
        self._rid = 0
        self._lock = threading.Lock()  # serialize socket access across HTTP threads

    # ---------- transport ----------
    def _reset(self):
        if self._sock is not None:
            try:
                self._sock.close()
            except Exception:
                pass
        self._sock = None

    def _connect(self):
        if self._sock is not None:
            return
        try:
            s = socket.create_connection((DAEMON_HOST, DAEMON_PORT),
                                         timeout=self.SOCKET_TIMEOUT_S)
        except (ConnectionRefusedError, OSError) as e:
            raise GuardInfrastructureError(
                f"cannot connect to daemon {DAEMON_HOST}:{DAEMON_PORT}: {type(e).__name__}: {e}"
            ) from e
        s.settimeout(self.SOCKET_TIMEOUT_S)
        self._sock = s
        try:
            self._raw_call("initialize", {
                "protocolVersion": "2024-11-05", "capabilities": {},
                "clientInfo": {"name": "dashboard-guard", "version": "2"},
            })
            self._sock.sendall((json.dumps({
                "jsonrpc": "2.0", "method": "notifications/initialized", "params": {},
            }) + "\n").encode())
        except GuardInfrastructureError:
            self._reset()
            raise

    def _raw_call(self, method: str, params: dict, timeout: float | None = None) -> dict:
        if self._sock is None:
            raise GuardInfrastructureError("socket not connected")
        self._rid += 1
        rid = self._rid
        tout = timeout or self.SOCKET_TIMEOUT_S
        try:
            self._sock.sendall((json.dumps({
                "jsonrpc": "2.0", "id": rid, "method": method, "params": params,
            }) + "\n").encode())
        except (BrokenPipeError, ConnectionResetError, OSError) as e:
            self._reset()
            raise GuardInfrastructureError(
                f"send failed on {method}: {type(e).__name__}: {e}") from e
        buf = b""
        deadline = time.time() + tout
        while time.time() < deadline:
            try:
                chunk = self._sock.recv(65536)
            except socket.timeout as e:
                self._reset()
                raise GuardInfrastructureError(f"recv timeout on {method} after {tout}s") from e
            except (ConnectionResetError, OSError) as e:
                self._reset()
                raise GuardInfrastructureError(
                    f"recv failed on {method}: {type(e).__name__}: {e}") from e
            if not chunk:
                self._reset()
                raise GuardInfrastructureError(f"daemon closed socket mid-{method}")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError as e:
                    raise GuardSchemaError(
                        f"{method} returned non-JSON line: {e} raw={line[:400]!r}") from e
                if obj.get("id") == rid:
                    return obj
                # else: notification or older response — keep draining
        self._reset()
        raise GuardInfrastructureError(f"{method} timed out after {tout}s")

    def _tool_call(self, name: str, args: dict, timeout: float | None = None) -> dict:
        """Invoke an MCP tool. Returns parsed content dict or raises."""
        resp = self._raw_call("tools/call", {"name": name, "arguments": args}, timeout)
        if "error" in resp:
            raise GuardInfrastructureError(
                f"MCP error on {name}: {resp['error']}")
        result = resp.get("result")
        if not isinstance(result, dict):
            raise GuardSchemaError(f"{name} response missing 'result' dict: {resp}")
        if result.get("isError"):
            raise GuardInfrastructureError(
                f"MCP tool {name} isError=True: {result}")
        content = result.get("content")
        if not content or not isinstance(content, list) or not content[0].get("text"):
            raise GuardSchemaError(f"{name} response missing content text: {result}")
        try:
            return json.loads(content[0]["text"])
        except json.JSONDecodeError as e:
            raise GuardSchemaError(
                f"{name} content not JSON: {e} raw={content[0]['text'][:400]!r}") from e

    # ---------- scoring ----------
    def score_vocab(self, text: str) -> tuple[float, int, int]:
        """Local Layer 3a: archaic-marker density. Returns (density, found, total)."""
        words = text.lower().split()
        if not words:
            return 0.0, 0, 0
        found = sum(1 for w in words if w.strip(".,;:!?'\"()—–-") in SHAKE_MARKERS)
        return found / len(words), found, len(words)

    def score_constellation(self, text: str) -> dict:
        """Layer 3b: store → score_against_constellation → forget.

        ALWAYS cleans up the temp fingerprint via try/finally. If anything fails
        before constellation scoring, the fingerprint is still forgotten.

        Returns a dict with:
            style_score: weighted style-only score in [0,1]
            combined_score: raw mean from score_against_constellation
            in_spread_p95: boolean from constellation scorer
            per_embedder_cosine: list of 13 floats
            fingerprint_id: the temp memory's uuid (already forgotten)
            score_ms: elapsed wall time in ms

        Raises GuardInfrastructureError / GuardSchemaError on any problem.
        """
        t0 = time.time()
        with self._lock:
            self._connect()
            fp_id: str | None = None
            try:
                store = self._tool_call("store_memory", {
                    "content": text,
                    "sessionId": "guard-check",
                })
                fp_id = store.get("fingerprintId")
                if not fp_id:
                    raise GuardSchemaError(f"store_memory missing fingerprintId: {store}")

                score = self._tool_call("score_against_constellation", {
                    "constellationId": SHAKESPEARE_CONSTELLATION_ID,
                    "memoryId": fp_id,
                })
                per = score.get("per_embedder_cosine")
                if not isinstance(per, list) or len(per) != 13:
                    raise GuardSchemaError(
                        f"score response missing 13-element per_embedder_cosine: {score}")
                combined = score.get("combined_score")
                if not isinstance(combined, (int, float)):
                    raise GuardSchemaError(
                        f"score response missing combined_score: {score}")

                style_score = 0.0
                for idx, weight in STYLE_EMBEDDER_WEIGHTS.items():
                    # Normalize negative cosines to 0 — a negative means "opposite direction"
                    # from the Shakespeare centroid, which is definitively NOT style.
                    style_score += weight * max(0.0, float(per[idx]))

                return {
                    "style_score": round(style_score, 4),
                    "combined_score": round(float(combined), 4),
                    "in_spread_p95": bool(score.get("in_spread_p95")),
                    "per_embedder_cosine": [round(float(x), 4) for x in per],
                    "fingerprint_id": fp_id,
                    "score_ms": round((time.time() - t0) * 1000, 1),
                }
            finally:
                if fp_id:
                    try:
                        self._tool_call("forget_concept", {
                            "node_id": fp_id,
                            "reason": "dashboard guard-check cleanup",
                        }, timeout=5.0)
                    except (GuardInfrastructureError, GuardSchemaError) as e:
                        print(f"[guard] WARN: forget_concept({fp_id}) failed: {e}",
                              file=sys.stderr)

    def full_score(self, text: str) -> dict:
        """Run Layer 3a + Layer 3b. Both must PASS (no bypass).

        Returns:
            {verdict: PASS|FAIL, style_score, vocab_density, combined_score,
             in_spread_p95, per_embedder_cosine, vocab_found, vocab_total,
             score_ms, style_pass, vocab_pass}

        Raises GuardInfrastructureError / GuardSchemaError — caller must not
        interpret errors as "FAIL"; they are distinct outcomes.
        """
        vocab_density, vocab_found, vocab_total = self.score_vocab(text)
        cg = self.score_constellation(text)
        vocab_pass = vocab_density >= VOCAB_DENSITY_THRESHOLD
        style_pass = cg["style_score"] >= STYLE_SCORE_THRESHOLD
        overall_pass = vocab_pass and style_pass
        return {
            "verdict": "PASS" if overall_pass else "FAIL",
            "style_score": cg["style_score"],
            "combined_score": cg["combined_score"],
            "in_spread_p95": cg["in_spread_p95"],
            "per_embedder_cosine": cg["per_embedder_cosine"],
            "vocab_density": round(vocab_density, 4),
            "vocab_found": vocab_found,
            "vocab_total": vocab_total,
            "vocab_pass": vocab_pass,
            "style_pass": style_pass,
            "fingerprint_id": cg["fingerprint_id"],
            "score_ms": cg["score_ms"],
        }


_scorer = DaemonScorer()
_supervisor = ServiceSupervisor()


# =============================================================================
# Engine — generates replies and enforces the verdict. No BEST fallback.
# =============================================================================
class Engine:
    def __init__(self):
        self._lock = threading.Lock()

    def respond(self, messages: list[dict]) -> dict:
        """Generate, verify, retry. Return structured result or raise GuardError.

        Retry policy:
          - FAIL verdict (guard ran successfully, style not met): retry at
            incrementally higher temp, up to MAX_ATTEMPTS. If all fail →
            GuardVerdictFailed (503).
          - INFRASTRUCTURE error (daemon unreachable, schema mismatch):
            propagate GuardInfrastructureError (503). Watchdog heals
            asynchronously; the NEXT request will succeed if heal worked.
        """
        attempts: list[dict] = []
        for attempt in range(1, MAX_ATTEMPTS + 1):
            temp = 0.7 + (attempt - 1) * 0.1
            t0 = time.time()
            with self._lock:
                result = self._generate(messages, temperature=temp)
            gen_ms = round((time.time() - t0) * 1000, 1)
            reply = (result.get("content") or "").strip()
            tokens = result.get("tokens", 0)
            tps = result.get("tokens_per_sec", 0)

            if not reply:
                raise GuardInfrastructureError(
                    f"inference server returned empty content at attempt {attempt}")

            # Layer 3 verification — propagates infra/schema errors untouched.
            guard = _scorer.full_score(reply)
            verdict = guard["verdict"]
            style_score = guard["style_score"]
            vocab_density = guard["vocab_density"]
            per_emb = guard["per_embedder_cosine"]

            verdict_long = (
                f"Style: score={style_score:.3f} "
                f"(threshold {STYLE_SCORE_THRESHOLD}) · "
                f"vocab={guard['vocab_found']}/{guard['vocab_total']} "
                f"({vocab_density:.1%}, threshold {VOCAB_DENSITY_THRESHOLD:.1%}) · "
                f"combined={guard['combined_score']:.3f} · "
                f"p95={'yes' if guard['in_spread_p95'] else 'no'}"
            )
            per_embedder_detail = self._format_per_embedder(per_emb)

            rec = {
                "attempt": attempt,
                "reply": reply,
                "temperature": round(temp, 2),
                "gen_ms": gen_ms,
                "score_ms": guard["score_ms"],
                "tokens": tokens,
                "tokens_per_sec": tps,
                "verdict": verdict,
                "verdict_long": verdict_long,
                "top_work": f"style {style_score:.2f}",
                "top_score": style_score,
                "top_in_p95": bool(guard["in_spread_p95"]),
                "second_work": f"vocab {vocab_density:.1%}",
                "second_score": vocab_density,
                "spread": guard["combined_score"],
                "per_embedder": per_embedder_detail,
                "style_pass": guard["style_pass"],
                "vocab_pass": guard["vocab_pass"],
            }
            attempts.append(rec)

            if verdict == "PASS":
                return {"final": rec, "attempts": attempts}

            print(f"[guard] attempt {attempt}/{MAX_ATTEMPTS} FAIL: "
                  f"style={style_score:.3f} vocab={vocab_density:.3f} "
                  f"combined={guard['combined_score']:.3f} — retrying at higher temp",
                  file=sys.stderr)

        # All attempts failed the verdict gate. No fallback.
        raise GuardVerdictFailed(
            f"All {MAX_ATTEMPTS} attempts failed Shakespeare style verdict. "
            f"Last style_score={style_score:.3f} vocab_density={vocab_density:.3f}. "
            f"This is the guard working as designed — the model broke style."
        )

    def _format_per_embedder(self, per_cosine: list[float]) -> list[dict]:
        """Format per-embedder cosines as UI-friendly records."""
        names = ["E1_Semantic", "E2_Recency", "E3_Periodic", "E4_Sequence",
                 "E5_Causal", "E6_Sparse", "E7_Code", "E8_Graph", "E9_HDC",
                 "E10_Multimodal", "E11_Entity", "E12_ColBERT", "E13_SPLADE"]
        out = []
        for i, cos in enumerate(per_cosine):
            in_style_panel = i in STYLE_EMBEDDER_WEIGHTS
            out.append({
                "embedder": names[i],
                "actual": cos,
                "weight": STYLE_EMBEDDER_WEIGHTS.get(i, 0.0),
                "style_contributor": in_style_panel,
                "pass": cos > 0 if in_style_panel else True,
            })
        return out

    def _generate(self, messages: list[dict], temperature: float = 0.8) -> dict:
        context = build_context_window(messages)
        body = {
            "messages": context,
            "temperature": temperature,
            "max_tokens": 512,
            "repetition_penalty": 1.2,
        }
        req = urllib.request.Request(
            f"{INFERENCE_URL}/api/generate",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=300) as resp:
                return json.loads(resp.read())
        except (urllib.error.URLError, socket.timeout) as e:
            raise GuardInfrastructureError(
                f"inference server unreachable: {type(e).__name__}: {e}") from e


HTML = r'''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>Talk with Shakespeare — Context Graph</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Playfair+Display:ital,wght@0,700;1,400&display=swap" rel="stylesheet">
<style>
*{box-sizing:border-box;margin:0;padding:0}
:root{
  --bg:#0a0a1a;
  --bg2:#0f0f2a;
  --surface:rgba(20,15,50,0.6);
  --surface2:rgba(30,20,70,0.4);
  --glass:rgba(120,80,255,0.08);
  --glass-border:rgba(120,80,255,0.15);
  --purple:#7c3aed;
  --purple-light:#a78bfa;
  --purple-glow:rgba(124,58,237,0.3);
  --purple-subtle:rgba(124,58,237,0.1);
  --text:#f0eef5;
  --text2:#a8a3b8;
  --text3:#6b6580;
  --green:#34d399;
  --green-bg:rgba(52,211,153,0.12);
  --yellow:#fbbf24;
  --yellow-bg:rgba(251,191,36,0.12);
  --red:#f87171;
  --red-bg:rgba(248,113,113,0.12);
  --mono:'Inter',system-ui,sans-serif;
  --display:'Playfair Display',Georgia,serif;
  --col:52rem;
}
html{height:100%;background:var(--bg)}
body{
  min-height:100%;display:flex;flex-direction:column;
  font-family:var(--mono);color:var(--text);
  background:radial-gradient(ellipse 80% 60% at 50% 0%,rgba(124,58,237,0.12),transparent 70%),
             radial-gradient(ellipse 60% 40% at 80% 100%,rgba(80,40,200,0.08),transparent 60%),
             var(--bg);
}

/* ─── Header ─── */
header{
  padding:16px 32px;display:flex;align-items:center;justify-content:space-between;
  border-bottom:1px solid var(--glass-border);
  backdrop-filter:blur(20px);-webkit-backdrop-filter:blur(20px);
  background:rgba(10,10,26,0.8);position:sticky;top:0;z-index:20;
}
.logo{display:flex;align-items:center;gap:10px}
.logo-icon{width:32px;height:32px;border-radius:8px;
  background:linear-gradient(135deg,var(--purple),#4f46e5);
  display:flex;align-items:center;justify-content:center;font-size:16px}
.logo-text{font-family:var(--display);font-size:18px;font-weight:700;color:var(--text)}
.header-pills{display:flex;gap:8px;align-items:center}
.pill{font-size:11px;padding:4px 10px;border-radius:20px;
  background:var(--glass);border:1px solid var(--glass-border);color:var(--text2)}
.pill-live{background:var(--green-bg);border-color:rgba(52,211,153,0.3);color:var(--green)}
.pill-live::before{content:"";display:inline-block;width:6px;height:6px;
  border-radius:50%;background:var(--green);margin-right:5px;
  animation:blink 2s infinite}

/* ─── Chat ─── */
#chat{
  flex:1;overflow-y:auto;padding:32px 24px 16px;
  display:flex;flex-direction:column;gap:24px;
  overscroll-behavior-y:contain;
}
#chat::-webkit-scrollbar{width:5px}
#chat::-webkit-scrollbar-thumb{background:rgba(124,58,237,0.3);border-radius:3px}

.turn{max-width:var(--col);width:100%;margin:0 auto}

/* User message */
.turn-user{display:flex;justify-content:flex-end}
.bubble-user{
  background:linear-gradient(135deg,rgba(124,58,237,0.2),rgba(80,40,200,0.15));
  border:1px solid rgba(124,58,237,0.25);
  padding:14px 20px;border-radius:20px 20px 6px 20px;
  max-width:75%;font-size:15px;line-height:1.6;
}

/* AI message */
.turn-ai{display:flex;flex-direction:column;gap:8px}
.bubble-ai{
  background:var(--surface);
  border:1px solid var(--glass-border);
  backdrop-filter:blur(12px);-webkit-backdrop-filter:blur(12px);
  padding:20px 24px;border-radius:20px 20px 20px 6px;
  font-family:var(--display);font-size:16px;line-height:1.7;
  font-style:italic;color:var(--text);
}

/* Guard badge bar */
.guard-bar{
  display:flex;flex-wrap:wrap;gap:6px;align-items:center;
  padding:0 4px;
}
.badge{
  font-family:var(--mono);font-size:11px;font-weight:600;
  padding:3px 10px;border-radius:20px;text-transform:uppercase;letter-spacing:0.5px;
}
.badge-pass{background:var(--green-bg);color:var(--green);border:1px solid rgba(52,211,153,0.25)}
.badge-warn{background:var(--yellow-bg);color:var(--yellow);border:1px solid rgba(251,191,36,0.25)}
.badge-fail{background:var(--red-bg);color:var(--red);border:1px solid rgba(248,113,113,0.25)}
.stat{font-family:var(--mono);font-size:11px;color:var(--text3);
  padding:3px 8px;background:var(--glass);border-radius:12px;border:1px solid var(--glass-border)}

/* Collapsible embedder details */
.details-wrap{
  background:var(--surface2);border:1px solid var(--glass-border);
  border-radius:14px;overflow:hidden;
}
.details-wrap summary{
  cursor:pointer;font-size:12px;color:var(--text3);padding:10px 16px;
  user-select:none;list-style:none;display:flex;align-items:center;gap:6px;
}
.details-wrap summary::before{content:"▸";transition:transform .2s}
.details-wrap[open] summary::before{transform:rotate(90deg)}
.details-wrap summary::-webkit-details-marker{display:none}
.details-inner{padding:0 16px 14px}

/* Embedder strip — bar chart */
.emb-chart{display:flex;gap:3px;height:48px;align-items:flex-end;margin:8px 0}
.emb-col{flex:1;min-width:16px;display:flex;flex-direction:column;align-items:center;gap:2px}
.emb-bar{width:100%;border-radius:3px 3px 0 0;transition:height .4s ease}
.emb-label{font-size:8px;color:var(--text3);font-family:var(--mono)}
.emb-val{font-size:8px;font-family:var(--mono)}
.emb-high{background:linear-gradient(180deg,var(--green),rgba(52,211,153,0.4))}
.emb-high .emb-val{color:var(--green)}
.emb-mid{background:linear-gradient(180deg,var(--purple-light),rgba(167,139,250,0.4))}
.emb-mid .emb-val{color:var(--purple-light)}
.emb-low{background:linear-gradient(180deg,var(--red),rgba(248,113,113,0.3))}
.emb-low .emb-val{color:var(--red)}
.emb-zero{background:rgba(107,101,128,0.2)}
.emb-zero .emb-val{color:var(--text3)}

.meta-row{display:grid;grid-template-columns:auto 1fr;gap:2px 14px;
  font-size:11px;font-family:var(--mono);color:var(--text2);margin-top:8px}
.meta-row .k{color:var(--purple-light)}

/* Attempt trail */
.trail{font-size:10px;font-family:var(--mono);color:var(--text3);margin-top:6px}

/* Typing */
.typing{font-family:var(--display);font-style:italic;color:var(--text3);font-size:15px;padding:4px 0}
@keyframes blink{0%,80%,100%{opacity:.2}40%{opacity:1}}
.dot{animation:blink 1.4s infinite}.dot:nth-child(2){animation-delay:.2s}.dot:nth-child(3){animation-delay:.4s}

/* ─── Welcome ─── */
.welcome{text-align:center;padding:80px 20px 40px;max-width:560px;margin:auto}
.welcome-glow{
  width:120px;height:120px;margin:0 auto 24px;border-radius:24px;
  background:linear-gradient(135deg,var(--purple),#4f46e5);
  box-shadow:0 0 80px var(--purple-glow),0 0 160px rgba(124,58,237,0.15);
  display:flex;align-items:center;justify-content:center;font-size:56px;
}
.welcome h2{font-family:var(--display);font-size:28px;color:var(--text);margin-bottom:12px;font-weight:700}
.welcome p{font-size:14px;color:var(--text2);line-height:1.7;margin-bottom:28px}
.suggestions{display:flex;flex-wrap:wrap;gap:10px;justify-content:center}
.sug{
  font-size:13px;padding:10px 16px;
  background:var(--glass);border:1px solid var(--glass-border);
  border-radius:12px;cursor:pointer;color:var(--text2);
  transition:all .2s;
}
.sug:hover{border-color:var(--purple);color:var(--text);background:var(--purple-subtle);
  box-shadow:0 0 20px rgba(124,58,237,0.15)}

/* ─── Input ─── */
#form-wrap{
  padding:16px 24px 20px;
  background:rgba(10,10,26,0.9);border-top:1px solid var(--glass-border);
  backdrop-filter:blur(20px);-webkit-backdrop-filter:blur(20px);
}
#form{max-width:var(--col);margin:0 auto;display:flex;align-items:flex-end;gap:10px}
#input{
  flex:1;resize:none;min-height:48px;max-height:200px;
  padding:14px 18px;border-radius:16px;
  border:1px solid var(--glass-border);
  background:var(--surface);color:var(--text);
  font:15px var(--mono);line-height:1.5;outline:none;overflow-y:auto;
  backdrop-filter:blur(8px);transition:border-color .2s;
}
#input:focus{border-color:var(--purple);box-shadow:0 0 20px rgba(124,58,237,0.1)}
#input::placeholder{color:var(--text3)}
#sendbtn{
  width:48px;height:48px;border-radius:14px;flex-shrink:0;border:none;
  background:linear-gradient(135deg,var(--purple),#4f46e5);
  color:#fff;cursor:pointer;font-size:20px;
  display:flex;align-items:center;justify-content:center;
  transition:all .2s;box-shadow:0 4px 20px rgba(124,58,237,0.3);
}
#sendbtn:disabled{opacity:.3;cursor:not-allowed;box-shadow:none}
#sendbtn:hover:not(:disabled){transform:translateY(-1px);box-shadow:0 6px 30px rgba(124,58,237,0.4)}

/* ─── Responsive ─── */
@media(min-width:2560px){:root{--col:60rem}}
@media(min-width:3440px){:root{--col:68rem}}
@media(max-width:767px){
  :root{--col:100%}
  header{padding:12px 16px}
  .header-pills{display:none}
  #chat{padding:16px 12px 8px;gap:16px}
  .bubble-user{max-width:90%}
  .bubble-ai{padding:14px 16px;font-size:15px}
  .emb-chart{overflow-x:auto;-webkit-overflow-scrolling:touch}
  .emb-col{min-width:26px;flex:0 0 26px}
  #sendbtn{width:48px;height:48px}
  .welcome{padding:40px 16px 20px}
  .welcome-glow{width:80px;height:80px;font-size:36px;border-radius:16px}
  .welcome h2{font-size:22px}
  .sug{font-size:12px;padding:8px 12px}
}
</style>
</head>
<body>
<header>
  <div class="logo">
    <div class="logo-icon">&#x1F3AD;</div>
    <span class="logo-text">Talk with Shakespeare</span>
  </div>
  <div class="header-pills">
    <span class="pill pill-live">Live</span>
    <span class="pill">Gemma 4 E4B</span>
    <span class="pill">Shakespeare LoRA</span>
    <span class="pill">SFT + DPO trained</span>
  </div>
</header>

<div id="chat">
  <div class="welcome" id="welcome">
    <div class="welcome-glow">&#x1F3AD;</div>
    <h2>Speak with William Shakespeare</h2>
    <p>This model was fine-tuned on Shakespeare's complete works using Context Graph's 13-embedder decomposition pipeline &mdash; 994 SFT examples with Gemma-generated prompts + 12,567 DPO contrastive pairs mined from cross-embedder anomalies.</p>
    <div class="suggestions">
      <span class="sug" onclick="useSug(this)">Tell me of love on a summer's morn</span>
      <span class="sug" onclick="useSug(this)">What thinkest thou of ambition?</span>
      <span class="sug" onclick="useSug(this)">My uncle hath stolen the throne &mdash; advise me</span>
      <span class="sug" onclick="useSug(this)">Speak of the stars and fate</span>
    </div>
  </div>
</div>

<div id="form-wrap">
  <form id="form" onsubmit="handleSend(event)">
    <textarea id="input" rows="1" placeholder="Speak unto Shakespeare..."
      onkeydown="if(event.key==='Enter'&&!event.shiftKey){event.preventDefault();handleSend(event)}"></textarea>
    <button id="sendbtn" type="submit" aria-label="Send">&#10148;</button>
  </form>
</div>

<script>
const chatEl=document.getElementById('chat'),inputEl=document.getElementById('input'),
      sendBtn=document.getElementById('sendbtn');
const hist=[];
const EMB=['E1','E2','E3','E4','E5','E6','E7','E8','E9','E10','E11','E12','E13'];

function esc(s){return(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')}
function useSug(el){inputEl.value=el.textContent;inputEl.focus()}
function scrollDown(){chatEl.scrollTo({top:chatEl.scrollHeight,behavior:'smooth'})}

function addUser(text){
  document.getElementById('welcome')?.remove();
  const d=document.createElement('div');d.className='turn turn-user';
  d.innerHTML='<div class="bubble-user">'+esc(text)+'</div>';
  chatEl.appendChild(d);scrollDown();
}

function addTyping(){
  const d=document.createElement('div');d.className='turn turn-ai';d.id='typing';
  d.innerHTML='<div class="bubble-ai"><span class="typing"><span class="dot">&#9679;</span> <span class="dot">&#9679;</span> <span class="dot">&#9679;</span> Shakespeare is composing&hellip;</span></div>';
  chatEl.appendChild(d);scrollDown();
}

function addAI(data){
  document.getElementById('typing')?.remove();
  const f=data.final;
  const vc=f.verdict==='REJECTED'||f.verdict==='EMPTY'||f.verdict==='FAIL'?'fail':f.verdict==='ARCHAIC'?'warn':'pass';
  const d=document.createElement('div');d.className='turn turn-ai';

  // Bubble
  let h='<div class="bubble-ai">&ldquo;'+esc(f.reply||'(Silence.)')+'&rdquo;</div>';

  // Guard bar
  h+='<div class="guard-bar">';
  h+='<span class="badge badge-'+vc+'">'+esc(f.verdict)+'</span>';
  if(f.top_work)h+='<span class="stat">'+esc(f.top_work)+'</span>';
  if(f.gen_ms)h+='<span class="stat">gen '+f.gen_ms+'ms</span>';
  if(f.score_ms)h+='<span class="stat">guard '+f.score_ms+'ms</span>';
  h+='<span class="stat">#'+f.attempt+'/'+data.attempts.length+'</span>';
  h+='</div>';

  // Collapsible details
  h+='<details class="details-wrap"><summary>Embedder details &amp; scores</summary><div class="details-inner">';

  // Bar chart
  if(f.per_embedder?.length){
    h+='<div class="emb-chart">';
    for(let i=0;i<f.per_embedder.length;i++){
      const raw=f.per_embedder[i];
      const v=typeof raw==='number'?raw:(raw?.actual??0);
      const label=typeof raw==='object'?(raw?.embedder??EMB[i]??'E'+(i+1)):EMB[i]??'E'+(i+1);
      const isStyle=typeof raw==='object'?raw?.style_contributor:false;
      const pct=Math.max(2,Math.min(100,Math.abs(v)*100));
      const cls=!isStyle?'zero':v>0.6?'high':v>0.3?'mid':v>0.01?'low':'zero';
      h+='<div class="emb-col emb-'+cls+'"><span class="emb-val">'+v.toFixed(2)+'</span>';
      h+='<div class="emb-bar" style="height:'+pct+'%"></div>';
      h+='<span class="emb-label">'+label.replace(/^E\d+_/,'').substring(0,6)+'</span></div>';
    }
    h+='</div>';
  }

  h+='<div class="meta-row">';
  h+='<span class="k">verdict</span><span>'+esc(f.verdict_long||f.verdict)+'</span>';
  const ts=typeof f.top_score==='number'?f.top_score.toFixed(3):(f.top_score??'');
  const ss=typeof f.second_score==='number'?f.second_score.toFixed(3):(f.second_score??'');
  const sp=typeof f.spread==='number'?f.spread.toFixed(3):(f.spread??'');
  h+='<span class="k">style</span><span>score='+ts+' (threshold 0.45) &middot; '+(f.style_pass?'PASS':'FAIL')+'</span>';
  h+='<span class="k">vocab</span><span>density '+ss+' &middot; combined '+sp+' &middot; '+(f.vocab_pass?'PASS':'FAIL')+'</span>';
  h+='<span class="k">timing</span><span>attempt '+f.attempt+' &middot; temp '+(f.temperature??'')+' &middot; gen '+f.gen_ms+'ms &middot; guard '+f.score_ms+'ms</span>';
  h+='</div>';

  if(data.attempts.length>1){
    h+='<div class="trail">'+data.attempts.length+' attempts: ';
    for(const a of data.attempts){
      const c2=a.verdict==='REJECTED'||a.verdict==='EMPTY'||a.verdict==='FAIL'?'badge-fail':a.verdict==='ARCHAIC'?'badge-warn':'badge-pass';
      h+='<span class="badge '+c2+'" style="font-size:9px">'+a.verdict+'</span> ';
    }
    h+='</div>';
  }
  h+='</div></details>';

  d.innerHTML=h;chatEl.appendChild(d);scrollDown();
}

async function handleSend(e){
  e.preventDefault();
  const text=inputEl.value.trim();if(!text)return;
  inputEl.value='';inputEl.style.height='auto';
  sendBtn.disabled=true;
  addUser(text);
  hist.push({role:'user',content:text});
  addTyping();
  try{
    const r=await fetch('/api/chat',{method:'POST',
      headers:{'Content-Type':'application/json'},
      body:JSON.stringify({messages:hist})});
    const data=await r.json();
    if(!r.ok){
      const msg=data?.error?.message||data?.error||('HTTP '+r.status);
      throw new Error(msg);
    }
    addAI(data);
    if(data.final.reply)hist.push({role:'assistant',content:data.final.reply});
  }catch(err){
    document.getElementById('typing')?.remove();
    const d=document.createElement('div');d.className='turn turn-ai';
    d.innerHTML='<div class="bubble-ai" style="color:var(--red);font-style:normal">Error: '+esc(err.message)+'</div>';
    chatEl.appendChild(d);
  }finally{sendBtn.disabled=false;inputEl.focus()}
}
inputEl.addEventListener('input',()=>{inputEl.style.height='auto';inputEl.style.height=Math.min(inputEl.scrollHeight,200)+'px'});
</script>
</body>
</html>
'''


class Handler(http.server.BaseHTTPRequestHandler):
    engine: Engine | None = None
    supervisor: ServiceSupervisor | None = None

    def log_message(self, f, *a):
        sys.stderr.write(f"[dashboard] {f % a}\n")

    def _json(self, code: int, obj: dict):
        b = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path in ("/", "/index.html"):
            b = HTML.encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(b)))
            self.end_headers()
            self.wfile.write(b)
        elif self.path == "/health":
            d_ok, d_detail = self.supervisor.probe_daemon()
            i_ok, i_detail = self.supervisor.probe_inference()
            code = 200 if (d_ok and i_ok) else 503
            self._json(code, {
                "ok": d_ok and i_ok,
                "daemon": {"ok": d_ok, "detail": d_detail, "port": DAEMON_PORT},
                "inference": {"ok": i_ok, "detail": i_detail, "port": INFERENCE_PORT},
                "constellation_id": SHAKESPEARE_CONSTELLATION_ID,
                "style_threshold": STYLE_SCORE_THRESHOLD,
                "vocab_threshold": VOCAB_DENSITY_THRESHOLD,
                "max_attempts": MAX_ATTEMPTS,
            })
        elif self.path == "/favicon.ico":
            self.send_response(204)
            self.end_headers()
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/api/chat":
            self.send_error(404)
            return
        try:
            raw = self.rfile.read(int(self.headers.get("Content-Length", 0)))
            p = json.loads(raw)
            msgs = p["messages"]
        except (json.JSONDecodeError, KeyError, ValueError) as e:
            self._json(400, {"error": {"kind": "bad_request", "message": str(e)}})
            return

        try:
            self._json(200, self.engine.respond(msgs))
        except GuardVerdictFailed as e:
            sys.stderr.write(f"[dashboard] guard verdict failed: {e}\n")
            self._json(503, {"error": {
                "kind": "style_verdict_failed",
                "message": str(e),
                "action": "The LoRA or constrained decoder broke style. Inspect recent generations; "
                          "consider retraining data or lowering temperature.",
            }})
        except GuardInfrastructureError as e:
            sys.stderr.write(f"[dashboard] guard infrastructure error: {e}\n")
            self._json(503, {"error": {
                "kind": "guard_infrastructure",
                "message": str(e),
                "action": "Daemon or inference server unreachable. Watchdog will attempt heal. "
                          "Retry in ~30s, or inspect /tmp/context-graph-daemon.log.",
            }})
        except GuardSchemaError as e:
            sys.stderr.write(f"[dashboard] guard schema error: {e}\n")
            self._json(500, {"error": {
                "kind": "guard_schema",
                "message": str(e),
                "action": "This is a PROTOCOL bug between dashboard and daemon. File a report "
                          "with the raw response above.",
            }})
        except Exception as e:
            sys.stderr.write(f"[dashboard] UNHANDLED {type(e).__name__}: {e}\n")
            import traceback; traceback.print_exc(file=sys.stderr)
            self._json(500, {"error": {
                "kind": "unhandled",
                "message": f"{type(e).__name__}: {e}",
            }})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8765)
    args = ap.parse_args()

    print(f"[dashboard] boot — three-layer guard with fail-fast + self-healing",
          file=sys.stderr)
    print(f"[dashboard] constellation anchor: {SHAKESPEARE_CONSTELLATION_ID} (Sonnets)",
          file=sys.stderr)
    print(f"[dashboard] thresholds: style>={STYLE_SCORE_THRESHOLD} vocab>={VOCAB_DENSITY_THRESHOLD} "
          f"max_attempts={MAX_ATTEMPTS}", file=sys.stderr)

    # Hard gate: don't bind the dashboard port unless both dependencies are live.
    _supervisor.ensure_all_ready(
        logger=lambda m: print(m, file=sys.stderr))

    watchdog = ServiceWatchdog(_supervisor)
    watchdog.start()

    Handler.engine = Engine()
    Handler.supervisor = _supervisor
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"[dashboard] READY — http://127.0.0.1:{args.port}", file=sys.stderr)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n[dashboard] shutdown", file=sys.stderr)
    finally:
        watchdog.stop()
        httpd.server_close()


if __name__ == "__main__":
    main()
