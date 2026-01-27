//! Disk Space Management Benchmark
//!
//! Validates that disk space management tooling is correctly configured.
//! This is a lightweight validation binary that checks:
//! - Cargo.toml profile settings
//! - Cleanup script presence and permissions
//! - Session hook disk check function
//! - .gitignore patterns
//!
//! Exit codes:
//! - 0: All checks passed
//! - 1: Some checks failed

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Result of a single check
struct CheckResult {
    name: &'static str,
    passed: bool,
    details: String,
}

impl CheckResult {
    fn pass(name: &'static str, details: impl Into<String>) -> Self {
        Self {
            name,
            passed: true,
            details: details.into(),
        }
    }

    fn fail(name: &'static str, details: impl Into<String>) -> Self {
        Self {
            name,
            passed: false,
            details: details.into(),
        }
    }

    fn print(&self) {
        let status = if self.passed { "PASS" } else { "FAIL" };
        println!("  {}: {} - {}", self.name, status, self.details);
    }
}

/// Find the project root by looking for Cargo.toml
fn find_project_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            // Check if it's the workspace root (has [workspace] section)
            if let Ok(content) = fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Some(current);
                }
            }
        }

        if !current.pop() {
            break;
        }
    }

    // Fallback: just use current directory
    std::env::current_dir().ok()
}

/// Check Cargo.toml profile settings
fn check_cargo_toml(project_root: &Path) -> Vec<CheckResult> {
    let mut results = Vec::new();
    let cargo_toml_path = project_root.join("Cargo.toml");

    let content = match fs::read_to_string(&cargo_toml_path) {
        Ok(c) => c,
        Err(e) => {
            results.push(CheckResult::fail(
                "cargo_toml_readable",
                format!("Failed to read: {}", e),
            ));
            return results;
        }
    };

    // Check for debug = 1
    if content.contains("debug = 1") {
        results.push(CheckResult::pass(
            "debug_level",
            "debug = 1 (line info only)",
        ));
    } else if content.contains("debug = true") {
        results.push(CheckResult::fail(
            "debug_level",
            "debug = true (full DWARF - larger artifacts)",
        ));
    } else {
        results.push(CheckResult::fail("debug_level", "No debug setting found"));
    }

    // Check for split-debuginfo
    if content.contains("split-debuginfo") {
        results.push(CheckResult::pass(
            "split_debuginfo",
            "split-debuginfo configured",
        ));
    } else {
        results.push(CheckResult::fail(
            "split_debuginfo",
            "split-debuginfo not configured",
        ));
    }

    // Check for package override
    if content.contains("[profile.dev.package.\"*\"]") && content.contains("debug = false") {
        results.push(CheckResult::pass(
            "package_override",
            "Dependencies have debug=false",
        ));
    } else {
        results.push(CheckResult::fail(
            "package_override",
            "Missing [profile.dev.package.\"*\"] with debug=false",
        ));
    }

    results
}

/// Check cleanup script
fn check_cleanup_script(project_root: &Path) -> Vec<CheckResult> {
    let mut results = Vec::new();
    let script_path = project_root.join("scripts/clean-build-artifacts.sh");

    // Check existence
    if !script_path.exists() {
        results.push(CheckResult::fail(
            "cleanup_script_exists",
            "scripts/clean-build-artifacts.sh not found",
        ));
        return results;
    }
    results.push(CheckResult::pass(
        "cleanup_script_exists",
        "Script exists",
    ));

    // Check executable permission
    match fs::metadata(&script_path) {
        Ok(meta) => {
            let mode = meta.permissions().mode();
            if mode & 0o111 != 0 {
                results.push(CheckResult::pass(
                    "cleanup_script_executable",
                    format!("Executable (mode: {:o})", mode & 0o777),
                ));
            } else {
                results.push(CheckResult::fail(
                    "cleanup_script_executable",
                    format!("Not executable (mode: {:o})", mode & 0o777),
                ));
            }
        }
        Err(e) => {
            results.push(CheckResult::fail(
                "cleanup_script_executable",
                format!("Could not read permissions: {}", e),
            ));
        }
    }

    // Check content for key features
    if let Ok(content) = fs::read_to_string(&script_path) {
        // Check for dry-run support
        if content.contains("--dry-run") {
            results.push(CheckResult::pass(
                "cleanup_has_dry_run",
                "--dry-run option supported",
            ));
        } else {
            results.push(CheckResult::fail(
                "cleanup_has_dry_run",
                "--dry-run option not found",
            ));
        }

        // Check for debug cleanup
        if content.contains("target/debug") || content.contains("Debug build") {
            results.push(CheckResult::pass(
                "cleanup_targets_debug",
                "Targets debug artifacts",
            ));
        } else {
            results.push(CheckResult::fail(
                "cleanup_targets_debug",
                "Does not target debug artifacts",
            ));
        }

        // Check for stale data cleanup
        if content.contains("contextgraph_data_incompatible") || content.contains("stale_dir") {
            results.push(CheckResult::pass(
                "cleanup_targets_stale",
                "Targets stale data folders",
            ));
        } else {
            results.push(CheckResult::fail(
                "cleanup_targets_stale",
                "Does not target stale data folders",
            ));
        }
    }

    results
}

