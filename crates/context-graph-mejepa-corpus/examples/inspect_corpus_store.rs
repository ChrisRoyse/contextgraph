use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};
use std::path::PathBuf;

const LOWER_MUTATION_CF: &str = "mejepa_mutation_corpus";
const LOWER_VERDICT_CF: &str = "mejepa_oracle_verdicts";
const LOWER_PROVENANCE_CF: &str = "mejepa_corpus_provenance";

const CANONICAL_MUTATION_CF: &str = "CF_MEJEPA_MUTATION_CORPUS";
const CANONICAL_VERDICT_CF: &str = "CF_MEJEPA_ORACLE_VERDICTS";
const CANONICAL_PROVENANCE_CF: &str = "CF_MEJEPA_CORPUS_PROVENANCE";

const TARGET_CFS: &[&str] = &[
    LOWER_MUTATION_CF,
    LOWER_VERDICT_CF,
    LOWER_PROVENANCE_CF,
    CANONICAL_MUTATION_CF,
    CANONICAL_VERDICT_CF,
    CANONICAL_PROVENANCE_CF,
];

type InspectResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> InspectResult<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: inspect_corpus_store <rocksdb-path>")?;

    let mut db_opts = Options::default();
    db_opts.create_if_missing(false);
    db_opts.create_missing_column_families(false);
    db_opts.set_paranoid_checks(true);

    let column_families = DB::list_cf(&db_opts, &path)?;
    let descriptors = column_families
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(name.clone(), cf_options()))
        .collect::<Vec<_>>();
    let db = DB::open_cf_descriptors_read_only(&db_opts, &path, descriptors, false)?;

    let target_scans = TARGET_CFS
        .iter()
        .map(|name| scan_target_cf(&db, name))
        .collect::<InspectResult<Vec<_>>>()?;

    let lower_mutation_count = count_cf(&db, LOWER_MUTATION_CF)?;
    let lower_verdict_count = count_cf(&db, LOWER_VERDICT_CF)?;
    let lower_provenance_count = count_cf(&db, LOWER_PROVENANCE_CF)?;
    let canonical_mutation_count = count_cf(&db, CANONICAL_MUTATION_CF)?;
    let canonical_verdict_count = count_cf(&db, CANONICAL_VERDICT_CF)?;
    let canonical_provenance_count = count_cf(&db, CANONICAL_PROVENANCE_CF)?;

    let lower_complete = lower_mutation_count == Some(2400) && lower_verdict_count == Some(2400);
    let canonical_complete =
        canonical_mutation_count == Some(2400) && canonical_verdict_count == Some(2400);

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "source_of_truth": {
                "rocksdb_path": path,
                "read_mode": "read_only",
                "discovered_column_family_count": column_families.len(),
                "discovered_column_families": column_families,
            },
            "target_column_families": target_scans,
            "complete_sets": {
                "corpus_oracle_store_json": {
                    "mutation_cf": LOWER_MUTATION_CF,
                    "mutation_cf_count": lower_mutation_count,
                    "verdict_cf": LOWER_VERDICT_CF,
                    "verdict_cf_count": lower_verdict_count,
                    "provenance_cf": LOWER_PROVENANCE_CF,
                    "provenance_cf_count": lower_provenance_count,
                    "is_2400_complete": lower_complete,
                    "gate_countable": false,
                    "payload_contract": "JSON MutationOutcome + JSON OracleVerdict corpus rows only",
                },
                "canonical_contextgraph_store": {
                    "mutation_cf": CANONICAL_MUTATION_CF,
                    "mutation_cf_count": canonical_mutation_count,
                    "verdict_cf": CANONICAL_VERDICT_CF,
                    "verdict_cf_count": canonical_verdict_count,
                    "provenance_cf": CANONICAL_PROVENANCE_CF,
                    "provenance_cf_count": canonical_provenance_count,
                    "is_2400_complete": canonical_complete,
                    "gate_countable": true,
                },
            },
            "corpus_oracle_store_complete": lower_complete,
            "canonical_contextgraph_store_complete": canonical_complete,
            "lowercase_only_is_not_gate_complete": lower_complete && !canonical_complete,
            "is_2400_complete": canonical_complete,
            "gate_contract_note": "lowercase corpus oracle CFs are read-only corpus source rows; they do not prove canonical runtime/gate CF completeness",
        }))?
    );
    Ok(())
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn scan_target_cf(db: &DB, cf_name: &str) -> InspectResult<Value> {
    let count = count_cf(db, cf_name)?;
    let sample_rows = if count.is_some() {
        sample_cf(db, cf_name, 5)?
    } else {
        Vec::new()
    };
    Ok(json!({
        "name": cf_name,
        "present": count.is_some(),
        "count": count,
        "sample_rows": sample_rows,
    }))
}

fn count_cf(db: &DB, cf_name: &str) -> InspectResult<Option<usize>> {
    let Some(cf) = db.cf_handle(cf_name) else {
        return Ok(None);
    };
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(Some(count))
}

fn sample_cf(db: &DB, cf_name: &str, limit: usize) -> InspectResult<Vec<Value>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing column family {cf_name}"))?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start).take(limit) {
        let (key, value) = item?;
        rows.push(json!({
            "key": lossy_utf8_or_hex(&key),
            "value_len": value.len(),
            "value_json": serde_json::from_slice::<Value>(&value).ok(),
        }));
    }
    Ok(rows)
}

fn lossy_utf8_or_hex(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(text) => json!({"utf8": text}),
        Err(_) => json!({"hex": hex_encode(bytes)}),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
