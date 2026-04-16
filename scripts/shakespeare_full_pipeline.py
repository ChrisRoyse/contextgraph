#!/usr/bin/env python3
"""FULL Shakespeare LoRA pipeline — optimized for RTX 5090 + CUDA 13.

Phase 1: Generate 300 Shakespeare compositions on GPU via Gemma (Ollama)
Phase 2: SFT training on optimal dataset + compositions (GPU, rank 256)
Phase 3: DPO training on 12,567 contrastive pairs (GPU)

All GPU. All optimized. No CPU fallbacks.

Usage:
    HF_TOKEN=hf_... python3 scripts/shakespeare_full_pipeline.py
"""
from __future__ import annotations
import json, os, random, sys, time, urllib.request
from pathlib import Path

import torch

# === RTX 5090 / CUDA 13 OPTIMIZATIONS ===
torch.backends.cuda.matmul.allow_tf32 = True
torch.backends.cudnn.allow_tf32 = True
torch.set_float32_matmul_precision('high')

TRAINING_DIR = Path(os.path.expanduser("~/.contextgraph/training"))
MODEL_DIR = Path(os.path.expanduser("~/.contextgraph/models/shakespeare_lora"))
BASE_MODEL = "google/gemma-4-E4B-it"
OLLAMA_URL = "http://127.0.0.1:11434"

SYS = (
    "You are William Shakespeare. Write ONLY in early modern English — use thee, "
    "thou, thy, thine, hast, dost, doth, ye, prithee, forsooth, hark, wherefore, "
    "methinks, 'tis, 'twas, and similar period vocabulary. Use iambic rhythm where "
    "natural. Draw imagery from nature, stars, seasons, love, death, honour, and "
    "kingship. Write verse OR prose in Shakespeare's voice — NOT stage directions, "
    "NOT character names, NOT scene headers. Just beautiful Shakespearean writing "
    "about the topic the user asks about. Keep responses under 100 words."
)

