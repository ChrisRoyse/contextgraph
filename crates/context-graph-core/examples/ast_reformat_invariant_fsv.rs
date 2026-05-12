use context_graph_core::memory::ast::{chunk_with_options, AstChunkOptions, EntityType, Language};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    language: Language,
    slug: &'static str,
    path: &'static str,
    compact: &'static str,
    formatted: &'static str,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-ast-reformat-fsv");
    reset_secure_dir(&root)?;
    let mut rows = Vec::new();
    for fixture in fixtures() {
        rows.push(run_fixture(&root, fixture)?);
    }
    let edge_cases = run_edge_cases(&root)?;
    let evidence_path = root.join("ast-reformat-evidence.json");
    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_path.display().to_string(),
            "artifact_dir": root.display().to_string(),
        },
        "happy_path": {
            "language_count": rows.len(),
            "rows": rows,
        },
        "edge_cases": edge_cases,
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "language_count",
        readback["happy_path"]["language_count"] == 11,
    )?;
    require(
        "all fixture hashes stable",
        readback["happy_path"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["passes"] == true),
    )?;
    require(
        "all edge cases pass",
        readback["edge_cases"]
            .as_object()
            .unwrap()
            .values()
            .all(|row| row["passes"] == true),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn run_fixture(root: &Path, fixture: Fixture) -> FsvResult<Value> {
    let before_path = root.join(format!("{}-before.txt", fixture.slug));
    let after_path = root.join(format!("{}-after.txt", fixture.slug));
    write_checked(&before_path, fixture.compact.as_bytes())?;
    write_checked(&after_path, fixture.formatted.as_bytes())?;
    let before_source = fs::read(&before_path)?;
    let after_source = fs::read(&after_path)?;
    let before_chunks = chunk_with_options(
        &before_source,
        fixture.language,
        &AstChunkOptions::for_path(fixture.path),
    )?;
    let after_chunks = chunk_with_options(
        &after_source,
        fixture.language,
        &AstChunkOptions::for_path(fixture.path),
    )?;
    let before_map = stable_chunk_map(&before_chunks);
    let after_map = stable_chunk_map(&after_chunks);
    let passes = before_map == after_map && !before_map.is_empty();
    let row = json!({
        "language": fixture.slug,
        "source_of_truth": {
            "before_file": before_path.display().to_string(),
            "after_file": after_path.display().to_string(),
        },
        "before_state": {
            "bytes": before_source.len(),
            "chunk_count": before_chunks.len(),
            "stable_hashes": before_map,
        },
        "after_state": {
            "bytes": after_source.len(),
            "chunk_count": after_chunks.len(),
            "stable_hashes": after_map,
        },
        "passes": passes,
    });
    require(
        &format!("{} reformat-invariant hashes", fixture.slug),
        passes,
    )?;
    Ok(row)
}

fn stable_chunk_map(
    chunks: &[context_graph_core::memory::ast::AstChunk],
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for chunk in chunks {
        if matches!(
            chunk.entity_type,
            EntityType::Function
                | EntityType::Method
                | EntityType::Class
                | EntityType::Struct
                | EntityType::Enum
                | EntityType::TraitOrInterface
                | EntityType::Impl
                | EntityType::Module
                | EntityType::Namespace
                | EntityType::TestFunction
        ) {
            map.insert(
                format!("{:?}::{}", chunk.entity_type, chunk.parent_chain.join("::")),
                chunk.sha256.clone(),
            );
        }
    }
    map
}

fn run_edge_cases(root: &Path) -> FsvResult<Value> {
    let empty = edge_case(
        "empty_input",
        b"",
        Language::Rust,
        "src/empty.rs",
        "MEJEPA_CHUNKER_EMPTY_SOURCE",
    );
    let invalid = edge_case(
        "invalid_format",
        b"def broken(:\n    return 1\n",
        Language::Python,
        "pkg/broken.py",
        "MEJEPA_CHUNKER_PARSE_FAILED",
    );
    let max = max_fixture_case(root)?;
    let edges = json!({
        "empty_input": empty,
        "invalid_format": invalid,
        "maximum_fixture": max,
    });
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

fn max_fixture_case(root: &Path) -> FsvResult<Value> {
    let mut compact = String::from("pub struct Big; impl Big {");
    let mut formatted = String::from("pub struct Big;\n\nimpl Big {\n");
    for idx in 0..64 {
        compact.push_str(&format!(" pub fn f{idx}(&self)->i32{{{idx}+1}}"));
        formatted.push_str(&format!(
            "    pub fn f{idx}(&self) -> i32 {{\n        {idx} + 1\n    }}\n"
        ));
    }
    compact.push_str(" }\n");
    formatted.push_str("}\n");
    let fixture = Fixture {
        language: Language::Rust,
        slug: "maximum_rust",
        path: "src/big.rs",
        compact: Box::leak(compact.into_boxed_str()),
        formatted: Box::leak(formatted.into_boxed_str()),
    };
    let row = run_fixture(root, fixture)?;
    Ok(json!({
        "before_state": row["before_state"].clone(),
        "after_state": row["after_state"].clone(),
        "passes": row["passes"].clone(),
    }))
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture { language: Language::Rust, slug: "rust", path: "src/lib.rs", compact: "pub struct Solver; impl Solver{pub fn compute(&self,x:i32)->i32{x+1}}\n", formatted: "pub struct Solver;\n\nimpl Solver {\n    pub fn compute(&self, x: i32) -> i32 {\n        x + 1\n    }\n}\n" },
        Fixture { language: Language::Python, slug: "python", path: "pkg/solver.py", compact: "class Solver:\n def compute(self,x:int)->int:\n  return x+1\n", formatted: "class Solver:\n    def compute(self, x: int) -> int:\n        return x + 1\n" },
        Fixture { language: Language::JavaScript, slug: "javascript", path: "src/solver.js", compact: "function compute(x){return x+1;} class Solver{run(x){return compute(x);}}\n", formatted: "function compute(x) {\n  return x + 1;\n}\nclass Solver {\n  run(x) {\n    return compute(x);\n  }\n}\n" },
        Fixture { language: Language::TypeScript, slug: "typescript", path: "src/solver.ts", compact: "export class Solver{compute(x:number):number{return x+1}}\n", formatted: "export class Solver {\n  compute(x: number): number {\n    return x + 1;\n  }\n}\n" },
        Fixture { language: Language::Go, slug: "go", path: "solver.go", compact: "package main\ntype Solver struct{}\nfunc (s Solver) Compute(x int) int{return x+1}\n", formatted: "package main\n\ntype Solver struct{}\n\nfunc (s Solver) Compute(x int) int {\n\treturn x + 1\n}\n" },
        Fixture { language: Language::Java, slug: "java", path: "Solver.java", compact: "class Solver{int compute(int x){return x+1;}}\n", formatted: "class Solver {\n    int compute(int x) {\n        return x + 1;\n    }\n}\n" },
        Fixture { language: Language::C, slug: "c", path: "solver.c", compact: "int compute(int x){return x+1;}\n", formatted: "int compute(int x) {\n    return x + 1;\n}\n" },
        Fixture { language: Language::Cpp, slug: "cpp", path: "solver.cpp", compact: "class Solver{public:int compute(int x){return x+1;}};\n", formatted: "class Solver {\npublic:\n    int compute(int x) {\n        return x + 1;\n    }\n};\n" },
        Fixture { language: Language::CSharp, slug: "csharp", path: "Solver.cs", compact: "class Solver{int Compute(int x){return x+1;}}\n", formatted: "class Solver {\n    int Compute(int x) {\n        return x + 1;\n    }\n}\n" },
        Fixture { language: Language::Ruby, slug: "ruby", path: "solver.rb", compact: "class Solver\n def compute(x)\n  x+1\n end\nend\n", formatted: "class Solver\n  def compute(x)\n    x + 1\n  end\nend\n" },
        Fixture { language: Language::Php, slug: "php", path: "solver.php", compact: "<?php\nclass Solver{function compute($x){return $x+1;}}\n", formatted: "<?php\nclass Solver {\n    function compute($x) {\n        return $x + 1;\n    }\n}\n" },
    ]
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    require(
        &format!("readback {}", path.display()),
        readback.as_slice() == bytes,
    )?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}
