use context_graph_core::memory::ast::{
    chunk_with_options, route_to_embedders, AstChunkOptions, EntityType, Language,
};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    language: Language,
    slug: &'static str,
    file_name: &'static str,
    chunk_path: &'static str,
    source: &'static str,
    expected: &'static [EntityType],
    toolchain: Toolchain,
}

#[derive(Clone, Copy)]
enum Toolchain {
    Rustc,
    PyCompile,
    NodeCheck,
    Tsc,
    GoTest,
    Javac,
    Gcc,
    Gxx,
    Dotnet,
    RubyCheck,
    PhpLint,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-ast-chunker-multilang-fsv");
    reset_secure_dir(&root)?;
    let mut languages = Vec::new();
    for fixture in fixtures() {
        languages.push(run_language_fsv(&root, fixture)?);
    }
    let edges = run_edge_cases(&root)?;
    let aggregate_path = root.join("aggregate-evidence.json");
    let aggregate = json!({
        "source_of_truth": {
            "aggregate_evidence_json": aggregate_path.display().to_string(),
            "per_language_dirs": languages.iter().map(|v| v["source_of_truth"]["dir"].clone()).collect::<Vec<_>>(),
        },
        "language_count": languages.len(),
        "languages": languages,
        "edge_cases": edges,
    });
    write_secure_json(&aggregate_path, &aggregate)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&aggregate_path)?)?;
    require(
        "aggregate language_count readback",
        readback["language_count"].as_u64() == Some(11),
    )?;
    require(
        "edge readback all pass",
        readback["edge_cases"]
            .as_object()
            .unwrap()
            .values()
            .all(|edge| edge["passes"] == Value::Bool(true)),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fsv": "ast_chunker_multilang",
            "aggregate_evidence": aggregate_path,
            "language_count": readback["language_count"],
            "edge_cases": readback["edge_cases"],
        }))?
    );
    Ok(())
}

