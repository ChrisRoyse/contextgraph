use crate::prng::{pick_index_or_fail, SplitMix64};
use crate::{
    Language, MutationCategory, MutationConfig, MutationError, MutationOutcome, MutationResult,
    MutationSite,
};
use context_graph_core::memory::ast::{
    chunk_with_options, AstChunkOptions, Language as AstLanguage,
};

pub(crate) fn apply(
    language: Language,
    category: MutationCategory,
    primary_source: &str,
    config: MutationConfig,
) -> MutationResult<MutationOutcome> {
    validate_parseable(language, primary_source, "primary_source")?;
    let site = match category {
        MutationCategory::KnownGood => {
            return Ok(MutationOutcome {
                category,
                mutated_source: primary_source.to_string(),
                seed: config.seed,
                mutation_site: None,
            });
        }
        MutationCategory::SubtleFlip => subtle_flip(language, primary_source, config.seed)?,
        MutationCategory::OffByOne => off_by_one(language, primary_source, config.seed)?,
        MutationCategory::SwapVariable => swap_variable(language, primary_source, config.seed)?,
        MutationCategory::DeleteTestCall => {
            delete_test_call(language, primary_source, config.seed)?
        }
        MutationCategory::WrongFile => {
            let alternate = config.alternate_source.as_deref().ok_or_else(|| {
                MutationError::invalid(
                    "alternate_source",
                    "WrongFile mutation requires `alternate_source` to be set",
                    "pass a different parseable source file in the same language",
                )
            })?;
            validate_parseable(language, alternate, "alternate_source")?;
            return Ok(MutationOutcome {
                category,
                mutated_source: alternate.to_string(),
                seed: config.seed,
                mutation_site: None,
            });
        }
        MutationCategory::OverEngineer => over_engineer(language, primary_source, config.seed)?,
        MutationCategory::CompileError => compile_error(language, primary_source, config.seed)?,
    };
    let outcome = apply_site(category, primary_source, config.seed, site)?;
    if category == MutationCategory::CompileError {
        require_parse_failure(language, &outcome.mutated_source)?;
    } else {
        validate_parseable(language, &outcome.mutated_source, "mutated_source")?;
    }
    Ok(outcome)
}

fn subtle_flip(language: Language, source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mask = code_mask(language, source)?;
    let specs = [
        ("==", "!=", "subtle_flip: equality flipped to inequality"),
        ("!=", "==", "subtle_flip: inequality flipped to equality"),
        ("<=", ">=", "subtle_flip: <= flipped to >="),
        (">=", "<=", "subtle_flip: >= flipped to <="),
        ("&&", "||", "subtle_flip: && flipped to ||"),
        ("||", "&&", "subtle_flip: || flipped to &&"),
        ("<", ">", "subtle_flip: < flipped to >"),
        (">", "<", "subtle_flip: > flipped to <"),
        ("true", "false", "subtle_flip: true flipped to false"),
        ("false", "true", "subtle_flip: false flipped to true"),
    ];
    let mut candidates = Vec::new();
    for (original, replacement, note) in specs {
        for offset in find_token_sites(source, &mask, original) {
            if !replacement_preserves_parse(language, source, offset, original, replacement) {
                continue;
            }
            candidates.push(MutationSite {
                byte_offset: offset,
                byte_length: original.len(),
                original_text: original.to_string(),
                replacement_text: replacement.to_string(),
                note: note.to_string(),
            });
        }
    }
    pick_site(candidates, seed, "boolean_operator")
}

fn replacement_preserves_parse(
    language: Language,
    source: &str,
    offset: usize,
    original: &str,
    replacement: &str,
) -> bool {
    let end = match offset.checked_add(original.len()) {
        Some(end) => end,
        None => return false,
    };
    if end > source.len() || source.get(offset..end) != Some(original) {
        return false;
    }
    let mut mutated = String::with_capacity(source.len() - original.len() + replacement.len());
    mutated.push_str(&source[..offset]);
    mutated.push_str(replacement);
    mutated.push_str(&source[end..]);
    validate_parseable(language, &mutated, "subtle_flip_candidate").is_ok()
}

