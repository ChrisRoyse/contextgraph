#!/bin/bash
#
# HuggingFace Multi-Dataset Benchmark Runner
#
# This script downloads datasets from HuggingFace, embeds them using the 13-embedder
# GPU pipeline, and runs comprehensive benchmarks to validate multi-space retrieval.
#
# Usage:
#   ./scripts/run_hf_benchmark.sh              # Full benchmark with GPU
#   ./scripts/run_hf_benchmark.sh --synthetic  # Test with synthetic embeddings
#   ./scripts/run_hf_benchmark.sh --download-only  # Only download datasets
#   ./scripts/run_hf_benchmark.sh --help       # Show help
#

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCHMARK_DIR="$PROJECT_ROOT/crates/context-graph-benchmark"
DATA_DIR="$PROJECT_ROOT/data/hf_benchmark"
OUTPUT_DIR="$PROJECT_ROOT/docs"

# Default settings
MAX_CHUNKS=20000
NUM_QUERIES=500
SYNTHETIC=false
DOWNLOAD_ONLY=false
SKIP_DOWNLOAD=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --synthetic)
            SYNTHETIC=true
            shift
            ;;
        --download-only)
            DOWNLOAD_ONLY=true
            shift
            ;;
        --skip-download)
            SKIP_DOWNLOAD=true
            shift
            ;;
        --max-chunks)
            MAX_CHUNKS="$2"
            shift 2
            ;;
        --num-queries)
            NUM_QUERIES="$2"
            shift 2
            ;;
        --data-dir)
            DATA_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "HuggingFace Multi-Dataset Benchmark Runner"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --synthetic       Use synthetic embeddings (no GPU required)"
            echo "  --download-only   Only download datasets, don't run benchmark"
            echo "  --skip-download   Skip download, use existing data"
            echo "  --max-chunks N    Maximum chunks to process (default: 20000)"
            echo "  --num-queries N   Number of query samples (default: 500)"
            echo "  --data-dir PATH   Data directory (default: data/hf_benchmark)"
            echo "  --help, -h        Show this help message"
            echo ""
            echo "Environment Variables:"
            echo "  HF_TOKEN          HuggingFace API token for authenticated access"
            echo ""
            echo "Examples:"
            echo "  # Full benchmark with GPU embeddings"
            echo "  HF_TOKEN=hf_xxx $0"
            echo ""
            echo "  # Test with synthetic embeddings"
            echo "  $0 --synthetic"
            echo ""
            echo "  # Download datasets only"
            echo "  HF_TOKEN=hf_xxx $0 --download-only"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Print header
echo -e "${BLUE}=======================================================================${NC}"
echo -e "${BLUE}  HuggingFace Multi-Dataset Benchmark${NC}"
echo -e "${BLUE}=======================================================================${NC}"
echo ""

# Check Python dependencies
check_python_deps() {
    echo -e "${YELLOW}Checking Python dependencies...${NC}"
    python3 -c "import datasets; import huggingface_hub; import tqdm" 2>/dev/null || {
        echo -e "${RED}Missing Python dependencies. Installing...${NC}"
        pip install datasets huggingface_hub tqdm
    }
    echo -e "${GREEN}Python dependencies OK${NC}"
}

# Step 1: Download datasets
download_datasets() {
    echo ""
    echo -e "${BLUE}=== Step 1: Downloading Datasets ===${NC}"
    echo ""

    if [[ "$SKIP_DOWNLOAD" == "true" ]]; then
        if [[ -f "$DATA_DIR/chunks.jsonl" ]]; then
            echo -e "${GREEN}Using existing data at $DATA_DIR${NC}"
            return 0
        else
            echo -e "${RED}Error: --skip-download specified but no data found at $DATA_DIR${NC}"
            exit 1
        fi
    fi

    check_python_deps

    # Check HF_TOKEN
    if [[ -z "$HF_TOKEN" ]]; then
        echo -e "${YELLOW}Warning: HF_TOKEN not set. Some datasets may be restricted.${NC}"
    fi

    echo "Running Python downloader..."
    echo "  Output: $DATA_DIR"
    echo "  Max chunks: $MAX_CHUNKS"
    echo ""

    cd "$BENCHMARK_DIR"
    HF_TOKEN="$HF_TOKEN" python3 scripts/prepare_huggingface.py \
        --output "$DATA_DIR" \
        --max-chunks "$MAX_CHUNKS"

    echo ""
    echo -e "${GREEN}Download complete!${NC}"
}