/// Check session hook disk monitoring
fn check_session_hook(project_root: &Path) -> Vec<CheckResult> {
    let mut results = Vec::new();
    let hook_path = project_root.join(".claude/hooks/session_start.sh");

    // Check existence
    if !hook_path.exists() {
        results.push(CheckResult::fail(
            "session_hook_exists",
            ".claude/hooks/session_start.sh not found",
        ));
        return results;
    }
    results.push(CheckResult::pass("session_hook_exists", "Hook exists"));

    // Check content
    let content = match fs::read_to_string(&hook_path) {
        Ok(c) => c,
        Err(e) => {
            results.push(CheckResult::fail(
                "session_hook_readable",
                format!("Failed to read: {}", e),
            ));
            return results;
        }
    };

    // Check for disk check function
    if content.contains("check_disk_space") {
        results.push(CheckResult::pass(
            "hook_has_disk_check",
            "check_disk_space function present",
        ));
    } else {
        results.push(CheckResult::fail(
            "hook_has_disk_check",
            "check_disk_space function not found",
        ));
    }

    // Check threshold is 85%
    if content.contains("threshold_percent=85") {
        results.push(CheckResult::pass(
            "hook_threshold_85",
            "Threshold set to 85%",
        ));
    } else if content.contains("threshold_percent=") {
        // Extract the actual value
        let threshold = content
            .lines()
            .find(|l| l.contains("threshold_percent="))
            .and_then(|l| l.split('=').nth(1))
            .unwrap_or("unknown");
        results.push(CheckResult::fail(
            "hook_threshold_85",
            format!("Threshold is {} (expected 85)", threshold),
        ));
    } else {
        results.push(CheckResult::fail(
            "hook_threshold_85",
            "No threshold_percent setting found",
        ));
    }

    // Check for non-blocking execution
    if content.contains("|| true") || content.contains("2>/dev/null || true") {
        results.push(CheckResult::pass(
            "hook_non_blocking",
            "Disk check is non-blocking",
        ));
    } else {
        results.push(CheckResult::fail(
            "hook_non_blocking",
            "Disk check may block on errors",
        ));
    }

    results
}