fn off_by_one(language: Language, source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mask = code_mask(language, source)?;
    let bytes = source.as_bytes();
    let mut candidates = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if !mask[i] || !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let prev = i.checked_sub(1).and_then(|idx| bytes.get(idx)).copied();
        if prev.is_some_and(|b| is_word_byte(b) || b == b'.') {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && mask[i] && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let next = bytes.get(i).copied();
        if next.is_some_and(|b| is_word_byte(b) || b == b'.') {
            continue;
        }
        let digits = &source[start..i];
        let value = digits.parse::<i64>().map_err(|err| {
            MutationError::op_failed(
                "numeric_literal",
                format!("failed to parse {digits:?}: {err}"),
                "operator scanner must only emit base-10 integer literals",
            )
        })?;
        let direction = if seed & 1 == 0 { 1 } else { -1 };
        let replacement = value.checked_add(direction).ok_or_else(|| {
            MutationError::op_failed(
                "numeric_literal",
                format!("overflow adding {direction} to {value}"),
                "pick a different seed or avoid i64 boundary literals",
            )
        })?;
        candidates.push(MutationSite {
            byte_offset: start,
            byte_length: i - start,
            original_text: digits.to_string(),
            replacement_text: replacement.to_string(),
            note: format!("off_by_one: numeric literal {digits} mutated by {direction}"),
        });
    }
    pick_site(candidates, seed, "numeric_literal")
}

fn swap_variable(language: Language, source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mask = code_mask(language, source)?;
    let mut identifiers = Vec::<(String, usize)>::new();
    for (offset, ident) in identifier_sites(source, &mask) {
        if !keyword(language, &ident)
            && !ident.chars().next().unwrap_or('_').is_uppercase()
            && is_variable_use_site(language, source, offset, ident.len())
        {
            identifiers.push((ident, offset));
        }
    }
    let mut names = identifiers
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    if names.len() < 2 {
        return Err(MutationError::no_site(
            "identifier",
            "swap requires at least two non-keyword identifiers",
            "supply source with multiple local variables or parameters",
        ));
    }
    let mut candidates = Vec::new();
    for (name, offset) in identifiers {
        if let Some(replacement) = names.iter().find(|candidate| **candidate != name) {
            candidates.push(MutationSite {
                byte_offset: offset,
                byte_length: name.len(),
                original_text: name.clone(),
                replacement_text: replacement.clone(),
                note: format!("swap_variable: identifier `{name}` changed to `{replacement}`"),
            });
        }
    }
    pick_site(candidates, seed, "identifier")
}

fn delete_test_call(language: Language, source: &str, seed: u64) -> MutationResult<MutationSite> {
    let assertions = [
        "assert",
        "assert_eq!",
        "assert_ne!",
        "expect",
        "should",
        "require.",
        "t.Fatal",
        "pytest",
        "PHPUnit",
    ];
    let mask = code_mask(language, source)?;
    let mut candidates = Vec::new();
    let mut line_start = 0usize;
    for line in source.split_inclusive('\n') {
        let line_end = line_start + line.len();
        let line_no_nl = line.trim_end_matches('\n');
        let has_assertion = !is_declaration_line(line_no_nl)
            && assertions
                .iter()
                .any(|needle| assertion_token_on_line(line_no_nl, line_start, &mask, needle));
        if has_assertion {
            candidates.push(MutationSite {
                byte_offset: line_start,
                byte_length: line.len(),
                original_text: line.to_string(),
                replacement_text: no_op_line(language, line_no_nl, line.ends_with('\n')),
                note: "delete_test_call: assertion line replaced by language no-op".to_string(),
            });
        }
        line_start = line_end;
    }
    pick_site(candidates, seed, "assertion_line")
}

