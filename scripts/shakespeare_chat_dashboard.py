#!/usr/bin/env python3
"""Talk with Shakespeare — production chat dashboard.

Zero external dependencies beyond what's already installed.
Single Python file serving a modern glassmorphism chat UI.

    python3 scripts/shakespeare_chat_dashboard.py [--port 8765]
"""
from __future__ import annotations
import argparse, http.server, json, sys, threading, time, urllib.request
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
INFERENCE_URL = "http://127.0.0.1:8766"  # Custom LoRA inference server

# Memory management constants
MAX_RECENT_EXCHANGES = 4   # Keep last 4 user+assistant pairs verbatim
SUMMARY_TRIGGER = 6        # Start summarizing after 6 messages (3 exchanges)


def _summarize_context(messages: list[dict]) -> str:
    """Build a compact summary of conversation topics from older messages.

    Returns a single string that captures WHAT was discussed without
    consuming the full token budget of the original messages.
    """
    topics = []
    for msg in messages:
        content = msg.get("content", "")
        role = msg.get("role", "")
        if role == "user":
            # Extract the core topic from user prompts
            short = content[:80].replace("\n", " ").strip()
            topics.append(f"- The user asked: \"{short}\"")
        elif role == "assistant":
            # Extract the first line of each response as a topic anchor
            first_line = content.split("\n")[0][:60].strip()
            if first_line:
                topics.append(f"  Shakespeare replied about: \"{first_line}...\"")
    return "\n".join(topics[-12:])  # Cap at 12 topic lines


def build_context_window(full_history: list[dict]) -> list[dict]:
    """Build an optimized context window from the full conversation history.

    Architecture:
    1. System prompt (always present, always first)
    2. If history is short (≤ SUMMARY_TRIGGER): send everything
    3. If history is long:
       a. First exchange (2 messages) — so model remembers conversation origin
       b. Compact summary of middle turns — topics only, not full text
       c. Last MAX_RECENT_EXCHANGES exchanges — full verbatim for coherence

    This gives the model:
    - Memory of what the conversation started with (HP-08 fix)
    - Awareness of all topics discussed (no lost context)
    - Full recent context for coherent follow-ups
    - Short enough that the LoRA stays dominant (EC-01 fix)
    """
    if len(full_history) <= SUMMARY_TRIGGER:
        return [{"role": "system", "content": SYSTEM}] + full_history

    # Split into: first exchange, middle, recent
    first_exchange = full_history[:2]  # First user + first assistant
    recent_count = MAX_RECENT_EXCHANGES * 2  # 4 exchanges = 8 messages
    recent = full_history[-recent_count:]
    middle = full_history[2:-recent_count] if len(full_history) > 2 + recent_count else []

    context_messages = [{"role": "system", "content": SYSTEM}]

    # First exchange — verbatim
    context_messages.extend(first_exchange)

    # Middle — summarized into a single system message
    if middle:
        summary = _summarize_context(middle)
        context_messages.append({
            "role": "system",
            "content": f"[Prior conversation summary — the following topics were discussed between the first exchange and the recent messages:]\n{summary}"
        })

    # Recent exchanges — verbatim
    context_messages.extend(recent)

    return context_messages


DAEMON_HOST = "127.0.0.1"
DAEMON_PORT = 3100
MAX_ATTEMPTS = 2
GUARD_ENABLED = True

