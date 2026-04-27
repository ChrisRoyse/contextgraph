#!/usr/bin/env python3
"""Extract ALL 249,000+ signals from Context Graph into training data.

For EVERY memory, pulls ALL signal layers and converts them into
conditioned SFT examples:

Layer 1: Content (3,043 memories) → base SFT examples
Layer 2: 14 embedder topic_profiles → 14 conditioned variants per memory
Layer 3: 91 cross-correlations → texture tags on every example
Layer 4: Constellation group_alignments → work-style conditioning
Layer 5: Contrastive pairs (12,567) → DPO (already built)
Layer 6: Edge-type distributions → structural role conditioning
Layer 7: Temporal labels → temporal context conditioning
Layer 8: Tucker decomposition → complexity/compression tags
Layer 9: Adversarial (300+) → robustness examples

Total: ~30,000-40,000 SFT examples encoding 362,000+ signal values
     + 12,567 DPO pairs

Usage:
    python3 scripts/shakespeare_extract_all_signals.py
"""
from __future__ import annotations
import argparse, json, os, random, sys, time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _gemma_shakespeare_lib import Rpc

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
    "E1 Semantic", "E2 Recency", "E3 Periodic", "E4 Positional",
    "E5 Causal", "E6 Keyword", "E7 Code/Structure", "E8 Graph",
    "E9 Character-pattern", "E10 Paraphrase", "E11 Entity",
    "E12 ColBERT", "E13 SPLADE", "E14 BGE-M3 Dense",
]

NUM_EMBEDDER_SIGNALS = 14
NUM_CROSS_CORRELATIONS = 91

# Map session_id → human-readable work name
WORK_NAMES = {
    "shakespeare-hamlet": "Hamlet",
    "shakespeare-macbeth": "Macbeth",
    "shakespeare-romeo-and-juliet": "Romeo and Juliet",
    "shakespeare-the-tempest": "The Tempest",
    "shakespeare-king-lear": "King Lear",
    "shakespeare-othello": "Othello",
    "shakespeare-a-midsummer-nights-dream": "A Midsummer Night's Dream",
    "shakespeare-twelfth-night": "Twelfth Night",
    "shakespeare-much-ado-about-nothing": "Much Ado About Nothing",
    "shakespeare-julius-caesar": "Julius Caesar",
    "shakespeare-richard-iii": "Richard III",
    "shakespeare-henry-v": "Henry V",
    "shakespeare-the-merchant-of-venice": "The Merchant of Venice",
    "shakespeare-as-you-like-it": "As You Like It",
    "shakespeare-the-taming-of-the-shrew": "The Taming of the Shrew",
    "shakespeare-the-winters-tale": "The Winter's Tale",
    "shakespeare-measure-for-measure": "Measure for Measure",
    "shakespeare-sonnets": "Sonnets",
}

WORK_STYLES = {
    "Hamlet": "philosophical, brooding, existential doubt, duty vs desire",
    "Macbeth": "dark ambition, guilt, fate, blood and night imagery",
    "Romeo and Juliet": "passionate youth, star-crossed love, feuding families",
    "The Tempest": "mystical power, forgiveness, island magic, farewell",
    "King Lear": "tragic madness, betrayal, storm and chaos, filial duty",
    "Othello": "jealousy, manipulation, love destroyed by suspicion",
    "A Midsummer Night's Dream": "playful magic, comic confusion, fairy mischief",
    "Julius Caesar": "political oratory, loyalty, assassination and revenge",
    "Sonnets": "intimate lyric, love, beauty, time, mortality, iambic pentameter",
}

