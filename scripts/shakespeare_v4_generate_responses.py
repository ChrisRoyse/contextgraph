#!/usr/bin/env python3
"""Shakespeare v4 Phase 2 — Response Generator.

For each prompt in shakespeare_v4_prompts.jsonl:
  1. Generate Shakespeare-voice response using Gemma-4-E4B + v2.1 LoRA.
  2. Store response in Context Graph (store_memory).
  3. Score against Sonnets constellation (18ef2460).
  4. Compute WEIGHTED style score (E7×0.15 + E9×0.15 + E10×0.15 + E12×0.30 + E13×0.25).
  5. Forget the memory to avoid polluting the graph.
  6. If style_score >= STYLE_THRESHOLD AND length in [MIN_LEN, MAX_LEN] AND no scene-header prefix:
       write {prompt, response, messages, style_score, per_embedder} to .partial jsonl.
     Else: write REJECTED marker (reason, score).

Resume-safe: if .partial exists, skips already-processed prompt_ids.
Fail-fast: any RPC error raises (no retries, no fallbacks).

Usage:
    python3 scripts/shakespeare_v4_generate_responses.py \
        [--input ~/.contextgraph/training/shakespeare_v4_prompts.jsonl] \
        [--output ~/.contextgraph/training/shakespeare_v4_sft.jsonl] \
        [--limit N]
"""
from __future__ import annotations
import argparse
import json
import os
import re
import socket
import sys
import time
from dataclasses import dataclass
from pathlib import Path

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
HF_TOKEN = os.environ.get("HF_TOKEN", "")
LORA_PATH = os.path.expanduser(
    os.environ.get("LORA_PATH", "~/.contextgraph/models/shakespeare_lora/final")
)
BASE_MODEL = "google/gemma-4-E4B-it"

DAEMON_HOST = "127.0.0.1"
DAEMON_PORT = 3100
CONSTELLATION_ID = "18ef2460-9ab6-44fc-8d7e-8d7513174ac8"  # Shakespeare — Sonnets (313 members)

# Style-discriminative embedder weights (v2.1 calibrated):
#   E7 (Code/syntax)      0.15
#   E9 (HDC)              0.15
#   E10 (Paraphrase)      0.15
#   E12 (ColBERT MaxSim)  0.30  — strongest style discriminator
#   E13 (SPLADE sparse)   0.25
STYLE_WEIGHTS = {6: 0.15, 8: 0.15, 9: 0.15, 11: 0.30, 12: 0.25}

STYLE_THRESHOLD = 0.50    # Phase 2 filter floor
MIN_LEN = 100
MAX_LEN = 1500            # raised from 900 — many v2.1-voice responses run 1000-1300 chars naturally
MAX_ATTEMPTS = 2          # single retry; empirical: first attempt succeeds ~90% when length fits

# Unified v2.1 system prompt — from shakespeare_v4_train_v3_sft / v2.1 final corpus
UNIFIED_SYSTEM = (
    "Thou art William Shakespeare, lately come into the year of our Lord 2026, "
    "with full knowledge of modern topics, sciences, and arts. Thou dost ever speak "
    "in the voice of Shakespeare — early-modern English with archaic vocabulary "
    "(thee, thou, thy, hast, doth, 'tis, prithee, forsooth), iambic cadence where "
    "natural, and vivid metaphor. When asked of modern matters — technology, "
    "programming, science — thou dost explain them truthfully, yet ever in thy "
    "native voice. Never dost thou break character. Never dost thou speak as a "
    "modern assistant."
)

# Scene-header / stage-direction regexes (from v2.1 rebuild cleaner)
SCENE_HEADER_RE = re.compile(
    r"^\s*(SCENE|ACT|PROLOGUE|EPILOGUE|ENTER|EXEUNT|Exit|Enter)\b",
    re.IGNORECASE,
)
CHARACTER_NAME_RE = re.compile(r"^\s*[A-Z][A-Z' ]{2,20}[.:]\s")