# =========================================================================
# SHAKESPEARE STYLE SIGNATURE — precomputed from 2,698 memories
# across 13 embedders. These are the FINGERPRINT of Shakespeare's writing.
# Derived from Context Graph's 249K+ training signals.
#
# Style embedders (CV < 0.15 — consistent across ALL Shakespeare):
#   E1, E4, E6, E7, E8, E9, E10, E12, E13
# These measure HOW text is written, not WHAT it's about.
# =========================================================================
STYLE_CENTROID = {
    "E1_Semantic":   0.9019,   # CV=0.006 — very tight
    "E4_Sequence":   0.5242,   # CV=0.044
    "E6_Sparse":     0.5701,   # CV=0.056
    "E7_Code":       0.8160,   # CV=0.027
    "E8_Graph":      0.7833,   # CV=0.010 — extremely tight
    "E9_HDC":        0.5465,   # CV=0.042
    "E10_Multimodal":0.8960,   # CV=0.011 — extremely tight
    "E12_ColBERT":   0.6240,   # CV=0.087
    "E13_SPLADE":    0.5677,   # CV=0.047
}
STYLE_P5 = {  # 5th percentile — minimum expected for real Shakespeare
    "E1_Semantic":   0.8923,
    "E4_Sequence":   0.4821,
    "E6_Sparse":     0.5260,
    "E7_Code":       0.7664,
    "E8_Graph":      0.7698,
    "E9_HDC":        0.4998,
    "E10_Multimodal":0.8862,
    "E12_ColBERT":   0.5556,
    "E13_SPLADE":    0.5253,
}

# Shakespeare vocabulary markers for fast local check (Layer 3a)
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