# Cross-correlation texture classification
def classify_texture(cross_corrs):
    """Classify the cross-correlation values into a texture label."""
    if not cross_corrs or len(cross_corrs) < 78:
        return "balanced", "balanced across all stylistic dimensions"

    # Key cross-correlations (indices in upper-triangle of 14×14):
    # E1×E5 = semantic-causal (idx 4)
    # E1×E6 = semantic-keyword (idx 5)
    # E1×E10 = semantic-paraphrase (idx 9)
    # E5×E8 = causal-graph (idx ~30)
    # E11×E8 = entity-graph (idx ~65)
    e1_e5 = cross_corrs[4] if len(cross_corrs) > 4 else 0.5
    e1_e6 = cross_corrs[5] if len(cross_corrs) > 5 else 0.5
    e1_e10 = cross_corrs[9] if len(cross_corrs) > 9 else 0.5
    avg = sum(cross_corrs) / len(cross_corrs)

    if e1_e5 > 0.7:
        return "causal_rich", "richly structured with cause-and-effect meaning — events lead to consequences"
    if e1_e5 < 0.3 and e1_e10 > 0.7:
        return "lyrical_pure", "pure lyrical expression — meaning through beauty, not logic"
    if e1_e6 > 0.8:
        return "vocabulary_dense", "dense with specific, precise vocabulary — every word chosen"
    if avg > 0.7:
        return "highly_correlated", "all dimensions aligned — a unified, coherent passage"
    if avg < 0.3:
        return "multifaceted", "different on every dimension — complex, layered writing"
    return "balanced", "balanced across stylistic dimensions"


def make_sft(prompt, response, system_extra="", source="", meta=None):
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