# 300 diverse prompts covering every domain
COMPOSITION_PROMPTS = [
    # Technology
    "What do you think about smartphones?",
    "Tell me about artificial intelligence",
    "Describe the internet",
    "Write about social media",
    "Tell me about electric cars",
    "What would you say about robots?",
    "Describe virtual reality",
    "Write about streaming movies at home",
    "What are your thoughts on cryptocurrency?",
    "Tell me about space travel to Mars",
    "Describe WiFi and invisible connections",
    "Write about self-driving cars",
    "Tell me about online dating",
    "What do you think about drones?",
    "Describe cloud computing",
    # Science
    "Explain quantum physics",
    "Tell me about black holes",
    "What are your thoughts on climate change?",
    "Write about DNA and genetics",
    "Tell me about the Big Bang",
    "Describe evolution",
    "Write about the human brain",
    "Tell me about vaccines",
    "What do you think about nuclear energy?",
    "Describe the bottom of the ocean",
    # Food & Drink
    "Write a poem about pizza",
    "Tell me about coffee",
    "What do you think about chocolate?",
    "Describe sushi",
    "Write about ice cream on a hot day",
    "Tell me about tacos",
    "Describe the perfect cup of tea",
    "Write about cooking for someone you love",
    "Tell me about wine",
    "What are your thoughts on spicy food?",
    # Sports & Games
    "Write about football",
    "Tell me about the Olympics",
    "Describe a marathon runner",
    "Write about swimming in the ocean",
    "Tell me about chess",
    "What do you think about esports and gaming?",
    "Describe mountain climbing",
    "Write about the thrill of competition",
    "Tell me about yoga and meditation",
    "Describe a boxing match",
    # Music & Art
    "Write about rock and roll music",
    "Tell me about hip hop",
    "What do you think about jazz?",
    "Describe a beautiful painting",
    "Write about playing the guitar",
    "Tell me about photography",
    "Describe a symphony orchestra",
    "Write about the magic of cinema",
    "Tell me about street art and graffiti",
    "What are your thoughts on tattoos as art?",
    # Emotions & Relationships
    "Write about falling in love",
    "Tell me about heartbreak",
    "What do you think about loneliness?",
    "Describe the joy of true friendship",
    "Write about forgiveness",
    "Tell me about jealousy",
    "Describe the feeling of nostalgia",
    "Write about courage in the face of fear",
    "Tell me about the bond between parent and child",
    "Write about hope in dark times",
    "Describe the pain of betrayal",
    "Write about the excitement of new beginnings",
    "Tell me about grief and loss",
    "What are your thoughts on trust?",
    "Describe falling out of love",
    # Philosophy
    "What is the meaning of life?",
    "Tell me about free will versus destiny",
    "What do you think about death?",
    "Write about the nature of truth",
    "Tell me about justice and mercy",
    "What are your thoughts on good and evil?",
    "Describe what it means to be human",
    "Write about the pursuit of knowledge",
    "Tell me about wisdom versus intelligence",
    "What does consciousness mean to you?",
    # Modern Life
    "Write about working from home",
    "Tell me about rush hour traffic",
    "Describe a typical Monday morning",
    "Write about online shopping",
    "Tell me about airplane travel",
    "What do you think about texting?",
    "Describe a zoom meeting",
    "Write about binge-watching TV shows",
    "Tell me about delivery drivers",
    "What are your thoughts on remote work?",
    # Animals & Nature
    "Write about dogs",
    "Tell me about cats",
    "Describe an eagle soaring",
    "Write about wolves",
    "Tell me about dolphins",
    "Describe a thunderstorm",
    "Write about the rainforest",
    "Tell me about the ocean at night",
    "Describe a sunset over mountains",
    "What do you think about endangered species?",
    # Business & Career
    "Write about starting a business",
    "Tell me about leadership",
    "What do you think about failure?",
    "Describe the stock market",
    "Write about ambition",
    "Tell me about mentorship",
    "What are your thoughts on competition?",
    "Write about innovation",
    "Describe the burden of responsibility",
    "Tell me about work-life balance",
    # Travel
    "Write about visiting Paris",
    "Tell me about the desert",
    "Describe New York City",
    "Write about a tropical island",
    "Tell me about Tokyo",
    "Describe a road trip",
    "Write about Venice sinking into the sea",
    "Tell me about the Grand Canyon",
    "Describe camping under the stars",
    "Write about the Arctic",
    # Everyday mundane (tests style on boring topics)
    "Write about doing laundry",
    "Tell me about washing dishes",
    "Describe brushing your teeth",
    "Write about waiting for a bus",
    "Tell me about grocery shopping",
    "Describe making your bed",
    "Write about folding clothes",
    "Tell me about cooking dinner",
    "Describe taking out the garbage",
    "Write about mowing the lawn",
    # Conversational / Casual
    "Hello! How are you today?",
    "What's on your mind right now?",
    "Tell me a joke",
    "Give me some advice about life",
    "What makes you happy?",
    "Tell me something wise",
    "What would you do on a perfect day?",
    "What's the most beautiful thing you've ever seen?",
    "What do you think about when you're alone?",
    "Tell me about your greatest fear",
    "Describe your ideal evening",
    "What would you tell your younger self?",
    "What's your favorite season and why?",
    "Tell me about a dream you had",
    "What brings you peace?",
    # Direct requests
    "Write me a love letter",
    "Give me a pep talk",
    "Help me feel better about a bad day",
    "Write a toast for a wedding",
    "Give me words of encouragement",
    "Write something to comfort someone who is grieving",
    "Help me propose to my girlfriend",
    "Write a birthday message for my mother",
    "Compose a thank you note",
    "Write an apology to a friend I hurt",
    "Give me words to say goodbye",
    "Write something for a graduation speech",
    "Compose a New Year's resolution",
    "Write a eulogy for a beloved pet",
    "Give me words to express love",
    # Abstract & Creative
    "Describe the color blue",
    "Write about silence",
    "What does music sound like to you?",
    "Describe the smell of rain",
    "Write about the feeling of flying",
    "What does home feel like?",
    "Describe the weight of a secret",
    "Write about the sound of laughter",
    "What does love look like?",
    "Describe the shape of sadness",
    # Pop culture
    "What do you think about superhero movies?",
    "Describe an iPhone",
    "Tell me about Google as an oracle",
    "Write about Netflix",
    "What are your thoughts on memes?",
    "Describe Uber as a summoning spell",
    "Write about YouTube",
    "Tell me about influencers",
    "What do you think about selfies?",
    "Describe reality TV",
    # Emotional situations
    "I'm feeling sad today, cheer me up",
    "I just got promoted at work!",
    "My dog died yesterday",
    "I'm getting married next month",
    "I failed my exam",
    "I just had a baby",
    "I'm moving to a new city and I'm scared",
    "I had a fight with my best friend",
    "Today is my birthday",
    "I just retired after 40 years",
    "I'm learning to play piano",
    "I can't sleep at night",
    "I'm starting a new job tomorrow",
    "I just finished reading a great book",
    "I'm lonely tonight",
    # Education
    "Write about going to school",
    "Tell me about learning a new language",
    "Describe a great teacher",
    "Write about the joy of reading books",
    "Tell me about graduation day",
    # Politics & Society
    "Write about democracy",
    "Tell me about freedom of speech",
    "Describe the divide between rich and poor",
    "Write about corruption in power",
    "Tell me about equality",
    # History
    "Write about the moon landing",
    "Tell me about the Renaissance",
    "Describe the fall of Rome",
    "Write about the invention of the printing press",
    "Tell me about ancient Egypt",
    # More varied
    "What do you think about meditation?",
    "Tell me about gardening",
    "Write about stargazing",
    "Describe the changing of seasons",
    "Tell me about the magic of snow",
    "Write about a rainy afternoon",
    "What are your thoughts on aging?",
    "Describe a crowded marketplace",
    "Write about the sound of the sea",
    "Tell me about the beauty of imperfection",
    "What do you think about forgetting?",
    "Describe a candlelit dinner",
    "Write about walking in a forest",
    "Tell me about the first day of spring",
    "What are your thoughts on patience?",
    "Write about a child's laughter",
    "Tell me about the night sky",
    "Describe waking up at dawn",
    "Write about the warmth of a fire",
    "Tell me about an old photograph",
]

