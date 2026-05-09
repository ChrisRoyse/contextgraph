use context_graph_core::memory::ast::{route_to_embedders, EmbedderId, EntityType, Language};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = Path::new("/tmp/contextgraph-mejepa-embedders-fsv");
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;

    let mut rows = Vec::new();
    for language in Language::all() {
        for entity_type in EntityType::all() {
            let actual = route_to_embedders(entity_type);
            let expected = expected_route(entity_type);
            require(
                &format!("{language:?} {entity_type:?} route"),
                actual == expected && !actual.is_empty(),
            )?;
            rows.push(json!({
                "language": language,
                "entity_type": entity_type,
                "routed_embedders": actual,
            }));
        }
    }

    let path = root.join("routing_coverage.json");
    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "routing_coverage_json": path.display().to_string(),
        },
        "before_state": {
            "language_count": Language::all().len(),
            "entity_type_count": EntityType::all().len(),
            "expected_rows": Language::all().len() * EntityType::all().len(),
        },
        "after_state": {
            "row_count": rows.len(),
            "rows": rows,
        },
    });
    write_secure_json(&path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    require(
        "routing row count readback",
        readback["after_state"]["row_count"] == Value::from(143),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fsv": "embedder_routing",
            "routing_coverage_json": path.display().to_string(),
            "row_count": readback["after_state"]["row_count"],
        }))?
    );
    Ok(())
}

fn expected_route(entity_type: EntityType) -> std::collections::BTreeSet<EmbedderId> {
    use EmbedderId::*;
    use EntityType::*;

    match entity_type {
        Function | Method => [E7, E14].into(),
        Class | Struct | Enum | TraitOrInterface | Impl => [E7, E11].into(),
        Module | Namespace => [E1, E8].into(),
        TestFunction => [E1, E7].into(),
        Import => [E11, E13].into(),
        CommentBlock | Docstring => [E1, E10].into(),
    }
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
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