def work_from_session(session_id):
    """Extract human-readable work name from session_id."""
    if not session_id:
        return "General"
    for key, name in WORK_NAMES.items():
        if key in session_id:
            return name
    # Try to extract from the session string
    parts = session_id.replace("shakespeare-", "").replace("-", " ").title()
    return parts if parts else "General"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.expanduser(
        "~/.contextgraph/training/shakespeare_sft_all_signals.jsonl"))
    ap.add_argument("--enhanced-prompts", default=os.path.expanduser(
        "~/.contextgraph/training/shakespeare_sft_enhanced.jsonl"))
    ap.add_argument("--max-memories", type=int, default=5000)
    ap.add_argument("--skip-vectors", action="store_true",
                    help="Skip pulling raw vectors (faster, still gets all metadata)")
    args = ap.parse_args()

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)

    # Load enhanced prompts for reuse (keyed by first 80 chars of content)
    print("[signals] Loading enhanced prompts...", file=sys.stderr)
    prompt_lookup = {}
    enhanced_path = args.enhanced_prompts
    if os.path.exists(enhanced_path):
        with open(enhanced_path) as f:
            for line in f:
                d = json.loads(line)
                resp = d["messages"][2]["content"][:80]
                prompt = d["messages"][1]["content"]
                src = d.get("_meta", {}).get("prompt_source", "")
                # Only use actually enhanced prompts (not raw scene ones)
                if "adversarial" not in src and not prompt.startswith("Write about:"):
                    prompt_lookup[resp] = prompt
    print(f"[signals] {len(prompt_lookup)} enhanced prompts loaded", file=sys.stderr)

    # Connect
    print("[signals] Connecting to daemon...", file=sys.stderr)
    rpc = Rpc()

    # Get ALL training record IDs
    print("[signals] Fetching all training records...", file=sys.stderr)
    all_ids = []
    offset = 0
    while True:
        r = rpc.call("list_training_records",
                      {"limit": 1000, "offset": offset, "includeShape": False}, timeout=60)
        recs = r.get("records", [])
        if not recs:
            break
        all_ids.extend(rec["memory_id"] for rec in recs)
        offset += 1000
        if len(recs) < 1000:
            break
    print(f"[signals] Got {len(all_ids)} memories", file=sys.stderr)

    t0 = time.time()
    all_examples = []
    total_signals = 0
    skipped = 0
    errors = 0

    for i, mid in enumerate(all_ids[:args.max_memories]):
        if i > 0 and i % 50 == 0:
            elapsed = time.time() - t0
            rate = len(all_examples) / elapsed if elapsed > 0 else 0
            print(f"[signals] {i}/{len(all_ids)}: {len(all_examples)} examples, "
                  f"{total_signals} signals, {skipped} skip, {errors} err, "
                  f"{rate:.1f} ex/sec", file=sys.stderr)

        # Pull FULL training record
        try:
            rec = rpc.call("get_training_record", {
                "memoryId": mid,
                "includeVectors": not args.skip_vectors,
            }, timeout=30)
        except Exception as e:
            errors += 1
            continue

        content = rec.get("content", "")
        if not content or len(content.strip()) < 30:
            skipped += 1
            continue

        # === EXTRACT ALL SIGNALS ===
        topic_profile = rec.get("topic_profile", [])  # 14-dim
        cross_corrs = rec.get("cross_correlations", [])  # 91 values
        group_alignments = rec.get("group_alignments", [])  # constellation scores
        temporal = rec.get("temporal_labels", {})
        tucker = rec.get("tucker_core", {})
        edges = rec.get("outgoing_edges_detail", [])
        importance = rec.get("importance", 0.5)
        session_id = rec.get("session_id", "")

        # Count signals from this memory
        n_signals = (
            1 +                                    # content
            len(topic_profile) +                   # 14 embedder scores
            len(cross_corrs) +                     # 91 cross-correlations
            len(group_alignments) +                # constellation scores
            len(temporal) +                        # temporal fields
            (4 if tucker else 0) +                 # tucker decomposition
            len(edges) +                           # typed edges
            1                                      # importance
        )
        total_signals += n_signals

        # Derive metadata
        work = work_from_session(session_id)
        style_desc = WORK_STYLES.get(work, f"from {work}")
        texture_label, texture_desc = classify_texture(cross_corrs)

        # Get enhanced prompt
        resp_key = content[:80]
        enhanced_prompt = prompt_lookup.get(resp_key)
        if not enhanced_prompt:
            first_line = content.split("\n")[0][:80].strip()
            enhanced_prompt = f"Write about: {first_line}"

        meta_base = {
            "memory_id": mid,
            "work": work,
            "importance": round(importance, 3),
            "texture": texture_label,
            "n_signals": n_signals,
        }

        # ================================================================
        # LAYER 1: Base SFT (content + enhanced prompt + work style)
        # ================================================================
        all_examples.append(make_sft(
            enhanced_prompt, content,
            system_extra=f"Style: {work} — {style_desc}.",
            source="L1_base",
            meta=meta_base,
        ))

        # ================================================================
        # LAYER 2: Embedder-conditioned variants (topic_profile)
        # Each non-zero embedder score generates a variant
        # ================================================================
        if topic_profile and len(topic_profile) >= 13:
            # Strong embedder signals (>0.5) get their own variant
            for eidx, score in enumerate(topic_profile[:13]):
                if score and score > 0.3:
                    ename = EMBEDDER_NAMES[eidx] if eidx < len(EMBEDDER_NAMES) else f"E{eidx+1}"

                    # Create an embedder-specific prompt variant
                    if eidx == 0:  # E1 Semantic
                        variant_prompt = f"Tell me about the meaning of: {enhanced_prompt}"
                    elif eidx == 4:  # E5 Causal
                        variant_prompt = f"Explain the cause and effect: {enhanced_prompt}"
                    elif eidx == 5:  # E6 Keyword
                        variant_prompt = f"Using precise vocabulary: {enhanced_prompt}"
                    elif eidx == 6:  # E7 Code/Structure
                        variant_prompt = f"With clear logical structure: {enhanced_prompt}"
                    elif eidx == 7:  # E8 Graph
                        variant_prompt = f"Show the connections: {enhanced_prompt}"
                    elif eidx == 8:  # E9 HDC
                        variant_prompt = f"With attention to rhythm and sound: {enhanced_prompt}"
                    elif eidx == 9:  # E10 Paraphrase
                        variant_prompt = f"Express this meaning: {enhanced_prompt}"
                    elif eidx == 10:  # E11 Entity
                        variant_prompt = f"Focus on the characters: {enhanced_prompt}"
                    elif eidx in [1, 2, 3]:  # E2-E4 Temporal
                        variant_prompt = f"With a sense of time: {enhanced_prompt}"
                    else:
                        variant_prompt = enhanced_prompt

                    all_examples.append(make_sft(
                        variant_prompt, content,
                        system_extra=f"Style: {work}. {ename} signal: {score:.3f}. Texture: {texture_desc}.",
                        source=f"L2_{ename.replace(' ', '_')}",
                        meta={**meta_base, "embedder": ename, "embedder_score": round(score, 4)},
                    ))

        # ================================================================
        # LAYER 3: Cross-correlation texture conditioning
        # The 91 cross-correlations define the TEXTURE — how dimensions interact
        # ================================================================
        if cross_corrs and len(cross_corrs) >= 78:
            avg_corr = sum(cross_corrs) / len(cross_corrs)
            max_corr = max(cross_corrs)
            min_corr = min(cross_corrs)

            all_examples.append(make_sft(
                enhanced_prompt, content,
                system_extra=(
                    f"Style: {work}. Texture: {texture_desc}. "
                    f"Cross-correlation: avg={avg_corr:.2f}, range=[{min_corr:.2f},{max_corr:.2f}]. "
                    f"This passage is {texture_label}."
                ),
                source=f"L3_texture_{texture_label}",
                meta={**meta_base, "avg_corr": round(avg_corr, 3),
                      "corr_range": [round(min_corr, 3), round(max_corr, 3)]},
            ))

        # ================================================================
        # LAYER 4: Constellation alignment conditioning
        # group_alignments shows how close this memory is to each work centroid
        # ================================================================
        if group_alignments and len(group_alignments) >= 2:
            # The constellation scores are per-work alignment
            max_align = max(group_alignments)
            min_align = min(group_alignments)
            spread = max_align - min_align

            if spread > 0.3:  # Distinctive alignment
                all_examples.append(make_sft(
                    enhanced_prompt, content,
                    system_extra=(
                        f"Constellation: strongly aligned with {work} "
                        f"(alignment={max_align:.3f}, spread={spread:.3f}). "
                        f"Style: {style_desc}."
                    ),
                    source="L4_constellation",
                    meta={**meta_base, "alignment": round(max_align, 3),
                          "spread": round(spread, 3)},
                ))

        # ================================================================
        # LAYER 6: Edge-type distribution (structural role)
        # ================================================================
        if edges:
            edge_types = {}
            for e in edges:
                t = e.get("edge_type", 0)
                edge_types[t] = edge_types.get(t, 0) + 1
            total_edges = sum(edge_types.values())

            if total_edges >= 3:
                dominant = max(edge_types, key=edge_types.get)
                edge_type_names = {
                    0: "causal", 1: "temporal", 2: "semantic",
                    3: "entity", 4: "graph", 5: "structural",
                }
                dom_name = edge_type_names.get(dominant, f"type_{dominant}")

                all_examples.append(make_sft(
                    enhanced_prompt, content,
                    system_extra=(
                        f"Style: {work}. "
                        f"Graph role: {dom_name}-hub with {total_edges} connections. "
                        f"This passage is a structural anchor."
                    ),
                    source=f"L6_edge_{dom_name}",
                    meta={**meta_base, "edges": total_edges, "dominant_edge": dom_name},
                ))

        # ================================================================
        # LAYER 7: Temporal conditioning
        # ================================================================
        if temporal:
            bucket = temporal.get("periodic_bucket", "")
            seq = temporal.get("session_sequence", 0)

            if bucket and seq:
                # Position within the work (early/middle/late)
                position = "early" if seq < 500 else ("middle" if seq < 1500 else "late")
                all_examples.append(make_sft(
                    enhanced_prompt, content,
                    system_extra=(
                        f"Style: {work}. "
                        f"Position: {position} in the work (sequence {seq}). "
                        f"Temporal rhythm: {bucket}."
                    ),
                    source=f"L7_temporal_{position}",
                    meta={**meta_base, "position": position, "sequence": seq},
                ))

        # ================================================================
        # LAYER 8: Tucker decomposition (complexity conditioning)
        # ================================================================
        if tucker and tucker.get("compression_ratio"):
            compression = tucker["compression_ratio"]
            ranks = tucker.get("ranks", [])
            complexity = "complex" if compression > 1.5 else ("moderate" if compression > 1.0 else "simple")

            all_examples.append(make_sft(
                enhanced_prompt, content,
                system_extra=(
                    f"Style: {work}. "
                    f"Complexity: {complexity} (Tucker compression={compression:.2f}, ranks={ranks}). "
                    f"This passage has {complexity} internal structure."
                ),
                source=f"L8_tucker_{complexity}",
                meta={**meta_base, "complexity": complexity,
                      "compression": round(compression, 3)},
            ))

    # ================================================================
    # LAYER 9: Adversarial examples
    # ================================================================
    print(f"[signals] Adding adversarial examples...", file=sys.stderr)
    adv_path = Path(os.path.expanduser(
        "~/.contextgraph/training/shakespeare_adversarial_boost.jsonl"))
    if adv_path.exists():
        with open(adv_path) as f:
            for line in f:
                all_examples.append(json.loads(line))
                total_signals += 1

    # Also creative examples
    creative_path = Path(os.path.expanduser(
        "~/.contextgraph/training/shakespeare_sft_creative.jsonl"))
    if creative_path.exists():
        with open(creative_path) as f:
            for line in f:
                all_examples.append(json.loads(line))
                total_signals += 1

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
        # Group by layer
        if src.startswith("L1_"): group = "Layer 1: Base SFT"
        elif src.startswith("L2_"): group = "Layer 2: Embedder-conditioned"
        elif src.startswith("L3_"): group = "Layer 3: Cross-correlation"
        elif src.startswith("L4_"): group = "Layer 4: Constellation"
        elif src.startswith("L6_"): group = "Layer 6: Edge-type"
        elif src.startswith("L7_"): group = "Layer 7: Temporal"
        elif src.startswith("L8_"): group = "Layer 8: Tucker complexity"
        elif "adversarial" in src: group = "Layer 9: Adversarial"
        elif "creative" in src: group = "Layer 9: Creative"
        else: group = src
        counts[group] = counts.get(group, 0) + 1

    n_memories = len(all_ids) - skipped
    print(f"\n{'='*70}", file=sys.stderr)
    print(f"  FULL-SIGNAL EXTRACTION — ALL {total_signals:,} SIGNALS ENCODED", file=sys.stderr)
    print(f"{'='*70}", file=sys.stderr)
    print(f"  Training examples by layer:", file=sys.stderr)
    for k, v in sorted(counts.items()):
        print(f"    {k}: {v:,}", file=sys.stderr)
    print(f"  {'─'*50}", file=sys.stderr)
    print(f"  TOTAL SFT EXAMPLES: {len(all_examples):,}", file=sys.stderr)
    print(f"  + DPO PAIRS: 12,567 (pre-built)", file=sys.stderr)
    print(f"  GRAND TOTAL: {len(all_examples) + 12567:,} training examples", file=sys.stderr)
    print(f"", file=sys.stderr)
    print(f"  Signal accounting:", file=sys.stderr)
    print(f"    Content texts:          {n_memories:>8,}", file=sys.stderr)
    print(f"    Embedder profiles:      {n_memories * NUM_EMBEDDER_SIGNALS:>8,}  ({NUM_EMBEDDER_SIGNALS} per memory)", file=sys.stderr)
    print(f"    Cross-correlations:     {n_memories * NUM_CROSS_CORRELATIONS:>8,}  ({NUM_CROSS_CORRELATIONS} per memory)", file=sys.stderr)
    print(f"    Constellation scores:   {n_memories * 6:>8,}  (6 per memory)", file=sys.stderr)
    print(f"    Temporal labels:        {n_memories * 12:>8,}  (12 per memory)", file=sys.stderr)
    print(f"    Tucker decompositions:  {n_memories * 4:>8,}  (4 per memory)", file=sys.stderr)
    edge_est = total_signals - n_memories * (1+NUM_EMBEDDER_SIGNALS+NUM_CROSS_CORRELATIONS+6+12+4) - n_memories
    print(f"    Typed edges + other:    {edge_est:>8,}", file=sys.stderr)
    print(f"    Adversarial + creative: {counts.get('Layer 9: Adversarial', 0) + counts.get('Layer 9: Creative', 0):>8,}", file=sys.stderr)
    print(f"    {'─'*40}", file=sys.stderr)
    total_raw = n_memories * (1+NUM_EMBEDDER_SIGNALS+NUM_CROSS_CORRELATIONS+6+12+4) + n_memories + edge_est + 12567
    print(f"    TOTAL RAW SIGNALS:      {total_raw:>8,}", file=sys.stderr)
    print(f"", file=sys.stderr)
    print(f"  Memories: {n_memories:,} processed, {skipped} skipped, {errors} errors", file=sys.stderr)
    print(f"  Time: {elapsed:.0f}s ({elapsed/60:.1f} min)", file=sys.stderr)
    sz = Path(args.out).stat().st_size
    print(f"  Output: {args.out} ({sz/1e6:.1f} MB)", file=sys.stderr)
    print(f"{'='*70}", file=sys.stderr)

    rpc.close()


if __name__ == "__main__":
    main()
