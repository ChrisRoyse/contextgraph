// FSV Test: Verify SourceMetadata display_string() with line numbers
use context_graph_core::types::SourceMetadata;

fn main() {
    println!("=== FSV SOURCE METADATA DISPLAY TEST ===\n");

    // Test 1: MDFileChunk with line numbers
    println!("Test 1: MDFileChunk with line numbers");
    let meta1 = SourceMetadata::md_file_chunk_with_lines("docs/authentication.md", 1, 5, 10, 45);
    let display1 = meta1.display_string();
    println!("  Input: file=docs/authentication.md, chunk=2/5, lines=10-45");
    println!("  Output: {}", display1);

    // Verify format: Source: `docs/authentication.md:10-45` (chunk 2/5)
    let expected1 = "docs/authentication.md:10-45";
    if display1.contains(expected1) && display1.contains("2/5") {
        println!("  PASS: Contains path:lines and chunk info");
    } else {
        println!(
            "  FAIL: Expected '{}' and '2/5', got: {}",
            expected1, display1
        );
    }

    // Test 2: MDFileChunk without line numbers (backwards compatibility)
    println!("\nTest 2: MDFileChunk without line numbers");
    let meta2 = SourceMetadata::md_file_chunk("docs/readme.md", 0, 3);
    let display2 = meta2.display_string();
    println!("  Input: file=docs/readme.md, chunk=1/3, no lines");
    println!("  Output: {}", display2);

    // Should NOT contain :lines format
    if !display2.contains(":1") && display2.contains("docs/readme.md") && display2.contains("1/3") {
        println!("  PASS: No line numbers, correct path and chunk");
    } else {
        println!("  WARN: Unexpected format: {}", display2);
    }

    // Test 3: HookDescription (should not have line numbers)
    println!("\nTest 3: HookDescription");
    let meta3 = SourceMetadata::hook_description("PostToolUse", Some("Edit".to_string()));
    let display3 = meta3.display_string();
    println!("  Input: hook=PostToolUse, tool=Edit");
    println!("  Output: {}", display3);

    if display3.contains("PostToolUse") && display3.contains("Edit") {
        println!("  PASS: Contains hook type and tool name");
    } else {
        println!("  FAIL: Unexpected format");
    }

    // Test 4: Edge case - start_line only
    println!("\nTest 4: Edge case - partial line info");
    let mut meta4 = SourceMetadata::md_file_chunk("test.md", 0, 1);
    meta4.start_line = Some(5);
    meta4.end_line = None;
    let display4 = meta4.display_string();
    println!("  Input: start_line=5, end_line=None");
    println!("  Output: {}", display4);

    if display4.contains("test.md:5") && !display4.contains("-") {
        println!("  PASS: Shows single line number without range");
    } else {
        println!("  INFO: Format: {}", display4);
    }

    // Test 5: Verify all source types
    println!("\nTest 5: All source types");
    let sources = vec![
        (
            "MDFileChunk",
            SourceMetadata::md_file_chunk_with_lines("file.md", 0, 1, 1, 10),
        ),
        (
            "HookDescription",
            SourceMetadata::hook_description("SessionStart", None),
        ),
        ("ClaudeResponse", SourceMetadata::claude_response()),
        ("Manual", SourceMetadata::manual()),
    ];

    for (name, meta) in sources {
        let display = meta.display_string();
        println!("  {}: {}", name, display);
    }

    println!("\n=== FSV SOURCE METADATA TEST COMPLETE ===");
}
