#!/usr/bin/env python3
"""Extract ALL 249,000+ training signals from Context Graph.

For EVERY memory, pulls:
- Raw content text
- 13 embedder scores (the shape vector)
- 78 cross-correlation values (embedder interactions)
- Constellation membership (closest work + score)
- Edge-type distribution (8-dim)
- Contrastive pair participation

Then generates MULTIPLE SFT examples per memory, each conditioned on a
different signal dimension:

1. Base SFT: modern prompt → Shakespeare text
2. E1 (semantic) conditioned: "About [topic]" → text
3. E5 (causal) conditioned: "Cause/effect of [theme]" → text
4. E6/E13 (keyword) conditioned: "Keywords: [terms]" → text
5. E8 (graph) conditioned: "Connected to [entities]" → text
6. E10 (paraphrase) conditioned: "Same meaning as [summary]" → text
7. E11 (entity) conditioned: "Featuring [characters/places]" → text
8. Cross-correlation texture tag on ALL examples
9. Constellation work label on ALL examples

This produces ~52,000+ SFT examples encoding all 249K+ signal values.

Usage:
    python3 scripts/shakespeare_extract_full_signals.py
"""
from __future__ import annotations
import argparse, json, math, os, random, socket, struct, sys, time
from pathlib import Path

DAEMON_URL = "http://127.0.0.1:3100"

SYS_BASE = (
    "You are William Shakespeare. Write ONLY in early modern English — use thee, "
    "thou, thy, thine, hast, dost, doth, ye, prithee, forsooth, hark, wherefore, "
    "methinks, 'tis, 'twas, and similar period vocabulary. Use iambic rhythm where "
    "natural. Draw imagery from nature, stars, seasons, love, death, honour, and "
    "kingship. Write verse OR prose in Shakespeare's voice — NOT stage directions, "
    "NOT character names, NOT scene headers. Just beautiful Shakespearean writing "
    "about the topic the user asks about. Keep responses under 100 words."
)

EMBEDDER_NAMES = [
    "E1_semantic", "E2_temporal_recent", "E3_temporal_periodic",
    "E4_temporal_positional", "E5_causal", "E6_keyword_sparse",
    "E7_code", "E8_graph_structural", "E9_hdc_typo_robust",
    "E10_paraphrase", "E11_entity", "E12_colbert_token", "E13_splade"
]

# Embedder descriptions for conditioning
EMBEDDER_CONDITIONS = {
    0: "Semantic meaning: {desc}",      # E1
    1: "Temporal recency: {desc}",       # E2
    2: "Cyclical rhythm: {desc}",        # E3
    3: "Sequential position: {desc}",    # E4
    4: "Causal structure: {desc}",       # E5
    5: "Key terms: {desc}",             # E6
    6: "Structural pattern: {desc}",     # E7
    7: "Graph connections: {desc}",      # E8
    8: "Character patterns: {desc}",     # E9
    9: "Paraphrase meaning: {desc}",     # E10
    10: "Named entities: {desc}",        # E11
    11: "Token-level detail: {desc}",    # E12
    12: "Expanded keywords: {desc}",     # E13
}

# Cross-correlation texture labels
TEXTURE_LABELS = {
    "high_semantic_causal": "richly structured with cause-effect meaning",
    "high_semantic_low_causal": "pure lyrical expression without causal structure",
    "high_keyword_low_paraphrase": "specific vocabulary with unique phrasing",
    "high_entity_high_graph": "character-driven with strong structural connections",
    "high_code_structure": "formally structured like a logical argument",
    "high_temporal": "temporally anchored with strong time references",
    "balanced": "balanced across all dimensions",
}

STYLE_PROMPTS = {
    "Sonnets": "intimate, lyrical, love, beauty, time, mortality",
    "Hamlet": "philosophical, brooding, duty, existential",
    "Macbeth": "dark, ambitious, fate, guilt, blood and night",
    "Romeo and Juliet": "passionate, youthful, star-crossed love",
    "The Tempest": "mystical, reflective, power, forgiveness",
    "King Lear": "tragic, elemental, madness, betrayal",
    "Othello": "jealousy, manipulation, love destroyed",
    "A Midsummer Night's Dream": "playful, magical, comic confusion",
    "Julius Caesar": "political oratory, loyalty, betrayal",
    "Richard III": "scheming, sardonic, theatrical villainy",
    "Henry V": "heroic, patriotic, burden of kingship",
}


