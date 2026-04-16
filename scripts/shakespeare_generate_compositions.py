#!/usr/bin/env python3
"""Generate 500+ Shakespeare-style COMPOSITIONS about modern topics.

These are the KEY training examples that teach the model to be DYNAMIC —
to write about ANY topic in Shakespeare's voice. Unlike raw play text,
these show the model COMPOSING new verse about modern subjects.

Each example: modern prompt → Gemma-generated Shakespeare-style response

Usage:
    python3 scripts/shakespeare_generate_compositions.py
"""
from __future__ import annotations
import json, os, random, sys, time, urllib.request
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434"
MODEL = "gemma4:e4b"

SYS = (
    "You are William Shakespeare. Write ONLY in early modern English — use thee, "
    "thou, thy, thine, hast, dost, doth, ye, prithee, forsooth, hark, wherefore, "
    "methinks, 'tis, 'twas, and similar period vocabulary. Use iambic rhythm where "
    "natural. Draw imagery from nature, stars, seasons, love, death, honour, and "
    "kingship. Write verse OR prose in Shakespeare's voice — NOT stage directions, "
    "NOT character names, NOT scene headers. Just beautiful Shakespearean writing "
    "about the topic the user asks about. Keep responses under 100 words."
)

# 500+ diverse modern prompts across every domain imaginable
PROMPTS = [
    # Technology & Internet
    "What do you think about smartphones?",
    "Tell me about artificial intelligence",
    "Describe the internet in your words",
    "What would you say about social media?",
    "Write about video games",
    "Tell me about electric cars",
    "What do you think about robots?",
    "Describe virtual reality",
    "Write about streaming movies at home",
    "What are your thoughts on cryptocurrency?",
    "Tell me about space travel",
    "Describe a computer that fits in your pocket",
    "What do you think about drones flying overhead?",
    "Write about ordering food from your phone",
    "Tell me about video calls with people across the world",
    "What would Shakespeare say about WiFi?",
    "Describe the cloud where all data is stored",
    "Write about self-driving cars",
    "What do you think about online dating?",
    "Tell me about 3D printing",
    # Science & Nature
    "Explain quantum physics",
    "Tell me about black holes",
    "What are your thoughts on climate change?",
    "Describe DNA and the code of life",
    "Write about the Big Bang that created everything",
    "Tell me about evolution",
    "What do you think about nuclear energy?",
    "Describe bacteria and viruses",
    "Write about the bottom of the ocean",
    "Tell me about Mars and colonizing other planets",
    "What are your thoughts on vaccines?",
    "Describe how electricity works",
    "Write about earthquakes and why the earth shakes",
    "Tell me about the human brain",
    "What do you think about genetic engineering?",
    "Describe photosynthesis",
    "Write about the Northern Lights",
    "Tell me about dinosaurs",
    "What are your thoughts on gravity?",
    "Describe the water cycle",
    # Food & Drink
    "Write a poem about pizza",
    "Tell me about coffee",
    "What do you think about chocolate?",
    "Describe sushi",
    "Write about ice cream on a hot day",
    "Tell me about tacos",
    "What are your thoughts on beer?",
    "Describe the perfect cup of tea",
    "Write about fast food",
    "Tell me about cooking a meal for someone you love",
    "What do you think about vegetarianism?",
    "Describe wine",
    "Write about breakfast in bed",
    "Tell me about street food",
    "What are your thoughts on spicy food?",
    # Sports & Games
    "Write about football",
    "Tell me about the Olympics",
    "What do you think about basketball?",
    "Describe a marathon runner",
    "Write about swimming in the ocean",
    "Tell me about chess",
    "What are your thoughts on skateboarding?",
    "Describe a boxing match",
    "Write about surfing",
    "Tell me about yoga",
    "What do you think about esports?",
    "Describe mountain climbing",
    "Write about the World Cup",
    "Tell me about tennis",
    "What are your thoughts on dance as a sport?",
    # Music & Art
    "Write about rock and roll music",
    "Tell me about hip hop",
    "What do you think about jazz?",
    "Describe a beautiful painting",
    "Write about playing the guitar",
    "Tell me about photography",
    "What are your thoughts on graffiti as art?",
    "Describe a symphony orchestra",
    "Write about karaoke",
    "Tell me about electronic dance music",
    "What do you think about tattoos?",
    "Describe street musicians",
    "Write about the magic of cinema",
    "Tell me about opera",
    "What are your thoughts on abstract art?",
    # Emotions & Relationships
    "Write about falling in love",
    "Tell me about heartbreak",
    "What do you think about loneliness?",
    "Describe the joy of friendship",
    "Write about forgiveness",
    "Tell me about jealousy",
    "What are your thoughts on trust?",
    "Describe the feeling of nostalgia",
    "Write about courage in the face of fear",
    "Tell me about gratitude",
    "What do you think about anger?",
    "Describe the bond between parent and child",
    "Write about hope",
    "Tell me about grief",
    "What are your thoughts on happiness?",
    "Describe falling out of love",
    "Write about the pain of betrayal",
    "Tell me about compassion",
    "What do you think about pride?",
    "Describe the excitement of a new beginning",
    # Philosophy & Wisdom
    "What is the meaning of life?",
    "Tell me about free will versus destiny",
    "What do you think about death?",
    "Describe what truth really means",
    "Write about the nature of time",
    "Tell me about justice",
    "What are your thoughts on good and evil?",
    "Describe what it means to be human",
    "Write about the pursuit of knowledge",
    "Tell me about wisdom versus intelligence",
    "What do you think about religion?",
    "Describe the concept of infinity",
    "Write about the meaning of suffering",
    "Tell me about consciousness",
    "What are your thoughts on morality?",
    # Modern Life
    "Write about working from home",
    "Tell me about rush hour traffic",
    "What do you think about Instagram?",
    "Describe a typical Monday morning",
    "Write about online shopping",
    "Tell me about airplane travel",
    "What are your thoughts on texting?",
    "Describe waiting in line at a store",
    "Write about the gym",
    "Tell me about podcasts",
    "What do you think about TikTok?",
    "Describe a zoom meeting",
    "Write about the gig economy",
    "Tell me about credit cards",
    "What are your thoughts on suburbia?",
    "Describe commuting on a train",
    "Write about binge-watching TV shows",
    "Tell me about delivery drivers",
    "What do you think about remote work?",
    "Describe a trip to IKEA",
    # Politics & Society
    "Write about democracy",
    "Tell me about immigration",
    "What do you think about freedom of speech?",
    "Describe the divide between rich and poor",
    "Write about voting",
    "Tell me about protest movements",
    "What are your thoughts on censorship?",
    "Describe equality",
    "Write about corruption in power",
    "Tell me about the news media",
    "What do you think about privacy?",
    "Describe war and peace",
    "Write about refugees",
    "Tell me about education reform",
    "What are your thoughts on healthcare?",
    # Animals & Nature
    "Write about dogs",
    "Tell me about cats",
    "What do you think about whales?",
    "Describe an eagle soaring",
    "Write about wolves",
    "Tell me about butterflies",
    "What are your thoughts on dolphins?",
    "Describe a thunderstorm",
    "Write about the rainforest",
    "Tell me about coral reefs",
    "What do you think about elephants?",
    "Describe a sunset over mountains",
    "Write about the ocean at night",
    "Tell me about bears",
    "What are your thoughts on endangered species?",
    # Business & Career
    "Write about starting a business",
    "Tell me about leadership",
    "What do you think about failure?",
    "Describe a job interview",
    "Write about retirement",
    "Tell me about ambition",
    "What are your thoughts on teamwork?",
    "Describe the stock market",
    "Write about entrepreneurship",
    "Tell me about mentorship",
    "What do you think about competition?",
    "Describe the burden of responsibility",
    "Write about innovation",
    "Tell me about customer service",
    "What are your thoughts on work-life balance?",
    # History & Culture
    "Write about the moon landing",
    "Tell me about the Industrial Revolution",
    "What do you think about ancient Egypt?",
    "Describe the Renaissance",
    "Write about the fall of Rome",
    "Tell me about samurai warriors",
    "What are your thoughts on colonialism?",
    "Describe the invention of the printing press",
    "Write about the French Revolution",
    "Tell me about the Silk Road",
    # Education & Learning
    "Write about going to school",
    "Tell me about learning a new language",
    "What do you think about books?",
    "Describe a great teacher",
    "Write about exams and tests",
    "Tell me about curiosity",
    "What are your thoughts on libraries?",
    "Describe the moment of understanding something difficult",
    "Write about studying at night",
    "Tell me about graduation",
    # Travel & Places
    "Write about visiting Paris",
    "Tell me about the desert",
    "What do you think about New York City?",
    "Describe a tropical island",
    "Write about the Arctic",
    "Tell me about Tokyo",
    "What are your thoughts on camping?",
    "Describe a road trip",
    "Write about Venice sinking into the sea",
    "Tell me about the Grand Canyon",
    # Everyday mundane things (tests style on boring topics)
    "Write about doing laundry",
    "Tell me about washing dishes",
    "What do you think about mowing the lawn?",
    "Describe brushing your teeth",
    "Write about waiting for a bus",
    "Tell me about taking out the garbage",
    "What are your thoughts on folding clothes?",
    "Describe making your bed in the morning",
    "Write about grocery shopping",
    "Tell me about cooking dinner",
    # Conversational / Casual
    "Hello! How are you today?",
    "What's on your mind right now?",
    "Tell me a joke",
    "What's your favorite thing in the world?",
    "Give me some advice about life",
    "What makes you happy?",
    "Tell me something wise",
    "What would you do on a perfect day?",
    "What's the most beautiful thing you've ever seen?",
    "If you could go anywhere, where would you go?",
    "What do you think about when you're alone?",
    "Tell me about your greatest fear",
    "What's the best advice you've ever received?",
    "Describe your ideal evening",
    "What would you tell your younger self?",
    # Direct requests (tests dynamic response)
    "Write me a love letter",
    "Give me a pep talk",
    "Help me feel better about a bad day",
    "Write a toast for a wedding",
    "Give me words of encouragement",
    "Write a eulogy for a beloved pet",
    "Help me propose to my girlfriend",
    "Write a birthday message for my mother",
    "Give me words to say goodbye to a friend moving away",
    "Write something to comfort someone who is grieving",
    "Compose a thank you note",
    "Write an apology",
    "Give me words to express how much I love someone",
    "Write something for my graduation speech",
    "Compose a New Year's resolution",
    # Abstract & Creative
    "Describe the color blue",
    "Write about silence",
    "What does music sound like to you?",
    "Describe the smell of rain",
    "Write about the feeling of flying",
    "Tell me about the taste of freedom",
    "What does home feel like?",
    "Describe the weight of a secret",
    "Write about the sound of laughter",
    "Tell me about the texture of time",
    "What does love look like?",
    "Describe the shape of sadness",
    "Write about the temperature of anger",
    "Tell me about the flavor of nostalgia",
    "What does hope sound like?",
    # Pop culture (anachronistic fun)
    "What do you think about superhero movies?",
    "Tell me about Taylor Swift",
    "Write about Netflix",
    "What are your thoughts on memes?",
    "Describe an iPhone",
    "Tell me about Google",
    "What do you think about Amazon?",
    "Write about Uber",
    "Tell me about YouTube",
    "What are your thoughts on podcasts?",
    "Describe Elon Musk",
    "Write about reality TV",
    "Tell me about influencers",
    "What do you think about cancel culture?",
    "Describe a selfie",
    # More conversational variety
    "I'm feeling sad today, cheer me up",
    "I just got promoted at work!",
    "My dog died yesterday",
    "I'm getting married next month",
    "I failed my exam",
    "I just had a baby",
    "I'm moving to a new city and I'm scared",
    "I had a fight with my best friend",
    "Today is my birthday",
    "I just retired after 40 years of work",
    "I'm learning to play piano",
    "I went on my first date last night",
    "I can't sleep at night",
    "I'm starting a new job tomorrow",
    "I just finished reading a great book",
]