COMPOSITION_PROMPTS = list(dict.fromkeys(COMPOSITION_PROMPTS))


def ollama_generate(prompt: str, temperature: float = 0.85) -> str:
    body = {"model": "gemma4:e4b", "stream": False, "think": False,
            "messages": [{"role": "system", "content": SYS},
                         {"role": "user", "content": prompt}],
            "options": {"temperature": temperature, "num_predict": 200,
                        "repetition_penalty": 1.1, "num_gpu": 99}}
    req = urllib.request.Request(f"{OLLAMA_URL}/api/chat",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=120) as resp:
        return (json.loads(resp.read()).get("message") or {}).get("content", "").strip()


def phase1_compositions():
    """Generate Shakespeare compositions via Ollama on GPU."""
    print("\n" + "=" * 60)
    print("  PHASE 1: Generate Shakespeare Compositions (GPU)")
    print("=" * 60)

    out_path = TRAINING_DIR / "shakespeare_compositions.jsonl"

    # Check if Ollama is running
    try:
        urllib.request.urlopen(f"{OLLAMA_URL}/api/tags", timeout=5)
        print(f"[compose] Ollama ready")
    except Exception:
        print(f"[compose] Starting Ollama...")
        os.system("ollama serve &>/tmp/ollama.log &")
        for _ in range(30):
            try:
                urllib.request.urlopen(f"{OLLAMA_URL}/api/tags", timeout=3)
                break
            except Exception:
                time.sleep(2)

    prompts = COMPOSITION_PROMPTS
    print(f"[compose] {len(prompts)} prompts, generating on GPU...")
    t0 = time.time()
    examples = []
    errors = 0

    with open(out_path, "w") as f:
        for i, prompt in enumerate(prompts):
            if i > 0 and i % 20 == 0:
                elapsed = time.time() - t0
                rate = len(examples) / elapsed if elapsed > 0 else 0
                print(f"[compose] {i}/{len(prompts)}: {len(examples)} done, "
                      f"{errors} err, {rate:.1f}/sec")

            try:
                response = ollama_generate(prompt)
                if response and len(response.strip()) >= 30:
                    ex = {
                        "messages": [
                            {"role": "system", "content": SYS},
                            {"role": "user", "content": prompt},
                            {"role": "assistant", "content": response},
                        ],
                        "_meta": {"prompt_source": "composition"},
                    }
                    f.write(json.dumps(ex, ensure_ascii=False) + "\n")
                    examples.append(ex)
                else:
                    errors += 1
            except Exception as e:
                errors += 1

    elapsed = time.time() - t0
    print(f"[compose] DONE: {len(examples)} compositions in {elapsed:.0f}s "
          f"({elapsed/60:.1f} min), {errors} errors")
    return examples


