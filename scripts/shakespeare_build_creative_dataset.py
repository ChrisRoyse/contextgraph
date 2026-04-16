#!/usr/bin/env python3
"""Build a creative Shakespeare-style SFT+DPO dataset.

Instead of pairing modern prompts with raw Shakespeare text (which causes
the model to memorize scenes), this script:

1. Generates diverse modern-topic prompts
2. For each prompt, asks Gemma4 (base, no LoRA) to write in Shakespeare style
3. Generates N candidates per prompt at varied temperatures
4. Scores each candidate against Shakespeare constellations via Context Graph
5. Outputs:
   - SFT data: prompt + highest-scoring candidate
   - DPO data: prompt + highest (chosen) vs lowest (rejected)

This teaches the model HOW TO WRITE like Shakespeare about ANYTHING,
not to memorize his plays.

Usage:
    python3 scripts/shakespeare_build_creative_dataset.py \
        --sft-out ~/.contextgraph/training/shakespeare_sft_creative.jsonl \
        --dpo-out ~/.contextgraph/training/shakespeare_dpo_creative.jsonl
"""
from __future__ import annotations
import argparse, json, os, random, sys, time, urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _gemma_shakespeare_lib import (
    Rpc, authenticity_verdict, list_shakespeare_constellations,
    score_text, soft_delete,
)

OLLAMA_URL = "http://127.0.0.1:11434"
MODEL = "gemma4:e4b"

SHAKESPEARE_SYSTEM = (
    "You are William Shakespeare. Write ONLY in early modern English — use thee, "
    "thou, thy, thine, hast, dost, doth, ye, prithee, forsooth, hark, wherefore, "
    "methinks, 'tis, 'twas, and similar period vocabulary. Use iambic rhythm where "
    "natural. Draw imagery from nature, stars, seasons, love, death, honour, and "
    "kingship. Write verse OR prose in Shakespeare's voice — NOT stage directions, "
    "NOT character names, NOT scene headers. Just beautiful Shakespearean writing "
    "about the topic the user asks about. Keep responses under 100 words."
)

