// Shared Python lexical masking for mutation operators.
//
// This is intentionally small and conservative. It marks Python comments and
// string literals (including common string prefixes and triple quotes) as
// non-code so mutation operators do not create false mutants by editing
// docstrings, comments, or literal text. Parser-backed syntax validation is
// used before mutation; the mask itself stays byte-local for cheap scans.

use crate::{MutationError, MutationResult};

pub fn ensure_parseable_python(source: &str, field: &'static str) -> MutationResult<()> {
    let _ = parse_python(source, field)?;
    Ok(())
}

pub fn python_code_mask(source: &str) -> MutationResult<Vec<bool>> {
    Ok(mask_python(source)?.code_by_byte)
}

pub fn mask_python(source: &str) -> MutationResult<LexicalMask> {
    let tree = parse_python(source, "source")?;
    let mut mask = vec![true; source.len()];
    mask_non_code_nodes(tree.root_node(), source, &mut mask);
    Ok(LexicalMask { code_by_byte: mask })
}

pub fn range_is_code(mask: &[bool], start: usize, len: usize) -> bool {
    start
        .checked_add(len)
        .and_then(|end| mask.get(start..end))
        .map(|items| items.iter().all(|is_code| *is_code))
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexicalMask {
    code_by_byte: Vec<bool>,
}

impl LexicalMask {
    pub fn is_code(&self, byte_offset: usize) -> bool {
        self.code_by_byte.get(byte_offset).copied().unwrap_or(false)
    }

    pub fn is_code_range(&self, start: usize, end: usize) -> bool {
        if start > end {
            return false;
        }
        self.code_by_byte
            .get(start..end)
            .map(|items| items.iter().all(|is_code| *is_code))
            .unwrap_or(false)
    }

    pub fn iter_code_chars<'a>(
        &'a self,
        source: &'a str,
    ) -> impl Iterator<Item = (usize, char)> + 'a {
        source
            .char_indices()
            .filter(move |(offset, _)| self.is_code(*offset))
    }
}

fn parse_python(source: &str, field: &'static str) -> MutationResult<tree_sitter::Tree> {
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
        let detail = first_parse_error(tree.root_node(), source)
            .unwrap_or_else(|| "first_error=<unavailable>".to_string());
        return Err(MutationError::invalid(
            field,
            format!("tree-sitter-python reported syntax errors in mutation input; {detail}"),
            "supply syntactically valid Python source before applying Phase-0 mutations",
        ));
    }
    Ok(tree)
}

