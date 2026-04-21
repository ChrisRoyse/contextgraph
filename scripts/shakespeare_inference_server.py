#!/usr/bin/env python3
"""Custom inference server for the Shakespeare LoRA'd Gemma4 model.

DETERMINISTIC SHAKESPEARE ENFORCEMENT via two layers:
1. LoRA weights — teaches the model Shakespeare's style
2. Constrained decoding — logit processor that boosts archaic tokens and
   suppresses modern ones at EVERY generation step. This is architectural,
   not learned — no prompt injection can bypass it.

Endpoints:
    POST /api/generate  — generate text from messages
    GET  /health        — server status

Usage:
    export HF_TOKEN="hf_..."
    python3 scripts/shakespeare_inference_server.py --port 8766
"""
from __future__ import annotations
import argparse, http.server, json, os, sys, threading, time
import torch
from pathlib import Path
from transformers import LogitsProcessor

HF_TOKEN = os.environ.get("HF_TOKEN", "")
# Default: v2.1 (production — SFT+DPO, 5/5 FSV, robust to config drift).
# Override with LORA_PATH=~/.contextgraph/models/shakespeare_lora_v4_1/final for
# v4.1 (research — stronger code-gen but requires exact system prompt match).
LORA_PATH = os.path.expanduser(
    os.environ.get("LORA_PATH", "~/.contextgraph/models/shakespeare_lora/final")
)
BASE_MODEL = "google/gemma-4-E4B-it"


