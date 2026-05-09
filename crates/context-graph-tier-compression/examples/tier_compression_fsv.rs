// Manual Full State Verification for the 8/7/5/3-bit tier compressor.
//
// Source of truth on disk:
//   /tmp/contextgraph-tier-compression-fsv/blob_8.bin
//   /tmp/contextgraph-tier-compression-fsv/blob_7.bin
//   /tmp/contextgraph-tier-compression-fsv/blob_5.bin
//   /tmp/contextgraph-tier-compression-fsv/blob_3.bin
//   /tmp/contextgraph-tier-compression-fsv/tier-compression-evidence.json
//
// Plan:
//   1. Build a 768-dim deterministic synthetic vector (Box-Muller via SplitMix64).
//   2. Encode at 8/7/5/3-bit; write each blob to disk.
//   3. Re-read each blob from disk, deserialize, decode, verify:
//        a. file size == header (22) + ceil(n*bits/8)
//        b. compression ratio ≈ 32/bits
//        c. min/max recovered exactly from the header
//        d. max abs reconstruction error <= (max-min) / (2*(2^bits - 1))
//        e. cosine similarity above per-tier thresholds
//   4. Boundary cases (each printing before/after state to evidence JSON):
//        a. Empty input -> TIER_COMPRESSION_INVALID_INPUT
//        b. NaN input  -> TIER_COMPRESSION_INVALID_INPUT
//        c. Truncated on-disk blob -> TIER_COMPRESSION_INVALID_INPUT
//        d. Wrong-magic on-disk blob -> TIER_COMPRESSION_INVALID_INPUT

use context_graph_tier_compression::{
    decode, deserialize, encode, max_reconstruction_error, packed_len, serialize, BitWidth,
    CompressedBlob,
};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn ensure(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("tier-compression FSV invariant failed: {label}").into())
    }
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn gaussian_sample(seed: u64) -> f32 {
    let r1 = splitmix64(seed.wrapping_mul(2)) >> 11;
    let r2 = splitmix64(seed.wrapping_mul(2).wrapping_add(1)) >> 11;
    let u1 = (r1 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
    let u2 = (r2 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * std::f64::consts::PI * u2;
    let z = r * theta.cos();
    z.tanh() as f32
}

fn make_synthetic_vector(n: usize, seed: u64) -> Vec<f32> {
    (0..n)
        .map(|i| gaussian_sample(seed.wrapping_add(i as u64)))
        .collect()
}

fn write_text_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    let readback = fs::read(path)?;
    ensure(
        &format!("byte readback must match write at {}", path.display()),
        readback == bytes,
    )?;
    Ok(())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}

fn max_abs_error(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max)
}

fn happy_path_for_width(evidence_root: &Path, values: &[f32], bits: BitWidth) -> FsvResult<Value> {
    let blob = encode(values, bits)?;
    let bytes = serialize(&blob)?;
    let path = evidence_root.join(format!("blob_{}.bin", bits.bits()));
    write_text_checked(&path, &bytes)?;
    let on_disk = fs::read(&path)?;
    ensure(
        &format!("on-disk size matches serialize() output at {bits:?}"),
        on_disk == bytes,
    )?;
    let blob_back = deserialize(&on_disk)?;
    ensure(
        &format!("deserialize == in-memory blob at {bits:?}"),
        blob_back == blob,
    )?;
    let decoded = decode(&blob_back)?;
    ensure(
        &format!("decoded length matches input at {bits:?}"),
        decoded.len() == values.len(),
    )?;
    let bound = max_reconstruction_error(blob.min, blob.max, bits);
    let err = max_abs_error(values, &decoded);
    ensure(
        &format!("{bits:?}: max abs error {err} exceeds bound {bound}"),
        err <= bound + 1e-6,
    )?;
    let cos = cosine_similarity(values, &decoded);
    let cos_threshold = match bits {
        BitWidth::Eight => 0.99_f32,
        BitWidth::Seven => 0.99,
        BitWidth::Five => 0.95,
        BitWidth::Three => 0.80,
    };
    ensure(
        &format!("{bits:?}: cosine similarity {cos} below threshold {cos_threshold}"),
        cos >= cos_threshold,
    )?;
    let expected_payload = packed_len(values.len(), bits.bits())
        .expect("packed_len cannot overflow for an in-memory values vector");
    let f32_size = values.len() * 4;
    let on_disk_size = on_disk.len();
    let compression_ratio = f32_size as f64 / on_disk_size as f64;
    Ok(json!({
        "bits": bits.bits(),
        "blob_path": path.display().to_string(),
        "n": values.len(),
        "min": blob.min,
        "max": blob.max,
        "expected_payload_bytes": expected_payload,
        "actual_on_disk_bytes": on_disk_size,
        "raw_f32_bytes": f32_size,
        "compression_ratio_vs_f32": compression_ratio,
        "max_reconstruction_error_bound": bound,
        "max_abs_reconstruction_error": err,
        "cosine_similarity_to_input": cos,
        "passes": true,
    }))
}

