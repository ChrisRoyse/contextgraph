#!/usr/bin/env python3
"""
Convert KEPLER (fairseq) checkpoint to safetensors format.

KEPLER: A Knowledge-Enhanced Pre-trained Language Representation for NLP
Paper: https://arxiv.org/abs/1911.06136
Model: RoBERTa-base + TransE training on Wikidata5M

The fairseq checkpoint uses different naming conventions than HuggingFace.
This script converts the weights to our Candle-compatible safetensors format.

Usage:
    # 1. Download KEPLER checkpoint from Tsinghua Cloud
    wget https://cloud.tsinghua.edu.cn/f/bc31e9c3bab545d58dd5/?dl=1 -O kepler_ke.tar.gz
    tar -xzf kepler_ke.tar.gz

    # 2. Run conversion
    python scripts/convert_kepler.py --input kepler_ke/ --output models/kepler/
"""

import argparse
import json
import shutil
from pathlib import Path
from typing import Dict, Any

import torch
from safetensors.torch import save_file
from transformers import RobertaTokenizer


# KEPLER (RoBERTa-base) configuration
KEPLER_CONFIG = {
    "vocab_size": 50265,
    "hidden_size": 768,
    "num_hidden_layers": 12,
    "num_attention_heads": 12,
    "intermediate_size": 3072,
    "hidden_act": "gelu",
    "hidden_dropout_prob": 0.1,
    "attention_probs_dropout_prob": 0.1,
    "max_position_embeddings": 514,  # RoBERTa uses 514 (512 + 2 special tokens)
    "type_vocab_size": 1,  # RoBERTa doesn't use token_type_ids
    "layer_norm_eps": 1e-5,
    "pad_token_id": 1,
    "model_type": "roberta",
    "architectures": ["RobertaModel"]
}


def map_fairseq_key(key: str) -> str | None:
    """Map fairseq key to HuggingFace/Candle naming convention.

    Fairseq format:
        encoder.sentence_encoder.embed_tokens.weight
        encoder.sentence_encoder.embed_positions.weight
        encoder.sentence_encoder.layers.0.self_attn.k_proj.weight
        encoder.sentence_encoder.layers.0.self_attn.v_proj.weight
        encoder.sentence_encoder.layers.0.self_attn.q_proj.weight
        encoder.sentence_encoder.layers.0.self_attn.out_proj.weight
        encoder.sentence_encoder.layers.0.self_attn_layer_norm.weight
        encoder.sentence_encoder.layers.0.fc1.weight
        encoder.sentence_encoder.layers.0.fc2.weight
        encoder.sentence_encoder.layers.0.final_layer_norm.weight
        encoder.sentence_encoder.emb_layer_norm.weight

    Our format (HuggingFace-compatible):
        embeddings.word_embeddings.weight
        embeddings.position_embeddings.weight
        embeddings.LayerNorm.weight/bias
        encoder.layer.0.attention.self.query.weight/bias
        encoder.layer.0.attention.self.key.weight/bias
        encoder.layer.0.attention.self.value.weight/bias
        encoder.layer.0.attention.output.dense.weight/bias
        encoder.layer.0.attention.output.LayerNorm.weight/bias
        encoder.layer.0.intermediate.dense.weight/bias
        encoder.layer.0.output.dense.weight/bias
        encoder.layer.0.output.LayerNorm.weight/bias
    """
    # Skip non-encoder weights (LM head, etc.)
    if not key.startswith("encoder.sentence_encoder."):
        return None

    # Remove prefix
    key = key.replace("encoder.sentence_encoder.", "")

    # Embeddings
    if key == "embed_tokens.weight":
        return "embeddings.word_embeddings.weight"
    if key == "embed_positions.weight":
        return "embeddings.position_embeddings.weight"
    if key == "emb_layer_norm.weight":
        return "embeddings.LayerNorm.weight"
    if key == "emb_layer_norm.bias":
        return "embeddings.LayerNorm.bias"

    # Encoder layers
    if key.startswith("layers."):
        parts = key.split(".")
        layer_idx = parts[1]
        rest = ".".join(parts[2:])

        # Self-attention
        if rest == "self_attn.q_proj.weight":
            return f"encoder.layer.{layer_idx}.attention.self.query.weight"
        if rest == "self_attn.q_proj.bias":
            return f"encoder.layer.{layer_idx}.attention.self.query.bias"
        if rest == "self_attn.k_proj.weight":
            return f"encoder.layer.{layer_idx}.attention.self.key.weight"
        if rest == "self_attn.k_proj.bias":
            return f"encoder.layer.{layer_idx}.attention.self.key.bias"
        if rest == "self_attn.v_proj.weight":
            return f"encoder.layer.{layer_idx}.attention.self.value.weight"
        if rest == "self_attn.v_proj.bias":
            return f"encoder.layer.{layer_idx}.attention.self.value.bias"
        if rest == "self_attn.out_proj.weight":
            return f"encoder.layer.{layer_idx}.attention.output.dense.weight"
        if rest == "self_attn.out_proj.bias":
            return f"encoder.layer.{layer_idx}.attention.output.dense.bias"

        # Self-attention layer norm
        if rest == "self_attn_layer_norm.weight":
            return f"encoder.layer.{layer_idx}.attention.output.LayerNorm.weight"
        if rest == "self_attn_layer_norm.bias":
            return f"encoder.layer.{layer_idx}.attention.output.LayerNorm.bias"

        # Feed-forward
        if rest == "fc1.weight":
            return f"encoder.layer.{layer_idx}.intermediate.dense.weight"
        if rest == "fc1.bias":
            return f"encoder.layer.{layer_idx}.intermediate.dense.bias"
        if rest == "fc2.weight":
            return f"encoder.layer.{layer_idx}.output.dense.weight"
        if rest == "fc2.bias":
            return f"encoder.layer.{layer_idx}.output.dense.bias"

        # FFN layer norm
        if rest == "final_layer_norm.weight":
            return f"encoder.layer.{layer_idx}.output.LayerNorm.weight"
        if rest == "final_layer_norm.bias":
            return f"encoder.layer.{layer_idx}.output.LayerNorm.bias"

    return None


