#!/usr/bin/env python3
"""
Prepare multi-dataset benchmark from HuggingFace.

Downloads diverse datasets from HuggingFace, chunks the text,
and saves in a format ready for the Rust benchmark loader.

Datasets:
- arxiv-classification: Scientific paper abstracts with categories
- code_search_net: Code docstrings with language/repo context
- stackoverflow-questions: Technical Q&A with tags
- wikipedia: General knowledge articles

Usage:
    HF_TOKEN=hf_xxx python prepare_huggingface.py --output /path/to/output --max-chunks 20000

Environment:
    HF_TOKEN: HuggingFace API token for authenticated access
"""

import argparse
import hashlib
import json
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterator, List, Optional, Tuple

# Check for required packages
try:
    from datasets import load_dataset
    from huggingface_hub import login
    from tqdm import tqdm
except ImportError as e:
    print(f"Error: Missing required package - {e}")
    print("Install with: pip install datasets huggingface_hub tqdm")
    sys.exit(1)


@dataclass
class DatasetConfig:
    """Configuration for a single HuggingFace dataset."""

    name: str
    hf_path: str
    hf_subset: Optional[str] = None
    split: str = "train"
    text_field: str = "text"
    title_field: Optional[str] = None
    topic_field: Optional[str] = None
    max_docs: int = 5000
    min_words: int = 50
    trust_remote_code: bool = False
    # Field mapping for normalization
    field_transforms: Dict[str, str] = field(default_factory=dict)


# Dataset configurations
DATASET_CONFIGS = [
    DatasetConfig(
        name="arxiv",
        hf_path="ccdv/arxiv-classification",
        text_field="text",
        title_field="title",
        topic_field="label",  # Category labels like cs.AI, math.ST
        max_docs=5000,
        min_words=50,
    ),
    DatasetConfig(
        name="code",
        hf_path="code_search_net",
        hf_subset="python",
        text_field="func_documentation_string",
        title_field="func_name",
        topic_field="language",  # Always "python" for this subset
        max_docs=5000,
        min_words=20,
        trust_remote_code=True,
    ),
    DatasetConfig(
        name="stackoverflow",
        hf_path="pacovaldez/stackoverflow-questions",
        text_field="body",
        title_field="title",
        topic_field="tags",  # Comma-separated tags
        max_docs=5000,
        min_words=30,
    ),
    DatasetConfig(
        name="wikipedia",
        hf_path="wikimedia/wikipedia",
        hf_subset="20231101.en",
        text_field="text",
        title_field="title",
        topic_field=None,  # Will extract from title
        max_docs=5000,
        min_words=100,
        trust_remote_code=True,
    ),
]


def chunk_text(
    text: str, chunk_size: int = 200, overlap: int = 50
) -> List[Tuple[str, int, int]]:
    """
    Chunk text into overlapping segments of approximately chunk_size words.

    Returns list of (chunk_text, start_word_idx, end_word_idx) tuples.
    """
    words = text.split()
    if len(words) <= chunk_size:
        return [(text, 0, len(words))]

    chunks = []
    start = 0
    while start < len(words):
        end = min(start + chunk_size, len(words))
        chunk_words = words[start:end]
        chunk_text = " ".join(chunk_words)
        chunks.append((chunk_text, start, end))

        # Move forward by (chunk_size - overlap) words
        start += chunk_size - overlap

        # Don't create tiny final chunks
        if len(words) - start < overlap:
            break

    return chunks


def generate_chunk_id(doc_id: str, chunk_idx: int, source: str) -> str:
    """Generate a deterministic UUID-like ID for a chunk."""
    content = f"{source}:{doc_id}:{chunk_idx}"
    hash_bytes = hashlib.sha256(content.encode()).digest()[:16]
    # Format as UUID
    return f"{hash_bytes[:4].hex()}-{hash_bytes[4:6].hex()}-{hash_bytes[6:8].hex()}-{hash_bytes[8:10].hex()}-{hash_bytes[10:16].hex()}"