fn mask_non_code_nodes(node: tree_sitter::Node<'_>, source: &str, mask: &mut [bool]) {
    match node.kind() {
        "comment" => {
            set_mask_range(mask, node.start_byte(), node.end_byte(), false);
        }
        "string" => {
            set_mask_range(mask, node.start_byte(), node.end_byte(), false);
            mark_interpolations(node, source, mask);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        mask_non_code_nodes(child, source, mask);
    }
}

fn mark_interpolations(node: tree_sitter::Node<'_>, source: &str, mask: &mut [bool]) {
    if matches!(node.kind(), "interpolation" | "format_expression") {
        mark_interpolation_expression(node, source, mask);
        if let Some(format_specifier) = node.child_by_field_name("format_specifier") {
            mark_interpolations(format_specifier, source, mask);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        mark_interpolations(child, source, mask);
    }
}

fn mark_interpolation_expression(node: tree_sitter::Node<'_>, source: &str, mask: &mut [bool]) {
    let Some(expression) = node.child_by_field_name("expression") else {
        return;
    };
    set_mask_range(mask, expression.start_byte(), expression.end_byte(), true);
    remask_nested_non_code(expression, source, mask);
}

fn remask_nested_non_code(node: tree_sitter::Node<'_>, source: &str, mask: &mut [bool]) {
    match node.kind() {
        "comment" => {
            set_mask_range(mask, node.start_byte(), node.end_byte(), false);
            return;
        }
        "string" => {
            set_mask_range(mask, node.start_byte(), node.end_byte(), false);
            mark_interpolations(node, source, mask);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        remask_nested_non_code(child, source, mask);
    }
}

fn set_mask_range(mask: &mut [bool], start: usize, end: usize, value: bool) {
    let bounded_end = end.min(mask.len());
    if start >= bounded_end {
        return;
    }
    for item in &mut mask[start..bounded_end] {
        *item = value;
    }
}

fn first_parse_error(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let error_node = first_error_node(node)?;
    let start = error_node.start_byte();
    let end = error_node.end_byte();
    let position = error_node.start_position();
    let snippet = source_line_snippet(source, start);
    Some(format!(
        "kind={}, missing={}, line={}, column={}, byte_range={}..{}, snippet={:?}",
        error_node.kind(),
        error_node.is_missing(),
        position.row + 1,
        position.column + 1,
        start,
        end,
        snippet
    ))
}

fn first_error_node(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() {
            if let Some(found) = first_error_node(child) {
                return Some(found);
            }
        }
    }
    if node.has_error() {
        Some(node)
    } else {
        None
    }
}

fn source_line_snippet(source: &str, byte_offset: usize) -> String {
    let bounded = byte_offset.min(source.len());
    let line_start = source[..bounded]
        .rfind('\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let line_end = source[bounded..]
        .find('\n')
        .map(|idx| bounded + idx)
        .unwrap_or(source.len());
    let mut line = source[line_start..line_end].replace('\t', "\\t");
    const MAX_SNIPPET_CHARS: usize = 160;
    if line.chars().count() > MAX_SNIPPET_CHARS {
        line = line.chars().take(MAX_SNIPPET_CHARS).collect::<String>();
        line.push_str("...");
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_comments_and_keeps_newline_code() {
        let src = "x = 1 # y == 2\nz = 3\n";
        let mask = python_code_mask(src).unwrap();
        let comment = src.find('#').unwrap();
        assert!(!mask[comment]);
        assert!(mask[src.find('\n').unwrap()]);
        assert!(range_is_code(&mask, src.find("z = 3").unwrap(), 5));
    }

    #[test]
    fn masks_prefixed_and_triple_quoted_strings() {
        let src = "a = r\"x == y\"\nb = f'''value {x > y}'''\nc = 1\n";
        let mask = python_code_mask(src).unwrap();
        assert!(!range_is_code(&mask, src.find("==").unwrap(), 2));
        assert!(!range_is_code(&mask, src.find("value").unwrap(), 5));
        assert!(range_is_code(&mask, src.find('>').unwrap(), 1));
        assert!(range_is_code(&mask, src.find("c = 1").unwrap(), 5));
    }

    #[test]
    fn masks_fstring_format_spec_literals_but_keeps_nested_replacement_fields() {
        let src =
            "width = 10\nvalue = 3\nfixed = f\"{value:>10}\"\nnested = f\"{value:>{width}}\"\n";
        let mask = python_code_mask(src).unwrap();
        let align = src.find(":>").unwrap() + 1;
        let literal_width = src.find(">10").unwrap() + 1;
        let nested_width = src.rfind("width").unwrap();
        assert!(!range_is_code(&mask, align, 1));
        assert!(!range_is_code(&mask, literal_width, 2));
        assert!(range_is_code(&mask, nested_width, "width".len()));
    }

    #[test]
    fn lexical_mask_api_masks_triple_quotes_and_exposes_code_chars() {
        let src = "'''x == y'''\nvalue = 42\n";
        let mask = mask_python(src).unwrap();
        let string_eq = src.find("==").unwrap();
        let value = src.find("value").unwrap();
        assert!(!mask.is_code_range(string_eq, string_eq + 2));
        assert!(mask.is_code_range(value, value + "value".len()));
        let code = mask
            .iter_code_chars(src)
            .map(|(_, ch)| ch)
            .collect::<String>();
        assert!(code.contains("value = 42"));
        assert!(!code.contains("x == y"));
    }

    #[test]
    fn lexical_mask_excludes_fstring_format_specs() {
        let src = "value = 7\nwidth = 4\ntext = f\"{value:>{width}}\"\n";
        let mask = mask_python(src).unwrap();
        let literal_arrow = src.find(":>").unwrap() + 1;
        let nested_width = src.rfind("width").unwrap();
        assert!(!mask.is_code(literal_arrow));
        assert!(mask.is_code_range(nested_width, nested_width + "width".len()));
    }
}