def convert_fairseq_checkpoint(
    checkpoint_path: Path,
    output_dir: Path,
    verbose: bool = True
) -> None:
    """Convert fairseq KEPLER checkpoint to safetensors."""

    output_dir.mkdir(parents=True, exist_ok=True)

    # Load fairseq checkpoint
    if verbose:
        print(f"Loading fairseq checkpoint from {checkpoint_path}")

    # fairseq checkpoints can be model.pt or checkpoint_best.pt
    model_file = checkpoint_path / "model.pt"
    if not model_file.exists():
        model_file = checkpoint_path / "checkpoint_best.pt"
    if not model_file.exists():
        raise FileNotFoundError(f"No model.pt or checkpoint_best.pt found in {checkpoint_path}")

    checkpoint = torch.load(model_file, map_location="cpu", weights_only=False)

    # Extract model state dict
    if "model" in checkpoint:
        state_dict = checkpoint["model"]
    else:
        state_dict = checkpoint

    if verbose:
        print(f"Found {len(state_dict)} keys in checkpoint")

    # Map keys and filter tensors
    mapped: Dict[str, torch.Tensor] = {}
    skipped = []

    for key, tensor in state_dict.items():
        new_key = map_fairseq_key(key)
        if new_key is None:
            skipped.append(key)
            continue

        # Convert to float32 if needed
        if tensor.dtype != torch.float32:
            tensor = tensor.float()

        mapped[new_key] = tensor
        if verbose:
            print(f"  {key} -> {new_key} {list(tensor.shape)}")

    if verbose:
        print(f"\nMapped {len(mapped)} weights, skipped {len(skipped)} weights")
        if skipped:
            print(f"Skipped keys: {skipped[:10]}...")

    # Validate we have expected weights
    expected_prefixes = [
        "embeddings.word_embeddings",
        "embeddings.position_embeddings",
        "embeddings.LayerNorm",
        "encoder.layer.0",
        "encoder.layer.11",  # Should have 12 layers (0-11)
    ]
    for prefix in expected_prefixes:
        if not any(k.startswith(prefix) for k in mapped):
            raise ValueError(f"Missing expected weight prefix: {prefix}")

    # RoBERTa doesn't have token_type_embeddings, but our loader expects them
    # Create dummy token_type_embeddings with a single embedding (all zeros or first word embedding)
    if "embeddings.token_type_embeddings.weight" not in mapped:
        hidden_size = KEPLER_CONFIG["hidden_size"]
        # Use zeros for dummy token type embeddings
        mapped["embeddings.token_type_embeddings.weight"] = torch.zeros(1, hidden_size)
        if verbose:
            print(f"Added dummy token_type_embeddings: [1, {hidden_size}]")

    # Save as safetensors
    safetensors_path = output_dir / "model.safetensors"
    if verbose:
        print(f"\nSaving to {safetensors_path}")
    save_file(mapped, safetensors_path)

    # Save config
    config_path = output_dir / "config.json"
    with open(config_path, "w") as f:
        json.dump(KEPLER_CONFIG, f, indent=2)
    if verbose:
        print(f"Saved config to {config_path}")

    # Copy or download tokenizer
    # KEPLER uses RoBERTa tokenizer (GPT-2 BPE)
    tokenizer_path = output_dir / "tokenizer.json"
    vocab_path = output_dir / "vocab.json"
    merges_path = output_dir / "merges.txt"

    if verbose:
        print("Downloading RoBERTa tokenizer...")

    try:
        tokenizer = RobertaTokenizer.from_pretrained("roberta-base")
        tokenizer.save_pretrained(str(output_dir))
        if verbose:
            print(f"Saved tokenizer to {output_dir}")
    except Exception as e:
        print(f"Warning: Could not download tokenizer: {e}")
        print("Please manually copy roberta-base tokenizer files to the output directory")

    # Compute checksums
    import hashlib
    sha_path = output_dir / "sha256.txt"
    with open(sha_path, "w") as f:
        for file in [safetensors_path, config_path]:
            if file.exists():
                sha = hashlib.sha256(file.read_bytes()).hexdigest()
                f.write(f"{sha}  {file.name}\n")

    if verbose:
        print(f"\nConversion complete!")
        print(f"Output directory: {output_dir}")
        print(f"Model size: {safetensors_path.stat().st_size / 1024 / 1024:.2f} MB")