fn run_language_fsv(root: &Path, fixture: Fixture) -> FsvResult<Value> {
    let lang_dir = PathBuf::from(format!(
        "/mnt/d/contextgraph/fsv/contextgraph-{}-ast-chunker-fsv",
        fixture.slug
    ));
    reset_secure_dir(&lang_dir)?;
    let source_path = lang_dir.join(fixture.file_name);
    write_checked(&source_path, fixture.source.as_bytes())?;
    write_toolchain_support_files(&lang_dir, &fixture)?;

    let toolchain = run_toolchain(&lang_dir, &source_path, fixture.toolchain)?;
    require(
        &format!("{} toolchain success", fixture.slug),
        toolchain["success"] == Value::Bool(true),
    )?;

    let source_readback = fs::read(&source_path)?;
    let chunks = chunk_with_options(
        &source_readback,
        fixture.language,
        &AstChunkOptions::for_path(fixture.chunk_path),
    )?;
    require(
        &format!("{} non-empty chunks", fixture.slug),
        !chunks.is_empty(),
    )?;
    for expected in fixture.expected {
        require(
            &format!("{} expected entity {expected:?}", fixture.slug),
            chunks.iter().any(|chunk| chunk.entity_type == *expected),
        )?;
    }
    for chunk in &chunks {
        require(
            &format!("{} routing {:?}", fixture.slug, chunk.entity_type),
            chunk.embedder_routing_set == route_to_embedders(chunk.entity_type),
        )?;
    }

    let chunk_rows: Vec<Value> = chunks
        .iter()
        .map(|chunk| {
            json!({
                "entity_type": chunk.entity_type,
                "parent_chain": chunk.parent_chain,
                "line_start": chunk.line_start,
                "line_end": chunk.line_end,
                "sha256": chunk.sha256,
                "routing": chunk.embedder_routing_set,
                "content_first_120": chunk.content.chars().take(120).collect::<String>(),
            })
        })
        .collect();
    let evidence_path = lang_dir.join("ast-chunker-evidence.json");
    let evidence = json!({
        "source_of_truth": {
            "dir": lang_dir.display().to_string(),
            "source_file": source_path.display().to_string(),
            "evidence_json": evidence_path.display().to_string(),
        },
        "before_state": {
            "language": fixture.slug,
            "source_bytes_written": fixture.source.len(),
            "expected_entities": fixture.expected,
        },
        "action": {
            "toolchain": toolchain,
            "chunker_language": fixture.language,
            "chunk_path": fixture.chunk_path,
        },
        "after_state": {
            "source_bytes_readback": source_readback.len(),
            "chunk_count": chunks.len(),
            "chunks": chunk_rows,
        },
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        &format!("{} evidence chunk_count readback", fixture.slug),
        readback["after_state"]["chunk_count"].as_u64() == Some(chunks.len() as u64),
    )?;
    let copy_path = root.join(format!("{}-evidence-copy.json", fixture.slug));
    write_secure_json(&copy_path, &readback)?;
    Ok(readback)
}

fn run_edge_cases(root: &Path) -> FsvResult<Value> {
    let empty = edge_case(
        "empty_rust",
        b"",
        Language::Rust,
        "src/empty.rs",
        "MEJEPA_CHUNKER_EMPTY_SOURCE",
    );
    let invalid = edge_case(
        "invalid_python",
        b"def broken(:\n    return 1\n",
        Language::Python,
        "pkg/broken.py",
        "MEJEPA_CHUNKER_PARSE_FAILED",
    );
    let mixed = edge_case(
        "mixed_php_html",
        b"<?php function ok() { return 1; ?> <html></html>",
        Language::Php,
        "index.php",
        "MEJEPA_CHUNKER_MIXED_LANGUAGE",
    );
    let edges = json!({ "empty": empty, "invalid_syntax": invalid, "mixed_language": mixed });
    write_secure_json(&root.join("edge-case-evidence.json"), &edges)?;
    Ok(edges)
}

fn edge_case(name: &str, source: &[u8], language: Language, path: &str, expected: &str) -> Value {
    let before = json!({
        "name": name,
        "language": language,
        "path": path,
        "source_bytes": source.len(),
    });
    let err = chunk_with_options(source, language, &AstChunkOptions::for_path(path)).unwrap_err();
    json!({
        "before_state": before,
        "after_state": {
            "accepted": false,
            "actual_code": err.code(),
            "error": err.to_string(),
        },
        "expected_code": expected,
        "passes": err.code() == expected,
    })
}

fn fixtures() -> Vec<Fixture> {
    use EntityType::*;
    vec![
        Fixture { language: Language::Rust, slug: "rust", file_name: "sample.rs", chunk_path: "src/lib.rs", source: "pub struct Solver;\nimpl Solver { pub fn compute(&self, x: i32) -> i32 { x + 1 } }\npub fn helper(x: i32) -> i32 { x + 2 }\n#[cfg(test)] mod tests { #[test] fn test_compute() { assert_eq!(3, super::helper(1)); } }\n", expected: &[Struct, Impl, Method, Function, TestFunction], toolchain: Toolchain::Rustc },
        Fixture { language: Language::Python, slug: "python", file_name: "sample.py", chunk_path: "tests/test_solver.py", source: "import math\n\nclass Solver:\n    def compute(self, x: int) -> int:\n        return x + 1\n\ndef test_compute():\n    assert Solver().compute(1) == 2\n", expected: &[Import, Class, Method, TestFunction], toolchain: Toolchain::PyCompile },
        Fixture { language: Language::JavaScript, slug: "javascript", file_name: "sample.js", chunk_path: "tests/sample.test.js", source: "class Solver { compute(x) { return x + 1; } }\nfunction helper(x) { return x + 2; }\nfunction testCompute() { if (helper(1) !== 3) throw new Error('bad'); }\n", expected: &[Class, Method, Function, TestFunction], toolchain: Toolchain::NodeCheck },
        Fixture { language: Language::TypeScript, slug: "typescript", file_name: "sample.ts", chunk_path: "tests/sample.test.ts", source: "interface Runnable { compute(x: number): number; }\nenum Mode { Fast }\nclass Solver implements Runnable { compute(x: number): number { return x + 1; } }\nfunction testCompute(): void { const s = new Solver(); if (s.compute(1) !== 2) throw new Error('bad'); }\n", expected: &[TraitOrInterface, Enum, Class, Method, TestFunction], toolchain: Toolchain::Tsc },
        Fixture { language: Language::Go, slug: "go", file_name: "sample_test.go", chunk_path: "calc/sample_test.go", source: "package calc\n\nimport \"testing\"\n\ntype Solver struct{}\nfunc (s Solver) Compute(x int) int { return x + 1 }\nfunc Helper(x int) int { return x + 2 }\nfunc TestCompute(t *testing.T) { if Helper(1) != 3 { t.Fatal(\"bad\") } }\n", expected: &[Namespace, Import, Struct, Method, Function, TestFunction], toolchain: Toolchain::GoTest },
        Fixture { language: Language::Java, slug: "java", file_name: "TestSample.java", chunk_path: "src/test/java/TestSample.java", source: "import java.util.Objects;\npublic class TestSample { int compute(int x) { return x + 1; } void testCompute() { if (!Objects.equals(compute(1), 2)) throw new RuntimeException(); } }\n", expected: &[Import, Class, Method, TestFunction], toolchain: Toolchain::Javac },
        Fixture { language: Language::C, slug: "c", file_name: "sample.c", chunk_path: "src/sample.c", source: "#include <assert.h>\nstruct Solver { int value; };\nenum Mode { MODE_FAST };\nint helper(int x) { return x + 1; }\nint main(void) { assert(helper(1) == 2); return 0; }\n", expected: &[Import, Struct, Enum, Function], toolchain: Toolchain::Gcc },
        Fixture { language: Language::Cpp, slug: "cpp", file_name: "sample.cpp", chunk_path: "src/sample.cpp", source: "#include <stdexcept>\nnamespace fsv { class Solver { public: int compute(int x) { return x + 1; } }; int helper(int x) { return x + 2; } }\nint main() { return fsv::helper(1) == 3 ? 0 : 1; }\n", expected: &[Import, Namespace, Class, Method, Function], toolchain: Toolchain::Gxx },
        Fixture { language: Language::CSharp, slug: "csharp", file_name: "SolverTests.cs", chunk_path: "Tests/SolverTests.cs", source: "namespace Fsv; public class Solver { public int Compute(int x) { return x + 1; } public void TestCompute() { if (Compute(1) != 2) throw new System.Exception(\"bad\"); } }\n", expected: &[Namespace, Class, Method, TestFunction], toolchain: Toolchain::Dotnet },
        Fixture { language: Language::Ruby, slug: "ruby", file_name: "sample.rb", chunk_path: "spec/sample_spec.rb", source: "class Solver\n  def compute(x)\n    x + 1\n  end\nend\n\ndef test_compute\n  raise 'bad' unless Solver.new.compute(1) == 2\nend\n", expected: &[Class, Method, TestFunction], toolchain: Toolchain::RubyCheck },
        Fixture { language: Language::Php, slug: "php", file_name: "sample.php", chunk_path: "tests/SolverTest.php", source: "<?php\nnamespace Fsv;\nclass Solver { public function compute(int $x): int { return $x + 1; } }\nfunction test_compute(): void { $s = new Solver(); if ($s->compute(1) !== 2) { throw new \\Exception('bad'); } }\n", expected: &[Namespace, Class, Method, TestFunction], toolchain: Toolchain::PhpLint },
    ]
}

fn write_toolchain_support_files(dir: &Path, fixture: &Fixture) -> FsvResult<()> {
    match fixture.toolchain {
        Toolchain::GoTest => write_checked(&dir.join("go.mod"), b"module contextgraph-fsv\n\ngo 1.22\n")?,
        Toolchain::Dotnet => write_checked(
            &dir.join("contextgraph-fsv.csproj"),
            b"<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup></Project>",
        )?,
        _ => {}
    }
    Ok(())
}

fn run_toolchain(dir: &Path, source_path: &Path, toolchain: Toolchain) -> FsvResult<Value> {
    let (program, args): (&str, Vec<String>) = match toolchain {
        Toolchain::Rustc => (
            "rustc",
            vec![
                "--crate-type".into(),
                "lib".into(),
                "--emit".into(),
                "metadata".into(),
                source_path.display().to_string(),
                "-o".into(),
                dir.join("sample.rmeta").display().to_string(),
            ],
        ),
        Toolchain::PyCompile => (
            "python3",
            vec![
                "-m".into(),
                "py_compile".into(),
                source_path.display().to_string(),
            ],
        ),
        Toolchain::NodeCheck => (
            "node",
            vec!["--check".into(), source_path.display().to_string()],
        ),
        Toolchain::Tsc => (
            "tsc",
            vec![
                "--noEmit".into(),
                "--skipLibCheck".into(),
                "--target".into(),
                "ES2020".into(),
                source_path.display().to_string(),
            ],
        ),
        Toolchain::GoTest => ("go", vec!["test".into(), "./...".into()]),
        Toolchain::Javac => ("javac", vec![source_path.display().to_string()]),
        Toolchain::Gcc => (
            "gcc",
            vec!["-fsyntax-only".into(), source_path.display().to_string()],
        ),
        Toolchain::Gxx => (
            "g++",
            vec![
                "-std=c++17".into(),
                "-fsyntax-only".into(),
                source_path.display().to_string(),
            ],
        ),
        Toolchain::Dotnet => ("dotnet", vec!["build".into(), "--nologo".into()]),
        Toolchain::RubyCheck => ("ruby", vec!["-c".into(), source_path.display().to_string()]),
        Toolchain::PhpLint => ("php", vec!["-l".into(), source_path.display().to_string()]),
    };
    let output = Command::new(program)
        .args(&args)
        .current_dir(dir)
        .output()?;
    Ok(json!({
        "program": program,
        "args": args,
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout).chars().take(400).collect::<String>(),
        "stderr": String::from_utf8_lossy(&output.stderr).chars().take(400).collect::<String>(),
    }))
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    write_checked(path, serde_json::to_string_pretty(value)?.as_bytes())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV requirement failed: {name}").into())
    }
}