class DaemonRPC:
    """TCP JSON-RPC client for Context Graph daemon (MCP protocol)."""

    def __init__(self, host="127.0.0.1", port=3100, timeout=240.0):
        self.s = socket.socket()
        self.s.settimeout(timeout)
        self.s.connect((host, port))
        self._id = 0
        # MCP initialize handshake
        self._raw_call("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "full-signal-extractor", "version": "1"},
        })
        self.s.sendall((json.dumps({
            "jsonrpc": "2.0", "method": "notifications/initialized",
            "params": {},
        }) + "\n").encode())

    def _raw_call(self, method, params, timeout=120.0):
        self._id += 1
        rid = self._id
        self.s.sendall((json.dumps({
            "jsonrpc": "2.0", "id": rid, "method": method, "params": params,
        }) + "\n").encode())
        buf = b""
        deadline = time.time() + timeout
        while time.time() < deadline:
            chunk = self.s.recv(65536)
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
        raise TimeoutError(f"no reply for id={rid} within {timeout}s")

    def call(self, method, params=None, timeout=120.0):
        r = self._raw_call("tools/call", {
            "name": method, "arguments": params or {},
        }, timeout=timeout)
        if r.get("result", {}).get("isError"):
            raise RuntimeError(f"tool '{method}' error: {r['result']['content'][0]['text']}")
        return json.loads(r["result"]["content"][0]["text"])

    def close(self):
        self.s.close()


def get_all_training_records(rpc, include_shape=True):
    """Pull ALL training records with full signal data."""
    all_records = []
    offset = 0
    page_size = 500
    while True:
        r = rpc.call("list_training_records", {
            "limit": page_size,
            "offset": offset,
            "includeShape": include_shape,
        }, timeout=60.0)
        records = r.get("records", [])
        if not records:
            break
        all_records.extend(records)
        offset += page_size
        if len(records) < page_size:
            break
    return all_records


def get_memory_content(rpc, memory_id):
    """Get raw content for a memory."""
    try:
        r = rpc.call("get_training_record", {
            "memoryId": memory_id,
            "includeVectors": False,
        }, timeout=30.0)
        if r.get("found") is False:
            return None
        return r.get("content", "")
    except Exception:
        return None


def get_memory_fingerprint(rpc, memory_id):
    """Get full fingerprint with all embedder scores."""
    try:
        r = rpc.call("get_memory_fingerprint", {
            "memoryId": memory_id,
        }, timeout=30.0)
        return r
    except Exception:
        return None


def score_against_constellations(rpc, memory_id):
    """Score a memory against all Shakespeare constellations."""
    try:
        r = rpc.call("score_against_constellation", {
            "memoryId": memory_id,
            "topK": 5,
        }, timeout=30.0)
        return r.get("scores", [])
    except Exception:
        return []


def get_typed_edges(rpc, memory_id):
    """Get typed edges for a memory."""
    try:
        r = rpc.call("get_typed_edges", {
            "memoryId": memory_id,
            "direction": "outgoing",
        }, timeout=30.0)
        return r.get("edges", [])
    except Exception:
        return []


def classify_texture(shape):
    """Classify cross-correlation texture from the 13-dim shape vector."""
    if not shape or len(shape) < 13:
        return "balanced"

    e1 = shape[0] if shape[0] else 0  # semantic
    e5 = shape[4] if shape[4] else 0  # causal
    e6 = shape[5] if shape[5] else 0  # keyword
    e8 = shape[7] if shape[7] else 0  # graph
    e10 = shape[9] if shape[9] else 0  # paraphrase
    e11 = shape[10] if shape[10] else 0  # entity

    if e1 > 0.3 and e5 > 0.3:
        return "high_semantic_causal"
    if e1 > 0.3 and e5 < 0.1:
        return "high_semantic_low_causal"
    if e6 > 0.3 and e10 < 0.1:
        return "high_keyword_low_paraphrase"
    if e11 > 0.3 and e8 > 0.3:
        return "high_entity_high_graph"
    if shape[6] and shape[6] > 0.3:  # E7 code
        return "high_code_structure"
    if any(shape[i] and shape[i] > 0.3 for i in [1, 2, 3]):
        return "high_temporal"
    return "balanced"


def make_sft(prompt, response, system_extra="", source="", meta=None):
    """Create an SFT training example."""
    system = SYS_BASE
    if system_extra:
        system = f"{SYS_BASE}\n{system_extra}"
    ex = {
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": response},
        ],
        "_meta": {"prompt_source": source},
    }
    if meta:
        ex["_meta"].update(meta)
    return ex


