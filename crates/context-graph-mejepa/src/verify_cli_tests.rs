// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::Path;

use super::*;

#[test]
fn splitmix64_seed_is_deterministic_per_task_id() {
    let first = sample_order("astropy__astropy-12907");
    let second = sample_order("astropy__astropy-12907");
    assert_eq!(first, second);
}

#[test]
fn task_id_validator_rejects_unknown_task() {
    assert!(validate_task_id("missing__repo-999999").is_err());
}

#[test]
fn patch_path_rejects_traversal() {
    assert!(canonicalize_patch_path(Path::new("../../etc/passwd")).is_err());
}

fn sample_order(task: &str) -> Vec<u64> {
    ["a", "b", "c"]
        .into_iter()
        .map(|candidate| splitmix64(seed64(task) ^ seed64(candidate)))
        .collect()
}