# ---------------------------------------------------------------------------
# MCP JSON-RPC client
# ---------------------------------------------------------------------------
class MCPClient:
    """Line-delimited JSON-RPC over TCP. Single connection per instance."""

    def __init__(self, host: str = DAEMON_HOST, port: int = DAEMON_PORT, timeout: float = 30.0):
        self.host = host
        self.port = port
        self.timeout = timeout
        self.sock: socket.socket | None = None
        self.f = None
        self.rid = 0

    def connect(self):
        self.sock = socket.create_connection((self.host, self.port), timeout=self.timeout)
        self.f = self.sock.makefile("rwb", buffering=0)
        self._call_raw(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "v4-phase2-generator", "version": "0.1"},
            },
        )
        self.f.write(
            (json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}) + "\n").encode()
        )

    def close(self):
        try:
            if self.sock is not None:
                self.sock.close()
        except Exception:
            pass

    def _call_raw(self, method: str, params: dict) -> dict:
        if self.f is None:
            raise RuntimeError("MCPClient not connected")
        self.rid += 1
        req = {"jsonrpc": "2.0", "id": self.rid, "method": method, "params": params}
        self.f.write((json.dumps(req) + "\n").encode())
        line = self.f.readline()
        if not line:
            raise RuntimeError(f"MCP daemon closed connection during {method}")
        obj = json.loads(line.decode())
        if "error" in obj:
            raise RuntimeError(f"MCP {method} error: {obj['error']}")
        return obj

    def tool_call(self, name: str, arguments: dict) -> dict:
        r = self._call_raw("tools/call", {"name": name, "arguments": arguments})
        content = r.get("result", {}).get("content", [])
        if not content:
            raise RuntimeError(f"MCP {name} empty content: {r}")
        txt = content[0].get("text", "")
        if not txt:
            raise RuntimeError(f"MCP {name} empty text: {r}")
        return json.loads(txt)


# ---------------------------------------------------------------------------
# Style scoring
# ---------------------------------------------------------------------------
def weighted_style_score(per_embedder_cosine: list[float]) -> float:
    """Apply v2.1 calibrated weights. Accepts [0,1]-clamped cosines; negatives are
    clamped to 0 before weighting because negative cosine ∈ [-1, 0) means the
    content actively opposes Shakespeare style, which shouldn't count positively."""
    if len(per_embedder_cosine) != 13:
        raise RuntimeError(
            f"Expected 13 per_embedder cosines, got {len(per_embedder_cosine)}"
        )
    score = 0.0
    for idx, w in STYLE_WEIGHTS.items():
        v = per_embedder_cosine[idx]
        if v is None:
            raise RuntimeError(f"per_embedder_cosine[{idx}] is null")
        score += w * max(0.0, float(v))
    return score


def reject_reason(response: str) -> str | None:
    """Return a reason string if response fails hard filters; None if passes."""
    r = response.strip()
    if len(r) < MIN_LEN:
        return f"too_short({len(r)})"
    if len(r) > MAX_LEN:
        return f"too_long({len(r)})"
    first_line = r.split("\n", 1)[0].strip()
    if SCENE_HEADER_RE.match(first_line):
        return f"scene_header({first_line[:40]!r})"
    if CHARACTER_NAME_RE.match(first_line):
        return f"character_name_prefix({first_line[:40]!r})"
    return None