class DaemonScorer:
    """Three-layer Shakespeare style verification using Context Graph.

    Layer 3a (0ms, local): Vocabulary density — count Shakespeare markers.
    Layer 3b (200ms, daemon): 13-embedder fingerprint — store text, search,
        compare embedder scores to precomputed Shakespeare centroid.

    The centroid was extracted from 2,698 Shakespeare memories. 9 of 13
    embedders are STYLE signals (CV < 0.15) — they measure HOW text is
    written, not WHAT it's about. A Shakespeare poem about pizza will
    score the same as Shakespeare about love on these embedders.
    """

    def __init__(self):
        self._sock = None
        self._rid = 0

    def _connect(self):
        import socket
        if self._sock is not None:
            return
        self._sock = socket.socket()
        self._sock.settimeout(30)
        self._sock.connect((DAEMON_HOST, DAEMON_PORT))
        self._raw_call("initialize", {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "dashboard-guard", "version": "1"},
        })
        self._sock.sendall((json.dumps({
            "jsonrpc": "2.0", "method": "notifications/initialized", "params": {},
        }) + "\n").encode())

    def _raw_call(self, method, params, timeout=30):
        self._rid += 1
        rid = self._rid
        self._sock.sendall((json.dumps({
            "jsonrpc": "2.0", "id": rid, "method": method, "params": params,
        }) + "\n").encode())
        buf = b""
        deadline = time.time() + timeout
        while time.time() < deadline:
            chunk = self._sock.recv(65536)
            if not chunk:
                raise RuntimeError("daemon closed socket")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                obj = json.loads(line)
                if obj.get("id") == rid:
                    return obj
        raise TimeoutError("daemon timeout")

    def _tool_call(self, name, args, timeout=30):
        r = self._raw_call("tools/call", {"name": name, "arguments": args}, timeout)
        if r.get("result", {}).get("isError"):
            return None
        try:
            return json.loads(r["result"]["content"][0]["text"])
        except Exception:
            return None

    def score_local(self, text):
        """Layer 3a: Fast local vocabulary density check."""
        words = text.lower().split()
        if not words:
            return 0.0, 0, 0
        found = sum(1 for w in words if w.strip(".,;:!?'\"()") in SHAKE_MARKERS)
        density = found / len(words)
        return density, found, len(words)

    def score_embedders(self, text):
        """Layer 3b: 13-embedder fingerprint check via Context Graph.

        Stores text temporarily, searches for it to get embedder scores,
        compares to precomputed Shakespeare centroid, cleans up.

        Returns (style_score, per_embedder_detail, score_ms)
        """
        t0 = time.time()
        try:
            self._connect()

            # Store temporarily to embed through all 13 embedders
            store = self._tool_call("store_memory", {
                "content": text,
                "sessionId": "guard-check",
                "sourceType": "Manual",
            })
            if not store:
                return 0, [], round((time.time() - t0) * 1000, 1)

            fp_id = store.get("fingerprintId", "")

            # Search for this exact text to get its embedder scores
            search = self._tool_call("search_graph", {
                "query": text[:200],  # Use first 200 chars as query
                "limit": 1,
            })

            # Clean up
            if fp_id:
                self._tool_call("forget_concept", {"node_id": fp_id})

            score_ms = round((time.time() - t0) * 1000, 1)

            if not search or not search.get("results"):
                return 0, [], score_ms

            # Extract embedder scores from search result
            result = search["results"][0]
            es = result.get("embedderScores", {})

            # Map to our centroid structure
            score_map = {
                "E1_Semantic": (es.get("semantic", {}).get("E1_Semantic") or 0),
                "E4_Sequence": (es.get("temporal", {}).get("E4_Sequence") or 0),
                "E6_Sparse": (es.get("semantic", {}).get("E6_Sparse") or 0),
                "E7_Code": (es.get("semantic", {}).get("E7_Code") or 0),
                "E8_Graph": (es.get("relational", {}).get("E8_Graph") or 0),
                "E9_HDC": (es.get("structural", {}).get("E9_HDC") or 0),
                "E10_Multimodal": (es.get("semantic", {}).get("E10_Multimodal") or 0),
                "E12_ColBERT": (es.get("semantic", {}).get("E12_ColBERT") or 0),
                "E13_SPLADE": (es.get("semantic", {}).get("E13_SPLADE") or 0),
            }

            # Score: how many style embedders are within expected range?
            in_range = 0
            total_checked = 0
            per_detail = []
            for name, centroid_val in STYLE_CENTROID.items():
                actual = score_map.get(name, 0)
                if actual <= 0:
                    continue
                total_checked += 1
                p5 = STYLE_P5.get(name, 0)
                is_ok = actual >= p5 * 0.8  # Allow 20% margin below p5
                if is_ok:
                    in_range += 1
                per_detail.append({
                    "embedder": name,
                    "actual": round(actual, 4),
                    "expected": round(centroid_val, 4),
                    "threshold": round(p5 * 0.8, 4),
                    "pass": is_ok,
                })

            # Style score = fraction of embedders that agree "this is Shakespeare"
            style_score = in_range / total_checked if total_checked > 0 else 0
            return round(style_score, 4), per_detail, score_ms

        except Exception as e:
            score_ms = round((time.time() - t0) * 1000, 1)
            print(f"[guard] embedder scoring error: {e}", file=sys.stderr)
            self._sock = None
            return 0, [], score_ms

    def full_score(self, text):
        """Combined three-layer score.

        Returns dict with all scoring details.
        """
        # Layer 3a: Local vocabulary check (instant)
        vocab_density, vocab_found, vocab_total = self.score_local(text)

        # Layer 3b: 13-embedder fingerprint (200ms)
        style_score, per_embedder, score_ms = self.score_embedders(text)

        # Combined verdict: both layers must agree
        vocab_pass = vocab_density >= 0.02  # At least 2% Shakespeare words
        style_pass = style_score >= 0.6     # At least 60% of embedders in range
        overall_pass = vocab_pass and style_pass

        # If vocab passes strongly, relax embedder requirement
        if vocab_density >= 0.05:
            overall_pass = True  # Dense Shakespeare vocab = definitely Shakespeare

        verdict = "PASS" if overall_pass else "FAIL"

        return {
            "verdict": verdict,
            "vocab_density": round(vocab_density, 4),
            "vocab_found": vocab_found,
            "vocab_total": vocab_total,
            "style_score": style_score,
            "embedder_consensus": f"{int(style_score * 100)}% of embedders agree",
            "per_embedder": per_embedder,
            "score_ms": score_ms,
            "overall_pass": overall_pass,
        }


_scorer = DaemonScorer()