fn is_declaration_line(line: &str) -> bool {
    let line = line.trim_start();
    [
        "fn ",
        "function ",
        "func ",
        "def ",
        "class ",
        "struct ",
        "enum ",
        "type ",
        "interface ",
        "trait ",
        "impl ",
        "void ",
        "bool ",
        "boolean ",
        "int ",
        "long ",
        "short ",
        "float ",
        "double ",
        "string ",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

fn assertion_token_on_line(line: &str, line_start: usize, mask: &[bool], needle: &str) -> bool {
    let mut offset = 0usize;
    while let Some(found) = line[offset..].find(needle) {
        let idx = offset + found;
        if range_is_code(mask, line_start + idx, needle.len())
            && assertion_token_boundary(line.as_bytes(), idx, needle)
        {
            return true;
        }
        offset = idx + needle.len();
    }
    false
}

fn assertion_token_boundary(bytes: &[u8], offset: usize, token: &str) -> bool {
    if matches!(token, "require." | "t.Fatal") {
        return true;
    }
    token_boundary(bytes, offset, token)
}

fn over_engineer(language: Language, source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mut rng = SplitMix64::new(seed);
    let suffix = rng.next_u64() & 0xffff_ffff;
    let helper = match language {
        Language::Rust => format!("\nfn unused_helper_{suffix:08x}() -> i32 {{ 0 }}\n"),
        Language::JavaScript => format!("\nfunction unusedHelper{suffix:08x}() {{ return 0; }}\n"),
        Language::TypeScript => {
            format!("\nfunction unusedHelper{suffix:08x}(): number {{ return 0; }}\n")
        }
        Language::Go => format!("\nfunc unusedHelper{suffix:08x}() int {{ return 0 }}\n"),
        Language::Java => {
            format!("\nclass UnusedHelper{suffix:08x} {{ int value() {{ return 0; }} }}\n")
        }
        Language::C => format!("\nstatic int unused_helper_{suffix:08x}(void) {{ return 0; }}\n"),
        Language::Cpp => format!("\nstatic int unused_helper_{suffix:08x}() {{ return 0; }}\n"),
        Language::CSharp => {
            format!("\nclass UnusedHelper{suffix:08x} {{ int Value() {{ return 0; }} }}\n")
        }
        Language::Ruby => format!("\ndef unused_helper_{suffix:08x}\n  0\nend\n"),
        Language::Php => format!("\nfunction unused_helper_{suffix:08x}() {{ return 0; }}\n"),
        Language::Python => unreachable!("python uses python-specific operator"),
    };
    Ok(MutationSite {
        byte_offset: append_offset(source),
        byte_length: 0,
        original_text: String::new(),
        replacement_text: helper,
        note: format!("over_engineer: appended unused helper {suffix:08x}"),
    })
}

fn compile_error(language: Language, source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mut rng = SplitMix64::new(seed);
    let variant = rng.next_u64() % 3;
    let snippet = match (language, variant) {
        (Language::Rust, 0) => "\nfn compile_error_marker( {\n",
        (Language::Rust, _) => "\nlet compile_error_marker = ;\n",
        (Language::JavaScript | Language::TypeScript, 0) => "\nfunction compileError( {\n",
        (Language::JavaScript | Language::TypeScript, _) => "\nconst compileError = ;\n",
        (Language::Go, 0) => "\nfunc compileError( {\n",
        (Language::Go, _) => "\nvar compileError = \n",
        (Language::Java | Language::CSharp, 0) => "\nclass CompileError { void broken( { } }\n",
        (Language::Java | Language::CSharp, _) => "\nclass CompileError { int broken = ; }\n",
        (Language::C | Language::Cpp, 0) => "\nint compile_error_marker( {\n",
        (Language::C | Language::Cpp, _) => "\nint compile_error_marker = ;\n",
        (Language::Ruby, 0) => "\ndef compile_error_marker(\nend\n",
        (Language::Ruby, _) => "\nif true\n",
        (Language::Php, 0) => "\nfunction compile_error_marker( {\n",
        (Language::Php, _) => "\n$compile_error_marker = ;\n",
        (Language::Python, _) => unreachable!("python uses python-specific operator"),
    };
    Ok(MutationSite {
        byte_offset: append_offset(source),
        byte_length: 0,
        original_text: String::new(),
        replacement_text: snippet.to_string(),
        note: "compile_error: appended language-specific syntax error".to_string(),
    })
}

fn apply_site(
    category: MutationCategory,
    source: &str,
    seed: u64,
    site: MutationSite,
) -> MutationResult<MutationOutcome> {
    let end = site
        .byte_offset
        .checked_add(site.byte_length)
        .ok_or_else(|| {
            MutationError::op_failed(
                "mutation_site",
                "site offset plus length overflowed",
                "report the operator that produced the invalid site",
            )
        })?;
    if site.byte_offset > source.len() || end > source.len() {
        return Err(MutationError::op_failed(
            "mutation_site",
            "site range is outside source",
            "report the operator that produced the invalid site",
        ));
    }
    if !source.is_char_boundary(site.byte_offset) || !source.is_char_boundary(end) {
        return Err(MutationError::op_failed(
            "mutation_site",
            "site range is not UTF-8 boundary aligned",
            "report the operator that produced the invalid site",
        ));
    }
    if source[site.byte_offset..end] != site.original_text {
        return Err(MutationError::op_failed(
            "mutation_site.original_text",
            "site original_text does not match source",
            "report the operator that produced the invalid site",
        ));
    }
    let mut out =
        String::with_capacity(source.len() - site.byte_length + site.replacement_text.len());
    out.push_str(&source[..site.byte_offset]);
    out.push_str(&site.replacement_text);
    out.push_str(&source[end..]);
    Ok(MutationOutcome {
        category,
        mutated_source: out,
        seed,
        mutation_site: Some(site),
    })
}

fn validate_parseable(language: Language, source: &str, field: &'static str) -> MutationResult<()> {
    if source.trim().is_empty() {
        return Err(MutationError::invalid(
            field,
            format!("{field} is empty or whitespace-only"),
            "supply non-empty source text before mutation",
        ));
    }
    let ast_language = ast_language(language);
    chunk_with_options(
        source.as_bytes(),
        ast_language,
        &AstChunkOptions::for_path(default_path(language)),
    )
    .map(|_| ())
    .map_err(|err| {
        MutationError::invalid(
            field,
            format!("{}: {err}", err.code()),
            "supply syntactically valid source for this language before mutation",
        )
    })
}

fn require_parse_failure(language: Language, source: &str) -> MutationResult<()> {
    match chunk_with_options(
        source.as_bytes(),
        ast_language(language),
        &AstChunkOptions::for_path(default_path(language)),
    ) {
        Ok(_) => Err(MutationError::op_failed(
            "compile_error",
            "CompileError mutation remained parseable",
            "fix the language-specific syntax-error snippet",
        )),
        Err(_) => Ok(()),
    }
}

fn ast_language(language: Language) -> AstLanguage {
    match language {
        Language::Rust => AstLanguage::Rust,
        Language::Python => AstLanguage::Python,
        Language::JavaScript => AstLanguage::JavaScript,
        Language::TypeScript => AstLanguage::TypeScript,
        Language::Go => AstLanguage::Go,
        Language::Java => AstLanguage::Java,
        Language::C => AstLanguage::C,
        Language::Cpp => AstLanguage::Cpp,
        Language::CSharp => AstLanguage::CSharp,
        Language::Ruby => AstLanguage::Ruby,
        Language::Php => AstLanguage::Php,
    }
}

fn default_path(language: Language) -> &'static str {
    match language {
        Language::Rust => "mutation.rs",
        Language::Python => "mutation.py",
        Language::JavaScript => "mutation.js",
        Language::TypeScript => "mutation.ts",
        Language::Go => "mutation.go",
        Language::Java => "Mutation.java",
        Language::C => "mutation.c",
        Language::Cpp => "mutation.cpp",
        Language::CSharp => "Mutation.cs",
        Language::Ruby => "mutation.rb",
        Language::Php => "mutation.php",
    }
}

fn pick_site(
    mut candidates: Vec<MutationSite>,
    seed: u64,
    field: &'static str,
) -> MutationResult<MutationSite> {
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            field,
            format!("no mutation site found for {field}"),
            "supply source containing an applicable language-level mutation site",
        ));
    }
    candidates.sort_by(|a, b| {
        a.byte_offset
            .cmp(&b.byte_offset)
            .then_with(|| a.note.cmp(&b.note))
    });
    let mut rng = SplitMix64::new(seed);
    let pick = pick_index_or_fail(&mut rng, candidates.len())?;
    Ok(candidates.swap_remove(pick))
}