def download_kepler(output_dir: Path, verbose: bool = True) -> Path:
    """Download KEPLER checkpoint from Tsinghua Cloud if not present."""
    import urllib.request
    import tarfile

    cache_dir = output_dir.parent / ".cache"
    cache_dir.mkdir(parents=True, exist_ok=True)

    tar_path = cache_dir / "kepler_ke.tar.gz"
    extract_dir = cache_dir / "kepler_ke"

    if not extract_dir.exists():
        if not tar_path.exists():
            url = "https://cloud.tsinghua.edu.cn/f/bc31e9c3bab545d58dd5/?dl=1"
            if verbose:
                print(f"Downloading KEPLER from {url}")
                print("This may take a few minutes...")

            # Download with progress
            def progress(count, block_size, total_size):
                percent = int(count * block_size * 100 / total_size)
                print(f"\r  Downloading: {percent}%", end="", flush=True)

            urllib.request.urlretrieve(url, tar_path, progress)
            print()  # newline after progress

        if verbose:
            print(f"Extracting {tar_path}")
        with tarfile.open(tar_path, "r:gz") as tar:
            tar.extractall(cache_dir)

    return extract_dir


def convert_from_huggingface(model_name: str, output_dir: Path, verbose: bool = True) -> None:
    """Alternative: Convert from HuggingFace if available."""
    from transformers import RobertaModel

    if verbose:
        print(f"Loading model from HuggingFace: {model_name}")

    model = RobertaModel.from_pretrained(model_name)
    state_dict = model.state_dict()

    # Already in correct format, just save
    output_dir.mkdir(parents=True, exist_ok=True)

    # Rename keys to match our format
    mapped = {}
    for key, tensor in state_dict.items():
        # Remove 'roberta.' prefix if present
        new_key = key.replace("roberta.", "")
        mapped[new_key] = tensor.float()

    safetensors_path = output_dir / "model.safetensors"
    save_file(mapped, safetensors_path)

    # Save config
    config_path = output_dir / "config.json"
    with open(config_path, "w") as f:
        json.dump(KEPLER_CONFIG, f, indent=2)

    # Save tokenizer
    tokenizer = RobertaTokenizer.from_pretrained(model_name)
    tokenizer.save_pretrained(str(output_dir))

    if verbose:
        print(f"Conversion complete! Output: {output_dir}")


def main():
    parser = argparse.ArgumentParser(
        description="Convert KEPLER checkpoint to safetensors"
    )
    parser.add_argument(
        "--input", "-i",
        type=Path,
        help="Path to fairseq KEPLER checkpoint directory (or 'download' to auto-download)"
    )
    parser.add_argument(
        "--output", "-o",
        type=Path,
        default=Path("/home/cabdru/contextgraph/models/kepler"),
        help="Output directory for converted model"
    )
    parser.add_argument(
        "--huggingface",
        type=str,
        help="HuggingFace model name (alternative to fairseq conversion)"
    )
    parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Suppress verbose output"
    )

    args = parser.parse_args()
    verbose = not args.quiet

    if args.huggingface:
        convert_from_huggingface(args.huggingface, args.output, verbose)
    elif args.input:
        if str(args.input).lower() == "download":
            checkpoint_path = download_kepler(args.output, verbose)
        else:
            checkpoint_path = args.input
        convert_fairseq_checkpoint(checkpoint_path, args.output, verbose)
    else:
        # Default: download and convert
        print("No input specified, downloading KEPLER checkpoint...")
        checkpoint_path = download_kepler(args.output, verbose)
        convert_fairseq_checkpoint(checkpoint_path, args.output, verbose)


if __name__ == "__main__":
    main()