# Step 2: Verify data
verify_data() {
    echo ""
    echo -e "${BLUE}=== Step 2: Verifying Data ===${NC}"
    echo ""

    if [[ ! -f "$DATA_DIR/chunks.jsonl" ]]; then
        echo -e "${RED}Error: chunks.jsonl not found at $DATA_DIR${NC}"
        exit 1
    fi

    if [[ ! -f "$DATA_DIR/metadata.json" ]]; then
        echo -e "${RED}Error: metadata.json not found at $DATA_DIR${NC}"
        exit 1
    fi

    CHUNK_COUNT=$(wc -l < "$DATA_DIR/chunks.jsonl")
    echo "  Chunks: $CHUNK_COUNT"

    # Show topic distribution
    echo "  Topic distribution:"
    jq -r '.topic_hint' "$DATA_DIR/chunks.jsonl" 2>/dev/null | sort | uniq -c | sort -rn | head -10 || true

    echo ""
    echo -e "${GREEN}Data verification complete!${NC}"
}

# Step 3: Run benchmark
run_benchmark() {
    echo ""
    echo -e "${BLUE}=== Step 3: Running Benchmark ===${NC}"
    echo ""

    cd "$PROJECT_ROOT"

    # Build flags
    CARGO_FLAGS="--release -p context-graph-benchmark --bin hf-bench"
    BENCH_FLAGS="--data-dir $DATA_DIR --output $OUTPUT_DIR/hf-benchmark-results.json --num-queries $NUM_QUERIES"

    if [[ "$SYNTHETIC" == "true" ]]; then
        echo "Using SYNTHETIC embeddings (no GPU)"
        BENCH_FLAGS="$BENCH_FLAGS --synthetic"
    else
        echo "Using REAL GPU embeddings"
        CARGO_FLAGS="$CARGO_FLAGS --features real-embeddings"
    fi

    echo ""
    echo "Building benchmark..."
    cargo build $CARGO_FLAGS

    echo ""
    echo "Running benchmark..."
    cargo run $CARGO_FLAGS -- $BENCH_FLAGS

    echo ""
    echo -e "${GREEN}Benchmark complete!${NC}"
}

# Step 4: Generate report
generate_report() {
    echo ""
    echo -e "${BLUE}=== Step 4: Generating Report ===${NC}"
    echo ""

    if [[ -f "$OUTPUT_DIR/hf-benchmark-results.json" ]]; then
        echo "Results saved to: $OUTPUT_DIR/hf-benchmark-results.json"

        if [[ -f "$OUTPUT_DIR/hf-benchmark-results.md" ]]; then
            echo "Report saved to: $OUTPUT_DIR/hf-benchmark-results.md"
            echo ""
            echo -e "${YELLOW}Report Summary:${NC}"
            head -50 "$OUTPUT_DIR/hf-benchmark-results.md"
        fi
    else
        echo -e "${RED}No results found to generate report${NC}"
    fi
}

# Main execution
main() {
    # Step 1: Download
    download_datasets

    if [[ "$DOWNLOAD_ONLY" == "true" ]]; then
        verify_data
        echo ""
        echo -e "${GREEN}Download complete. Run again without --download-only to run benchmark.${NC}"
        exit 0
    fi

    # Step 2: Verify
    verify_data

    # Step 3: Benchmark
    run_benchmark

    # Step 4: Report
    generate_report

    echo ""
    echo -e "${GREEN}=======================================================================${NC}"
    echo -e "${GREEN}  Benchmark Complete!${NC}"
    echo -e "${GREEN}=======================================================================${NC}"
    echo ""
    echo "Output files:"
    echo "  Results: $OUTPUT_DIR/hf-benchmark-results.json"
    echo "  Report:  $OUTPUT_DIR/hf-benchmark-results.md"
}

main
