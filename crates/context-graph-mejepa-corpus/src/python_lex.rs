// Shared Python lexical masking for mutation operators.
//
// This is intentionally small and conservative. It marks Python comments and
// string literals (including common string prefixes and triple quotes) as
// non-code so mutation operators do not create false mutants by editing
// docstrings, comments, or literal text. Parser-backed syntax validation is
// used before mutation; the mask itself stays byte-local for cheap scans.

use crate::{MutationError, MutationResult};

pub(crate) fn ensure_parseable_python(source: &str, field: &'static str) -> MutationResult<()> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    parser.set_language(&language).map_err(|err| {
        MutationError::op_failed(
            "python_parser.language",
            format!("tree-sitter-python language setup failed: {err}"),
            "verify tree-sitter and tree-sitter-python crate versions",
        )
    })?;
    let tree = parser.parse(source, None).ok_or_else(|| {
        MutationError::op_failed(
            "python_parser.tree",
            "tree-sitter-python returned no parse tree",
            "verify parser compatibility and source bytes",
        )
    })?;
    if tree.root_node().has_error() {
        return Err(MutationError::invalid(
            field,
            "tree-sitter-python reported syntax errors in mutation input",
            "supply syntactically valid Python source before applying Phase-0 mutations",
        ));
    }
    Ok(())
}

pub(crate) fn python_code_mask(source: &str) -> Vec<bool> {
    let bytes = source.as_bytes();
    let mut mask = vec![true; bytes.len()];
    let mut i = 0usize;

    while i < bytes.len() {
        if let Some(string_start) = prefixed_string_at(bytes, i) {
            let end = string_end(
                bytes,
                string_start.quote_idx,
                string_start.quote,
                string_start.triple,
            );
            for item in mask.iter_mut().take(end).skip(i) {
                *item = false;
            }
            if string_start.is_f_string {
                mark_fstring_replacement_fields(bytes, string_start, end, &mut mask);
            }
            i = end.max(i + 1);
            continue;
        }

        if bytes[i] == b'#' {
            let mut j = i;
            while j < bytes.len() && bytes[j] != b'\n' {
                mask[j] = false;
                j += 1;
            }
            i = j.max(i + 1);
            continue;
        }

        i += 1;
    }

    mask
}

pub(crate) fn range_is_code(mask: &[bool], start: usize, len: usize) -> bool {
    start
        .checked_add(len)
        .and_then(|end| mask.get(start..end))
        .map(|items| items.iter().all(|is_code| *is_code))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
struct StringStart {
    quote_idx: usize,
    quote: u8,
    triple: bool,
    is_f_string: bool,
}

fn prefixed_string_at(bytes: &[u8], i: usize) -> Option<StringStart> {
    if i >= bytes.len() {
        return None;
    }
    if is_quote(bytes[i]) {
        let triple = is_triple_quote(bytes, i, bytes[i]);
        return Some(StringStart {
            quote_idx: i,
            quote: bytes[i],
            triple,
            is_f_string: false,
        });
    }
    if i > 0 && is_ident_continue(bytes[i - 1]) {
        return None;
    }

    for prefix_len in [2usize, 1usize] {
        let quote_idx = i.checked_add(prefix_len)?;
        if quote_idx >= bytes.len() || !is_quote(bytes[quote_idx]) {
            continue;
        }
        if is_valid_string_prefix(&bytes[i..quote_idx]) {
            let quote = bytes[quote_idx];
            let triple = is_triple_quote(bytes, quote_idx, quote);
            let is_f_string = bytes[i..quote_idx].iter().any(|b| *b == b'f' || *b == b'F');
            return Some(StringStart {
                quote_idx,
                quote,
                triple,
                is_f_string,
            });
        }
    }
    None
}

fn mark_fstring_replacement_fields(
    bytes: &[u8],
    string_start: StringStart,
    string_end: usize,
    mask: &mut [bool],
) {
    let quote_len = if string_start.triple { 3 } else { 1 };
    let mut i = string_start.quote_idx + quote_len;
    let content_end = string_end.saturating_sub(quote_len);
    while i < content_end {
        match bytes[i] {
            b'{' if bytes.get(i + 1) == Some(&b'{') => i += 2,
            b'}' if bytes.get(i + 1) == Some(&b'}') => i += 2,
            b'{' => {
                let end = mark_replacement_expr(bytes, i + 1, content_end, mask);
                i = end;
            }
            b'\\' if !string_start.triple && i + 1 < content_end => i += 2,
            _ => i += 1,
        }
    }
}

fn mark_replacement_expr(bytes: &[u8], start: usize, limit: usize, mask: &mut [bool]) -> usize {
    let mut depth = 1usize;
    let mut i = start;
    while i < limit {
        if let Some(string_start) = prefixed_string_at(bytes, i) {
            let end = string_end(
                bytes,
                string_start.quote_idx,
                string_start.quote,
                string_start.triple,
            )
            .min(limit);
            for item in mask.iter_mut().take(end).skip(i) {
                *item = false;
            }
            i = end.max(i + 1);
            continue;
        }
        match bytes[i] {
            b'#' => {
                while i < limit && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'{' => {
                mask[i] = true;
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
                mask[i] = true;
                i += 1;
            }
            _ => {
                mask[i] = true;
                i += 1;
            }
        }
    }
    limit
}

fn string_end(bytes: &[u8], quote_idx: usize, quote: u8, triple: bool) -> usize {
    if triple {
        let mut j = quote_idx + 3;
        while j + 2 < bytes.len() {
            if bytes[j] == quote && bytes[j + 1] == quote && bytes[j + 2] == quote {
                return j + 3;
            }
            j += 1;
        }
        return bytes.len();
    }

    let mut j = quote_idx + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' if j + 1 < bytes.len() => j += 2,
            b'\n' => return j,
            b if b == quote => return j + 1,
            _ => j += 1,
        }
    }
    bytes.len()
}

fn is_valid_string_prefix(prefix: &[u8]) -> bool {
    let lowered = prefix
        .iter()
        .map(|b| b.to_ascii_lowercase())
        .collect::<Vec<_>>();
    matches!(
        lowered.as_slice(),
        b"r" | b"u" | b"b" | b"f" | b"br" | b"rb" | b"fr" | b"rf"
    )
}

fn is_triple_quote(bytes: &[u8], quote_idx: usize, quote: u8) -> bool {
    bytes.get(quote_idx..quote_idx + 3) == Some([quote, quote, quote].as_slice())
}

fn is_quote(b: u8) -> bool {
    b == b'\'' || b == b'"'
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_comments_and_keeps_newline_code() {
        let src = "x = 1 # y == 2\nz = 3\n";
        let mask = python_code_mask(src);
        let comment = src.find('#').unwrap();
        assert!(!mask[comment]);
        assert!(mask[src.find('\n').unwrap()]);
        assert!(range_is_code(&mask, src.find("z = 3").unwrap(), 5));
    }

    #[test]
    fn masks_prefixed_and_triple_quoted_strings() {
        let src = "a = r\"x == y\"\nb = f'''value {x > y}'''\nc = 1\n";
        let mask = python_code_mask(src);
        assert!(!range_is_code(&mask, src.find("==").unwrap(), 2));
        assert!(!range_is_code(&mask, src.find("value").unwrap(), 5));
        assert!(range_is_code(&mask, src.find('>').unwrap(), 1));
        assert!(range_is_code(&mask, src.find("c = 1").unwrap(), 5));
    }
}