/// Check .gitignore patterns
fn check_gitignore(project_root: &Path) -> Vec<CheckResult> {
    let mut results = Vec::new();
    let gitignore_path = project_root.join(".gitignore");

    let content = match fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(e) => {
            results.push(CheckResult::fail(
                "gitignore_readable",
                format!("Failed to read: {}", e),
            ));
            return results;
        }
    };

    // Check for incompatible data pattern
    if content.contains("contextgraph_data_incompatible_") {
        results.push(CheckResult::pass(
            "gitignore_incompatible",
            "contextgraph_data_incompatible_*/ pattern present",
        ));
    } else {
        results.push(CheckResult::fail(
            "gitignore_incompatible",
            "contextgraph_data_incompatible_*/ pattern missing",
        ));
    }

    // Check for backup data pattern
    if content.contains("contextgraph_data_backup_") {
        results.push(CheckResult::pass(
            "gitignore_backup",
            "contextgraph_data_backup_*/ pattern present",
        ));
    } else {
        results.push(CheckResult::fail(
            "gitignore_backup",
            "contextgraph_data_backup_*/ pattern missing",
        ));
    }

    // Check for rmeta files
    if content.contains(".rmeta") || content.contains("*.rmeta") {
        results.push(CheckResult::pass(
            "gitignore_rmeta",
            "*.rmeta pattern present",
        ));
    } else {
        results.push(CheckResult::fail(
            "gitignore_rmeta",
            "*.rmeta pattern missing",
        ));
    }

    // Check for target/
    if content.lines().any(|l| l.trim() == "target/") {
        results.push(CheckResult::pass(
            "gitignore_target",
            "target/ pattern present",
        ));
    } else {
        results.push(CheckResult::fail(
            "gitignore_target",
            "target/ pattern missing",
        ));
    }

    results
}

fn main() {
    println!("=== Disk Space Management Benchmark ===");
    println!();

    let project_root = match find_project_root() {
        Some(p) => p,
        None => {
            eprintln!("ERROR: Could not find project root");
            std::process::exit(1);
        }
    };

    println!("Project root: {}", project_root.display());
    println!();

    let mut all_results = Vec::new();
    let mut category_pass_counts: Vec<(&str, usize, usize)> = Vec::new();

    // Test 1: Cargo.toml profile settings
    println!("### Test 1: Cargo.toml Profile Settings ###");
    let cargo_results = check_cargo_toml(&project_root);
    let passed = cargo_results.iter().filter(|r| r.passed).count();
    let total = cargo_results.len();
    for result in &cargo_results {
        result.print();
    }
    category_pass_counts.push(("Cargo.toml", passed, total));
    all_results.extend(cargo_results);
    println!();

    // Test 2: Cleanup script
    println!("### Test 2: Cleanup Script ###");
    let script_results = check_cleanup_script(&project_root);
    let passed = script_results.iter().filter(|r| r.passed).count();
    let total = script_results.len();
    for result in &script_results {
        result.print();
    }
    category_pass_counts.push(("Cleanup Script", passed, total));
    all_results.extend(script_results);
    println!();

    // Test 3: Session hook
    println!("### Test 3: Session Hook ###");
    let hook_results = check_session_hook(&project_root);
    let passed = hook_results.iter().filter(|r| r.passed).count();
    let total = hook_results.len();
    for result in &hook_results {
        result.print();
    }
    category_pass_counts.push(("Session Hook", passed, total));
    all_results.extend(hook_results);
    println!();

    // Test 4: .gitignore patterns
    println!("### Test 4: .gitignore Patterns ###");
    let gitignore_results = check_gitignore(&project_root);
    let passed = gitignore_results.iter().filter(|r| r.passed).count();
    let total = gitignore_results.len();
    for result in &gitignore_results {
        result.print();
    }
    category_pass_counts.push((".gitignore", passed, total));
    all_results.extend(gitignore_results);
    println!();

    // Summary
    println!("### Summary ###");
    println!();

    let total_passed = all_results.iter().filter(|r| r.passed).count();
    let total_checks = all_results.len();

    for (category, passed, total) in &category_pass_counts {
        let status = if *passed == *total { "PASS" } else { "PARTIAL" };
        println!("  {}: {} ({}/{})", category, status, passed, total);
    }
    println!();

    let all_pass = total_passed == total_checks;
    if all_pass {
        println!("All disk space management checks PASSED ({}/{})", total_passed, total_checks);
        std::process::exit(0);
    } else {
        println!(
            "Some checks FAILED ({}/{} passed) - review output above",
            total_passed, total_checks
        );

        // List failed checks
        println!();
        println!("Failed checks:");
        for result in all_results.iter().filter(|r| !r.passed) {
            println!("  - {}: {}", result.name, result.details);
        }

        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_result_creation() {
        let pass = CheckResult::pass("test", "details");
        assert!(pass.passed);
        assert_eq!(pass.name, "test");

        let fail = CheckResult::fail("test", "details");
        assert!(!fail.passed);
    }
}
