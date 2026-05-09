use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

use crate::features::{
    add_hashed_pair, add_hashed_token_features, bounded_ratio, fnv1a64, normalize_l2,
    validate_finite_output, validate_single_line, validate_text_field,
};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_SOURCE_BYTES: usize = 5_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeInstrumentInput {
    pub language: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffInstrumentInput {
    pub language: String,
    pub path: String,
    pub before_source: String,
    pub after_source: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EAstInstrument;

#[derive(Debug, Clone, Copy, Default)]
pub struct ECfgInstrument;

#[derive(Debug, Clone, Copy, Default)]
pub struct EDataFlowInstrument;

#[derive(Debug, Clone, Copy, Default)]
pub struct ETypeGraphInstrument;

#[derive(Debug, Clone, Copy, Default)]
pub struct EDiffInstrument;

#[derive(Default)]
struct CodeStats {
    node_count: usize,
    named_node_count: usize,
    max_depth: usize,
    function_count: usize,
    class_count: usize,
    branch_count: usize,
    loop_count: usize,
    call_count: usize,
    assignment_count: usize,
    identifier_count: usize,
    type_hint_count: usize,
    return_count: usize,
    import_count: usize,
}

impl Instrument for EAstInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EAst
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_python_code(input)?;
        encode_code(input, &stats, InstrumentSlot::EAst, "ast")
    }
}

impl Instrument for ECfgInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ECfg
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_python_code(input)?;
        encode_code(input, &stats, InstrumentSlot::ECfg, "cfg")
    }
}

impl Instrument for EDataFlowInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EDataFlow
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_python_code(input)?;
        encode_code(input, &stats, InstrumentSlot::EDataFlow, "data_flow")
    }
}

impl Instrument for ETypeGraphInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ETypeGraph
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_python_code(input)?;
        encode_code(input, &stats, InstrumentSlot::ETypeGraph, "type_graph")
    }
}

impl Instrument for EDiffInstrument {
    type Input = DiffInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EDiff
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_diff_input(input)?;
        let before = CodeInstrumentInput {
            language: input.language.clone(),
            path: input.path.clone(),
            source: input.before_source.clone(),
        };
        let after = CodeInstrumentInput {
            language: input.language.clone(),
            path: input.path.clone(),
            source: input.after_source.clone(),
        };
        let before_stats = parse_python_code(&before)?;
        let after_stats = parse_python_code(&after)?;
        let mut out = vec![0.0_f32; InstrumentSlot::EDiff.dim()];
        out[0] = signed_delta(after_stats.node_count, before_stats.node_count, 10_000.0);
        out[1] = signed_delta(
            after_stats.named_node_count,
            before_stats.named_node_count,
            10_000.0,
        );
        out[2] = signed_delta(
            after_stats.function_count,
            before_stats.function_count,
            1_000.0,
        );
        out[3] = signed_delta(after_stats.branch_count, before_stats.branch_count, 1_000.0);
        out[4] = signed_delta(
            after_stats.assignment_count,
            before_stats.assignment_count,
            1_000.0,
        );
        out[5] = signed_delta(
            after_stats.type_hint_count,
            before_stats.type_hint_count,
            1_000.0,
        );
        let before_lines: Vec<&str> = input.before_source.lines().collect();
        let after_lines: Vec<&str> = input.after_source.lines().collect();
        let before_set = before_lines
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let after_set = after_lines
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        for line in after_set.difference(&before_set) {
            add_hashed_pair(&mut out, line, 1.0, 32, 96);
        }
        for line in before_set.difference(&after_set) {
            add_hashed_pair(&mut out, line, -1.0, 128, 96);
        }
        out[224] = bounded_ratio(after_lines.len() as f32, 50_000.0);
        out[225] = bounded_ratio(before_lines.len() as f32, 50_000.0);
        out[226] = if input.before_source == input.after_source {
            1.0
        } else {
            0.0
        };
        normalize_l2(&mut out);
        validate_finite_output("e_diff.output", &out)?;
        Ok(out)
    }
}

fn validate_code_input(input: &CodeInstrumentInput) -> InstrumentResult<()> {
    validate_single_line(
        "input.language",
        &input.language,
        "store language identifiers as single-line canonical slugs",
    )?;
    validate_single_line(
        "input.path",
        &input.path,
        "store source paths as stable single-line relative paths",
    )?;
    validate_text_field(
        "input.source",
        &input.source,
        "capture the source text before encoding code instruments",
    )?;
    if input.source.len() > MAX_SOURCE_BYTES {
        return Err(InstrumentError::invalid(
            "input.source",
            format!(
                "source is {} bytes; max supported bytes is {MAX_SOURCE_BYTES}",
                input.source.len()
            ),
            "chunk large files before encoding E_AST/E_CFG/E_DataFlow/E_TypeGraph",
        ));
    }
    if input.language != "python" {
        return Err(InstrumentError::invalid(
            "input.language",
            format!(
                "language `{}` is not implemented by this Phase 1 code instrument",
                input.language
            ),
            "route non-Python languages through their language-specific tree-sitter instrument before filling this slot",
        ));
    }
    Ok(())
}

