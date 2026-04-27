// FSV Test: Verify TextChunker line number tracking
use context_graph_core::memory::TextChunker;

fn main() {
    println!("=== FSV CHUNKER LINE NUMBER TEST ===\n");

    // Test Case 1: Content with clear line boundaries
    let chunker = TextChunker::new(50, 10).expect("valid config");

    // Create content: 20 lines, 5 words each = 100 words
    let mut lines = Vec::new();
    for i in 1..=20 {
        lines.push(format!("Line{} word1 word2 word3 word4", i));
    }
    let content = lines.join("\n");

    println!("INPUT:");
    println!("  Total lines: 20");
    println!("  Words per line: 5");
    println!("  Total words: {}", content.split_whitespace().count());
    println!("  Chunk size: 50 words, Overlap: 10 words\n");

    let chunks = chunker.chunk_text(&content, "test.md").expect("chunk");

    println!("OUTPUT:");
    println!("  Total chunks: {}\n", chunks.len());

    for (i, chunk) in chunks.iter().enumerate() {
        let first_word = chunk.content.split_whitespace().next().unwrap_or("?");
        let last_word = chunk.content.split_whitespace().last().unwrap_or("?");
        println!("  Chunk[{}]:", i);
        println!("    start_line: {}", chunk.metadata.start_line);
        println!("    end_line: {}", chunk.metadata.end_line);
        println!("    word_count: {}", chunk.word_count);
        println!("    first_word: {}", first_word);
        println!("    last_word: {}", last_word);
        println!();
    }

    // Verification
    println!("VERIFICATION:");
    let mut all_pass = true;

    // Check 1: First chunk starts at line 1
    if chunks[0].metadata.start_line != 1 {
        println!(
            "  FAIL: First chunk should start at line 1, got {}",
            chunks[0].metadata.start_line
        );
        all_pass = false;
    } else {
        println!("  PASS: First chunk starts at line 1");
    }

    // Check 2: All chunks have start_line <= end_line
    for (i, chunk) in chunks.iter().enumerate() {
        if chunk.metadata.start_line > chunk.metadata.end_line {
            println!(
                "  FAIL: Chunk[{}] start_line ({}) > end_line ({})",
                i, chunk.metadata.start_line, chunk.metadata.end_line
            );
            all_pass = false;
        }
    }
    if all_pass {
        println!("  PASS: All chunks have valid line ranges (start <= end)");
    }

    // Check 3: Last chunk should end near line 20
    let last_chunk = chunks.last().unwrap();
    if last_chunk.metadata.end_line < 18 {
        println!(
            "  FAIL: Last chunk should end near line 20, got {}",
            last_chunk.metadata.end_line
        );
        all_pass = false;
    } else {
        println!(
            "  PASS: Last chunk ends at line {} (near 20)",
            last_chunk.metadata.end_line
        );
    }

    // Check 4: Line numbers increase across chunks
    let mut prev_start = 0;
    let mut line_increase_ok = true;
    for (i, chunk) in chunks.iter().enumerate() {
        if i > 0 && chunk.metadata.start_line < prev_start {
            println!(
                "  FAIL: Chunk[{}] start_line ({}) < prev start ({})",
                i, chunk.metadata.start_line, prev_start
            );
            line_increase_ok = false;
            all_pass = false;
        }
        prev_start = chunk.metadata.start_line;
    }
    if line_increase_ok {
        println!("  PASS: Line numbers increase across chunks");
    }

    println!(
        "\n=== FSV RESULT: {} ===",
        if all_pass { "PASSED" } else { "FAILED" }
    );

    // Edge case tests
    println!("\n=== EDGE CASE TESTS ===\n");

    // Edge 1: Single line content
    println!("Edge Case 1: Single line content");
    let single_line = "This is a single line of content with several words.";
    let chunks1 = chunker.chunk_text(single_line, "single.md").expect("chunk");
    println!(
        "  Input: {} words on 1 line",
        single_line.split_whitespace().count()
    );
    println!(
        "  Output: start_line={}, end_line={}",
        chunks1[0].metadata.start_line, chunks1[0].metadata.end_line
    );
    if chunks1[0].metadata.start_line == 1 && chunks1[0].metadata.end_line == 1 {
        println!("  PASS: Single line has lines 1-1");
    } else {
        println!("  FAIL: Expected lines 1-1");
    }

    // Edge 2: Empty lines (newlines only between content)
    println!("\nEdge Case 2: Content with empty lines");
    let with_empty = "First line content.\n\n\nFourth line content.";
    let chunks2 = chunker.chunk_text(with_empty, "empty.md").expect("chunk");
    println!("  Input: Content on lines 1 and 4 (empty lines 2-3)");
    println!(
        "  Output: start_line={}, end_line={}",
        chunks2[0].metadata.start_line, chunks2[0].metadata.end_line
    );
    // The end_line should be 4 since content spans to line 4
    if chunks2[0].metadata.end_line >= 4 {
        println!("  PASS: End line includes line 4");
    } else {
        println!(
            "  WARN: End line is {}, expected >= 4",
            chunks2[0].metadata.end_line
        );
    }

    // Edge 3: Very long single line
    println!("\nEdge Case 3: Long single line (100 words)");
    let long_words: Vec<String> = (0..100).map(|i| format!("word{}", i)).collect();
    let long_line = long_words.join(" ");
    let chunks3 = chunker.chunk_text(&long_line, "long.md").expect("chunk");
    println!(
        "  Input: {} words on 1 line",
        long_line.split_whitespace().count()
    );
    println!("  Output: {} chunks", chunks3.len());
    for (i, c) in chunks3.iter().enumerate() {
        println!(
            "    Chunk[{}]: lines {}-{}, words={}",
            i, c.metadata.start_line, c.metadata.end_line, c.word_count
        );
    }
    // All chunks should have start_line=1 and end_line=1 since it's all on line 1
    let all_line_1 = chunks3
        .iter()
        .all(|c| c.metadata.start_line == 1 && c.metadata.end_line == 1);
    if all_line_1 {
        println!("  PASS: All chunks correctly report lines 1-1");
    } else {
        println!("  FAIL: Expected all chunks to be on line 1");
    }
}
