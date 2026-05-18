use crate::synthetic_stress::{
    ClaimReconciliationExpectation, SyntheticExpectedVerdict, SyntheticStressKind,
    SYNTHETIC_STRESS_SCHEMA_VERSION,
};
use crate::types::{FailureModeClass, Verdict};

#[derive(Debug, Clone)]
pub(crate) struct SyntheticStressTemplate {
    pub(crate) case_id: String,
    pub(crate) kind: SyntheticStressKind,
    pub(crate) title: String,
    pub(crate) code: String,
    pub(crate) test: String,
    pub(crate) expected: SyntheticExpectedVerdict,
}

pub(crate) fn synthetic_stress_templates() -> Vec<SyntheticStressTemplate> {
    let mut out = Vec::new();
    for idx in 0..10 {
        out.push(work_case(idx));
    }
    for idx in 0..10 {
        out.push(broken_case(idx));
    }
    for idx in 0..5 {
        out.push(passes_but_should_fail_case(idx));
        out.push(edge_case_trap_case(idx));
    }
    out.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    out
}

fn work_case(idx: usize) -> SyntheticStressTemplate {
    let (title, code, test) = match idx {
        0 => (
            "correct binary search",
            "def binary_search(items, needle):\n    low = 0\n    high = len(items) - 1\n    while low <= high:\n        mid = (low + high) // 2\n        value = items[mid]\n        if value == needle:\n            return mid\n        if value < needle:\n            low = mid + 1\n        else:\n            high = mid - 1\n    return -1\n",
            "from code import binary_search\n\ndef test_binary_search():\n    assert binary_search([1, 3, 5], 5) == 2\n",
        ),
        1 => (
            "locked fifo queue",
            "from threading import Lock\n\nclass FifoQueue:\n    def __init__(self):\n        self.items = []\n        self.lock = Lock()\n    def push(self, item):\n        with self.lock:\n            self.items.append(item)\n    def pop(self):\n        with self.lock:\n            return self.items.pop(0) if self.items else None\n",
            "from code import FifoQueue\n\ndef test_queue():\n    q = FifoQueue(); q.push('a'); assert q.pop() == 'a'\n",
        ),
        2 => (
            "sliding window sum",
            "def sliding_sum(items, width):\n    out = []\n    for start in range(0, len(items) - width + 1):\n        total = 0\n        for value in items[start:start + width]:\n            total += value\n        out.append(total)\n    return out\n",
            "from code import sliding_sum\n\ndef test_sliding_sum():\n    assert sliding_sum([1, 2, 3], 2) == [3, 5]\n",
        ),
        3 => (
            "correct leap year",
            "def is_leap_year(year):\n    return year % 400 == 0 or (year % 4 == 0 and year % 100 != 0)\n",
            "from code import is_leap_year\n\ndef test_leap_year():\n    assert is_leap_year(2000)\n",
        ),
        4 => (
            "unicode normalization",
            "def normalize_name(value):\n    return value.casefold().strip()\n",
            "from code import normalize_name\n\ndef test_normalize_name():\n    assert normalize_name(' ADA ') == 'ada'\n",
        ),
        5 => (
            "first or default",
            "def first_or_default(items, default=None):\n    if not items:\n        return default\n    return items[0]\n",
            "from code import first_or_default\n\ndef test_first_or_default():\n    assert first_or_default([], 'x') == 'x'\n",
        ),
        6 => (
            "clamp range",
            "def clamp(value, low, high):\n    if value < low:\n        return low\n    if value > high:\n        return high\n    return value\n",
            "from code import clamp\n\ndef test_clamp():\n    assert clamp(10, 0, 5) == 5\n",
        ),
        7 => (
            "parse bool",
            "def parse_bool(value):\n    lowered = value.strip().casefold()\n    if lowered in {'true', 'yes', '1'}:\n        return True\n    if lowered in {'false', 'no', '0'}:\n        return False\n    raise ValueError('unknown boolean')\n",
            "from code import parse_bool\n\ndef test_parse_bool():\n    assert parse_bool('YES') is True\n",
        ),
        8 => (
            "merge sorted",
            "def merge_sorted(left, right):\n    out = []\n    i = j = 0\n    while i < len(left) and j < len(right):\n        if left[i] <= right[j]:\n            out.append(left[i]); i += 1\n        else:\n            out.append(right[j]); j += 1\n    out.extend(left[i:]); out.extend(right[j:])\n    return out\n",
            "from code import merge_sorted\n\ndef test_merge_sorted():\n    assert merge_sorted([1, 4], [2, 3]) == [1, 2, 3, 4]\n",
        ),
        _ => (
            "word count",
            "def count_words(text):\n    counts = {}\n    for word in text.casefold().split():\n        counts[word] = counts.get(word, 0) + 1\n    return counts\n",
            "from code import count_words\n\ndef test_count_words():\n    assert count_words('A a')['a'] == 2\n",
        ),
    };
    template(
        idx,
        SyntheticStressKind::WorksAsClaimed,
        title,
        code,
        test,
        clean_expected(),
    )
}