def extract_topic(config: DatasetConfig, doc: dict) -> str:
    """Extract topic/category from document based on config."""
    if config.topic_field and config.topic_field in doc:
        topic = doc[config.topic_field]
        # Handle different formats
        if isinstance(topic, list):
            return topic[0] if topic else "unknown"
        if isinstance(topic, str):
            # For comma-separated tags (stackoverflow), take first
            if "," in topic:
                return topic.split(",")[0].strip().lower()
            # For labels like "cs.AI", normalize
            return topic.lower().replace(".", "_")
        if isinstance(topic, int):
            # Numeric label - convert to string
            return f"label_{topic}"
        return str(topic)

    # Fallback: extract from title
    if config.title_field and config.title_field in doc:
        title = doc.get(config.title_field, "")
        if title:
            # Use first word of title as rough topic
            return title.split()[0].lower()[:20]

    return f"{config.name}_general"


def process_dataset(
    config: DatasetConfig,
    output_chunks: List[dict],
    topic_counts: Dict[str, int],
    pbar: tqdm,
    chunk_size: int = 200,
    overlap: int = 50,
) -> Dict[str, int]:
    """Process a single dataset and append chunks to output list."""
    stats = {
        "documents": 0,
        "chunks": 0,
        "words": 0,
        "skipped_short": 0,
        "skipped_empty": 0,
    }

    try:
        # Load dataset with streaming
        load_kwargs = {
            "path": config.hf_path,
            "split": config.split,
            "streaming": True,
            "trust_remote_code": config.trust_remote_code,
        }
        if config.hf_subset:
            load_kwargs["name"] = config.hf_subset

        dataset = load_dataset(**load_kwargs)

    except Exception as e:
        print(f"\n  Warning: Failed to load {config.name}: {e}")
        return stats

    for i, doc in enumerate(dataset):
        if i >= config.max_docs:
            break

        pbar.update(1)
        pbar.set_description(f"Processing {config.name}")

        # Get text content
        text = doc.get(config.text_field, "")
        if not text or not isinstance(text, str):
            stats["skipped_empty"] += 1
            continue

        # Clean text
        text = text.strip()
        words = text.split()

        if len(words) < config.min_words:
            stats["skipped_short"] += 1
            continue

        # Get metadata
        title = doc.get(config.title_field, f"{config.name}_{i}") or f"{config.name}_{i}"
        doc_id = f"{config.name}_{i}"
        topic = extract_topic(config, doc)

        # Track topic
        topic_counts[topic] = topic_counts.get(topic, 0) + 1

        # Chunk the document
        chunks = chunk_text(text, chunk_size, overlap)

        for chunk_idx, (chunk_text_content, start_word, end_word) in enumerate(chunks):
            chunk_id = generate_chunk_id(doc_id, chunk_idx, config.name)

            chunk_record = {
                "id": chunk_id,
                "doc_id": doc_id,
                "title": str(title)[:200],  # Truncate long titles
                "chunk_idx": chunk_idx,
                "text": chunk_text_content,
                "word_count": len(chunk_text_content.split()),
                "start_word": start_word,
                "end_word": end_word,
                "topic_hint": topic,
                "source_dataset": config.name,
            }

            output_chunks.append(chunk_record)
            stats["chunks"] += 1
            stats["words"] += len(chunk_text_content.split())

        stats["documents"] += 1

    return stats


def save_checkpoint(
    output_dir: Path,
    chunks: List[dict],
    metadata: dict,
    checkpoint_name: str = "checkpoint",
):
    """Save checkpoint for resume capability."""
    checkpoint_file = output_dir / f"{checkpoint_name}.jsonl"
    metadata_file = output_dir / f"{checkpoint_name}_meta.json"

    with open(checkpoint_file, "w") as f:
        for chunk in chunks:
            f.write(json.dumps(chunk) + "\n")

    with open(metadata_file, "w") as f:
        json.dump(metadata, f, indent=2)


