# TASK-CORPUS-002 — Language Enum + 11-Language Gate

```xml
<task_spec id="TASK-CORPUS-002" version="1.0">
<metadata>
  <title>Language Enum + 11-Language Gate</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-13</requirement_ref>
    <requirement_ref>REQ-CORPUS-01</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Defines the `Language` enum mirroring the
constitution's 11-language matrix (Rust, Python, JavaScript,
TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php). Today only
`Language::Python` is implemented; every other variant returns
`MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` from `apply_mutation` and
`generate_swe_bench_mutation_corpus`. The exhaustive `match`
arm is what guarantees compile-time enforcement of EC-CORPUS-13
("future PR adds Rust operators but does not update Language —
compile error").
</context>

<input_context_files>
  <file purpose="constitution_tech_stack">../../constitution.md</file>
  <file purpose="data_model">../../technical/TECH-CORPUS.md#data_models (Language)</file>
  <file purpose="error_code_root">../../constitution.md#failure_taxonomy (MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>tree-sitter and tree-sitter-python in Cargo.toml</check>
  <check>baseline cargo build green</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `Language` enum with all 11 variants in canonical order
      (Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp,
      CSharp, Ruby, Php).
    - `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash,
      serde::Serialize, serde::Deserialize)]`,
      `#[serde(rename_all = "snake_case")]`.
    - Implement `Language::is_supported()` returning true ONLY for
      Python; every other variant false.
    - Implement `Language::all() -&gt; &amp;'static [Language; 11]`.
    - Implement `Language::slug() -&gt; &amp;'static str` and
      `Language::from_slug(s: &amp;str) -&gt; Option&lt;Self&gt;`.
    - Implement `parse_languages(raw: &amp;[String]) -&gt;
      MutationResult&lt;Vec&lt;Language&gt;&gt;`. Reject any
      unknown slug with `MutationError::InvalidInput { field:
      "languages", ... }`. Reject unsupported (non-Python) with a
      message including the supported set.
    - Add unit tests covering: all 11 slugs round-trip; parse_languages
      rejects "lua"; parse_languages rejects "rust" (unsupported)
      with the supported-set message; parse_languages accepts
      ["python"].
  </in_scope>
  <out_of_scope>
    - Per-language tree-sitter grammar wiring beyond Python
      (separate phase per `<out_of_scope>` in SPEC-CORPUS).
    - Mutation operators (TASK-CORPUS-012..015).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Language {
        Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php,
    }

    impl Language {
        pub const fn all() -&gt; &amp;'static [Language; 11];
        pub const fn slug(&amp;self) -&gt; &amp;'static str;
        pub fn from_slug(slug: &amp;str) -&gt; Option&lt;Self&gt;;
        pub const fn is_supported(&amp;self) -&gt; bool;
    }

    pub fn parse_languages(raw: &amp;[String]) -&gt; MutationResult&lt;Vec&lt;Language&gt;&gt;;
    </signature>
  </signatures>

  <constraints>
    - Variants in canonical constitution order — Rust, Python, JS,
      TS, Go, Java, C, Cpp, CSharp, Ruby, Php — so the array index
      remains stable across PRs.
    - Snake-case slug serialization for `Cpp` -&gt; `cpp`,
      `CSharp` -&gt; `c_sharp`, `JavaScript` -&gt; `java_script`
      MUST instead use a hand-spelled match producing the
      colloquial `cpp`, `c_sharp`, `javascript`, `typescript`,
      `php`, `php` — verify with explicit unit tests.
    - Error message for unsupported language includes:
      `current supported set: [python]`.
    - No `unwrap()` in production code paths.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- language_enum`
      passes (all_slug_round_trip, parse_unknown_rejected,
      parse_unsupported_rejected_with_supported_set,
      parse_python_accepted).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl Language {
    pub const fn slug(&amp;self) -&gt; &amp;'static str {
        match self {
            Self::Rust =&gt; "rust",
            Self::Python =&gt; "python",
            Self::JavaScript =&gt; "javascript",
            Self::TypeScript =&gt; "typescript",
            Self::Go =&gt; "go",
            Self::Java =&gt; "java",
            Self::C =&gt; "c",
            Self::Cpp =&gt; "cpp",
            Self::CSharp =&gt; "c_sharp",
            Self::Ruby =&gt; "ruby",
            Self::Php =&gt; "php",
        }
    }
    pub const fn is_supported(&amp;self) -&gt; bool { matches!(self, Self::Python) }
}

pub fn parse_languages(raw: &amp;[String]) -&gt; MutationResult&lt;Vec&lt;Language&gt;&gt; {
    let mut out = Vec::with_capacity(raw.len());
    for r in raw {
        let lang = Language::from_slug(r).ok_or_else(|| MutationError::invalid(
            "languages",
            format!("unknown language slug `{r}`"),
            "use one of: rust, python, javascript, typescript, go, java, c, cpp, c_sharp, ruby, php",
        ))?;
        if !lang.is_supported() {
            return Err(MutationError::invalid(
                "languages",
                format!("language `{r}` is not yet supported; current supported set: [python]"),
                "drop unsupported flag values OR wait for the language to ship",
            ));
        }
        out.push(lang);
    }
    Ok(out)
}
</pseudo_code>

<files_to_create>
  <!-- All edits land in existing files. -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add Language enum + impl + parse_languages.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 4 language_enum tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Language has 11 variants in canonical order</criterion>
  <criterion>Only Python is supported; others return error with supported-set message</criterion>
  <criterion>All 11 slugs round-trip via from_slug</criterion>
  <criterion>parse_languages rejects unknown slugs and unsupported languages with structured errors</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- language</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```
