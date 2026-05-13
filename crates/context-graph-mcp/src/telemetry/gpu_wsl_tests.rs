use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use super::*;

#[test]
fn parses_happy_path_fields() {
    let telemetry =
        parse_nvidia_smi_output(b"NVIDIA RTX 5090, 32607, 30000, 2607\n", DEFAULT_QUERY).unwrap();
    assert_eq!(telemetry.fields["memory.total"].value, Some(32607.0));
    assert!(telemetry.unavailable_fields.is_empty());
}

#[test]
fn marks_na_sentinel_unavailable_without_fake_value() {
    let telemetry =
        parse_nvidia_smi_output(b"NVIDIA RTX 5090, N/A, [N/A], 10\n", DEFAULT_QUERY).unwrap();
    assert_eq!(
        telemetry.unavailable_fields,
        vec!["memory.total".to_string(), "memory.free".to_string()]
    );
    assert!(telemetry.fields["memory.total"].unavailable);
    assert_eq!(telemetry.fields["memory.total"].value, None);
}

#[test]
fn rejects_malformed_column_count() {
    let err = parse_nvidia_smi_output(b"NVIDIA RTX 5090, 32607\n", DEFAULT_QUERY).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_GPU_WSL_MALFORMED_OUTPUT");
}

#[test]
fn rejects_bad_numeric_field() {
    let err = parse_nvidia_smi_output(b"NVIDIA RTX 5090, thirty-two, 30000, 2607\n", DEFAULT_QUERY)
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_GPU_WSL_INVALID_NUMERIC_FIELD");
}

#[tokio::test]
async fn retries_transient_timeouts() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let telemetry = nvidia_smi_query_with_runner(DEFAULT_QUERY, {
        let attempts = Arc::clone(&attempts);
        move || {
            let attempts = Arc::clone(&attempts);
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err(GpuError::Timeout)
                } else {
                    Ok(b"NVIDIA RTX 5090, 32607, 30000, 2607\n".to_vec())
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(telemetry.attempt_count, 3);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}