fn find_token_sites(source: &str, mask: &[bool], token: &str) -> Vec<usize> {
    let mut sites = Vec::new();
    let mut offset = 0usize;
    while let Some(found) = source[offset..].find(token) {
        let idx = offset + found;
        if range_is_code(mask, idx, token.len()) && token_boundary(source.as_bytes(), idx, token) {
            sites.push(idx);
        }
        offset = idx + token.len();
    }
    sites
}

fn token_boundary(bytes: &[u8], offset: usize, token: &str) -> bool {
    if token.bytes().all(is_word_byte) {
        let prev_ok = offset == 0 || !is_word_byte(bytes[offset - 1]);
        let next = offset + token.len();
        let next_ok = next >= bytes.len() || !is_word_byte(bytes[next]);
        prev_ok && next_ok
    } else {
        true
    }
}

fn identifier_sites(source: &str, mask: &[bool]) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if mask[i] && is_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && mask[i] && is_word_byte(bytes[i]) {
                i += 1;
            }
            out.push((start, source[start..i].to_string()));
        } else {
            i += 1;
        }
    }
    out
}

fn code_mask(language: Language, source: &str) -> MutationResult<Vec<bool>> {
    let bytes = source.as_bytes();
    let mut mask = vec![true; bytes.len()];
    let mut i = 0usize;
    while i < bytes.len() {
        if starts_line_comment(language, bytes, i) {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            mask[start..i].fill(false);
            continue;
        }
        if starts_block_comment(language, bytes, i) {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            mask[start..i].fill(false);
            continue;
        }
        if matches!(bytes[i], b'\'' | b'"' | b'`') {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            mask[start..i].fill(false);
            continue;
        }
        i += 1;
    }
    Ok(mask)
}