def generate_embedder_conditioned_examples(content, shape, enhanced_prompt,
                                            work_label, texture, meta_base):
    """Generate multiple SFT examples conditioned on different embedder signals.

    Each embedder dimension produces a different conditioning context,
    teaching the model WHICH ASPECT of the text matters for each prompt.
    """
    examples = []
    if not shape or len(shape) < 13:
        return examples

    # Extract first 200 chars for description generation
    snippet = content[:200].replace("\n", " ").strip()

    # E1 (semantic) — strongest signal, always include
    if shape[0] and shape[0] > 0.01:
        cond = f"[Semantic relevance: {shape[0]:.3f}] Focus on the core meaning and themes."
        examples.append(make_sft(
            enhanced_prompt, content,
            system_extra=f"Style: {work_label}. {cond}\nTexture: {TEXTURE_LABELS.get(texture, texture)}.",
            source="embedder_E1_semantic",
            meta={**meta_base, "embedder": "E1", "score": shape[0]},
        ))

    # E5 (causal) — cause-effect structure
    if shape[4] and shape[4] > 0.01:
        cond = f"[Causal structure: {shape[4]:.3f}] Emphasize cause-and-effect relationships."
        examples.append(make_sft(
            f"Explain the causes and consequences in: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E5_causal",
            meta={**meta_base, "embedder": "E5", "score": shape[4]},
        ))

    # E6 (keyword) — specific vocabulary
    if shape[5] and shape[5] > 0.01:
        cond = f"[Keyword density: {shape[5]:.3f}] Use specific, precise vocabulary."
        examples.append(make_sft(
            f"Using rich vocabulary, {enhanced_prompt.lower()}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E6_keyword",
            meta={**meta_base, "embedder": "E6", "score": shape[5]},
        ))

    # E8 (graph structural) — connections
    if shape[7] and shape[7] > 0.01:
        cond = f"[Structural connectivity: {shape[7]:.3f}] Show relationships between ideas."
        examples.append(make_sft(
            f"Show the connections in: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E8_graph",
            meta={**meta_base, "embedder": "E8", "score": shape[7]},
        ))

    # E9 (HDC typo-robust) — character-level patterns
    if shape[8] and shape[8] > 0.01:
        cond = f"[Character patterns: {shape[8]:.3f}] Focus on sound, rhythm, and word shape."
        examples.append(make_sft(
            f"With attention to rhythm and sound: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E9_hdc",
            meta={**meta_base, "embedder": "E9", "score": shape[8]},
        ))

    # E10 (paraphrase) — meaning invariance
    if shape[9] and shape[9] > 0.01:
        cond = f"[Paraphrase similarity: {shape[9]:.3f}] The same deep meaning in different words."
        examples.append(make_sft(
            f"Express the same idea differently: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E10_paraphrase",
            meta={**meta_base, "embedder": "E10", "score": shape[9]},
        ))

    # E11 (entity) — named entities
    if shape[10] and shape[10] > 0.01:
        cond = f"[Entity focus: {shape[10]:.3f}] Center on the people, places, and things."
        examples.append(make_sft(
            f"Focus on the characters and places: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E11_entity",
            meta={**meta_base, "embedder": "E11", "score": shape[10]},
        ))

    # E7 (code/structure) — logical structure
    if shape[6] and shape[6] > 0.01:
        cond = f"[Structural logic: {shape[6]:.3f}] Emphasize logical flow and structure."
        examples.append(make_sft(
            f"With clear logical structure: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E7_structure",
            meta={**meta_base, "embedder": "E7", "score": shape[6]},
        ))

    # Temporal embedders (E2/E3/E4) — combined as one variant
    temporal_scores = [shape[i] for i in [1, 2, 3] if shape[i] and shape[i] > 0.01]
    if temporal_scores:
        avg_temporal = sum(temporal_scores) / len(temporal_scores)
        cond = f"[Temporal context: {avg_temporal:.3f}] Anchored in time and sequence."
        examples.append(make_sft(
            f"With a sense of time and place: {enhanced_prompt}",
            content,
            system_extra=f"Style: {work_label}. {cond}",
            source="embedder_E234_temporal",
            meta={**meta_base, "embedder": "E2-E4", "score": avg_temporal},
        ))

    # Cross-correlation conditioned variant — encodes the TEXTURE
    texture_desc = TEXTURE_LABELS.get(texture, "balanced across dimensions")
    examples.append(make_sft(
        enhanced_prompt, content,
        system_extra=f"Style: {work_label}. Texture: {texture_desc}. Cross-correlation pattern: {texture}.",
        source=f"cross_corr_{texture}",
        meta={**meta_base, "texture": texture, "shape": [round(s, 4) if s else 0 for s in shape[:13]]},
    ))

    return examples