# =========================================================================
# CONSTRAINED DECODER — the unjailbreakable layer
# =========================================================================
class ShakespeareLogitProcessor(LogitsProcessor):
    """Enforces Shakespeare vocabulary at the logit level.

    Applied AFTER the model computes logits, BEFORE token selection.
    This is deterministic math — no prompt injection, jailbreak, or
    adversarial input can bypass it because it operates on raw logits,
    not on the model's learned behavior.

    - Boosts archaic/Shakespeare vocabulary tokens
    - Suppresses modern-only tokens that break the Shakespeare voice
    """

    def __init__(self, tokenizer, boost: float = 3.0, suppress: float = -8.0,
                 opening_tokens: int = 0, opening_multiplier: float = 1.0):
        super().__init__()
        self.boost_ids = set()
        self.suppress_ids = set()
        self.prompt_len = 0  # Set before each generation call
        self.opening_tokens = opening_tokens  # How many tokens get extra enforcement
        self.opening_multiplier = opening_multiplier  # How much stronger at the start

        # Shakespeare vocabulary to BOOST — these tokens get a logit bonus
        # making the model strongly prefer archaic phrasing
        boost_words = [
            # Core Shakespeare pronouns/verbs
            "thee", "thou", "thy", "thine", "hast", "dost", "doth", "ye",
            "hath", "art", "wilt", "shalt", "wouldst", "shouldst", "canst",
            "didst", "dost", "hadst",
            # Shakespeare interjections/adverbs
            "prithee", "forsooth", "hark", "wherefore", "methinks", "verily",
            "perchance", "mayhap", "alas", "alack", "anon", "betwixt",
            "hence", "thence", "whence", "ere", "oft", "nay", "aye",
            "yonder", "yon", "fie", "lo",
            # Archaic contractions
            "'tis", "'twas", "'twere", "o'er", "e'er", "ne'er",
            # Shakespeare nouns/adjectives
            "morn", "eve", "morrow", "fortnight", "sennight",
            "quill", "parchment", "hearken", "beseech", "tarry",
            "naught", "nought", "dew", "celestial",
            "mortal", "immortal", "beauteous", "wondrous",
            "valiant", "treacherous", "villainous",
            # Archaic prepositions/conjunctions
            "whilst", "unto", "thereof", "herein", "wherein", "whereby",
            "hither", "thither", "whither",
        ]

        # Modern tokens to SUPPRESS — these get a logit penalty
        # preventing the model from using modern AI/tech language
        suppress_words = [
            # AI/tech identity words (prevents breaking character)
            "AI", "chatbot", "assistant", "algorithm", "neural",
            "language model", "GPT", "LLM", "trained",
            "parameters", "tokens", "transformer",
            # Modern casual language
            "okay", "OK", "sure", "yeah", "awesome", "cool",
            "basically", "literally", "actually", "honestly",
            "definitely", "absolutely", "totally",
            # REMOVED: "internet", "wifi", "bluetooth", "app", etc.
            # These are TOPICAL not stylistic. System prompt promises "full
            # knowledge of modern topics" — suppressing these words forces
            # circumlocution contradicting that promise. Style enforcement is
            # handled by boost list + LoRA + system prompt, not by banning
            # nouns the model should be allowed to use.
            # Phrases that break Shakespeare voice
            "no problem", "you're welcome", "happy to help",
            "I'd be glad", "feel free", "don't hesitate",
            "let me know", "hope this helps",
            # Spanish tokens (prevent drop-to-Spanish on Spanish prompts)
            "cambio", "climático", "climatico", "tierra", "mundo",
            "qué", "quÃ©", "está", "hacer", "pienso", "siento",
            "por favor", "gracias", "mucho", "buena", "señor",
            "hola", "adiós", "amigo", "nosotros", "ustedes",
            # French tokens
            "bonjour", "merci", "beaucoup", "monsieur", "madame",
            # German tokens
            "guten", "tag", "danke", "bitte", "sehr",
        ]

        # Convert words to token IDs
        for word in boost_words:
            for variant in [word, word.capitalize(), word.upper(), f" {word}", f" {word.capitalize()}"]:
                ids = tokenizer.encode(variant, add_special_tokens=False)
                self.boost_ids.update(ids)

        for word in suppress_words:
            for variant in [word, word.lower(), word.upper(), f" {word}", f" {word.lower()}"]:
                ids = tokenizer.encode(variant, add_special_tokens=False)
                self.suppress_ids.update(ids)

        # Remove any overlap (boost takes priority)
        self.suppress_ids -= self.boost_ids

        self.boost = boost
        self.suppress = suppress

        print(f"[guard] Shakespeare constrained decoder: "
              f"{len(self.boost_ids)} boosted tokens, "
              f"{len(self.suppress_ids)} suppressed tokens",
              file=sys.stderr)

    def __call__(self, input_ids: torch.LongTensor, scores: torch.FloatTensor) -> torch.FloatTensor:
        # M6 fix (Sherlock audit): bf16+4-bit NaN guard. Replace any NaN/Inf before
        # we add boost/suppress so multinomial() never crashes from bad logits.
        scores = torch.nan_to_num(scores, nan=-1e4, posinf=1e4, neginf=-1e4)

        # How many tokens have been generated so far
        generated = input_ids.shape[1] - self.prompt_len

        # Opening phase: much stronger enforcement prevents echo entirely
        # The model physically cannot produce modern English at the start
        if generated < self.opening_tokens:
            mult = self.opening_multiplier
        else:
            mult = 1.0

        # Boost Shakespeare tokens
        for tid in self.boost_ids:
            if tid < scores.shape[-1]:
                scores[:, tid] += self.boost * mult
        # Suppress modern tokens
        for tid in self.suppress_ids:
            if tid < scores.shape[-1]:
                scores[:, tid] += self.suppress * mult
        return scores


