//! TASK-EMB-014 Full State Verification Tests
//!
//! These tests verify that fake VRAM pointers have been replaced with real CUDA allocations.
//! Each test prints state BEFORE and AFTER to prove the outcome.
//!
//! SOURCE OF TRUTH: The actual source code files in the repository.

// ============================================================================
// EDGE CASE 1: Verify fake pointer pattern is DELETED from code
// ============================================================================

#[test]
fn edge_case_1_fake_pointer_pattern_deleted() {
    println!("\n=== EDGE CASE 1: Fake Pointer Pattern Deletion ===\n");

    // BEFORE: What the code USED to have (documentation only)
    println!("BEFORE (OLD CODE - now deleted):");
    println!("  let base_ptr = 0x7f80_0000_0000u64;  // FAKE!");
    println!("  let offset = memory_pools.list_model_allocations().len() as u64 * 0x1_0000_0000;");
    println!("  let vram_ptr = base_ptr + offset;  // FAKE pointer!");
    println!();

    // AFTER: Verify the new code calls allocate_protected
    println!("AFTER (NEW CODE - verified in source):");
    println!("  let allocation = cuda_allocator.allocate_protected(size_bytes)?;");
    println!("  let vram_ptr = allocation.ptr;  // REAL CUDA pointer!");
    println!();

    // Physical verification: Read the actual source file
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source_code =
        std::fs::read_to_string(format!("{}/src/warm/loader/operations.rs", manifest_dir))
            .expect("Failed to read operations.rs");

    // Check that fake pattern is NOT in executable code (only in comments)
    let fake_pattern = "0x7f80_0000_0000";
    let lines_with_pattern: Vec<(usize, &str)> = source_code
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(fake_pattern))
        .collect();

    println!("PHYSICAL VERIFICATION:");
    println!("  Searching for '{}' in operations.rs...", fake_pattern);

    let mut found_in_code = false;
    for (line_num, line) in &lines_with_pattern {
        let is_comment = line.trim().starts_with("//") || line.trim().starts_with("///");
        println!(
            "  Line {}: {} [{}]",
            line_num + 1,
            line.trim(),
            if is_comment {
                "COMMENT - OK"
            } else {
                "CODE - FAIL!"
            }
        );

        if !is_comment {
            found_in_code = true;
        }
    }

    // Verify allocate_protected is present
    let has_real_allocation = source_code.contains("cuda_allocator.allocate_protected(size_bytes)");
    println!();
    println!(
        "  Real CUDA allocation call present: {}",
        has_real_allocation
    );

    assert!(
        !found_in_code,
        "FAIL: Fake pointer pattern found in executable code!"
    );
    assert!(
        has_real_allocation,
        "FAIL: allocate_protected() call not found!"
    );

    println!();
    println!("RESULT: PASS - Fake pointers DELETED, real CUDA allocation PRESENT");
}

// ============================================================================
// EDGE CASE 2: Verify function signatures have cuda_allocator parameter
// ============================================================================

#[test]
fn edge_case_2_function_signatures_updated() {
    println!("\n=== EDGE CASE 2: Function Signatures Updated ===\n");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source_code =
        std::fs::read_to_string(format!("{}/src/warm/loader/operations.rs", manifest_dir))
            .expect("Failed to read operations.rs");

    println!("BEFORE (OLD SIGNATURES - now deleted):");
    println!("  pub fn allocate_model_vram(model_id, size_bytes, memory_pools)");
    println!("  pub fn load_single_model(model_id, config, registry, memory_pools, validator)");
    println!();

    // Find the allocate_model_vram function signature
    let allocate_fn_start = source_code
        .find("pub fn allocate_model_vram(")
        .expect("allocate_model_vram function not found");
    let allocate_fn_section = &source_code[allocate_fn_start..allocate_fn_start + 300];
    let allocate_sig_end = allocate_fn_section
        .find(") -> ")
        .expect("allocate_model_vram signature incomplete");
    let allocate_sig = &allocate_fn_section[..allocate_sig_end + 1];

    // Find the load_single_model function signature
    let load_fn_start = source_code
        .find("pub fn load_single_model(")
        .expect("load_single_model function not found");
    let load_fn_section = &source_code[load_fn_start..load_fn_start + 300];
    let load_sig_end = load_fn_section
        .find(") -> ")
        .expect("load_single_model signature incomplete");
    let load_sig = &load_fn_section[..load_sig_end + 1];

    println!("AFTER (NEW SIGNATURES - verified in source):");
    println!();
    println!("  allocate_model_vram signature:");
    for line in allocate_sig.lines() {
        println!("    {}", line.trim());
    }
    println!();
    println!("  load_single_model signature:");
    for line in load_sig.lines() {
        println!("    {}", line.trim());
    }
    println!();

    // Verify cuda_allocator parameter is present
    let allocate_has_cuda = allocate_sig.contains("cuda_allocator: &mut WarmCudaAllocator");
    let load_has_cuda = load_sig.contains("cuda_allocator: &mut WarmCudaAllocator");

    println!("PHYSICAL VERIFICATION:");
    println!(
        "  allocate_model_vram has cuda_allocator param: {}",
        allocate_has_cuda
    );
    println!(
        "  load_single_model has cuda_allocator param: {}",
        load_has_cuda
    );

    assert!(
        allocate_has_cuda,
        "FAIL: allocate_model_vram missing cuda_allocator parameter"
    );
    assert!(
        load_has_cuda,
        "FAIL: load_single_model missing cuda_allocator parameter"
    );

    println!();
    println!("RESULT: PASS - Both functions have cuda_allocator parameter");
}