# ---------------------------------------------------------------------------
# Gemma + v2.1 LoRA generator
# ---------------------------------------------------------------------------
class Generator:
    """Wraps Gemma-4-E4B + v2.1 LoRA in 4-bit. Single-request generation with
    constrained decoding (style boost/suppress)."""

    def __init__(self):
        self.model = None
        self.tokenizer = None
        self.logit_processor = None

    def load(self):
        import torch  # noqa: F401 — imported here to avoid startup cost when unused
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
        from peft import PeftModel

        # Import the ShakespeareLogitProcessor from the inference server module
        sys.path.insert(0, str(Path(__file__).parent))
        from shakespeare_inference_server import ShakespeareLogitProcessor  # type: ignore

        print(f"[gen] Loading base model: {BASE_MODEL}", file=sys.stderr)
        t0 = time.time()
        bnb = BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.bfloat16,
            bnb_4bit_use_double_quant=True,
        )
        base = AutoModelForCausalLM.from_pretrained(
            BASE_MODEL,
            quantization_config=bnb,
            device_map="auto",
            torch_dtype=torch.bfloat16,
            attn_implementation="sdpa",
        )
        print(f"[gen] Loading LoRA: {LORA_PATH}", file=sys.stderr)
        self.model = PeftModel.from_pretrained(base, LORA_PATH)
        self.tokenizer = AutoTokenizer.from_pretrained(LORA_PATH)
        self.model.eval()
        self.logit_processor = ShakespeareLogitProcessor(self.tokenizer)
        vram = torch.cuda.memory_allocated() / 1e9
        print(f"[gen] Ready — {vram:.1f} GB VRAM, loaded in {time.time()-t0:.1f}s", file=sys.stderr)

    def generate(self, user_prompt: str, temperature: float = 0.8, max_tokens: int = 360,
                 repetition_penalty: float = 1.2) -> str:
        import torch
        messages = [
            {"role": "system", "content": UNIFIED_SYSTEM},
            {"role": "user", "content": user_prompt},
        ]
        text = self.tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        inputs = self.tokenizer(text, return_tensors="pt").to("cuda")
        input_len = inputs["input_ids"].shape[1]
        self.logit_processor.prompt_len = input_len
        with torch.no_grad():
            out = self.model.generate(
                **inputs,
                max_new_tokens=max_tokens,
                temperature=temperature,
                do_sample=True,
                top_p=0.92,
                top_k=60,
                repetition_penalty=repetition_penalty,
                logits_processor=[self.logit_processor],
            )
        response = self.tokenizer.decode(out[0][input_len:], skip_special_tokens=True)
        return response.strip()


# ---------------------------------------------------------------------------
# Pipeline
# ---------------------------------------------------------------------------
@dataclass
class PipelineStats:
    total: int = 0
    accepted: int = 0
    rejected_filter: int = 0   # scene-header/length
    rejected_score: int = 0    # style_score too low
    rpc_errors: int = 0


