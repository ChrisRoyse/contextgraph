// E_Oracle instrument — encodes the actual oracle outcome of an attempt
// into a 128-d latent. Per doc 09 §2.2 row 9.
//
// Layout of the 128-d output:
//   dims  0..  4: aggregate test-outcome counts (Pass / Fail / Skip / Error)
//                 normalized by total test count (so the vector is
//                 distribution-shaped). If `total == 0`, all four are 0.
//   dim         4: total test count (raw; bounded to 1e6 so f32 stores it
//                 exactly and a pathological runner output cannot allocate
//                 unbounded panel work)
//   dim         5: has_exception flag (0.0 or 1.0)
//   dims  6.. 16: one-hot exception class. Slot order matches
//                 `ExceptionClass::all()`.
//   dims 16..128: zero-padding reserved for future signal dims (per
//                 doc 09 §3.5: stack-trace embedding extension to E_Oracle).
//
// Inputs are real Python pytest oracle outputs. No mocks; `OracleVerdict`
// is a faithful representation of `pytest --tb=long --json` per-test rows
// plus the top-level exception class if execution crashed before tests
// ran.

use serde::{Deserialize, Serialize};

use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

fn is_false(value: &bool) -> bool {
    !*value
}

/// Per-test outcome from a Python test runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    Pass,
    Fail,
    Skip,
    /// pytest's "error" — the test could not be collected / set up.
    Error,
}

impl TestOutcome {
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Error => "error",
        }
    }
}

/// Top-level exception class observed when execution crashed before tests
/// ran (or the test framework itself raised one). 10 classes covering the
/// most-common Python exception types in SWE-bench Lite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionClass {
    AssertionError,
    TypeError,
    ValueError,
    ImportError,
    AttributeError,
    KeyError,
    IndexError,
    NameError,
    RuntimeError,
    /// Catch-all for everything not in the curated list above.
    Other,
}

impl ExceptionClass {
    pub fn all() -> [Self; 10] {
        [
            Self::AssertionError,
            Self::TypeError,
            Self::ValueError,
            Self::ImportError,
            Self::AttributeError,
            Self::KeyError,
            Self::IndexError,
            Self::NameError,
            Self::RuntimeError,
            Self::Other,
        ]
    }

    pub fn slug(&self) -> &'static str {
        match self {
            Self::AssertionError => "assertion_error",
            Self::TypeError => "type_error",
            Self::ValueError => "value_error",
            Self::ImportError => "import_error",
            Self::AttributeError => "attribute_error",
            Self::KeyError => "key_error",
            Self::IndexError => "index_error",
            Self::NameError => "name_error",
            Self::RuntimeError => "runtime_error",
            Self::Other => "other",
        }
    }

    pub fn one_hot_index(&self) -> usize {
        Self::all()
            .iter()
            .position(|x| x == self)
            .expect("ExceptionClass::all is exhaustive")
    }
}

/// One row of pytest output. `test_id` is the full pytest test identifier
/// (e.g. `tests/test_compat.py::TestCompat::test_unicode`). `runtime_ms`
/// is per-test wall-clock from `pytest --durations=0`; -1 if unavailable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerTestOutcome {
    pub test_id: String,
    pub outcome: TestOutcome,
    pub runtime_ms: i64,
}

/// Top-level oracle verdict for one attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleVerdict {
    pub per_test: Vec<PerTestOutcome>,
    /// Set if execution crashed before tests ran OR the test framework
    /// itself raised an exception.
    pub exception: Option<ExceptionClass>,
    /// True when the oracle has not run yet and the slot is intentionally
    /// unavailable. This is distinct from observed Skip/Error outcomes.
    #[serde(default, skip_serializing_if = "is_false")]
    pub evidence_unavailable: bool,
}

impl OracleVerdict {
    /// True if every test passed AND no exception was raised. The "happy"
    /// oracle outcome ME-JEPA's predict head is trying to learn.
    pub fn all_passed(&self) -> bool {
        !self.evidence_unavailable
            && self.exception.is_none()
            && !self.per_test.is_empty()
            && self.per_test.iter().all(|t| t.outcome == TestOutcome::Pass)
    }
}

/// Encoder for E_Oracle (slot 9 of 15 in the panel).
#[derive(Debug, Default, Clone, Copy)]
pub struct EOracleInstrument;

impl EOracleInstrument {
    pub const MAX_TEST_COUNT: usize = 1_000_000;
}

impl Instrument for EOracleInstrument {
    type Input = OracleVerdict;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EOracle
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        if input.evidence_unavailable {
            if !input.per_test.is_empty() || input.exception.is_some() {
                return Err(InstrumentError::invalid(
                    "input.evidence_unavailable",
                    "unavailable oracle evidence cannot also carry observed outcomes",
                    "set evidence_unavailable=false before recording observed oracle rows",
                ));
            }
            let mut out = vec![0.0_f32; self.output_dim()];
            out[16] = 1.0;
            return Ok(out);
        }
        if input.per_test.is_empty() && input.exception.is_none() {
            return Err(InstrumentError::invalid(
                "input",
                "oracle verdict has no per-test outcomes and no exception",
                "record at least one PerTestOutcome, or set exception=Other for no-tests-collected / runner-failed outcomes",
            ));
        }
        if input.per_test.len() > Self::MAX_TEST_COUNT {
            return Err(InstrumentError::invalid(
                "input.per_test",
                format!(
                    "per_test contains {} rows; max supported rows is {}",
                    input.per_test.len(),
                    Self::MAX_TEST_COUNT
                ),
                "aggregate or shard the oracle output before encoding this instrument",
            ));
        }
        for (i, test) in input.per_test.iter().enumerate() {
            if test.test_id.trim().is_empty() {
                return Err(InstrumentError::invalid(
                    "input.per_test[i].test_id",
                    format!("per_test[{i}].test_id is empty or whitespace-only"),
                    "every PerTestOutcome must have a non-empty test_id",
                ));
            }
            if test.test_id.chars().any(char::is_control) {
                return Err(InstrumentError::invalid(
                    "input.per_test[i].test_id",
                    format!("per_test[{i}].test_id contains a control character"),
                    "store pytest node ids as single-line UTF-8 strings without control characters",
                ));
            }
            if test.runtime_ms < -1 {
                return Err(InstrumentError::invalid(
                    "input.per_test[i].runtime_ms",
                    format!("per_test[{i}].runtime_ms is {}", test.runtime_ms),
                    "use -1 only when runtime is unavailable; otherwise runtime_ms must be >= 0",
                ));
            }
        }
        let dim = self.output_dim();
        let mut out = vec![0.0_f32; dim];
        let total = input.per_test.len();
        if total > 0 {
            let mut counts = [0u32; 4];
            for test in &input.per_test {
                let idx = match test.outcome {
                    TestOutcome::Pass => 0,
                    TestOutcome::Fail => 1,
                    TestOutcome::Skip => 2,
                    TestOutcome::Error => 3,
                };
                counts[idx] += 1;
            }
            let total_f = total as f32;
            for k in 0..4 {
                out[k] = counts[k] as f32 / total_f;
            }
        }
        out[4] = total as f32;
        out[5] = if input.exception.is_some() { 1.0 } else { 0.0 };
        if let Some(class) = input.exception {
            out[6 + class.one_hot_index()] = 1.0;
        }
        Ok(out)
    }
}