fn boundary_empty_input() -> FsvResult<Value> {
    let before = json!({"input": "[]", "expected_code": "TIER_COMPRESSION_INVALID_INPUT"});
    let after = match encode(&[], BitWidth::Eight) {
        Err(e) => json!({
            "result": "error",
            "code": e.code(),
            "message": e.to_string(),
        }),
        Ok(_) => json!({"result": "ok", "code": "UNEXPECTED_OK"}),
    };
    ensure(
        "empty input must reject with TIER_COMPRESSION_INVALID_INPUT",
        after["code"].as_str() == Some("TIER_COMPRESSION_INVALID_INPUT"),
    )?;
    Ok(json!({"before": before, "after": after, "passes": true}))
}

fn boundary_nan_input() -> FsvResult<Value> {
    let v = vec![1.0_f32, 2.0, f32::NAN, 4.0];
    let before = json!({
        "input": ["1.0", "2.0", "NaN", "4.0"],
        "expected_code": "TIER_COMPRESSION_INVALID_INPUT",
    });
    let after = match encode(&v, BitWidth::Eight) {
        Err(e) => json!({"result": "error", "code": e.code(), "message": e.to_string()}),
        Ok(_) => json!({"result": "ok", "code": "UNEXPECTED_OK"}),
    };
    ensure(
        "NaN input must reject with TIER_COMPRESSION_INVALID_INPUT",
        after["code"].as_str() == Some("TIER_COMPRESSION_INVALID_INPUT"),
    )?;
    Ok(json!({"before": before, "after": after, "passes": true}))
}

fn boundary_truncated_blob(evidence_root: &Path) -> FsvResult<Value> {
    let v = make_synthetic_vector(64, 31);
    let blob = encode(&v, BitWidth::Five)?;
    let bytes = serialize(&blob)?;
    let path = evidence_root.join("truncated_blob.bin");
    let truncated = bytes[..bytes.len().saturating_sub(8)].to_vec();
    write_text_checked(&path, &truncated)?;
    let on_disk = fs::read(&path)?;
    let before = json!({
        "input_bytes_len": bytes.len(),
        "truncated_bytes_len": truncated.len(),
        "expected_code": "TIER_COMPRESSION_INVALID_INPUT",
    });
    let after = match deserialize(&on_disk) {
        Err(e) => json!({"result": "error", "code": e.code(), "message": e.to_string()}),
        Ok(_) => json!({"result": "ok", "code": "UNEXPECTED_OK"}),
    };
    ensure(
        "truncated blob must reject with TIER_COMPRESSION_INVALID_INPUT",
        after["code"].as_str() == Some("TIER_COMPRESSION_INVALID_INPUT"),
    )?;
    Ok(json!({"before": before, "after": after, "passes": true}))
}

fn boundary_wrong_magic(evidence_root: &Path) -> FsvResult<Value> {
    let v = vec![0.0_f32, 1.0];
    let blob = encode(&v, BitWidth::Eight)?;
    let mut bytes = serialize(&blob)?;
    bytes[0] = b'X';
    bytes[1] = b'X';
    bytes[2] = b'X';
    bytes[3] = b'X';
    let path = evidence_root.join("wrong_magic_blob.bin");
    write_text_checked(&path, &bytes)?;
    let on_disk = fs::read(&path)?;
    let before = json!({
        "magic_bytes_in_input": "XXXX (instead of CGTC)",
        "expected_code": "TIER_COMPRESSION_INVALID_INPUT",
    });
    let after = match deserialize(&on_disk) {
        Err(e) => json!({"result": "error", "code": e.code(), "message": e.to_string()}),
        Ok(_) => json!({"result": "ok", "code": "UNEXPECTED_OK"}),
    };
    ensure(
        "wrong magic must reject with TIER_COMPRESSION_INVALID_INPUT",
        after["code"].as_str() == Some("TIER_COMPRESSION_INVALID_INPUT"),
    )?;
    Ok(json!({"before": before, "after": after, "passes": true}))
}

fn main() -> FsvResult<()> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-tier-compression-fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root)?;
    }
    fs::create_dir_all(&evidence_root)?;

    let n = 768;
    let values = make_synthetic_vector(n, 42);
    ensure("input vector length matches plan", values.len() == n)?;

    let mut per_bit_evidence = serde_json::Map::new();
    for bits in BitWidth::all() {
        let case = happy_path_for_width(&evidence_root, &values, bits)?;
        per_bit_evidence.insert(format!("{}_bit", bits.bits()), case);
    }

    let evidence = json!({
        "header_size_bytes": CompressedBlob::HEADER_LEN,
        "input_n": n,
        "input_seed": 42u64,
        "input_min": values.iter().cloned().fold(f32::INFINITY, f32::min),
        "input_max": values.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
        "happy_path_per_width": per_bit_evidence,
        "boundary_empty_input": boundary_empty_input()?,
        "boundary_nan_input": boundary_nan_input()?,
        "boundary_truncated_blob": boundary_truncated_blob(&evidence_root)?,
        "boundary_wrong_magic": boundary_wrong_magic(&evidence_root)?,
        "passes": true,
    });

    let evidence_path = evidence_root.join("tier-compression-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    write_text_checked(&evidence_path, text.as_bytes())?;

    println!("Tier-compression FSV evidence: {}", evidence_path.display());
    println!("{}", text);
    Ok(())
}