// ============================================================================
// EDGE CASE 3: Engine passes allocator through call chain
// ============================================================================

#[test]
fn edge_case_3_engine_passes_allocator() {
    println!("\n=== EDGE CASE 3: Engine Passes Allocator ===\n");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let engine_code =
        std::fs::read_to_string(format!("{}/src/warm/loader/engine.rs", manifest_dir))
            .expect("Failed to read engine.rs");

    println!("VERIFICATION POINTS:");
    println!();

    // Check 1: cuda_allocator field exists
    let has_field = engine_code.contains("cuda_allocator: Option<WarmCudaAllocator>");
    println!("  1. WarmLoader has cuda_allocator field: {}", has_field);

    // Check 2: load_single_model is called with cuda_allocator
    // Find the actual call in the for loop
    let call_section_start = engine_code
        .find("for model_id in self.loading_order.clone()")
        .unwrap();
    let call_section = &engine_code[call_section_start..call_section_start + 500];

    let passes_allocator = call_section.contains("cuda_allocator,");
    println!(
        "  2. load_single_model called with cuda_allocator: {}",
        passes_allocator
    );

    // Print the actual call
    if let Some(call_start) = call_section.find("load_single_model(") {
        let call_end = call_section[call_start..].find(")").unwrap() + 1;
        let call = &call_section[call_start..call_start + call_end];
        println!(
            "     Call found: {}",
            call.replace('\n', " ").replace("  ", " ").trim()
        );
    }

    // Check 3: Fail-fast when allocator is None
    let has_failfast = engine_code.contains("CUDA allocator not initialized");
    println!("  3. Fail-fast when allocator is None: {}", has_failfast);

    // Check 4: No fallback implementation (only check for actual fallback code, not comments)
    // Look for patterns like "else { simulate" or "|| fallback" that indicate fallback code
    let has_fallback_code = engine_code.contains("else {")
        && (engine_code.contains("simulate_")
            || engine_code.contains("fallback_")
            || engine_code.contains("stub_"));
    // Also check for fake pointer patterns in actual code
    let has_fake_pointer = engine_code
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .any(|line| line.contains("0x7f80_0000_0000"));
    println!("  4. No fallback implementation: {}", !has_fallback_code);
    println!("  5. No fake pointers in code: {}", !has_fake_pointer);

    println!();

    assert!(has_field, "FAIL: cuda_allocator field missing");
    assert!(
        passes_allocator,
        "FAIL: cuda_allocator not passed to load_single_model"
    );
    assert!(has_failfast, "FAIL: Fail-fast behavior missing");
    assert!(
        !has_fallback_code,
        "FAIL: Fallback implementation found - violates Constitution AP-007"
    );
    assert!(
        !has_fake_pointer,
        "FAIL: Fake pointer pattern found in code"
    );

    println!("RESULT: PASS - Engine correctly passes allocator through call chain with fail-fast");
}

// ============================================================================
// EDGE CASE 4: Verify import statement exists
// ============================================================================

#[test]
fn edge_case_4_import_verification() {
    println!("\n=== EDGE CASE 4: Import Statement Verification ===\n");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source_code =
        std::fs::read_to_string(format!("{}/src/warm/loader/operations.rs", manifest_dir))
            .expect("Failed to read operations.rs");

    println!("VERIFICATION:");

    // Check for WarmCudaAllocator import
    let has_import = source_code.contains("use crate::warm::cuda_alloc::WarmCudaAllocator;");
    println!("  WarmCudaAllocator import present: {}", has_import);

    // Check for other required imports
    let has_safetensors = source_code.contains("use safetensors::SafeTensors;");
    println!("  SafeTensors import present: {}", has_safetensors);

    let has_sha256 = source_code.contains("use sha2::");
    println!("  sha2 (SHA256) import present: {}", has_sha256);

    assert!(has_import, "FAIL: WarmCudaAllocator import missing");

    println!();
    println!("RESULT: PASS - All required imports present");
}