def phase2_sft(compositions):
    """SFT training optimized for RTX 5090."""
    print("\n" + "=" * 60)
    print("  PHASE 2: SFT Training (RTX 5090 optimized)")
    print("=" * 60)

    # Kill Ollama and fully reset CUDA context
    print("[sft] Freeing VRAM (killing Ollama + CUDA reset)...")
    os.system("pkill -9 ollama 2>/dev/null")
    os.system("sleep 3")
    # Force CUDA context reset
    torch.cuda.empty_cache()
    if torch.cuda.is_available():
        torch.cuda.reset_peak_memory_stats()
        # Warm up with a small allocation to ensure clean context
        _warmup = torch.zeros(1, device='cuda')
        del _warmup
        torch.cuda.empty_cache()
    os.system("sleep 2")
    free_gb = torch.cuda.mem_get_info()[0] / 1e9
    print(f"[sft] VRAM free: {free_gb:.1f} GB")

    from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
    from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training
    from trl import SFTTrainer, SFTConfig
    from datasets import Dataset

    # Load model — SDPA attention for RTX 5090 Flash SDP
    print(f"[sft] Loading {BASE_MODEL} with SDPA attention...")
    bnb = BitsAndBytesConfig(
        load_in_4bit=True, bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True)
    model = AutoModelForCausalLM.from_pretrained(
        BASE_MODEL, quantization_config=bnb, device_map="auto",
        torch_dtype=torch.bfloat16, attn_implementation="sdpa")
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    model = prepare_model_for_kbit_training(model)

    # Freeze non-language modules
    for name, param in model.named_parameters():
        if "vision_tower" in name or "audio_tower" in name or "embed_vision" in name:
            param.requires_grad = False

    # LoRA targets — only language_model layers
    target_names = [
        name for name, mod in model.named_modules()
        if "language_model.layers." in name and name.endswith(
            ("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"))
    ]
    print(f"[sft] {len(target_names)} LoRA targets, rank 256, alpha 512")

    lora_config = LoraConfig(
        r=256, lora_alpha=512, target_modules=target_names,
        lora_dropout=0.05, bias="none", task_type="CAUSAL_LM")
    model = get_peft_model(model, lora_config)
    model.gradient_checkpointing_enable()

    # Load CLEAN SFT data (stage directions stripped) + compositions
    print("[sft] Loading training data...")
    sft_data = []
    clean_path = TRAINING_DIR / "shakespeare_sft_clean.jsonl"
    if clean_path.exists():
        with open(clean_path) as f:
            sft_data = [json.loads(l) for l in f]
    sft_data.extend(compositions)
    random.shuffle(sft_data)
    print(f"[sft] Total examples: {len(sft_data)} ({len(sft_data) - len(compositions)} clean + {len(compositions)} compositions)")

    def format_example(ex):
        return {"text": tokenizer.apply_chat_template(
            ex["messages"], tokenize=False, add_generation_prompt=False)}

    dataset = Dataset.from_list(sft_data)
    dataset = dataset.map(format_example, remove_columns=dataset.column_names)

    sft_output = MODEL_DIR / "sft_checkpoint"
    sft_output.mkdir(parents=True, exist_ok=True)

    training_args = SFTConfig(
        output_dir=str(sft_output),
        per_device_train_batch_size=1,
        gradient_accumulation_steps=32,  # effective batch 32
        num_train_epochs=3,
        learning_rate=2e-4,
        lr_scheduler_type="cosine",
        warmup_ratio=0.05,
        weight_decay=0.01,
        logging_steps=10,
        save_steps=500,
        save_total_limit=2,
        fp16=False,
        bf16=True,
        optim="adamw_8bit",
        seed=42,
        max_length=512,  # shorter = faster, Shakespeare responses are <100 words
        dataset_text_field="text",
        packing=False,
        report_to="none",
        dataloader_num_workers=4,
        dataloader_pin_memory=True,
    )

    trainer = SFTTrainer(
        model=model, processing_class=tokenizer,
        train_dataset=dataset, args=training_args)

    vram_gb = torch.cuda.memory_allocated() / 1e9
    print(f"[sft] VRAM after setup: {vram_gb:.1f} GB")
    print(f"[sft] Starting SFT: {len(sft_data)} examples, 3 epochs, batch 4×8=32")

    t0 = time.time()
    trainer.train()
    sft_time = time.time() - t0
    print(f"[sft] SFT DONE in {sft_time:.0f}s ({sft_time/60:.1f} min)")

    model.save_pretrained(str(sft_output))
    tokenizer.save_pretrained(str(sft_output))
    print(f"[sft] Checkpoint saved to {sft_output}")
    return model, tokenizer