fn starts_line_comment(language: Language, bytes: &[u8], i: usize) -> bool {
    match language {
        Language::Ruby => bytes[i] == b'#',
        Language::Php => bytes[i] == b'#' || bytes.get(i..i + 2) == Some(b"//"),
        _ => bytes.get(i..i + 2) == Some(b"//"),
    }
}

fn starts_block_comment(language: Language, bytes: &[u8], i: usize) -> bool {
    !matches!(language, Language::Ruby) && bytes.get(i..i + 2) == Some(b"/*")
}

fn range_is_code(mask: &[bool], start: usize, len: usize) -> bool {
    start
        .checked_add(len)
        .and_then(|end| mask.get(start..end))
        .is_some_and(|slice| slice.iter().all(|value| *value))
}

fn no_op_line(language: Language, line: &str, had_newline: bool) -> String {
    let indent = line
        .bytes()
        .take_while(|byte| *byte == b' ' || *byte == b'\t')
        .count();
    let prefix = &line[..indent];
    let body = match language {
        Language::Ruby => "",
        _ => ";",
    };
    let newline = if had_newline { "\n" } else { "" };
    format!("{prefix}{body}{newline}")
}

fn keyword(language: Language, ident: &str) -> bool {
    let common = [
        "return",
        "if",
        "else",
        "for",
        "while",
        "class",
        "struct",
        "enum",
        "interface",
        "trait",
        "impl",
        "function",
        "func",
        "fn",
        "def",
        "let",
        "const",
        "var",
        "public",
        "private",
        "protected",
        "static",
        "void",
        "int",
        "float",
        "double",
        "string",
        "bool",
        "boolean",
        "byte",
        "char",
        "decimal",
        "long",
        "short",
        "signed",
        "unsigned",
        "number",
        "object",
        "symbol",
        "bigint",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "f32",
        "f64",
        "true",
        "false",
        "null",
        "nil",
        "None",
        "self",
        "this",
        "Self",
        "new",
        "package",
        "import",
        "include",
        "use",
        "using",
        "namespace",
        "module",
        "end",
        "do",
        "assert",
        "assert_eq",
        "assert_ne",
        "console",
        "Debug",
        "t",
        "Fatal",
    ];
    common.contains(&ident)
        || matches!(language, Language::Go) && ident == "type"
        || matches!(language, Language::Php) && ident.starts_with("php")
}