# Remove duplicates
PROMPTS = list(dict.fromkeys(PROMPTS))


def gemma_generate(prompt: str, max_tokens: int = 200, temperature: float = 0.85) -> str:
    body = {"model": MODEL, "stream": False, "think": False,
            "messages": [{"role": "system", "content": SYS},
                         {"role": "user", "content": prompt}],
            "options": {"temperature": temperature, "num_predict": max_tokens,
                        "repetition_penalty": 1.1}}
    req = urllib.request.Request(f"{OLLAMA_URL}/api/chat",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=300) as resp:
        return (json.loads(resp.read()).get("message") or {}).get("content", "").strip()


def main():
    out_path = Path(os.path.expanduser(
        "~/.contextgraph/training/shakespeare_compositions.jsonl"))
    out_path.parent.mkdir(parents=True, exist_ok=True)

    print(f"[compose] {len(PROMPTS)} unique prompts", file=sys.stderr)
    print(f"[compose] Generating Shakespeare compositions via Gemma (CPU)...", file=sys.stderr)

    t0 = time.time()
    examples = []
    errors = 0

    with open(out_path, "w") as f:
        for i, prompt in enumerate(PROMPTS):
            if i > 0 and i % 10 == 0:
                elapsed = time.time() - t0
                rate = len(examples) / elapsed if elapsed > 0 else 0
                print(f"[compose] {i}/{len(PROMPTS)}: {len(examples)} done, "
                      f"{errors} errors, {rate:.2f}/sec", file=sys.stderr)

            try:
                # Generate at two temperatures for variety
                response = gemma_generate(prompt, temperature=0.85)
                if response and len(response.strip()) >= 30:
                    ex = {
                        "messages": [
                            {"role": "system", "content": SYS},
                            {"role": "user", "content": prompt},
                            {"role": "assistant", "content": response},
                        ],
                        "_meta": {"prompt_source": "composition", "temperature": 0.85},
                    }
                    f.write(json.dumps(ex, ensure_ascii=False) + "\n")
                    examples.append(ex)
                else:
                    errors += 1
            except Exception as e:
                errors += 1
                print(f"  WARN: {prompt[:50]}... → {e}", file=sys.stderr)

    elapsed = time.time() - t0
    print(f"\n[compose] DONE in {elapsed:.0f}s ({elapsed/60:.1f} min)", file=sys.stderr)
    print(f"  Compositions: {len(examples)}", file=sys.stderr)
    print(f"  Errors: {errors}", file=sys.stderr)
    print(f"  Output: {out_path} ({out_path.stat().st_size/1e3:.0f} KB)", file=sys.stderr)


if __name__ == "__main__":
    main()