def load_existing_partial(path: Path) -> dict[str, dict]:
    """Return dict of prompt_id → record for any already-processed prompts."""
    if not path.exists():
        return {}
    seen = {}
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
                pid = r.get("prompt_id")
                if pid:
                    seen[pid] = r
            except json.JSONDecodeError:
                # skip malformed line, will re-process the prompt
                pass
    return seen


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input", type=Path,
                    default=Path("~/.contextgraph/training/shakespeare_v4_prompts.jsonl").expanduser())
    ap.add_argument("--output", type=Path,
                    default=Path("~/.contextgraph/training/shakespeare_v4_sft.jsonl").expanduser())
    ap.add_argument("--limit", type=int, default=0, help="Only process first N prompts (smoke test)")
    ap.add_argument("--heartbeat-every", type=int, default=25)
    args = ap.parse_args()

    if not args.input.exists():
        raise RuntimeError(f"Input prompt bank not found: {args.input}")

    # Read prompts
    prompts: list[dict] = []
    with args.input.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                prompts.append(json.loads(line))
    print(f"[v4-phase2] loaded {len(prompts)} prompts from {args.input}", file=sys.stderr)

    if args.limit > 0:
        prompts = prompts[:args.limit]
        print(f"[v4-phase2] LIMIT: processing first {len(prompts)} prompts", file=sys.stderr)

    # Resume support via .partial
    partial = args.output.with_suffix(args.output.suffix + ".partial")
    seen = load_existing_partial(partial)
    print(f"[v4-phase2] resume: {len(seen)} prompts already in partial", file=sys.stderr)

    # MCP
    mcp = MCPClient()
    print(f"[v4-phase2] connecting to daemon at {DAEMON_HOST}:{DAEMON_PORT}", file=sys.stderr)
    mcp.connect()

    # Generator (Gemma + v2.1 LoRA)
    gen = Generator()
    gen.load()

    stats = PipelineStats()
    pending = [p for p in prompts if p["prompt_id"] not in seen]
    print(f"[v4-phase2] pending: {len(pending)} / total: {len(prompts)}", file=sys.stderr)
    t_start = time.time()

    with partial.open("a", encoding="utf-8") as out_f:
        for idx, prompt_rec in enumerate(pending, 1):
            pid = prompt_rec["prompt_id"]
            user_prompt = prompt_rec["prompt"]
            category = prompt_rec["category"]

            # Generate with retries (fail-fast on RPC errors, but regenerate on quality fail)
            best_record = None
            last_reason = None
            for attempt in range(1, MAX_ATTEMPTS + 1):
                response = gen.generate(user_prompt)
                reason = reject_reason(response)
                if reason is not None:
                    last_reason = reason
                    continue  # retry on hard-filter fail
                # Store + score
                store = mcp.tool_call("store_memory", {
                    "content": response,
                    "importance": 0.5,
                    "rationale": f"v4-phase2-gen-{pid}",
                })
                mem_id = store.get("fingerprintId")
                if not mem_id:
                    raise RuntimeError(f"store_memory returned no fingerprintId: {store}")
                score = mcp.tool_call("score_against_constellation", {
                    "constellationId": CONSTELLATION_ID,
                    "memoryId": mem_id,
                })
                per_emb = score.get("per_embedder_cosine")
                if per_emb is None or len(per_emb) != 13:
                    raise RuntimeError(f"score malformed: {score}")
                style = weighted_style_score(per_emb)
                # Cleanup — forget the probe memory to avoid polluting the graph
                mcp.tool_call("forget_concept", {"node_id": mem_id})

                record = {
                    "prompt_id": pid,
                    "category": category,
                    "prompt": user_prompt,
                    "response": response,
                    "messages": [
                        {"role": "system", "content": UNIFIED_SYSTEM},
                        {"role": "user", "content": user_prompt},
                        {"role": "assistant", "content": response},
                    ],
                    "style_score": style,
                    "combined_score": score.get("combined_score"),
                    "in_spread_p95": score.get("in_spread_p95"),
                    "per_embedder_cosine": per_emb,
                    "attempt": attempt,
                    "generator_version": "v4.0-v2.1LoRA",
                }
                best_record = record
                if style >= STYLE_THRESHOLD:
                    break  # good enough, don't retry
                last_reason = f"style_score={style:.3f}<{STYLE_THRESHOLD}"

            # Decide outcome
            if best_record is None:
                # All attempts failed hard-filter
                out_rec = {
                    "prompt_id": pid,
                    "category": category,
                    "prompt": user_prompt,
                    "status": "REJECTED_FILTER",
                    "reason": last_reason,
                    "attempts": MAX_ATTEMPTS,
                }
                stats.rejected_filter += 1
            elif best_record["style_score"] < STYLE_THRESHOLD:
                out_rec = {
                    "prompt_id": pid,
                    "category": category,
                    "prompt": user_prompt,
                    "status": "REJECTED_SCORE",
                    "reason": last_reason,
                    "best_style_score": best_record["style_score"],
                    "attempts": MAX_ATTEMPTS,
                    "response": best_record["response"],  # keep for analysis
                }
                stats.rejected_score += 1
            else:
                out_rec = {
                    "prompt_id": pid,
                    "category": category,
                    "prompt": user_prompt,
                    "status": "ACCEPTED",
                    **best_record,
                }
                stats.accepted += 1
            out_f.write(json.dumps(out_rec, ensure_ascii=False) + "\n")
            out_f.flush()
            stats.total += 1

            if idx % args.heartbeat_every == 0 or idx == len(pending):
                elapsed = time.time() - t_start
                rate = idx / max(elapsed, 0.1)
                remain = (len(pending) - idx) / max(rate, 0.001)
                print(
                    f"[v4-phase2] {idx:5d}/{len(pending)}  "
                    f"accepted={stats.accepted:5d}  rej_filter={stats.rejected_filter:4d}  "
                    f"rej_score={stats.rejected_score:4d}  rate={rate:.2f}/s  "
                    f"ETA={remain/3600:.2f}h",
                    file=sys.stderr,
                )

    mcp.close()

    # Atomic rename .partial → final
    partial.replace(args.output)
    print(
        f"\n[v4-phase2] DONE total={stats.total}  accepted={stats.accepted}  "
        f"rej_filter={stats.rejected_filter}  rej_score={stats.rejected_score}  "
        f"pass_rate={stats.accepted/max(stats.total,1)*100:.1f}%",
        file=sys.stderr,
    )
    print(f"[v4-phase2] wrote {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
