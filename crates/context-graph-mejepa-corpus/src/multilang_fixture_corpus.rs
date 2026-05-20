use crate::timed_subprocess::run_capture_timed;
use crate::{Language, MutationError, MutationResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureTask {
    pub task_id: String,
    pub repo: String,
    pub language: Language,
    pub extension: &'static str,
    pub source: String,
    pub alternate_source: String,
    pub source_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainReport {
    pub language: Language,
    pub validator: String,
    pub command: Vec<String>,
    pub expected_success: bool,
    pub status_success: bool,
    pub status_code: Option<i32>,
    pub elapsed_ms: u128,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

pub fn build_fixture_tasks(
    tasks_per_language: usize,
    seed: u64,
) -> MutationResult<Vec<FixtureTask>> {
    if tasks_per_language == 0 {
        return Err(MutationError::invalid(
            "tasks_per_language",
            "tasks_per_language must be greater than zero",
            "request at least one fixture per non-Python language",
        ));
    }
    let mut tasks = Vec::new();
    for language in non_python_languages() {
        for ordinal in 0..tasks_per_language {
            let (extension, source, alternate_source) = fixture_source(language, ordinal, seed)?;
            let task_id = format!("{}__fixture-{ordinal:04}", language.slug());
            tasks.push(FixtureTask {
                task_id,
                repo: format!("fixture-{}", language.slug()),
                language,
                extension,
                source_sha256: sha256_text(&source),
                source,
                alternate_source,
            });
        }
    }
    Ok(tasks)
}

pub fn non_python_languages() -> [Language; 10] {
    [
        Language::Rust,
        Language::JavaScript,
        Language::TypeScript,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Ruby,
        Language::Php,
    ]
}

pub fn validate_toolchain(
    language: Language,
    source_path: &Path,
    work_dir: &Path,
    expected_success: bool,
    timeout: Duration,
) -> Result<ToolchainReport, Box<dyn std::error::Error>> {
    fs::create_dir_all(work_dir)?;
    let (validator, program, args) = toolchain_command(language, source_path, work_dir)?;
    let started = Instant::now();
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_capture_timed(Path::new(&program), &arg_refs, timeout, "phase_c_toolchain")?;
    let elapsed_ms = started.elapsed().as_millis();
    let status_success = output.status.success();
    let report = ToolchainReport {
        language,
        validator,
        command: std::iter::once(program).chain(args).collect(),
        expected_success,
        status_success,
        status_code: output.status.code(),
        elapsed_ms,
        stdout_tail: tail_bytes(&output.stdout),
        stderr_tail: tail_bytes(&output.stderr),
    };
    if status_success != expected_success {
        return Err(format!(
            "toolchain verdict mismatch for {} at {}: expected_success={} actual_success={} stderr_tail={}",
            language.slug(),
            source_path.display(),
            expected_success,
            status_success,
            report.stderr_tail
        )
        .into());
    }
    Ok(report)
}

fn fixture_source(
    language: Language,
    ordinal: usize,
    seed: u64,
) -> MutationResult<(&'static str, String, String)> {
    let n = 3 + ((ordinal as u64 + seed) % 17);
    let expected = n + 2;
    let suffix = format!("{ordinal:04}");
    let out = match language {
        Language::Rust => (
            "rs",
            format!(
                "fn check_{suffix}(alpha: i32, beta: i32) -> bool {{\n    let total = alpha + beta + {n};\n    let expected = {expected};\n    assert!(total >= 0);\n    total == expected && true\n}}\n"
            ),
            format!(
                "fn alternate_{suffix}(alpha: i32) -> bool {{\n    alpha == 1 && true\n}}\n"
            ),
        ),
        Language::JavaScript => (
            "js",
            format!(
                "function check{suffix}(alpha, beta) {{\n  const total = alpha + beta + {n};\n  const expected = {expected};\n  console.assert(total >= 0);\n  return total == expected && true;\n}}\n"
            ),
            format!(
                "function alternate{suffix}(alpha) {{\n  return alpha == 1 && true;\n}}\n"
            ),
        ),
        Language::TypeScript => (
            "ts",
            format!(
                "function check{suffix}(alpha: number, beta: number): boolean {{\n  const total = alpha + beta + {n};\n  const expected = {expected};\n  console.assert(total >= 0);\n  return total == expected && true;\n}}\n"
            ),
            format!(
                "function alternate{suffix}(alpha: number): boolean {{\n  return alpha == 1 && true;\n}}\n"
            ),
        ),
        Language::Go => (
            "go",
            format!(
                "package main\n\ntype tester{suffix} struct{{}}\nfunc (tester{suffix}) Fatal(message string) {{}}\n\nfunc check{suffix}(alpha int, beta int) bool {{\n    var t tester{suffix}\n    total := alpha + beta + {n}\n    expected := {expected}\n    if total >= 0 {{ t.Fatal(\"fixture assertion\") }}\n    _ = t\n    if expected < 0 {{ return false }}\n    return total == expected && true\n}}\n"
            ),
            format!(
                "package main\n\nfunc alternate{suffix}(alpha int) bool {{\n    return alpha == 1 && true\n}}\n"
            ),
        ),
        Language::Java => (
            "java",
            format!(
                "class MutationFixture{suffix} {{\n  boolean check(int alpha, int beta) {{\n    int total = alpha + beta + {n};\n    int expected = {expected};\n    assert total >= 0;\n    return total == expected && true;\n  }}\n}}\n"
            ),
            format!(
                "class MutationAlt{suffix} {{\n  boolean alternate(int alpha) {{ return alpha == 1 && true; }}\n}}\n"
            ),
        ),
        Language::C => (
            "c",
            format!(
                "#include <assert.h>\nint check_{suffix}(int alpha, int beta) {{\n    int total = alpha + beta + {n};\n    int expected = {expected};\n    assert(total >= 0);\n    return total == expected && 1;\n}}\n"
            ),
            format!(
                "#include <assert.h>\nint alternate_{suffix}(int alpha) {{\n    return alpha == 1 && 1;\n}}\n"
            ),
        ),
        Language::Cpp => (
            "cpp",
            format!(
                "#include <cassert>\nbool check_{suffix}(int alpha, int beta) {{\n    int total = alpha + beta + {n};\n    int expected = {expected};\n    assert(total >= 0);\n    return total == expected && true;\n}}\n"
            ),
            format!(
                "#include <cassert>\nbool alternate_{suffix}(int alpha) {{\n    return alpha == 1 && true;\n}}\n"
            ),
        ),
        Language::CSharp => (
            "cs",
            format!(
                "class MutationFixture{suffix} {{\n  void assert(bool condition) {{}}\n  bool Check(int alpha, int beta) {{\n    int total = alpha + beta + {n};\n    int expected = {expected};\n    assert(total >= 0);\n    return total == expected && true;\n  }}\n}}\n"
            ),
            format!(
                "class MutationAlt{suffix} {{\n  bool Alternate(int alpha) {{ return alpha == 1 && true; }}\n}}\n"
            ),
        ),
        Language::Ruby => (
            "rb",
            format!(
                "def check_{suffix}(alpha, beta)\n  total = alpha + beta + {n}\n  expected = {expected}\n  assert total >= 0\n  total == expected && true\nend\n"
            ),
            format!(
                "def alternate_{suffix}(alpha)\n  alpha == 1 && true\nend\n"
            ),
        ),
        Language::Php => (
            "php",
            format!(
                "<?php\nfunction check_{suffix}($alpha, $beta) {{\n    $total = $alpha + $beta + {n};\n    $expected = {expected};\n    assert($total >= 0);\n    return $total == $expected && true;\n}}\n"
            ),
            format!(
                "<?php\nfunction alternate_{suffix}($alpha) {{\n    return $alpha == 1 && true;\n}}\n"
            ),
        ),
        Language::Python => {
            return Err(MutationError::invalid(
                "language",
                "Python fixtures are generated by the SWE-bench corpus path",
                "use one of the 10 non-Python fixture languages",
            ))
        }
    };
    Ok(out)
}

fn toolchain_command(
    language: Language,
    source_path: &Path,
    work_dir: &Path,
) -> Result<(String, String, Vec<String>), Box<dyn std::error::Error>> {
    let source = source_path.display().to_string();
    let out = match language {
        Language::Rust => (
            "rustc --emit=metadata".to_string(),
            "rustc".to_string(),
            vec![
                "--edition=2021".to_string(),
                "--crate-type".to_string(),
                "lib".to_string(),
                "--emit=metadata".to_string(),
                "-o".to_string(),
                work_dir.join("lib.rmeta").display().to_string(),
                source,
            ],
        ),
        Language::JavaScript => (
            "node --check".to_string(),
            "node".to_string(),
            vec!["--check".to_string(), source],
        ),
        Language::TypeScript => (
            "tsc --noEmit".to_string(),
            "tsc".to_string(),
            vec![
                "--noEmit".to_string(),
                "--skipLibCheck".to_string(),
                "--pretty".to_string(),
                "false".to_string(),
                source,
            ],
        ),
        Language::Go => (
            "go test file".to_string(),
            "go".to_string(),
            vec!["test".to_string(), source],
        ),
        Language::Java => (
            "javac".to_string(),
            "javac".to_string(),
            vec!["-d".to_string(), work_dir.display().to_string(), source],
        ),
        Language::C => (
            "cc -fsyntax-only".to_string(),
            "cc".to_string(),
            vec!["-fsyntax-only".to_string(), source],
        ),
        Language::Cpp => (
            "c++ -fsyntax-only".to_string(),
            "c++".to_string(),
            vec![
                "-std=c++17".to_string(),
                "-fsyntax-only".to_string(),
                source,
            ],
        ),
        Language::CSharp => csharp_command(source_path, work_dir)?,
        Language::Ruby => (
            "ruby -c".to_string(),
            "ruby".to_string(),
            vec!["-c".to_string(), source],
        ),
        Language::Php => (
            "php -l".to_string(),
            "php".to_string(),
            vec!["-l".to_string(), source],
        ),
        Language::Python => {
            return Err(
                "Python is validated by the SWE-bench Docker oracle, not fixture toolchains".into(),
            )
        }
    };
    Ok(out)
}

fn csharp_command(
    source_path: &Path,
    work_dir: &Path,
) -> Result<(String, String, Vec<String>), Box<dyn std::error::Error>> {
    let dotnet_root = resolve_dotnet_root()?;
    let csc = newest_matching_file(&dotnet_root.join("sdk"), &["Roslyn", "bincore", "csc.dll"])?;
    let system_runtime = newest_matching_file(
        &dotnet_root.join("packs").join("Microsoft.NETCore.App.Ref"),
        &["ref", "net8.0", "System.Runtime.dll"],
    )?;
    Ok((
        "dotnet Roslyn csc.dll".to_string(),
        "dotnet".to_string(),
        vec![
            csc.display().to_string(),
            "-nologo".to_string(),
            "-target:library".to_string(),
            "-nostdlib".to_string(),
            format!("-r:{}", system_runtime.display()),
            format!("-out:{}", work_dir.join("Mutation.dll").display()),
            source_path.display().to_string(),
        ],
    ))
}

/// Resolve `DOTNET_ROOT` for the C# fixture toolchain with explicit fail-closed
/// semantics. Resolution order is:
///   1. `DOTNET_ROOT` env var, if set to a non-empty value.
///   2. `$HOME/.dotnet`, if `HOME` is set to a non-empty value.
///   3. Structured `MutationError::InvalidInput` with code `MEJEPA_CORPUS_INVALID_INPUT`.
///
/// No operator-specific absolute path is baked into source (was
/// `/home/cabdru/.dotnet`, see F-006 / #459).
fn resolve_dotnet_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    resolve_dotnet_root_from(
        std::env::var("DOTNET_ROOT").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

/// Pure resolver used by both the production path and FSV tests. Splitting out
/// the env-read keeps tests free of `std::env::set_var` races (Rust tests run
/// in parallel and Rust 1.78+ marks `set_var` unsafe due to those races).
fn resolve_dotnet_root_from(
    dotnet_root_env: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(raw) = dotnet_root_env {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Some(raw) = home_env {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join(".dotnet"));
        }
    }
    Err(MutationError::invalid(
        "dotnet_root",
        "DOTNET_ROOT_NOT_RESOLVED: neither DOTNET_ROOT nor HOME is set to a non-empty value; \
         the C# fixture toolchain cannot locate Roslyn csc.dll",
        "export DOTNET_ROOT to the .NET SDK root, or set HOME so $HOME/.dotnet can be used",
    )
    .into())
}

#[cfg(test)]
mod dotnet_root_tests {
    use super::resolve_dotnet_root_from;
    use std::path::PathBuf;

    #[test]
    fn fails_closed_when_both_env_unset() {
        let err = resolve_dotnet_root_from(None, None)
            .expect_err("must fail when neither DOTNET_ROOT nor HOME is set");
        let msg = err.to_string();
        assert!(
            msg.contains("DOTNET_ROOT_NOT_RESOLVED"),
            "expected structured DOTNET_ROOT_NOT_RESOLVED marker, got: {msg}"
        );
    }

    #[test]
    fn fails_closed_when_both_env_empty_after_trim() {
        let err = resolve_dotnet_root_from(Some("   "), Some(""))
            .expect_err("must fail when both env values trim to empty");
        assert!(err.to_string().contains("DOTNET_ROOT_NOT_RESOLVED"));
    }

    #[test]
    fn uses_dotnet_root_when_set() {
        let resolved = resolve_dotnet_root_from(Some("/opt/dotnet-9.0"), Some("/home/operator"))
            .expect("DOTNET_ROOT must take precedence");
        assert_eq!(resolved, PathBuf::from("/opt/dotnet-9.0"));
    }

    #[test]
    fn falls_back_to_home_dotnet_when_only_home_set() {
        let resolved = resolve_dotnet_root_from(None, Some("/home/operator"))
            .expect("HOME fallback must succeed when set");
        assert_eq!(resolved, PathBuf::from("/home/operator/.dotnet"));
    }

    #[test]
    fn falls_back_to_home_when_dotnet_root_empty() {
        let resolved = resolve_dotnet_root_from(Some(""), Some("/home/operator"))
            .expect("empty DOTNET_ROOT must defer to HOME");
        assert_eq!(resolved, PathBuf::from("/home/operator/.dotnet"));
    }

    #[test]
    fn dotnet_root_whitespace_only_does_not_satisfy() {
        let resolved = resolve_dotnet_root_from(Some("   "), Some("/home/operator"))
            .expect("whitespace-only DOTNET_ROOT must defer to HOME");
        assert_eq!(resolved, PathBuf::from("/home/operator/.dotnet"));
    }

    #[test]
    fn does_not_bake_legacy_operator_path() {
        // Regression test for F-006: ensure the retired /home/cabdru/.dotnet
        // fallback is gone. With both env unset, we must error, not return
        // any specific operator path.
        let err = resolve_dotnet_root_from(None, None).unwrap_err();
        assert!(
            !err.to_string().contains("/home/cabdru/.dotnet"),
            "fallback must not leak the retired /home/cabdru/.dotnet operator path"
        );
    }
}

fn newest_matching_file(
    root: &Path,
    suffix: &[&str],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut matches = Vec::new();
    collect_matching_files(root, suffix, &mut matches)?;
    matches.sort();
    matches.pop().ok_or_else(|| {
        format!(
            "required .NET toolchain file under {} with suffix {:?} was not found",
            root.display(),
            suffix
        )
        .into()
    })
}

fn collect_matching_files(
    root: &Path,
    suffix: &[&str],
    matches: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_matching_files(&path, suffix, matches)?;
        } else if path_ends_with(&path, suffix) {
            matches.push(path);
        }
    }
    Ok(())
}

fn path_ends_with(path: &Path, suffix: &[&str]) -> bool {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components
        .windows(suffix.len())
        .last()
        .is_some_and(|window| window == suffix)
}

pub fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn tail_bytes(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars().rev().take(2000).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}