def load_checkpoint(output_dir: Path) -> Tuple[List[dict], dict, set]:
    """Load checkpoint if exists. Returns (chunks, metadata, processed_datasets)."""
    checkpoint_file = output_dir / "checkpoint.jsonl"
    metadata_file = output_dir / "checkpoint_meta.json"

    if not checkpoint_file.exists() or not metadata_file.exists():
        return [], {}, set()

    chunks = []
    with open(checkpoint_file) as f:
        for line in f:
            chunks.append(json.loads(line))

    with open(metadata_file) as f:
        metadata = json.load(f)

    processed = set(metadata.get("processed_datasets", []))
    return chunks, metadata, processed


def prepare_huggingface_benchmark(
    output_dir: Path,
    max_chunks: int = 20000,
    chunk_size: int = 200,
    overlap: int = 50,
    datasets: Optional[List[str]] = None,
    resume: bool = True,
) -> dict:
    """
    Download and process multiple HuggingFace datasets.

    Returns statistics about the processed data.
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Try HuggingFace login
    hf_token = os.environ.get("HF_TOKEN")
    if hf_token:
        try:
            login(token=hf_token)
            print("✓ Logged in to HuggingFace")
        except Exception as e:
            print(f"Warning: HuggingFace login failed: {e}")
    else:
        print("Note: No HF_TOKEN found. Some datasets may be restricted.")

    # Filter configs if specific datasets requested
    configs = DATASET_CONFIGS
    if datasets:
        configs = [c for c in configs if c.name in datasets]
        if not configs:
            print(f"Error: No matching datasets found for: {datasets}")
            sys.exit(1)

    # Load checkpoint if resuming
    all_chunks = []
    topic_counts: Dict[str, int] = {}
    processed_datasets: set = set()

    if resume:
        all_chunks, checkpoint_meta, processed_datasets = load_checkpoint(output_dir)
        if all_chunks:
            print(f"✓ Resuming from checkpoint: {len(all_chunks)} chunks")
            topic_counts = checkpoint_meta.get("topic_counts", {})

    # Calculate per-dataset limits
    remaining_configs = [c for c in configs if c.name not in processed_datasets]
    if not remaining_configs:
        print("All datasets already processed!")
    else:
        chunks_remaining = max_chunks - len(all_chunks)
        per_dataset_limit = chunks_remaining // len(remaining_configs)

        # Adjust max_docs based on average chunks per doc (~2-3)
        for config in remaining_configs:
            config.max_docs = min(config.max_docs, per_dataset_limit // 2)

    # Overall statistics
    stats = {
        "total_documents": 0,
        "total_chunks": len(all_chunks),
        "total_words": sum(c.get("word_count", 0) for c in all_chunks),
        "chunk_size": chunk_size,
        "overlap": overlap,
        "source_datasets": [],
        "dataset_stats": {},
        "topic_counts": topic_counts,
        "top_topics": [],
    }

    # Calculate total docs for progress bar
    total_docs = sum(c.max_docs for c in remaining_configs)

    print(f"\nProcessing {len(remaining_configs)} datasets...")
    print(f"  Target: {max_chunks} total chunks")
    print(f"  Chunk size: {chunk_size} words, overlap: {overlap}")
    print()

    with tqdm(total=total_docs, desc="Downloading", unit="docs") as pbar:
        for config in remaining_configs:
            if len(all_chunks) >= max_chunks:
                break

            dataset_stats = process_dataset(
                config=config,
                output_chunks=all_chunks,
                topic_counts=topic_counts,
                pbar=pbar,
                chunk_size=chunk_size,
                overlap=overlap,
            )

            stats["dataset_stats"][config.name] = dataset_stats
            stats["source_datasets"].append(config.name)
            stats["total_documents"] += dataset_stats["documents"]
            stats["total_chunks"] = len(all_chunks)
            stats["total_words"] += dataset_stats["words"]

            processed_datasets.add(config.name)

            # Save checkpoint after each dataset
            checkpoint_meta = {
                "processed_datasets": list(processed_datasets),
                "topic_counts": topic_counts,
            }
            save_checkpoint(output_dir, all_chunks, checkpoint_meta)

            print(f"\n  ✓ {config.name}: {dataset_stats['documents']} docs, {dataset_stats['chunks']} chunks")

    # Compute top topics
    sorted_topics = sorted(topic_counts.items(), key=lambda x: -x[1])[:100]
    stats["top_topics"] = [t[0] for t in sorted_topics]
    stats["topic_counts"] = dict(sorted_topics)

    # Save final output
    chunks_file = output_dir / "chunks.jsonl"
    metadata_file = output_dir / "metadata.json"

    print(f"\nWriting {len(all_chunks)} chunks to {chunks_file}...")
    with open(chunks_file, "w") as f:
        for chunk in all_chunks:
            f.write(json.dumps(chunk) + "\n")

    with open(metadata_file, "w") as f:
        json.dump(stats, f, indent=2)

    # Clean up checkpoint
    checkpoint_file = output_dir / "checkpoint.jsonl"
    checkpoint_meta = output_dir / "checkpoint_meta.json"
    if checkpoint_file.exists():
        checkpoint_file.unlink()
    if checkpoint_meta.exists():
        checkpoint_meta.unlink()

    # Print summary
    print("\n" + "=" * 60)
    print("Processing Complete!")
    print("=" * 60)
    print(f"  Total documents: {stats['total_documents']:,}")
    print(f"  Total chunks: {stats['total_chunks']:,}")
    print(f"  Total words: {stats['total_words']:,}")
    print(f"  Avg words/chunk: {stats['total_words'] / max(1, stats['total_chunks']):.1f}")
    print(f"  Unique topics: {len(topic_counts)}")
    print()
    print("Dataset breakdown:")
    for name, ds_stats in stats["dataset_stats"].items():
        print(f"  {name}: {ds_stats['documents']:,} docs, {ds_stats['chunks']:,} chunks")
    print()
    print(f"Top 10 topics:")
    for topic, count in sorted_topics[:10]:
        print(f"  {topic}: {count}")
    print()
    print(f"Output files:")
    print(f"  Chunks: {chunks_file}")
    print(f"  Metadata: {metadata_file}")

    return stats


def main():
    parser = argparse.ArgumentParser(
        description="Prepare multi-dataset HuggingFace benchmark"
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        default="./data/hf_benchmark",
        help="Output directory for processed data",
    )
    parser.add_argument(
        "--max-chunks",
        "-n",
        type=int,
        default=20000,
        help="Maximum total chunks to generate (default: 20000)",
    )
    parser.add_argument(
        "--chunk-size",
        type=int,
        default=200,
        help="Target chunk size in words (default: 200)",
    )
    parser.add_argument(
        "--overlap",
        type=int,
        default=50,
        help="Overlap between chunks in words (default: 50)",
    )
    parser.add_argument(
        "--datasets",
        type=str,
        nargs="+",
        choices=["arxiv", "code", "stackoverflow", "wikipedia"],
        help="Specific datasets to process (default: all)",
    )
    parser.add_argument(
        "--no-resume",
        action="store_true",
        help="Don't resume from checkpoint, start fresh",
    )
    parser.add_argument(
        "--list-datasets",
        action="store_true",
        help="List available datasets and exit",
    )

    args = parser.parse_args()

    if args.list_datasets:
        print("Available datasets:")
        for config in DATASET_CONFIGS:
            subset = f" ({config.hf_subset})" if config.hf_subset else ""
            print(f"  {config.name}: {config.hf_path}{subset}")
            print(f"    Text field: {config.text_field}")
            print(f"    Topic field: {config.topic_field or 'derived from title'}")
            print(f"    Max docs: {config.max_docs}")
            print()
        return

    prepare_huggingface_benchmark(
        output_dir=args.output,
        max_chunks=args.max_chunks,
        chunk_size=args.chunk_size,
        overlap=args.overlap,
        datasets=args.datasets,
        resume=not args.no_resume,
    )


if __name__ == "__main__":
    main()