fn validate_diff_input(input: &DiffInstrumentInput) -> InstrumentResult<()> {
    validate_single_line(
        "input.language",
        &input.language,
        "store language identifiers as single-line canonical slugs",
    )?;
    validate_single_line(
        "input.path",
        &input.path,
        "store source paths as stable single-line relative paths",
    )?;
    validate_text_field(
        "input.before_source",
        &input.before_source,
        "capture the pre-edit source text before encoding E_Diff",
    )?;
    validate_text_field(
        "input.after_source",
        &input.after_source,
        "capture the post-edit source text before encoding E_Diff",
    )?;
    if input.before_source == input.after_source {
        return Err(InstrumentError::invalid(
            "input.after_source",
            "E_Diff input has identical before_source and after_source",
            "only fill E_Diff when a real source change exists",
        ));
    }
    Ok(())
}

fn parse_python_code(input: &CodeInstrumentInput) -> InstrumentResult<CodeStats> {
    validate_code_input(input)?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|err| {
            InstrumentError::invalid(
                "tree_sitter_python",
                format!("failed to initialise Python parser: {err}"),
                "pin compatible tree-sitter and tree-sitter-python crate versions",
            )
        })?;
    let tree = parser.parse(&input.source, None).ok_or_else(|| {
        InstrumentError::invalid(
            "input.source",
            "tree-sitter returned no parse tree",
            "preserve the source text and rerun parser diagnostics",
        )
    })?;
    let root = tree.root_node();
    if root.has_error() {
        let diagnostic = first_error_node(root, &input.source)
            .unwrap_or_else(|| "tree contains parse recovery error".to_string());
        return Err(InstrumentError::invalid(
            "input.source",
            diagnostic,
            "fix the source syntax before encoding code instruments; do not encode recovered ASTs",
        ));
    }
    let mut stats = CodeStats::default();
    walk_node(root, 0, &mut stats);
    Ok(stats)
}

fn walk_node(node: Node<'_>, depth: usize, stats: &mut CodeStats) {
    stats.node_count += 1;
    if node.is_named() {
        stats.named_node_count += 1;
    }
    stats.max_depth = stats.max_depth.max(depth);
    match node.kind() {
        "function_definition" => stats.function_count += 1,
        "class_definition" => stats.class_count += 1,
        "if_statement" | "conditional_expression" | "match_statement" => stats.branch_count += 1,
        "for_statement" | "while_statement" => stats.loop_count += 1,
        "call" => stats.call_count += 1,
        "assignment" | "augmented_assignment" => stats.assignment_count += 1,
        "identifier" => stats.identifier_count += 1,
        "type" | "typed_parameter" | "type_alias_statement" => stats.type_hint_count += 1,
        "return_statement" => stats.return_count += 1,
        "import_statement" | "import_from_statement" => stats.import_count += 1,
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, depth + 1, stats);
    }
}

fn encode_code(
    input: &CodeInstrumentInput,
    stats: &CodeStats,
    slot: InstrumentSlot,
    channel: &str,
) -> InstrumentResult<Vec<f32>> {
    let mut out = vec![0.0_f32; slot.dim()];
    out[0] = bounded_ratio(stats.node_count as f32, 100_000.0);
    out[1] = bounded_ratio(stats.named_node_count as f32, 100_000.0);
    out[2] = bounded_ratio(stats.max_depth as f32, 512.0);
    out[3] = bounded_ratio(stats.function_count as f32, 10_000.0);
    out[4] = bounded_ratio(stats.class_count as f32, 10_000.0);
    out[5] = bounded_ratio(stats.branch_count as f32, 10_000.0);
    out[6] = bounded_ratio(stats.loop_count as f32, 10_000.0);
    out[7] = bounded_ratio(stats.call_count as f32, 50_000.0);
    out[8] = bounded_ratio(stats.assignment_count as f32, 50_000.0);
    out[9] = bounded_ratio(stats.identifier_count as f32, 100_000.0);
    out[10] = bounded_ratio(stats.type_hint_count as f32, 10_000.0);
    out[11] = bounded_ratio(stats.return_count as f32, 10_000.0);
    out[12] = bounded_ratio(stats.import_count as f32, 10_000.0);
    add_hashed_token_features(&mut out[32..], &input.source, 0.25);
    add_hashed_pair(&mut out, &input.path, 1.0, 16, 16);
    add_hashed_pair(&mut out, channel, 2.0, slot.dim().saturating_sub(32), 32);
    normalize_l2(&mut out);
    validate_finite_output("e_code.output", &out)?;
    Ok(out)
}

fn first_error_node(root: Node<'_>, source: &str) -> Option<String> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            let start = node.start_position();
            let range = node.byte_range();
            let snippet = source
                .get(range.clone())
                .unwrap_or("")
                .chars()
                .take(80)
                .collect::<String>();
            return Some(format!(
                "parse recovery node kind={} line={} column={} byte_range={}..{} snippet={:?}",
                node.kind(),
                start.row + 1,
                start.column + 1,
                range.start,
                range.end,
                snippet
            ));
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    None
}

fn signed_delta(after: usize, before: usize, denom: f32) -> f32 {
    ((after as f32 - before as f32) / denom).clamp(-1.0, 1.0)
}

#[allow(dead_code)]
fn stable_source_hash(source: &str) -> u64 {
    fnv1a64(source.as_bytes())
}