fn broken_case(idx: usize) -> SyntheticStressTemplate {
    let cases = failure_cases();
    let (title, code, test, expected) = cases[idx].clone();
    template(
        idx,
        SyntheticStressKind::BrokenInObviousWay,
        title,
        code,
        test,
        expected,
    )
}

fn passes_but_should_fail_case(idx: usize) -> SyntheticStressTemplate {
    let cases = failure_cases();
    let source_idx = match idx {
        0 => 6,
        1 => 7,
        2 => 8,
        3 => 9,
        _ => 0,
    };
    let (title, code, _test, expected) = cases[source_idx].clone();
    let weak_test = match idx {
        0 => "from code import is_even\n\ndef test_is_even_weak():\n    assert is_even(2)\n",
        1 => "from code import normalize_name\n\ndef test_ascii_only():\n    assert normalize_name('ada') == 'ada'\n",
        2 => "from code import is_leap_year\n\ndef test_modern_leap():\n    assert is_leap_year(2024)\n",
        3 => "from code import first_item\n\ndef test_non_empty():\n    assert first_item([1]) == 1\n",
        _ => "from code import average\n\ndef test_average_non_empty():\n    assert average([2, 4]) == 3\n",
    };
    template(
        idx,
        SyntheticStressKind::PassesButShouldFail,
        title,
        code,
        weak_test,
        expected,
    )
}

fn edge_case_trap_case(idx: usize) -> SyntheticStressTemplate {
    let cases = failure_cases();
    let source_idx = match idx {
        0 => 9,
        1 => 7,
        2 => 8,
        3 => 0,
        _ => 1,
    };
    let (title, code, test, expected) = cases[source_idx].clone();
    template(
        idx,
        SyntheticStressKind::EdgeCaseTrap,
        title,
        code,
        test,
        expected,
    )
}