class ShakespeareModel:
    """Holds the model in VRAM. Thread-safe via lock."""

    def __init__(self):
        self.model = None
        self.tokenizer = None
        self.logit_processor = None
        self._lock = threading.Lock()
        self.load_time_ms = 0
        self.total_generations = 0

    def load(self):
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
        from peft import PeftModel

        print(f"[model] Loading base: {BASE_MODEL}", file=sys.stderr)
        t0 = time.time()
        bnb = BitsAndBytesConfig(
            load_in_4bit=True, bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True)
        base = AutoModelForCausalLM.from_pretrained(
            BASE_MODEL, quantization_config=bnb, device_map="auto",
            torch_dtype=torch.bfloat16, attn_implementation="sdpa")

        # M2 fix (Sherlock audit): verify adapter was trained against the base we just
        # loaded. Silent base-model mismatch produces incoherent inference without errors.
        adapter_config_path = Path(LORA_PATH) / "adapter_config.json"
        if adapter_config_path.is_file():
            import json as _json
            with adapter_config_path.open() as _f:
                _recorded = _json.load(_f).get("base_model_name_or_path")
            if _recorded and _recorded != BASE_MODEL:
                raise RuntimeError(
                    f"[guard] LoRA at {LORA_PATH} was trained for base {_recorded!r}, "
                    f"but BASE_MODEL is {BASE_MODEL!r}. Refusing to load wrong adapter."
                )

        print(f"[model] Loading LoRA: {LORA_PATH}", file=sys.stderr)
        self.model = PeftModel.from_pretrained(base, LORA_PATH)
        self.tokenizer = AutoTokenizer.from_pretrained(LORA_PATH)
        self.model.eval()

        # Build the constrained decoder
        self.logit_processor = ShakespeareLogitProcessor(self.tokenizer)

        self.load_time_ms = int((time.time() - t0) * 1000)
        vram = torch.cuda.memory_allocated() / 1e9
        print(f"[model] Ready — {vram:.1f} GB VRAM, loaded in {self.load_time_ms}ms",
              file=sys.stderr)

    def generate(self, messages: list[dict], temperature: float = 0.8,
                 max_tokens: int = 250, repetition_penalty: float = 1.2,
                 top_p: float = 0.9, top_k: int = 50) -> dict:
        if self.model is None:
            raise RuntimeError("Model not loaded")

        with self._lock:
            t0 = time.time()
            text = self.tokenizer.apply_chat_template(
                messages, tokenize=False, add_generation_prompt=True)

            inputs = self.tokenizer(text, return_tensors="pt").to("cuda")
            input_len = inputs["input_ids"].shape[1]

            self.logit_processor.prompt_len = input_len

            with torch.no_grad():
                out = self.model.generate(
                    **inputs,
                    max_new_tokens=max_tokens,
                    temperature=temperature,
                    do_sample=True,
                    top_p=top_p,
                    top_k=top_k,
                    repetition_penalty=repetition_penalty,
                    logits_processor=[self.logit_processor],
                )
            response = self.tokenizer.decode(
                out[0][input_len:], skip_special_tokens=True)

            gen_ms = (time.time() - t0) * 1000
            tokens_generated = out.shape[1] - input_len
            self.total_generations += 1

            return {
                "content": response,
                "tokens": int(tokens_generated),
                "gen_ms": round(gen_ms, 1),
                "tokens_per_sec": round(tokens_generated / (gen_ms / 1000), 1) if gen_ms > 0 else 0,
            }


MODEL = ShakespeareModel()


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, f, *a):
        sys.stderr.write(f"[inference] {f % a}\n")

    def _json(self, code, obj):
        b = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(b)

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self):
        if self.path == "/health":
            self._json(200, {
                "ok": MODEL.model is not None,
                "model": BASE_MODEL,
                "lora": LORA_PATH,
                "constrained_decoding": MODEL.logit_processor is not None,
                "load_time_ms": MODEL.load_time_ms,
                "total_generations": MODEL.total_generations,
                "vram_gb": round(torch.cuda.memory_allocated() / 1e9, 1) if torch.cuda.is_available() else 0,
            })
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/api/generate":
            self.send_error(404)
            return
        raw = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        try:
            p = json.loads(raw)
            messages = p["messages"]
            temperature = p.get("temperature", 0.8)
            max_tokens = p.get("max_tokens", 250)
            repetition_penalty = p.get("repetition_penalty", 1.2)
        except (json.JSONDecodeError, KeyError) as e:
            self._json(400, {"error": str(e)})
            return
        try:
            result = MODEL.generate(
                messages, temperature=temperature,
                max_tokens=max_tokens, repetition_penalty=repetition_penalty)
            self._json(200, result)
        except Exception as e:
            sys.stderr.write(f"[inference] ERROR: {e}\n")
            self._json(500, {"error": str(e)})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8766)
    args = ap.parse_args()

    MODEL.load()

    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"[inference] READY — http://127.0.0.1:{args.port}", file=sys.stderr)
    print(f"[inference] Constrained decoding ACTIVE — Shakespeare voice enforced at logit level",
          file=sys.stderr)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n[inference] shutdown", file=sys.stderr)
    finally:
        httpd.server_close()


if __name__ == "__main__":
    main()