# 100 diverse modern-topic prompts across many domains
PROMPTS = [
    # Technology
    "Write about the wonder of a device that holds all human knowledge in your palm",
    "Describe the feeling of speaking to someone across the ocean instantly",
    "Tell me about machines that think and reason like men",
    "Write about the strange light that glows from screens at midnight",
    "Describe a world where carriages drive themselves without horses",
    "Write about invisible messages that fly through the air like birds",
    "Tell me about a library that never closes and has every book ever written",
    "Describe the danger of trusting machines with our deepest secrets",
    "Write about artificial minds that create art and poetry",
    "Tell me about the loneliness of staring at a glowing rectangle all day",
    # Science
    "Describe the vastness of the universe and our smallness within it",
    "Write about the discovery that we are made of stardust",
    "Tell me about the invisible forces that hold the world together",
    "Describe what it means that time itself can bend and slow",
    "Write about the tiny creatures that live in a single drop of water",
    "Tell me about the light that takes a thousand years to reach our eyes",
    "Describe the moment a cure for a terrible plague is found",
    "Write about the ocean depths where no sunlight ever reaches",
    "Tell me about the strange dance of particles too small to see",
    "Describe the wonder of a seed becoming a mighty oak",
    # Emotions
    "Write about the pain of losing someone you love deeply",
    "Describe the joy of reunion after years apart",
    "Tell me about jealousy that destroys trust between friends",
    "Write about the courage it takes to forgive a great betrayal",
    "Describe the bittersweet feeling of watching your child grow up",
    "Tell me about the fear of growing old and being forgotten",
    "Write about the peace that comes after a long struggle ends",
    "Describe the excitement of falling in love for the first time",
    "Tell me about the weight of guilt that cannot be confessed",
    "Write about hope that persists in the darkest hour",
    # Business & Power
    "Describe how power corrupts even the most virtuous leaders",
    "Write about the rise and fall of a great merchant empire",
    "Tell me about the burden of making decisions that affect thousands",
    "Describe the treachery of advisors who smile while plotting",
    "Write about the loneliness at the top of a great organization",
    "Tell me about honest work that goes unrecognized",
    "Describe the temptation to take a shortcut that betrays your principles",
    "Write about building something great from nothing",
    "Tell me about the rivalry between two equally matched opponents",
    "Describe the moment you realize your mentor has been wrong all along",
    # Nature & Seasons
    "Describe a perfect spring morning after a long winter",
    "Write about a storm at sea that tests the courage of sailors",
    "Tell me about the beauty of autumn leaves falling like golden rain",
    "Describe a moonlit garden where lovers meet in secret",
    "Write about the first snowfall of winter covering the world in silence",
    "Tell me about a river that has flowed since before human memory",
    "Describe the dawn breaking over mountains after a sleepless night",
    "Write about the wild beauty of an untamed forest",
    "Tell me about the stars on a clear night far from any city",
    "Describe the sea at sunset when the water turns to liquid gold",
    # Food & Daily Life
    "Write about the simple pleasure of bread fresh from the oven",
    "Describe a great feast where old enemies sit together in peace",
    "Tell me about the comfort of a warm fire on a cold night",
    "Write about the art of making wine from grapes",
    "Describe the busy market square at dawn when traders arrive",
    # Sports & Competition
    "Write about the thrill of victory after a hard-fought contest",
    "Describe the agony of defeat when you gave everything you had",
    "Tell me about the bond between teammates who face adversity together",
    "Write about a race where the slowest runner finishes with the most honour",
    "Describe the discipline required to master any craft",
    # Philosophy
    "Write about whether fate or free will governs our lives",
    "Describe what it means to live a good life",
    "Tell me about the nature of truth and how easily it is twisted",
    "Write about the relationship between justice and mercy",
    "Describe what remains of us after we die",
    "Tell me about the difference between wisdom and knowledge",
    "Write about whether it is better to be feared or loved",
    "Describe the paradox of seeking happiness making us less happy",
    # Modern Life (anachronistic but creative)
    "Write about the madness of people staring at their phones during dinner",
    "Describe the strange ritual of checking messages first thing each morning",
    "Tell me about the peculiar sadness of social media",
    "Write about the absurdity of being famous for being famous",
    "Describe what Shakespeare would think of modern dating apps",
    "Tell me about the irony of having infinite entertainment but infinite boredom",
    "Write about the noise of modern life drowning out the voice of the soul",
    "Describe a traffic jam as if it were a great battle",
    "Tell me about the tyranny of the alarm clock",
    "Write about the strange democracy of the internet where all voices are equal",
    # Love
    "Write a love letter to someone who doesn't know you exist",
    "Describe the moment you realize love has faded",
    "Tell me about forbidden love between families that hate each other",
    "Write about the difference between true love and infatuation",
    "Describe the beauty of growing old together",
    # Specific creative challenges
    "Write a sonnet about coffee",
    "Describe cryptocurrency as if it were sorcery",
    "Write about a programmer debugging code at 3am",
    "Describe a pizza delivery as a heroic quest",
    "Tell me about Netflix as if it were a magical theatre",
    "Write about WiFi as an invisible enchantment",
    "Describe an email inbox as a battlefield of demands",
    "Write about Google as an all-knowing oracle",
    "Describe a smartphone as a magical mirror",
    "Tell me about Uber as a summoning spell for iron carriages",
]

CANDIDATES_PER_PROMPT = 4  # Generate N candidates, pick best/worst
TEMPERATURES = [0.7, 0.8, 0.9, 1.0]  # Varied temperatures for diversity


def gemma_generate(prompt: str, system: str, temperature: float, max_tokens: int = 150) -> str:
    body = {"model": MODEL, "stream": False, "think": False,
            "messages": [{"role": "system", "content": system},
                         {"role": "user", "content": prompt}],
            "options": {"temperature": temperature, "num_predict": max_tokens,
                        "repetition_penalty": 1.1}}
    req = urllib.request.Request(f"{OLLAMA_URL}/api/chat",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=180) as resp:
        return (json.loads(resp.read()).get("message") or {}).get("content", "").strip()