fn is_variable_use_site(language: Language, source: &str, offset: usize, len: usize) -> bool {
    let line_start = source[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|idx| offset + idx)
        .unwrap_or(source.len());
    let line = source[line_start..line_end].trim_start();
    let declaration_prefixes = [
        "fn ",
        "function ",
        "func ",
        "def ",
        "class ",
        "struct ",
        "enum ",
        "type ",
        "interface ",
        "trait ",
        "impl ",
        "let ",
        "const ",
        "var ",
        "int ",
        "long ",
        "short ",
        "float ",
        "double ",
        "void ",
        "bool ",
        "boolean ",
        "string ",
        "package ",
        "import ",
        "using ",
        "use ",
        "#include",
        "<?php",
    ];
    if declaration_prefixes
        .iter()
        .any(|prefix| line.starts_with(prefix))
    {
        return false;
    }
    if line.contains(":=") {
        return false;
    }

    let bytes = source.as_bytes();
    let next = next_non_ws(bytes, offset + len);
    let next_at = next_non_ws_at(bytes, offset + len);
    let prev = prev_non_ws(bytes, offset);
    if next.is_some_and(|byte| matches!(byte, b'(' | b'!' | b':' | b'.'))
        || prev.is_some_and(|byte| byte == b'.')
    {
        return false;
    }
    if next_at.is_some_and(|(idx, byte)| {
        byte == b'=' && next_non_ws(bytes, idx + 1).is_none_or(|next| next != b'=')
    }) {
        return false;
    }

    if matches!(language, Language::Rust | Language::TypeScript)
        && next.is_some_and(|byte| byte == b':')
    {
        return false;
    }

    true
}

fn next_non_ws(bytes: &[u8], mut offset: usize) -> Option<u8> {
    while let Some(byte) = bytes.get(offset).copied() {
        if !byte.is_ascii_whitespace() {
            return Some(byte);
        }
        offset += 1;
    }
    None
}

fn next_non_ws_at(bytes: &[u8], mut offset: usize) -> Option<(usize, u8)> {
    while let Some(byte) = bytes.get(offset).copied() {
        if !byte.is_ascii_whitespace() {
            return Some((offset, byte));
        }
        offset += 1;
    }
    None
}

fn prev_non_ws(bytes: &[u8], offset: usize) -> Option<u8> {
    let mut i = offset.checked_sub(1)?;
    loop {
        let byte = bytes[i];
        if !byte.is_ascii_whitespace() {
            return Some(byte);
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn append_offset(source: &str) -> usize {
    if source.ends_with('\n') {
        source.len() - 1
    } else {
        source.len()
    }
}
