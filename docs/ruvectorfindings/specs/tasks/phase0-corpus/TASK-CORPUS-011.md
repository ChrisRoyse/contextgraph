# TASK-CORPUS-011 — SplitMix64 PRNG Helper

```xml
<task_spec id="TASK-CORPUS-011" version="1.0">
<metadata>
  <title>SplitMix64 PRNG helper (prng::pick_index_or_fail / prng::next_u64)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
    <requirement_ref>REQ-CORPUS-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Logic task. SplitMix64 is the only PRNG used in this crate (per
TECH-CORPUS `<implementation_notes category="determinism">`).
Operators and the splitter both consume it. Already shipped per
the project memory — this task verifies the shipped state and
adds any missing tests.
</context>

<input_context_files>
  <file purpose="determinism_rule">../../technical/TECH-CORPUS.md#determinism_contract</file>
  <file purpose="splitmix_reference">https://prng.di.unimi.it/splitmix64.c (read-only — clean-room reimplementation)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/prng.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-001 complete (MutationError exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `pick_index_or_fail(seed: u64,
      candidate_count: usize) -&gt; MutationResult&lt;usize&gt;`
      that returns Err if candidate_count == 0 and otherwise
      returns a deterministic index in `0..candidate_count`.
    - Verify shipped `next_u64(state: &amp;mut u64) -&gt; u64`
      using the canonical SplitMix64 algorithm:
        z = state += 0x9e3779b97f4a7c15
        z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9
        z = (z ^ (z >> 27)) * 0x94d049bb133111eb
        z = z ^ (z >> 31)
    - Add `pub(crate) fn shuffle_in_place&lt;T&gt;(slice:
      &amp;mut [T], seed: u64)` — Fisher–Yates shuffle using
      `next_u64` for index generation. Used by split_corpus.
    - Tests: SplitMix64 first 4 outputs from seed 0 match the
      reference vector
      (e57ac68a37cf, 4d6e2c5b8d52, 6a31a4d4b8b1, 9d05f88b1e8b — or
      whichever the canonical reference implementation produces);
      pick_index_or_fail rejects 0; pick_index_or_fail is
      deterministic for fixed seed; shuffle is permutation
      (no element added/removed) and deterministic.
  </in_scope>
  <out_of_scope>
    - Use of the PRNG in operators / split (later tasks).
    - Cryptographic randomness (no security claims).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/prng.rs">
    pub(crate) fn next_u64(state: &amp;mut u64) -&gt; u64;
    pub(crate) fn pick_index_or_fail(seed: u64, candidate_count: usize) -&gt; crate::MutationResult&lt;usize&gt;;
    pub(crate) fn shuffle_in_place&lt;T&gt;(slice: &amp;mut [T], seed: u64);
    </signature>
  </signatures>

  <constraints>
    - The three SplitMix64 magic constants MUST be exact:
      0x9E3779B97F4A7C15, 0xBF58476D1CE4E5B9, 0x94D049BB133111EB.
    - `next_u64` mutates state in place — no Box, no Cell.
    - `shuffle_in_place` uses `slice.len() - 1` iterations
      descending so the last element has zero swap chance per
      Fisher–Yates correctness.
    - No allocation in the hot path beyond what the algorithm needs.
    - Match the reference SplitMix64 byte-for-byte; documented in
      a comment with the source citation
      (`// Inspired by https://prng.di.unimi.it/splitmix64.c
      (clean-room reimplementation; original by Sebastiano Vigna).`).
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- prng` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub(crate) fn next_u64(state: &amp;mut u64) -&gt; u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z &gt;&gt; 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z &gt;&gt; 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z &gt;&gt; 31)
}

pub(crate) fn pick_index_or_fail(seed: u64, candidate_count: usize) -&gt; MutationResult&lt;usize&gt; {
    if candidate_count == 0 {
        return Err(MutationError::no_site("candidates", "no candidate sites available", "see operator docs"));
    }
    let mut state = seed;
    Ok((next_u64(&amp;mut state) as usize) % candidate_count)
}

pub(crate) fn shuffle_in_place&lt;T&gt;(slice: &amp;mut [T], seed: u64) {
    let mut state = seed;
    for i in (1..slice.len()).rev() {
        let j = (next_u64(&amp;mut state) as usize) % (i + 1);
        slice.swap(i, j);
    }
}

#[test]
fn splitmix64_canonical_output_seed_zero() {
    let mut s = 0u64;
    let v: Vec&lt;u64&gt; = (0..4).map(|_| next_u64(&amp;mut s)).collect();
    assert_eq!(v, vec![/* canonical reference values */]);
}

#[test]
fn shuffle_is_permutation_and_deterministic() {
    let mut a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut b = a.clone();
    shuffle_in_place(&amp;mut a, 42);
    shuffle_in_place(&amp;mut b, 42);
    assert_eq!(a, b);
    let mut sorted = a.clone(); sorted.sort();
    assert_eq!(sorted, vec![1,2,3,4,5,6,7,8,9,10]);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/prng.rs">Verify next_u64 + pick_index_or_fail; add shuffle_in_place if missing; add reference-vector test.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 4 prng tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>next_u64 produces canonical SplitMix64 output sequence from seed 0</criterion>
  <criterion>pick_index_or_fail rejects candidate_count == 0</criterion>
  <criterion>shuffle_in_place is a deterministic permutation (preserves multiset)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- prng</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