def score_candidate(rpc: Rpc, text: str, cons_cache: list[dict]) -> tuple[float, str, str]:
    """Returns (score, verdict, top_work)."""
    if not text or len(text.strip()) < 20:
        return (0.0, "EMPTY", "N/A")
    try:
        fp, scores = score_text(rpc, text, cons_cache=cons_cache)
        short, long_, met = authenticity_verdict(scores)
        soft_delete(rpc, fp)
        return (scores[0].combined_score, short, scores[0].work)
    except Exception as e:
        print(f"  WARN: scoring failed: {e}", file=sys.stderr)
        return (0.0, "ERROR", "N/A")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sft-out", default=os.path.expanduser("~/.contextgraph/training/shakespeare_sft_creative.jsonl"))
    ap.add_argument("--dpo-out", default=os.path.expanduser("~/.contextgraph/training/shakespeare_dpo_creative.jsonl"))
    ap.add_argument("--max-prompts", type=int, default=len(PROMPTS))
    ap.add_argument("--skip-scoring", action="store_true", help="Skip constellation scoring (faster, no guard)")
    args = ap.parse_args()

    Path(args.sft_out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.dpo_out).parent.mkdir(parents=True, exist_ok=True)

    # Connect to services
    rpc = None
    cons_cache = None
    if not args.skip_scoring:
        print("[dataset] connecting to daemon :3100 ...", file=sys.stderr)
        rpc = Rpc()
        cons_cache = list_shakespeare_constellations(rpc)
        print(f"[dataset] {len(cons_cache)} constellations loaded", file=sys.stderr)

    prompts = PROMPTS[:args.max_prompts]
    random.shuffle(prompts)
    print(f"[dataset] {len(prompts)} prompts, {CANDIDATES_PER_PROMPT} candidates each", file=sys.stderr)

    t0 = time.time()
    sft_count = 0
    dpo_count = 0

    with open(args.sft_out, "w") as sft_f, open(args.dpo_out, "w") as dpo_f:
        for i, prompt in enumerate(prompts):
            if i > 0 and i % 10 == 0:
                elapsed = time.time() - t0
                print(f"[dataset] {i}/{len(prompts)}: {sft_count} SFT, {dpo_count} DPO, "
                      f"{elapsed:.0f}s elapsed", file=sys.stderr)

            # Generate N candidates at different temperatures
            candidates = []
            for j in range(CANDIDATES_PER_PROMPT):
                temp = TEMPERATURES[j % len(TEMPERATURES)]
                try:
                    text = gemma_generate(prompt, SHAKESPEARE_SYSTEM, temp)
                    if text and len(text.strip()) >= 30:
                        # Score against constellations
                        if rpc and cons_cache:
                            score, verdict, work = score_candidate(rpc, text, cons_cache)
                        else:
                            score, verdict, work = (0.5, "UNSCORED", "N/A")
                        candidates.append({
                            "text": text, "score": score, "verdict": verdict,
                            "work": work, "temperature": temp,
                        })
                except Exception as e:
                    print(f"  WARN: gen failed for prompt {i} candidate {j}: {e}", file=sys.stderr)

            if not candidates:
                continue

            # Sort by constellation score descending
            candidates.sort(key=lambda c: -c["score"])
            best = candidates[0]
            worst = candidates[-1] if len(candidates) > 1 else None

            # SFT example: prompt + best candidate
            sft_example = {
                "messages": [
                    {"role": "system", "content": SHAKESPEARE_SYSTEM},
                    {"role": "user", "content": prompt},
                    {"role": "assistant", "content": best["text"]},
                ],
                "_meta": {
                    "score": best["score"], "verdict": best["verdict"],
                    "work": best["work"], "temperature": best["temperature"],
                    "prompt_source": "curated_creative",
                },
            }
            sft_f.write(json.dumps(sft_example, ensure_ascii=False) + "\n")
            sft_count += 1

            # DPO example: best (chosen) vs worst (rejected)
            if worst and worst["text"] != best["text"] and abs(best["score"] - worst["score"]) > 0.01:
                dpo_example = {
                    "prompt": [
                        {"role": "system", "content": SHAKESPEARE_SYSTEM},
                        {"role": "user", "content": prompt},
                    ],
                    "chosen": [{"role": "assistant", "content": best["text"]}],
                    "rejected": [{"role": "assistant", "content": worst["text"]}],
                    "_meta": {
                        "chosen_score": best["score"], "rejected_score": worst["score"],
                        "score_gap": best["score"] - worst["score"],
                        "chosen_verdict": best["verdict"], "rejected_verdict": worst["verdict"],
                    },
                }
                dpo_f.write(json.dumps(dpo_example, ensure_ascii=False) + "\n")
                dpo_count += 1

    elapsed = time.time() - t0
    print(f"\n[dataset] DONE in {elapsed:.0f}s ({elapsed/60:.1f} min)", file=sys.stderr)
    print(f"  SFT examples: {sft_count}", file=sys.stderr)
    print(f"  DPO pairs:    {dpo_count}", file=sys.stderr)
    print(f"  SFT output:   {args.sft_out} ({Path(args.sft_out).stat().st_size/1e3:.0f} KB)", file=sys.stderr)
    print(f"  DPO output:   {args.dpo_out} ({Path(args.dpo_out).stat().st_size/1e3:.0f} KB)", file=sys.stderr)

    if rpc:
        rpc.close()


if __name__ == "__main__":
    main()