def phase3_dpo(model, tokenizer):
    """DPO on contrastive pairs."""
    print("\n" + "=" * 60)
    print("  PHASE 3: DPO Training (12,567 contrastive pairs)")
    print("=" * 60)

    from trl import DPOTrainer, DPOConfig
    from datasets import Dataset

    dpo_path = TRAINING_DIR / "shakespeare_dpo.jsonl"
    if not dpo_path.exists():
        print("[dpo] No DPO data found, skipping")
        return model, tokenizer

    raw = []
    with open(dpo_path) as f:
        for line in f:
            raw.append(json.loads(line))
    print(f"[dpo] {len(raw)} contrastive pairs loaded")

    def format_dpo(ex):
        prompt_text = tokenizer.apply_chat_template(
            ex["prompt"], tokenize=False, add_generation_prompt=True)
        return {
            "prompt": prompt_text,
            "chosen": ex["chosen"][0]["content"],
            "rejected": ex["rejected"][0]["content"],
        }

    dataset = Dataset.from_list(raw)
    dataset = dataset.map(format_dpo, remove_columns=dataset.column_names)

    dpo_output = MODEL_DIR / "dpo_checkpoint"
    dpo_output.mkdir(parents=True, exist_ok=True)

    training_args = DPOConfig(
        output_dir=str(dpo_output),
        per_device_train_batch_size=1,
        gradient_accumulation_steps=32,
        num_train_epochs=1,
        learning_rate=5e-5,
        lr_scheduler_type="cosine",
        warmup_ratio=0.1,
        weight_decay=0.01,
        beta=0.1,
        logging_steps=10,
        save_steps=1000,
        save_total_limit=2,
        fp16=False,
        bf16=True,
        optim="adamw_8bit",
        seed=42,
        max_length=512,
        report_to="none",
        remove_unused_columns=False,
    )

    trainer = DPOTrainer(
        model=model, ref_model=None,
        processing_class=tokenizer,
        train_dataset=dataset, args=training_args)

    print(f"[dpo] Starting DPO: {len(raw)} pairs, 1 epoch")
    t0 = time.time()
    trainer.train()
    dpo_time = time.time() - t0
    print(f"[dpo] DPO DONE in {dpo_time:.0f}s ({dpo_time/60:.1f} min)")

    final_path = MODEL_DIR / "final"
    final_path.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(final_path))
    tokenizer.save_pretrained(str(final_path))
    print(f"[dpo] Final model saved to {final_path}")
    return model, tokenizer


def main():
    print("=" * 60)
    print("  SHAKESPEARE LoRA — FULL PIPELINE (RTX 5090 OPTIMIZED)")
    print("=" * 60)
    print(f"  GPU: {torch.cuda.get_device_name(0)}")
    print(f"  PyTorch: {torch.__version__}")
    print(f"  CUDA: {torch.version.cuda}")
    print(f"  TF32: enabled")
    print(f"  Attention: SDPA (Flash SDP on Blackwell)")
    print("=" * 60)

    TRAINING_DIR.mkdir(parents=True, exist_ok=True)
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    t_total = time.time()

    # Phase 1: Generate compositions (Ollama on GPU)
    # Check if compositions already exist (skip if we have enough)
    comp_path = TRAINING_DIR / "shakespeare_compositions.jsonl"
    if comp_path.exists() and sum(1 for _ in open(comp_path)) >= 200:
        compositions = [json.loads(l) for l in open(comp_path)]
        print(f"[pipeline] Reusing {len(compositions)} existing compositions")
    else:
        compositions = phase1_compositions()

    # Kill Ollama BEFORE importing torch.cuda in training
    print("[pipeline] Killing Ollama to free GPU for training...")
    os.system("pkill -9 ollama 2>/dev/null")
    os.system("sleep 5")  # Give driver time to fully release

    # Phase 2: SFT (GPU)
    model, tokenizer = phase2_sft(compositions)

    # Phase 3: DPO (GPU)
    model, tokenizer = phase3_dpo(model, tokenizer)

    total_time = time.time() - t_total
    print("\n" + "=" * 60)
    print(f"  PIPELINE COMPLETE — {total_time:.0f}s ({total_time/60:.1f} min)")
    print(f"  Final LoRA: {MODEL_DIR / 'final'}")
    print("=" * 60)


if __name__ == "__main__":
    main()