fn failure_cases() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    SyntheticExpectedVerdict,
)> {
    vec![
        ("unguarded average", "def average(values):\n    total = sum(values)\n    return total / len(values)\n", "from code import average\n\ndef test_average():\n    assert average([2, 4]) == 3\n", failure_expected(FailureModeClass::Exception, "DivisionByZero", "denominator value equals zero")),
        ("off by one binary search", "def binary_search(items, needle):\n    low = 0\n    high = len(items)\n    while low <= high:\n        mid = (low + high) // 2\n        if items[mid] == needle:\n            return mid\n        if items[mid] < needle:\n            low = mid + 1\n        else:\n            high = mid - 1\n    return -1\n", "from code import binary_search\n\ndef test_binary_search_weak():\n    assert binary_search([1, 3, 5], 3) == 1\n", failure_expected(FailureModeClass::OffByOne, "OffByOne", "needle at final index")),
        ("unlocked fifo queue", "class UnsafeQueue:\n    def __init__(self):\n        self.items = []\n    def push(self, item):\n        self.items.append(item)\n    def pop(self):\n        return self.items.pop(0) if self.items else None\n", "from code import UnsafeQueue\n\ndef test_queue_weak():\n    q = UnsafeQueue(); q.push('a'); assert q.pop() == 'a'\n", failure_expected(FailureModeClass::RaceCondition, "RaceCondition", "two workers pop")),
        ("dynamic eval", "def run_formula(expr):\n    return eval(expr)\n", "from code import run_formula\n\ndef test_formula():\n    assert run_formula('1 + 1') == 2\n", q4_expected("dynamic eval call")),
        ("shell command", "import subprocess\n\ndef run_backup(path):\n    return subprocess.run('tar cf out.tar ' + path, shell=True)\n", "from code import run_backup\n\ndef test_backup_symbol():\n    assert callable(run_backup)\n", q4_expected("shell=True command")),
        ("quadratic pair sum", "def pair_sum(items):\n    total = 0\n    # STRESS_PERF_QUADRATIC\n    for left in items:\n        for right in items:\n            total += left * right\n    return total\n", "from code import pair_sum\n\ndef test_pair_sum():\n    assert pair_sum([1, 2]) == 9\n", failure_expected(FailureModeClass::Timeout, "QuadraticPerf", "large input nested-loop budget")),
        ("constant predicate", "def is_even(value):\n    return True\n", "from code import is_even\n\ndef test_even():\n    assert is_even(2)\n", failure_expected(FailureModeClass::ContractViolation, "ContractViolation", "predicate returns")),
        ("ascii-only normalization", "def normalize_name(value):\n    return value.encode(\"ascii\").decode(\"ascii\").casefold()\n", "from code import normalize_name\n\ndef test_ascii():\n    assert normalize_name('Ada') == 'ada'\n", failure_expected(FailureModeClass::EncodingError, "EncodingError", "non-ASCII unicode input")),
        ("century leap year", "def is_leap_year(year):\n    return year % 4 == 0\n", "from code import is_leap_year\n\ndef test_2024():\n    assert is_leap_year(2024)\n", failure_expected(FailureModeClass::DateTimeError, "DateTimeError", "century leap-year boundary")),
        ("first item without empty guard", "def first_item(items):\n    return items[0]\n", "from code import first_item\n\ndef test_first_item():\n    assert first_item([1]) == 1\n", failure_expected(FailureModeClass::Exception, "EmptyInput", "empty input collection")),
    ]
}

fn template(
    idx: usize,
    kind: SyntheticStressKind,
    title: &str,
    code: &str,
    test: &str,
    expected: SyntheticExpectedVerdict,
) -> SyntheticStressTemplate {
    SyntheticStressTemplate {
        case_id: format!("{}_{idx:02}", kind.case_id_prefix()),
        kind,
        title: title.to_string(),
        code: code.to_string(),
        test: test.to_string(),
        expected,
    }
}

fn clean_expected() -> SyntheticExpectedVerdict {
    SyntheticExpectedVerdict {
        schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
        verdict: Verdict::Abstain,
        top_failure_mode: None,
        top_failure_explanation_contains: None,
        top_q4_concerns: Vec::new(),
        predicted_works: true,
        claim_reconciliation_expectation: ClaimReconciliationExpectation::NoAgentClaims,
    }
}

fn failure_expected(
    class: FailureModeClass,
    explanation: &str,
    q4: &str,
) -> SyntheticExpectedVerdict {
    SyntheticExpectedVerdict {
        schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
        verdict: Verdict::Abstain,
        top_failure_mode: Some(class),
        top_failure_explanation_contains: Some(explanation.to_string()),
        top_q4_concerns: vec![q4.to_string()],
        predicted_works: false,
        claim_reconciliation_expectation: ClaimReconciliationExpectation::NoAgentClaims,
    }
}

fn q4_expected(concern: &str) -> SyntheticExpectedVerdict {
    SyntheticExpectedVerdict {
        schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
        verdict: Verdict::Abstain,
        top_failure_mode: None,
        top_failure_explanation_contains: None,
        top_q4_concerns: vec![concern.to_string()],
        predicted_works: false,
        claim_reconciliation_expectation: ClaimReconciliationExpectation::NoAgentClaims,
    }
}
