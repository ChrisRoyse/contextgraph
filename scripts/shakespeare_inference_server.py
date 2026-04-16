#!/usr/bin/env python3
"""Custom inference server for the Shakespeare LoRA'd Gemma4 model.

Holds the 4-bit quantized model + LoRA adapter in VRAM and serves
a simple HTTP API compatible with the chat dashboard.

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

HF_TOKEN = os.environ.get("HF_TOKEN", "")
LORA_PATH = os.path.expanduser("~/.contextgraph/models/shakespeare_lora/final")
BASE_MODEL = "google/gemma-4-E4B-it"


class ShakespeareModel:
    """Holds the model in VRAM. Thread-safe via lock."""

    def __init__(self):
        self.model = None
        self.tokenizer = None
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
            bnb_4bit_compute_torch_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True)
        base = AutoModelForCausalLM.from_pretrained(
            BASE_MODEL, quantization_config=bnb, device_map="auto", torch_dtype=torch.bfloat16)

        print(f"[model] Loading LoRA: {LORA_PATH}", file=sys.stderr)
        self.model = PeftModel.from_pretrained(base, LORA_PATH)
        self.tokenizer = AutoTokenizer.from_pretrained(LORA_PATH)
        self.model.eval()
        self.load_time_ms = int((time.time() - t0) * 1000)

        vram = torch.cuda.memory_allocated() / 1e9
        print(f"[model] Ready — {vram:.1f} GB VRAM, loaded in {self.load_time_ms}ms", file=sys.stderr)

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

            with torch.no_grad():
                out = self.model.generate(
                    **inputs,
                    max_new_tokens=max_tokens,
                    temperature=temperature,
                    do_sample=True,
                    top_p=top_p,
                    top_k=top_k,
                    repetition_penalty=repetition_penalty,
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
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n[inference] shutdown", file=sys.stderr)
    finally:
        httpd.server_close()


if __name__ == "__main__":
    main()