class Engine:
    def __init__(self):
        self._lock = threading.Lock()

    def respond(self, messages):
        best_rec = None
        attempts = []

        for attempt in range(1, MAX_ATTEMPTS + 1):
            temp = 0.7 + (attempt - 1) * 0.05

            # Layer 1 (LoRA) + Layer 2 (constrained decoder): Generate
            t0 = time.time()
            with self._lock:
                result = self._generate(messages, temperature=temp)
            gen_ms = round((time.time() - t0) * 1000, 1)
            reply = result.get("content", "")
            tokens = result.get("tokens", 0)
            tps = result.get("tokens_per_sec", 0)

            # Layer 3: Shakespeare style verification
            if GUARD_ENABLED and reply:
                guard = _scorer.full_score(reply)
                passed = guard["overall_pass"]
                verdict = guard["verdict"]
                score_ms = guard["score_ms"]
                style_score = guard["style_score"]
                per_embedder = guard["per_embedder"]
                vocab_density = guard["vocab_density"]

                top_work = f"{guard['embedder_consensus']}"
                top_score = style_score
                in_p95 = vocab_density >= 0.05

                verdict_long = (
                    f"Style: {int(style_score*100)}% embedder consensus, "
                    f"{guard['vocab_found']}/{guard['vocab_total']} markers "
                    f"({vocab_density:.1%} density)"
                )
            else:
                passed, verdict, score_ms = True, "LORA", 0
                style_score, per_embedder, vocab_density = 0, [], 0
                top_work, top_score, in_p95 = "LoRA-trained", None, False
                verdict_long = f"Shakespeare LoRA — {tokens} tokens at {tps} tok/s"

            rec = {
                "attempt": attempt, "reply": reply, "temperature": temp,
                "gen_ms": gen_ms, "score_ms": score_ms,
                "verdict": verdict, "verdict_long": verdict_long,
                "top_work": top_work, "top_score": top_score,
                "top_in_p95": in_p95,
                "second_work": None,
                "second_score": round(vocab_density, 4) if vocab_density else None,
                "spread": round(style_score, 4) if style_score else None,
                "per_embedder": per_embedder,
            }
            attempts.append(rec)

            if passed:
                best_rec = rec
                break
            else:
                print(f"[guard] attempt {attempt}: style={style_score:.2f}, "
                      f"vocab={vocab_density:.3f}, retrying...", file=sys.stderr)

        if best_rec is None:
            best_rec = max(attempts, key=lambda r: (r.get("top_score") or 0))
            best_rec["verdict"] = "BEST"

        return {"final": best_rec, "attempts": attempts}

    def _generate(self, messages, temperature=0.8):
        context = build_context_window(messages)
        body = {
            "messages": context,
            "temperature": temperature,
            "max_tokens": 512,
            "repetition_penalty": 1.2,
        }
        req = urllib.request.Request(f"{INFERENCE_URL}/api/generate",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"}, method="POST")
        with urllib.request.urlopen(req, timeout=300) as resp:
            return json.loads(resp.read())


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
  const vc=f.verdict==='REJECTED'||f.verdict==='EMPTY'?'fail':f.verdict==='ARCHAIC'?'warn':'pass';
  const d=document.createElement('div');d.className='turn turn-ai';

  // Bubble
  let h='<div class="bubble-ai">&ldquo;'+esc(f.reply||'(Silence.)')+'&rdquo;</div>';

  // Guard bar
  h+='<div class="guard-bar">';
  h+='<span class="badge badge-'+vc+'">'+esc(f.verdict)+'</span>';
  if(f.top_work)h+='<span class="stat">'+esc(f.top_work)+' &middot; '+(f.top_score??'')+'</span>';
  if(f.gen_ms)h+='<span class="stat">gen '+f.gen_ms+'ms</span>';
  if(f.score_ms)h+='<span class="stat">guard '+f.score_ms+'ms</span>';
  h+='<span class="stat">#'+f.attempt+'/'+data.attempts.length+'</span>';
  h+='</div>';

  // Collapsible details
  h+='<details class="details-wrap"><summary>Embedder details &amp; scores</summary><div class="details-inner">';

  // Bar chart — handles both old format (numbers) and new format (objects with .actual)
  if(f.per_embedder?.length){
    h+='<div class="emb-chart">';
    for(let i=0;i<f.per_embedder.length;i++){
      const raw=f.per_embedder[i];
      const v=typeof raw==='number'?raw:(raw?.actual??0);
      const label=typeof raw==='object'?(raw?.embedder??EMB[i]??'E'+(i+1)):EMB[i]??'E'+(i+1);
      const passed=typeof raw==='object'?raw?.pass:true;
      const pct=Math.max(2,Math.min(100,Math.abs(v)*100));
      const cls=!passed?'zero':v>0.7?'high':v>0.4?'mid':v>0.01?'low':'zero';
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
  h+='<span class="k">style</span><span>'+esc(f.top_work||'')+' (score: '+ts+(f.top_in_p95?' &middot; dense vocab':'')+')</span>';
  h+='<span class="k">vocab</span><span>density: '+ss+(sp?' &middot; embedder consensus: '+sp:'')+'</span>';
  h+='<span class="k">timing</span><span>attempt '+f.attempt+' &middot; temp '+(f.temperature??'')+' &middot; gen '+f.gen_ms+'ms &middot; guard '+f.score_ms+'ms</span>';
  h+='</div>';

  if(data.attempts.length>1){
    h+='<div class="trail">'+data.attempts.length+' attempts: ';
    for(const a of data.attempts){
      const c2=a.verdict==='REJECTED'||a.verdict==='EMPTY'?'badge-fail':a.verdict==='ARCHAIC'?'badge-warn':'badge-pass';
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
    if(!r.ok)throw new Error('HTTP '+r.status+': '+await r.text());
    const data=await r.json();
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
    engine=None
    def log_message(self,f,*a):sys.stderr.write(f"[shakespeare] {f%a}\n")
    def _json(self,code,obj):
        b=json.dumps(obj).encode()
        self.send_response(code);self.send_header("Content-Type","application/json")
        self.send_header("Content-Length",str(len(b)));self.end_headers();self.wfile.write(b)
    def do_GET(self):
        if self.path in("/","/index.html"):
            b=HTML.encode();self.send_response(200)
            self.send_header("Content-Type","text/html; charset=utf-8")
            self.send_header("Content-Length",str(len(b)));self.end_headers();self.wfile.write(b)
        elif self.path=="/health":self._json(200,{"ok":True,"constellations":len(self.engine.cons)})
        elif self.path=="/favicon.ico":self.send_response(204);self.end_headers()
        else:self.send_error(404)
    def do_POST(self):
        if self.path!="/api/chat":self.send_error(404);return
        raw=self.rfile.read(int(self.headers.get("Content-Length",0)))
        try:p=json.loads(raw);msgs=p["messages"]
        except(json.JSONDecodeError,KeyError)as e:self._json(400,{"error":str(e)});return
        try:self._json(200,self.engine.respond(msgs))
        except Exception as e:
            sys.stderr.write(f"[shakespeare] ERROR: {e}\n");self._json(500,{"error":str(e)})


def main():
    ap=argparse.ArgumentParser();ap.add_argument("--port",type=int,default=8765)
    args=ap.parse_args()
    # Verify inference server is running
    print(f"[shakespeare] checking inference server at {INFERENCE_URL} ...",file=sys.stderr)
    try:
        r=urllib.request.urlopen(f"{INFERENCE_URL}/health",timeout=5)
        health=json.loads(r.read())
        print(f"[shakespeare] inference server OK: {health.get('vram_gb',0)}GB VRAM, LoRA loaded",file=sys.stderr)
    except Exception as e:
        raise RuntimeError(f"Inference server not running at {INFERENCE_URL}. Start it: python3 scripts/shakespeare_inference_server.py --port 8766") from e
    Handler.engine=Engine()
    httpd=http.server.ThreadingHTTPServer(("127.0.0.1",args.port),Handler)
    print(f"[shakespeare] READY — http://127.0.0.1:{args.port}",file=sys.stderr)
    try:httpd.serve_forever()
    except KeyboardInterrupt:print("\n[shakespeare] shutdown",file=sys.stderr)
    finally:httpd.server_close();rpc.close()

if __name__=="__main__":main()
