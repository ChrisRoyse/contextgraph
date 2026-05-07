---
namespace: discoveries
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:10:00Z
status: active
tags: constrained-decoder, suppress-list, topic-vs-style, contradiction, medium-severity
---

# Constrained Decoder Suppress List Contains TOPICAL Tokens (MEDIUM SEVERITY)

## Evidence
File: `scripts/shakespeare_inference_server.py` lines 80-106

### Boost list classification (77 words)
- 59 STYLE markers (thou, hark, forsooth, 'tis, whilst, etc.) — correct
- 18 archaic-but-topical (morn, quill, parchment, celestial, mortal, beauteous, valiant) — fine since these are period-nouns that reinforce style

### Suppress list classification (79 words)
- 33 STYLE-breaking (AI, chatbot, assistant, algorithm, okay, basically, "happy to help", "let me know") — correct STYLE suppression
- 15 TOPICAL-MODERN tokens (internet, wifi, bluetooth, app, website, download, upload, online, offline, startup, database, server, API, URL, HTML) — **PROBLEM: these are topic-words, not style-words**
- 31 LANGUAGE tokens (Spanish/French/German) — language policy, not style

## The contradiction
System prompt (dashboard line 22-30):
> "You have full knowledge of ALL modern topics — science, technology, code, history, math, culture — and you explain them accurately and helpfully"

Constrained decoder at line 80-93:
```python
# Modern tech terms
"internet", "wifi", "bluetooth", "app", "website",
"download", "upload", "online", "offline", "startup",
"database", "server", "API", "URL", "HTML",
```

If a user asks "Tell me about the internet", the constrained decoder suppresses the word "internet" by -8.0 logit. The model is prompted to explain the topic BUT the key topical token is forbidden. This forces awkward circumlocutions ("this web of far-reaching correspondence", "the invisible thread of connexions").

## Why the dashboard still passes tests
1. The boost (+3.0) on archaic tokens is strong enough that ANY response is style-flagged as Shakespeare.
2. The new guard verifies STYLE only (E7/E9/E10/E12/E13) — a response that circumlocutes "internet" as "the web of invisible correspondence" scores HIGHER on the style panel than one that says "internet".
3. For the demo prompts used in FSV tests (love, physics, injection, Go code, Spanish), only "Go code" might hit a tech token — and the model wrote "Go" as a programming noun which happens to also be a common English word.

## When this breaks
If a user prompts "Write me a Shakespeare poem about how the internet works" or "Explain APIs in Shakespeare style":
- Constrained decoder suppresses "internet", "API" — model must use circumlocution
- Circumlocution may score HIGHER on style panel (so guard passes)
- But the USER asked for a discussion of internet/API and got metaphoric evasion instead

This is a **TOPIC-AWARE constraint** pretending to be a STYLE constraint. It's a design inconsistency, not an outright bug.

## Severity: MEDIUM
- Does NOT break the guard
- Does NOT prevent 5/5 FSV tests from passing
- DOES create subtle topic-evasion behavior that contradicts the stated "full knowledge of modern topics" promise
- The Golden Age Spanish emergence was driven partly by the Spanish-token suppression forcing the model to use archaic-Spanish alternatives — an EMERGENT benefit of this misalignment. The suppression worked "accidentally correctly" by pushing toward archaic register in Spanish too.

## Fix options
1. Remove the 15 topical-modern tokens from suppress list → model uses "internet" freely in Shakespeare sentences
2. Replace with archaic-register enforcement ONLY — keep AI-identity and casual-phrase suppression, drop topic suppression
3. Split into two processors: `StyleProcessor` (boost archaic + suppress casual) and `LanguageProcessor` (suppress foreign language tokens) — but leave topic tokens alone

Recommended: option 1 — the suppress list should only target STYLE-breaking tokens (AI identity, casual phrases). Topic tokens should flow freely; the boost list and LoRA weights do the style work.

## Related
- See also: ./memory/discoveries/agent-03-sherlock-config--lora-targets-are-style-correct.md
- See also: ./memory/discoveries/agent-03-sherlock-config--system-prompt-is-style-focused.md