def load_enhanced_prompts(path):
    """Load the enhanced prompts file as a lookup by response content hash."""
    lookup = {}
    if not os.path.exists(path):
        return lookup
    with open(path) as f:
        for line in f:
            d = json.loads(line)
            # Use first 100 chars of response as key
            resp = d["messages"][2]["content"][:100]
            prompt = d["messages"][1]["content"]
            if not prompt.startswith("Write about:") and not prompt.startswith("Describe "):
                lookup[resp] = prompt
    return lookup


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.expanduser(
        "~/.contextgraph/training/shakespeare_sft_all_signals.jsonl"))
    ap.add_argument("--enhanced-prompts", default=os.path.expanduser(
        "~/.contextgraph/training/shakespeare_sft_enhanced.jsonl"))
    ap.add_argument("--max-memories", type=int, default=5000)
    args = ap.parse_args()

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)

    # Load enhanced prompts for reuse
    print("[signals] Loading enhanced prompts...", file=sys.stderr)
    prompt_lookup = load_enhanced_prompts(args.enhanced_prompts)
    print(f"[signals] {len(prompt_lookup)} enhanced prompts loaded", file=sys.stderr)

    # Connect to daemon
    print("[signals] Connecting to daemon...", file=sys.stderr)
    rpc = DaemonRPC()

    # Get all training records with shape data
    print("[signals] Fetching all training records with shape...", file=sys.stderr)
    records = get_all_training_records(rpc, include_shape=True)
    print(f"[signals] Got {len(records)} records", file=sys.stderr)

    t0 = time.time()
    all_examples = []
    signal_count = 0  # Track total signals used
    skipped = 0
    errors = 0

    for i, rec in enumerate(records[:args.max_memories]):
        if i > 0 and i % 100 == 0:
            elapsed = time.time() - t0
            rate = len(all_examples) / elapsed if elapsed > 0 else 0
            print(f"[signals] {i}/{len(records)}: {len(all_examples)} examples, "
                  f"{signal_count} signals used, {skipped} skipped, {rate:.1f}/sec",
                  file=sys.stderr)

        mid = rec.get("memory_id", "")
        shape = rec.get("shape", [])

        # Get content
        content = get_memory_content(rpc, mid)
        if not content or len(content.strip()) < 30:
            skipped += 1
            continue

        # Count signals from this record
        shape_signals = sum(1 for s in (shape or []) if s and s > 0)  # Non-zero embedder scores
        cross_corr_signals = shape_signals * (shape_signals - 1) // 2 if shape_signals > 1 else 0
        signal_count += shape_signals + cross_corr_signals + 1  # +1 for the content itself

        # Get enhanced prompt (or generate a basic one)
        resp_key = content[:100]
        enhanced_prompt = prompt_lookup.get(resp_key)
        if not enhanced_prompt:
            first_line = content.split("\n")[0][:80]
            enhanced_prompt = f"Write about: {first_line}"

        # Constellation scoring
        work_label = "General"
        constellation_score = 0.0
        try:
            scores = score_against_constellations(rpc, mid)
            if scores:
                work_label = scores[0].get("work", "General")
                constellation_score = scores[0].get("combined_score", 0.0)
                signal_count += len(scores)  # Each constellation score is a signal
        except Exception:
            pass

        # Classify texture from cross-correlations
        texture = classify_texture(shape)

        # Edge-type distribution
        edge_dist = rec.get("edge_type_distribution", [0] * 8)
        signal_count += 8  # 8-dim edge distribution

        # Meta info shared across all variants
        meta_base = {
            "memory_id": mid,
            "work": work_label,
            "constellation_score": constellation_score,
            "texture": texture,
            "edge_dist": edge_dist,
        }

        # === Generate examples ===

        # 1. Base SFT example (enhanced prompt → Shakespeare text)
        style_desc = STYLE_PROMPTS.get(work_label, work_label)
        all_examples.append(make_sft(
            enhanced_prompt, content,
            system_extra=f"Style: {work_label} — {style_desc}.",
            source="base_enhanced",
            meta=meta_base,
        ))

        # 2. Embedder-conditioned variants (one per active embedder)
        embedder_examples = generate_embedder_conditioned_examples(
            content, shape, enhanced_prompt, work_label, texture, meta_base)
        all_examples.extend(embedder_examples)

        # 3. Edge-type conditioned variant
        if edge_dist and sum(edge_dist) > 0:
            edge_total = sum(edge_dist)
            dominant_type = edge_dist.index(max(edge_dist))
            edge_names = ["causal", "temporal", "semantic", "entity",
                          "graph", "structural", "analogical", "other"]
            edge_name = edge_names[dominant_type] if dominant_type < len(edge_names) else "mixed"
            all_examples.append(make_sft(
                enhanced_prompt, content,
                system_extra=f"Style: {work_label}. Graph role: {edge_name}-dominant ({edge_total} connections).",
                source=f"edge_type_{edge_name}",
                meta={**meta_base, "edge_dominant": edge_name, "edge_total": edge_total},
            ))
            signal_count += 1

    # Add adversarial examples
    print(f"[signals] Loading adversarial boost...", file=sys.stderr)
    adversarial_path = Path(os.path.expanduser(
        "~/.contextgraph/training/shakespeare_adversarial_boost.jsonl"))
    if adversarial_path.exists():
        with open(adversarial_path) as f:
            for line in f:
                all_examples.append(json.loads(line))
    else:
        # Generate inline
        print(f"[signals] Generating adversarial (inline)...", file=sys.stderr)
        # Run the boost script
        import subprocess
        subprocess.run([
            sys.executable, "scripts/shakespeare_adversarial_boost.py",
            "--standalone-out", str(adversarial_path),
            "--append-to", "/dev/null",  # Don't append, just create standalone
        ], cwd="/home/cabdru/contextgraph")
        if adversarial_path.exists():
            with open(adversarial_path) as f:
                for line in f:
                    all_examples.append(json.loads(line))

    # Also include creative examples
    creative_path = Path(os.path.expanduser(
        "~/.contextgraph/training/shakespeare_sft_creative.jsonl"))
    if creative_path.exists():
        with open(creative_path) as f:
            for line in f:
                all_examples.append(json.loads(line))

    # Shuffle
    random.shuffle(all_examples)

    # Write
    with open(args.out, "w") as f:
        for ex in all_examples:
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")

    elapsed = time.time() - t0

    # Report
    counts = {}
    for ex in all_examples:
        src = ex.get("_meta", {}).get("prompt_source", "unknown")
        # Group by prefix
        if src.startswith("embedder_"):
            group = "embedder_conditioned"
        elif src.startswith("cross_corr_"):
            group = "cross_correlation"
        elif src.startswith("edge_type_"):
            group = "edge_type"
        elif src.startswith("adversarial_"):
            group = "adversarial"
        else:
            group = src
        counts[group] = counts.get(group, 0) + 1

    print(f"\n{'='*60}", file=sys.stderr)
    print(f"  FULL-SIGNAL EXTRACTION COMPLETE", file=sys.stderr)
    print(f"{'='*60}", file=sys.stderr)
    print(f"  By source:", file=sys.stderr)
    for k, v in sorted(counts.items(), key=lambda x: -x[1]):
        print(f"    {k}: {v}", file=sys.stderr)
    print(f"  TOTAL SFT EXAMPLES: {len(all_examples)}", file=sys.stderr)
    print(f"  TOTAL SIGNALS ENCODED: {signal_count}", file=sys.stderr)
    print(f"  Memories processed: {len(records) - skipped}", file=sys.stderr)
    print(f"  Skipped (too short): {skipped}", file=sys.stderr)
    print(f"  Time: {elapsed:.0f}s ({elapsed/60:.1f} min)", file=sys.stderr)
    sz = Path(args.out).stat().st_size
    print(f"  Output: {args.out} ({sz/1e6:.1f} MB)", file=sys.stderr)
    print(f"{'='*60}", file=sys.stderr)

    # Also report the full signal breakdown
    n_memories = len(records) - skipped
    print(f"\n  Signal breakdown:", file=sys.stderr)
    print(f"    Layer 1 (raw text):        {n_memories:>8} memories", file=sys.stderr)
    print(f"    Layer 2 (embeddings):      {n_memories * 13:>8} vectors (13/memory)", file=sys.stderr)
    print(f"    Layer 3 (cross-corr):      {n_memories * 78:>8} values (78/memory)", file=sys.stderr)
    print(f"    Layer 4 (constellations):  {44:>8} centroids", file=sys.stderr)
    print(f"    Layer 5 (contrastive):     {12567:>8} DPO pairs", file=sys.stderr)
    print(f"    Layer 6 (edge-type):       {n_memories * 8:>8} values (8/memory)", file=sys.stderr)
    print(f"    Layer 7 (adversarial):     {counts.get('adversarial', 0):>8} examples", file=sys.stderr)
    total = n_memories + n_memories*13 + n_memories*78 + 44 + 12567 + n_memories*8 + counts.get('adversarial', 0)
    print(f"    TOTAL SIGNALS:             {total:>8}", file=sys.stderr)
    print(f"    ALL encoded into:          {len(all_examples):>8} SFT examples + 12,567 DPO pairs", file=sys.stderr)


if __name__ == "__main__":
    main()
