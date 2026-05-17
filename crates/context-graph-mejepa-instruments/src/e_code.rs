use context_graph_core::memory::ast::{
    chunk_with_options, AstChunk, AstChunkOptions, AstChunkerError, EntityType, Language,
};
use ruff_python_ast::{
    Arguments, Comprehension, ElifElseClause, ExceptHandler, Expr, ExprCall, ExprContext,
    Parameters, Stmt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::features::{
    add_hashed_pair, add_hashed_token_features, bounded_ratio, normalize_l2,
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
    analyzer: CodeAnalyzerKind,
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
    ast_stmt_count: usize,
    ast_expr_count: usize,
    decorator_count: usize,
    comprehension_count: usize,
    match_case_count: usize,
    try_count: usize,
    with_count: usize,
    await_count: usize,
    yield_count: usize,
    cfg_block_count: usize,
    cfg_edge_count: usize,
    cfg_exit_count: usize,
    cfg_branch_count: usize,
    cfg_loop_back_edge_count: usize,
    cfg_unreachable_stmt_count: usize,
    cfg_exception_edge_count: usize,
    data_def_count: usize,
    data_use_count: usize,
    data_def_use_edge_count: usize,
    data_param_source_count: usize,
    data_call_sink_count: usize,
    data_undefined_read_count: usize,
    data_attribute_flow_count: usize,
    data_subscript_flow_count: usize,
    data_unused_def_count: usize,
    type_annotated_binding_count: usize,
    type_unannotated_binding_count: usize,
    type_return_annotation_count: usize,
    type_class_base_count: usize,
    type_generic_count: usize,
    type_union_optional_count: usize,
    type_literal_inference_count: usize,
    type_any_like_count: usize,
    type_signature_count: usize,
    type_call_site_count: usize,
    type_known_call_site_count: usize,
    type_unknown_call_site_count: usize,
    type_typed_argument_count: usize,
    type_untyped_argument_count: usize,
    type_return_edge_count: usize,
    type_any_call_site_count: usize,
    type_arity_mismatch_count: usize,
    type_signatures: BTreeMap<String, PythonTypeSignature>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CodeAnalyzerKind {
    #[default]
    GenericAstChunker,
    PythonRuffSemanticV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PythonTypeSignature {
    parameter_annotations: Vec<Option<String>>,
    return_annotation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PythonSemanticFacts {
    pub analyzer: String,
    pub ast_stmt_count: usize,
    pub ast_expr_count: usize,
    pub function_count: usize,
    pub class_count: usize,
    pub branch_count: usize,
    pub loop_count: usize,
    pub call_count: usize,
    pub cfg_block_count: usize,
    pub cfg_edge_count: usize,
    pub cfg_exit_count: usize,
    pub cfg_branch_count: usize,
    pub cfg_loop_back_edge_count: usize,
    pub cfg_unreachable_stmt_count: usize,
    pub cfg_exception_edge_count: usize,
    pub data_def_count: usize,
    pub data_use_count: usize,
    pub data_def_use_edge_count: usize,
    pub data_param_source_count: usize,
    pub data_call_sink_count: usize,
    pub data_undefined_read_count: usize,
    pub data_attribute_flow_count: usize,
    pub data_subscript_flow_count: usize,
    pub data_unused_def_count: usize,
    pub type_annotated_binding_count: usize,
    pub type_unannotated_binding_count: usize,
    pub type_return_annotation_count: usize,
    pub type_class_base_count: usize,
    pub type_generic_count: usize,
    pub type_union_optional_count: usize,
    pub type_literal_inference_count: usize,
    pub type_any_like_count: usize,
    pub type_signature_count: usize,
    pub type_call_site_count: usize,
    pub type_known_call_site_count: usize,
    pub type_unknown_call_site_count: usize,
    pub type_typed_argument_count: usize,
    pub type_untyped_argument_count: usize,
    pub type_return_edge_count: usize,
    pub type_any_call_site_count: usize,
    pub type_arity_mismatch_count: usize,
}

impl From<&CodeStats> for PythonSemanticFacts {
    fn from(stats: &CodeStats) -> Self {
        Self {
            analyzer: match stats.analyzer {
                CodeAnalyzerKind::GenericAstChunker => "generic_ast_chunker".to_string(),
                CodeAnalyzerKind::PythonRuffSemanticV1 => "python_ruff_semantic_v1".to_string(),
            },
            ast_stmt_count: stats.ast_stmt_count,
            ast_expr_count: stats.ast_expr_count,
            function_count: stats.function_count,
            class_count: stats.class_count,
            branch_count: stats.branch_count,
            loop_count: stats.loop_count,
            call_count: stats.call_count,
            cfg_block_count: stats.cfg_block_count,
            cfg_edge_count: stats.cfg_edge_count,
            cfg_exit_count: stats.cfg_exit_count,
            cfg_branch_count: stats.cfg_branch_count,
            cfg_loop_back_edge_count: stats.cfg_loop_back_edge_count,
            cfg_unreachable_stmt_count: stats.cfg_unreachable_stmt_count,
            cfg_exception_edge_count: stats.cfg_exception_edge_count,
            data_def_count: stats.data_def_count,
            data_use_count: stats.data_use_count,
            data_def_use_edge_count: stats.data_def_use_edge_count,
            data_param_source_count: stats.data_param_source_count,
            data_call_sink_count: stats.data_call_sink_count,
            data_undefined_read_count: stats.data_undefined_read_count,
            data_attribute_flow_count: stats.data_attribute_flow_count,
            data_subscript_flow_count: stats.data_subscript_flow_count,
            data_unused_def_count: stats.data_unused_def_count,
            type_annotated_binding_count: stats.type_annotated_binding_count,
            type_unannotated_binding_count: stats.type_unannotated_binding_count,
            type_return_annotation_count: stats.type_return_annotation_count,
            type_class_base_count: stats.type_class_base_count,
            type_generic_count: stats.type_generic_count,
            type_union_optional_count: stats.type_union_optional_count,
            type_literal_inference_count: stats.type_literal_inference_count,
            type_any_like_count: stats.type_any_like_count,
            type_signature_count: stats.type_signature_count,
            type_call_site_count: stats.type_call_site_count,
            type_known_call_site_count: stats.type_known_call_site_count,
            type_unknown_call_site_count: stats.type_unknown_call_site_count,
            type_typed_argument_count: stats.type_typed_argument_count,
            type_untyped_argument_count: stats.type_untyped_argument_count,
            type_return_edge_count: stats.type_return_edge_count,
            type_any_call_site_count: stats.type_any_call_site_count,
            type_arity_mismatch_count: stats.type_arity_mismatch_count,
        }
    }
}

impl Instrument for EAstInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EAst
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_code(input)?;
        encode_code(input, &stats, InstrumentSlot::EAst, "ast")
    }
}

impl Instrument for ECfgInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ECfg
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_code(input)?;
        encode_code(input, &stats, InstrumentSlot::ECfg, "cfg")
    }
}

impl Instrument for EDataFlowInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EDataFlow
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_code(input)?;
        encode_code(input, &stats, InstrumentSlot::EDataFlow, "data_flow")
    }
}

impl Instrument for ETypeGraphInstrument {
    type Input = CodeInstrumentInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ETypeGraph
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let stats = parse_code(input)?;
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
        let before_stats = parse_code(&before)?;
        let after_stats = parse_code(&after)?;
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
    parse_language(&input.language)?;
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
    if input.before_source.len() > MAX_SOURCE_BYTES {
        return Err(InstrumentError::invalid(
            "input.before_source",
            format!(
                "source is {} bytes; max supported bytes is {MAX_SOURCE_BYTES}",
                input.before_source.len()
            ),
            "chunk large files before encoding E_Diff",
        ));
    }
    if input.after_source.len() > MAX_SOURCE_BYTES {
        return Err(InstrumentError::invalid(
            "input.after_source",
            format!(
                "source is {} bytes; max supported bytes is {MAX_SOURCE_BYTES}",
                input.after_source.len()
            ),
            "chunk large files before encoding E_Diff",
        ));
    }
    parse_language(&input.language)?;
    Ok(())
}

fn parse_code(input: &CodeInstrumentInput) -> InstrumentResult<CodeStats> {
    validate_code_input(input)?;
    let language = parse_language(&input.language)?;
    if language == Language::Python {
        return parse_python_semantic_code(input);
    }
    let chunks = chunk_with_options(
        input.source.as_bytes(),
        language,
        &AstChunkOptions::for_path(&input.path),
    )
    .map_err(map_chunker_error)?;
    stats_from_chunks(&chunks, &input.source)
}

pub fn analyze_python_semantic_facts(
    input: &CodeInstrumentInput,
) -> InstrumentResult<PythonSemanticFacts> {
    validate_code_input(input)?;
    let language = parse_language(&input.language)?;
    if language != Language::Python {
        return Err(InstrumentError::invalid(
            "input.language",
            format!(
                "python semantic analyzer only accepts python, got {}",
                input.language
            ),
            "route non-Python code through its language-specific analyzer once that phase is active",
        ));
    }
    parse_python_semantic_code(input).map(|stats| PythonSemanticFacts::from(&stats))
}

fn parse_language(language: &str) -> InstrumentResult<Language> {
    Language::from_slug(language).map_err(|err| {
        InstrumentError::invalid(
            "input.language",
            format!("{}: {err}", err.code()),
            "use one of the canonical 11 AST languages before filling code instruments",
        )
    })
}

fn map_chunker_error(err: AstChunkerError) -> InstrumentError {
    let field = match &err {
        AstChunkerError::UnsupportedLanguage { .. } => "input.language",
        AstChunkerError::EmptySource
        | AstChunkerError::MixedLanguage { .. }
        | AstChunkerError::ParseFailed { .. }
        | AstChunkerError::LanguageSetFailed { .. }
        | AstChunkerError::RoutingMissing { .. } => "input.source",
    };
    InstrumentError::invalid(
        field,
        format!("{}: {err}", err.code()),
        "fix the source, language, or parser configuration before encoding code instruments; recovered ASTs are never encoded",
    )
}

fn stats_from_chunks(chunks: &[AstChunk], source: &str) -> InstrumentResult<CodeStats> {
    if chunks.is_empty() {
        return Err(InstrumentError::invalid(
            "input.source",
            "AST chunker returned zero chunks for non-empty source",
            "treat zero emitted chunks as a chunker invariant violation and inspect parser evidence",
        ));
    }
    let mut stats = CodeStats {
        node_count: chunks.len(),
        named_node_count: chunks.len(),
        max_depth: chunks
            .iter()
            .map(|chunk| chunk.parent_chain.len() + 1)
            .max()
            .unwrap_or(1),
        ..CodeStats::default()
    };
    for chunk in chunks {
        match chunk.entity_type {
            EntityType::Function | EntityType::Method | EntityType::TestFunction => {
                stats.function_count += 1
            }
            EntityType::Class
            | EntityType::Struct
            | EntityType::Enum
            | EntityType::TraitOrInterface
            | EntityType::Impl => stats.class_count += 1,
            EntityType::Import => stats.import_count += 1,
            EntityType::Module
            | EntityType::Namespace
            | EntityType::CommentBlock
            | EntityType::Docstring => {}
        }
    }
    stats.branch_count = count_words(source, &["if", "else", "match", "case", "switch"]);
    stats.loop_count = count_words(source, &["for", "while", "loop", "foreach"]);
    stats.call_count = source
        .matches('(')
        .count()
        .saturating_sub(stats.function_count);
    stats.assignment_count = source.matches('=').count();
    stats.identifier_count = source
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .count();
    stats.type_hint_count = source.matches(':').count() + source.matches("->").count();
    stats.return_count = count_words(source, &["return", "yield"]);
    Ok(stats)
}

fn parse_python_semantic_code(input: &CodeInstrumentInput) -> InstrumentResult<CodeStats> {
    let parsed = ruff_python_parser::parse_module(&input.source).map_err(|err| {
        InstrumentError::invalid(
            "input.source",
            format!("ruff-python-parser rejected source: {err}"),
            "fix Python syntax before encoding semantic instruments; recovered ASTs are not accepted",
        )
    })?;
    let module = parsed.into_syntax();
    let mut stats = CodeStats {
        analyzer: CodeAnalyzerKind::PythonRuffSemanticV1,
        ..CodeStats::default()
    };
    collect_python_type_signatures(&module.body, None, &mut stats);
    let mut scope = PythonScope::module();
    analyze_python_suite(&module.body, &mut scope, &mut stats, 1);
    scope.finish(&mut stats);
    if stats.ast_stmt_count == 0 {
        stats.node_count = 1;
        stats.named_node_count = 1;
        stats.max_depth = 1;
        stats.identifier_count = count_python_identifiers(&input.source);
        return Ok(stats);
    }
    stats.node_count = stats.ast_stmt_count + stats.ast_expr_count;
    stats.named_node_count = stats.node_count;
    stats.identifier_count = count_python_identifiers(&input.source);
    stats.assignment_count = stats.data_def_count;
    stats.type_hint_count = stats.type_annotated_binding_count + stats.type_return_annotation_count;
    Ok(stats)
}

fn collect_python_type_signatures(
    suite: &[Stmt],
    parent_qualname: Option<&str>,
    stats: &mut CodeStats,
) {
    for stmt in suite {
        match stmt {
            Stmt::FunctionDef(function) => {
                let name = function.name.as_str();
                let qualname = parent_qualname
                    .map(|parent| format!("{parent}.{name}"))
                    .unwrap_or_else(|| name.to_string());
                let signature =
                    python_type_signature(&function.parameters, function.returns.as_deref());
                stats.type_signature_count += 1;
                stats
                    .type_signatures
                    .entry(qualname.clone())
                    .or_insert_with(|| signature.clone());
                stats
                    .type_signatures
                    .entry(name.to_string())
                    .or_insert_with(|| signature.clone());
                collect_python_type_signatures(&function.body, Some(&qualname), stats);
            }
            Stmt::ClassDef(class_def) => {
                let name = class_def.name.as_str();
                let qualname = parent_qualname
                    .map(|parent| format!("{parent}.{name}"))
                    .unwrap_or_else(|| name.to_string());
                stats.type_signatures.entry(qualname.clone()).or_default();
                stats.type_signatures.entry(name.to_string()).or_default();
                collect_python_type_signatures(&class_def.body, Some(&qualname), stats);
            }
            _ => {}
        }
    }
}

fn python_type_signature(parameters: &Parameters, returns: Option<&Expr>) -> PythonTypeSignature {
    let mut parameter_annotations = Vec::new();
    for (idx, param) in parameters.iter().enumerate() {
        let name = param.name().as_str();
        if idx == 0 && matches!(name, "self" | "cls") {
            continue;
        }
        parameter_annotations.push(param.annotation().map(annotation_label));
    }
    PythonTypeSignature {
        parameter_annotations,
        return_annotation: returns.map(annotation_label),
    }
}

#[derive(Debug, Default)]
struct PythonScope {
    defined: BTreeMap<String, usize>,
    read: BTreeMap<String, usize>,
    type_bindings: BTreeMap<String, String>,
}

impl PythonScope {
    fn module() -> Self {
        let mut scope = Self::default();
        for builtin in PYTHON_BUILTINS {
            scope.defined.insert((*builtin).to_string(), 1);
        }
        scope
    }

    fn function(parameters: &Parameters, stats: &mut CodeStats) -> Self {
        let mut scope = Self::module();
        for param in parameters.iter() {
            define_name(param.name().as_str(), &mut scope, stats);
            stats.data_param_source_count += 1;
            if let Some(annotation) = param.annotation() {
                scope.type_bindings.insert(
                    param.name().as_str().to_string(),
                    annotation_label(annotation),
                );
                record_type_annotation(annotation, stats);
            } else {
                stats.type_unannotated_binding_count += 1;
            }
        }
        scope
    }

    fn finish(self, stats: &mut CodeStats) {
        for (name, count) in self.defined {
            if name == "_" || is_python_builtin(&name) {
                continue;
            }
            if self.read.get(&name).copied().unwrap_or(0) == 0 {
                stats.data_unused_def_count += count;
            }
        }
    }
}

fn analyze_python_suite(
    suite: &[Stmt],
    scope: &mut PythonScope,
    stats: &mut CodeStats,
    depth: usize,
) -> bool {
    let mut reachable = true;
    let mut saw_stmt = false;
    for stmt in suite {
        if reachable && saw_stmt {
            stats.cfg_edge_count += 1;
        }
        if !reachable {
            stats.cfg_unreachable_stmt_count += 1;
        }
        let falls_through = analyze_python_stmt(stmt, scope, stats, depth, reachable);
        reachable = reachable && falls_through;
        saw_stmt = true;
    }
    !saw_stmt || reachable
}

fn analyze_python_stmt(
    stmt: &Stmt,
    scope: &mut PythonScope,
    stats: &mut CodeStats,
    depth: usize,
    reachable: bool,
) -> bool {
    stats.ast_stmt_count += 1;
    stats.cfg_block_count += 1;
    stats.max_depth = stats.max_depth.max(depth);
    match stmt {
        Stmt::FunctionDef(function) => {
            stats.function_count += 1;
            define_name(function.name.as_str(), scope, stats);
            stats.decorator_count += function.decorator_list.len();
            for decorator in &function.decorator_list {
                analyze_python_expr(&decorator.expression, scope, stats, depth + 1);
            }
            if let Some(returns) = &function.returns {
                stats.type_return_annotation_count += 1;
                record_type_annotation(returns, stats);
            }
            let mut function_scope = PythonScope::function(&function.parameters, stats);
            analyze_python_suite(&function.body, &mut function_scope, stats, depth + 1);
            function_scope.finish(stats);
            true
        }
        Stmt::ClassDef(class_def) => {
            stats.class_count += 1;
            define_name(class_def.name.as_str(), scope, stats);
            stats.decorator_count += class_def.decorator_list.len();
            for decorator in &class_def.decorator_list {
                analyze_python_expr(&decorator.expression, scope, stats, depth + 1);
            }
            if let Some(arguments) = &class_def.arguments {
                stats.type_class_base_count += arguments.len();
                analyze_python_arguments(arguments, scope, stats, depth + 1);
            }
            let mut class_scope = PythonScope::module();
            analyze_python_suite(&class_def.body, &mut class_scope, stats, depth + 1);
            class_scope.finish(stats);
            true
        }
        Stmt::Return(stmt_return) => {
            stats.return_count += 1;
            if let Some(value) = &stmt_return.value {
                analyze_python_expr(value, scope, stats, depth + 1);
            }
            if reachable {
                stats.cfg_exit_count += 1;
            }
            false
        }
        Stmt::Delete(stmt_delete) => {
            for target in &stmt_delete.targets {
                analyze_python_expr(target, scope, stats, depth + 1);
            }
            true
        }
        Stmt::TypeAlias(type_alias) => {
            define_python_target(&type_alias.name, scope, stats, depth + 1);
            record_type_annotation(&type_alias.value, stats);
            true
        }
        Stmt::Assign(assign) => {
            analyze_python_expr(&assign.value, scope, stats, depth + 1);
            if let Some(kind) = literal_type_kind(&assign.value) {
                stats.type_literal_inference_count += 1;
                add_hashed_pair_to_counts(kind, stats);
                for target in &assign.targets {
                    bind_python_target_type(target, kind, scope);
                }
            }
            for target in &assign.targets {
                define_python_target(target, scope, stats, depth + 1);
                stats.type_unannotated_binding_count += 1;
            }
            true
        }
        Stmt::AugAssign(aug_assign) => {
            analyze_python_expr(&aug_assign.target, scope, stats, depth + 1);
            analyze_python_expr(&aug_assign.value, scope, stats, depth + 1);
            define_python_target(&aug_assign.target, scope, stats, depth + 1);
            true
        }
        Stmt::AnnAssign(ann_assign) => {
            record_type_annotation(&ann_assign.annotation, stats);
            bind_python_target_type(
                &ann_assign.target,
                &annotation_label(&ann_assign.annotation),
                scope,
            );
            if let Some(value) = &ann_assign.value {
                analyze_python_expr(value, scope, stats, depth + 1);
            }
            define_python_target(&ann_assign.target, scope, stats, depth + 1);
            true
        }
        Stmt::For(stmt_for) => {
            stats.loop_count += 1;
            stats.cfg_branch_count += 1;
            stats.cfg_edge_count += usize::from(reachable) * 2;
            stats.cfg_loop_back_edge_count += usize::from(reachable);
            analyze_python_expr(&stmt_for.iter, scope, stats, depth + 1);
            define_python_target(&stmt_for.target, scope, stats, depth + 1);
            analyze_python_suite(&stmt_for.body, scope, stats, depth + 1);
            analyze_python_suite(&stmt_for.orelse, scope, stats, depth + 1);
            true
        }
        Stmt::While(stmt_while) => {
            stats.loop_count += 1;
            stats.cfg_branch_count += 1;
            stats.cfg_edge_count += usize::from(reachable) * 2;
            stats.cfg_loop_back_edge_count += usize::from(reachable);
            analyze_python_expr(&stmt_while.test, scope, stats, depth + 1);
            analyze_python_suite(&stmt_while.body, scope, stats, depth + 1);
            analyze_python_suite(&stmt_while.orelse, scope, stats, depth + 1);
            true
        }
        Stmt::If(stmt_if) => {
            stats.branch_count += 1 + stmt_if.elif_else_clauses.len();
            let branch_count = 1 + stmt_if.elif_else_clauses.len();
            stats.cfg_branch_count += branch_count;
            stats.cfg_edge_count += usize::from(reachable) * branch_count;
            analyze_python_expr(&stmt_if.test, scope, stats, depth + 1);
            let mut any_branch_falls_through =
                analyze_python_suite(&stmt_if.body, scope, stats, depth + 1);
            let mut has_else = false;
            for clause in &stmt_if.elif_else_clauses {
                any_branch_falls_through |=
                    analyze_python_elif_else_clause(clause, scope, stats, depth + 1);
                has_else |= clause.test.is_none();
            }
            !has_else || any_branch_falls_through
        }
        Stmt::With(stmt_with) => {
            stats.with_count += 1;
            for item in &stmt_with.items {
                analyze_python_expr(&item.context_expr, scope, stats, depth + 1);
                if let Some(optional_vars) = &item.optional_vars {
                    define_python_target(optional_vars, scope, stats, depth + 1);
                }
            }
            analyze_python_suite(&stmt_with.body, scope, stats, depth + 1);
            true
        }
        Stmt::Match(stmt_match) => {
            analyze_python_expr(&stmt_match.subject, scope, stats, depth + 1);
            stats.match_case_count += stmt_match.cases.len();
            stats.branch_count += stmt_match.cases.len();
            stats.cfg_branch_count += stmt_match.cases.len();
            stats.cfg_edge_count += usize::from(reachable) * stmt_match.cases.len();
            let mut any_case_falls_through = false;
            for case in &stmt_match.cases {
                if let Some(guard) = &case.guard {
                    analyze_python_expr(guard, scope, stats, depth + 1);
                }
                any_case_falls_through |= analyze_python_suite(&case.body, scope, stats, depth + 1);
            }
            any_case_falls_through
        }
        Stmt::Raise(stmt_raise) => {
            if let Some(exc) = &stmt_raise.exc {
                analyze_python_expr(exc, scope, stats, depth + 1);
            }
            if let Some(cause) = &stmt_raise.cause {
                analyze_python_expr(cause, scope, stats, depth + 1);
            }
            if reachable {
                stats.cfg_exit_count += 1;
            }
            false
        }
        Stmt::Try(stmt_try) => {
            stats.try_count += 1;
            stats.cfg_exception_edge_count += stmt_try.handlers.len();
            stats.cfg_edge_count += usize::from(reachable) * stmt_try.handlers.len();
            let mut falls_through = analyze_python_suite(&stmt_try.body, scope, stats, depth + 1);
            for handler in &stmt_try.handlers {
                let ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = &handler.type_ {
                    analyze_python_expr(type_, scope, stats, depth + 1);
                }
                if let Some(name) = &handler.name {
                    define_name(name.as_str(), scope, stats);
                }
                falls_through |= analyze_python_suite(&handler.body, scope, stats, depth + 1);
            }
            falls_through |= analyze_python_suite(&stmt_try.orelse, scope, stats, depth + 1);
            falls_through |= analyze_python_suite(&stmt_try.finalbody, scope, stats, depth + 1);
            falls_through
        }
        Stmt::Assert(stmt_assert) => {
            analyze_python_expr(&stmt_assert.test, scope, stats, depth + 1);
            if let Some(msg) = &stmt_assert.msg {
                analyze_python_expr(msg, scope, stats, depth + 1);
            }
            true
        }
        Stmt::Import(stmt_import) => {
            stats.import_count += stmt_import.names.len();
            for alias in &stmt_import.names {
                let name = alias
                    .asname
                    .as_ref()
                    .map(|asname| asname.as_str())
                    .unwrap_or_else(|| alias.name.as_str().split('.').next().unwrap_or(""));
                define_name(name, scope, stats);
            }
            true
        }
        Stmt::ImportFrom(stmt_import_from) => {
            stats.import_count += stmt_import_from.names.len();
            for alias in &stmt_import_from.names {
                let name = alias
                    .asname
                    .as_ref()
                    .map(|asname| asname.as_str())
                    .unwrap_or_else(|| alias.name.as_str());
                if name != "*" {
                    define_name(name, scope, stats);
                }
            }
            true
        }
        Stmt::Global(_) | Stmt::Nonlocal(_) | Stmt::Pass(_) | Stmt::IpyEscapeCommand(_) => true,
        Stmt::Expr(stmt_expr) => {
            analyze_python_expr(&stmt_expr.value, scope, stats, depth + 1);
            true
        }
        Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

fn analyze_python_elif_else_clause(
    clause: &ElifElseClause,
    scope: &mut PythonScope,
    stats: &mut CodeStats,
    depth: usize,
) -> bool {
    if let Some(test) = &clause.test {
        analyze_python_expr(test, scope, stats, depth + 1);
    }
    analyze_python_suite(&clause.body, scope, stats, depth + 1)
}

fn analyze_python_expr(expr: &Expr, scope: &mut PythonScope, stats: &mut CodeStats, depth: usize) {
    stats.ast_expr_count += 1;
    stats.max_depth = stats.max_depth.max(depth);
    match expr {
        Expr::BoolOp(bool_op) => {
            stats.branch_count += bool_op.values.len().saturating_sub(1);
            for value in &bool_op.values {
                analyze_python_expr(value, scope, stats, depth + 1);
            }
        }
        Expr::Named(named) => {
            analyze_python_expr(&named.value, scope, stats, depth + 1);
            define_python_target(&named.target, scope, stats, depth + 1);
        }
        Expr::BinOp(bin_op) => {
            analyze_python_expr(&bin_op.left, scope, stats, depth + 1);
            analyze_python_expr(&bin_op.right, scope, stats, depth + 1);
        }
        Expr::UnaryOp(unary_op) => analyze_python_expr(&unary_op.operand, scope, stats, depth + 1),
        Expr::Lambda(lambda) => {
            if let Some(parameters) = &lambda.parameters {
                let mut lambda_scope = PythonScope::function(parameters, stats);
                analyze_python_expr(&lambda.body, &mut lambda_scope, stats, depth + 1);
                lambda_scope.finish(stats);
            } else {
                analyze_python_expr(&lambda.body, scope, stats, depth + 1);
            }
        }
        Expr::If(expr_if) => {
            stats.branch_count += 1;
            analyze_python_expr(&expr_if.test, scope, stats, depth + 1);
            analyze_python_expr(&expr_if.body, scope, stats, depth + 1);
            analyze_python_expr(&expr_if.orelse, scope, stats, depth + 1);
        }
        Expr::Dict(dict) => {
            for item in &dict.items {
                if let Some(key) = &item.key {
                    analyze_python_expr(key, scope, stats, depth + 1);
                }
                analyze_python_expr(&item.value, scope, stats, depth + 1);
            }
        }
        Expr::Set(set) => {
            for elt in &set.elts {
                analyze_python_expr(elt, scope, stats, depth + 1);
            }
        }
        Expr::ListComp(comp) => {
            analyze_python_comprehensions(&comp.generators, scope, stats, depth + 1);
            analyze_python_expr(&comp.elt, scope, stats, depth + 1);
        }
        Expr::SetComp(comp) => {
            analyze_python_comprehensions(&comp.generators, scope, stats, depth + 1);
            analyze_python_expr(&comp.elt, scope, stats, depth + 1);
        }
        Expr::DictComp(comp) => {
            analyze_python_comprehensions(&comp.generators, scope, stats, depth + 1);
            analyze_python_expr(&comp.key, scope, stats, depth + 1);
            analyze_python_expr(&comp.value, scope, stats, depth + 1);
        }
        Expr::Generator(generator) => {
            analyze_python_comprehensions(&generator.generators, scope, stats, depth + 1);
            analyze_python_expr(&generator.elt, scope, stats, depth + 1);
        }
        Expr::Await(await_expr) => {
            stats.await_count += 1;
            analyze_python_expr(&await_expr.value, scope, stats, depth + 1);
        }
        Expr::Yield(yield_expr) => {
            stats.yield_count += 1;
            if let Some(value) = &yield_expr.value {
                analyze_python_expr(value, scope, stats, depth + 1);
            }
        }
        Expr::YieldFrom(yield_from) => {
            stats.yield_count += 1;
            analyze_python_expr(&yield_from.value, scope, stats, depth + 1);
        }
        Expr::Compare(compare) => {
            analyze_python_expr(&compare.left, scope, stats, depth + 1);
            for comparator in &compare.comparators {
                analyze_python_expr(comparator, scope, stats, depth + 1);
            }
        }
        Expr::Call(call) => {
            stats.call_count += 1;
            stats.data_call_sink_count += 1;
            analyze_python_call_type(call, scope, stats);
            analyze_python_expr(&call.func, scope, stats, depth + 1);
            analyze_python_arguments(&call.arguments, scope, stats, depth + 1);
        }
        Expr::Attribute(attribute) => {
            stats.data_attribute_flow_count += 1;
            analyze_python_expr(&attribute.value, scope, stats, depth + 1);
        }
        Expr::Subscript(subscript) => {
            stats.data_subscript_flow_count += 1;
            analyze_python_expr(&subscript.value, scope, stats, depth + 1);
            analyze_python_expr(&subscript.slice, scope, stats, depth + 1);
        }
        Expr::Starred(starred) => analyze_python_expr(&starred.value, scope, stats, depth + 1),
        Expr::Name(name) => match name.ctx {
            ExprContext::Load => read_name(name.id.as_str(), scope, stats),
            ExprContext::Store => define_name(name.id.as_str(), scope, stats),
            ExprContext::Del | ExprContext::Invalid => {}
        },
        Expr::List(list) => {
            for elt in &list.elts {
                analyze_python_expr(elt, scope, stats, depth + 1);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                analyze_python_expr(elt, scope, stats, depth + 1);
            }
        }
        Expr::Slice(slice) => {
            if let Some(lower) = &slice.lower {
                analyze_python_expr(lower, scope, stats, depth + 1);
            }
            if let Some(upper) = &slice.upper {
                analyze_python_expr(upper, scope, stats, depth + 1);
            }
            if let Some(step) = &slice.step {
                analyze_python_expr(step, scope, stats, depth + 1);
            }
        }
        Expr::FString(_)
        | Expr::TString(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_)
        | Expr::IpyEscapeCommand(_) => {}
    }
}

fn analyze_python_comprehensions(
    comprehensions: &[Comprehension],
    scope: &mut PythonScope,
    stats: &mut CodeStats,
    depth: usize,
) {
    stats.comprehension_count += comprehensions.len();
    for comprehension in comprehensions {
        analyze_python_expr(&comprehension.iter, scope, stats, depth + 1);
        define_python_target(&comprehension.target, scope, stats, depth + 1);
        for if_expr in &comprehension.ifs {
            analyze_python_expr(if_expr, scope, stats, depth + 1);
        }
    }
}

fn analyze_python_arguments(
    arguments: &Arguments,
    scope: &mut PythonScope,
    stats: &mut CodeStats,
    depth: usize,
) {
    for arg in &arguments.args {
        analyze_python_expr(arg, scope, stats, depth + 1);
    }
    for keyword in &arguments.keywords {
        analyze_python_expr(&keyword.value, scope, stats, depth + 1);
    }
}

fn define_python_target(
    target: &Expr,
    scope: &mut PythonScope,
    stats: &mut CodeStats,
    depth: usize,
) {
    match target {
        Expr::Name(name) => {
            if matches!(name.ctx, ExprContext::Store | ExprContext::Load) {
                define_name(name.id.as_str(), scope, stats);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                define_python_target(elt, scope, stats, depth + 1);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                define_python_target(elt, scope, stats, depth + 1);
            }
        }
        Expr::Starred(starred) => define_python_target(&starred.value, scope, stats, depth + 1),
        Expr::Attribute(attribute) => {
            stats.data_attribute_flow_count += 1;
            analyze_python_expr(&attribute.value, scope, stats, depth + 1);
        }
        Expr::Subscript(subscript) => {
            stats.data_subscript_flow_count += 1;
            analyze_python_expr(&subscript.value, scope, stats, depth + 1);
            analyze_python_expr(&subscript.slice, scope, stats, depth + 1);
        }
        _ => analyze_python_expr(target, scope, stats, depth + 1),
    }
}

fn define_name(name: &str, scope: &mut PythonScope, stats: &mut CodeStats) {
    if name.trim().is_empty() {
        return;
    }
    *scope.defined.entry(name.to_string()).or_insert(0) += 1;
    stats.data_def_count += 1;
}

fn read_name(name: &str, scope: &mut PythonScope, stats: &mut CodeStats) {
    if name.trim().is_empty() {
        return;
    }
    *scope.read.entry(name.to_string()).or_insert(0) += 1;
    stats.data_use_count += 1;
    if scope.defined.contains_key(name) || is_python_builtin(name) {
        stats.data_def_use_edge_count += 1;
    } else {
        stats.data_undefined_read_count += 1;
    }
}

fn analyze_python_call_type(call: &ExprCall, scope: &PythonScope, stats: &mut CodeStats) {
    stats.type_call_site_count += 1;
    let Some(target) = python_call_target_name(&call.func) else {
        stats.type_unknown_call_site_count += 1;
        return;
    };
    let signature = resolve_python_type_signature(&target, stats).cloned();
    if let Some(signature) = signature {
        stats.type_known_call_site_count += 1;
        if signature.return_annotation.is_some() {
            stats.type_return_edge_count += 1;
        }
        if signature_has_any(&signature) {
            stats.type_any_call_site_count += 1;
        }
        let positional_count = call.arguments.args.len();
        if positional_count != signature.parameter_annotations.len() {
            stats.type_arity_mismatch_count += 1;
        }
        for (idx, arg) in call.arguments.args.iter().enumerate() {
            let arg_type = infer_python_expr_type(arg, scope);
            let parameter_annotation = signature
                .parameter_annotations
                .get(idx)
                .and_then(|annotation| annotation.as_ref());
            if arg_type.is_some() || parameter_annotation.is_some() {
                stats.type_typed_argument_count += 1;
            } else {
                stats.type_untyped_argument_count += 1;
            }
            if arg_type.as_deref().is_some_and(annotation_text_is_any_like)
                || parameter_annotation
                    .map(String::as_str)
                    .is_some_and(annotation_text_is_any_like)
            {
                stats.type_any_call_site_count += 1;
            }
        }
    } else {
        stats.type_unknown_call_site_count += 1;
        for arg in &call.arguments.args {
            if infer_python_expr_type(arg, scope).is_some() {
                stats.type_typed_argument_count += 1;
            } else {
                stats.type_untyped_argument_count += 1;
            }
        }
    }
}

fn python_call_target_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str().to_string()),
        Expr::Attribute(attribute) => python_call_target_name(&attribute.value)
            .map(|prefix| format!("{prefix}.{}", attribute.attr.as_str()))
            .or_else(|| Some(attribute.attr.as_str().to_string())),
        _ => None,
    }
}

fn resolve_python_type_signature<'a>(
    target: &str,
    stats: &'a CodeStats,
) -> Option<&'a PythonTypeSignature> {
    if let Some(signature) = stats.type_signatures.get(target) {
        return Some(signature);
    }
    let suffix = target.rsplit('.').next().unwrap_or(target);
    if let Some(signature) = stats.type_signatures.get(suffix) {
        return Some(signature);
    }
    let suffix_key = format!(".{suffix}");
    let mut matches = stats
        .type_signatures
        .iter()
        .filter(|(name, _)| name.ends_with(&suffix_key));
    let first = matches.next().map(|(_, signature)| signature)?;
    if matches.next().is_none() {
        Some(first)
    } else {
        None
    }
}

fn infer_python_expr_type(expr: &Expr, scope: &PythonScope) -> Option<String> {
    if let Some(kind) = literal_type_kind(expr) {
        return Some(kind.to_string());
    }
    match expr {
        Expr::Name(name) => scope.type_bindings.get(name.id.as_str()).cloned(),
        Expr::List(_) | Expr::ListComp(_) => Some("list".to_string()),
        Expr::Tuple(_) => Some("tuple".to_string()),
        Expr::Dict(_) | Expr::DictComp(_) => Some("dict".to_string()),
        Expr::Set(_) | Expr::SetComp(_) => Some("set".to_string()),
        _ => None,
    }
}

fn bind_python_target_type(target: &Expr, ty: &str, scope: &mut PythonScope) {
    match target {
        Expr::Name(name) => {
            scope
                .type_bindings
                .insert(name.id.as_str().to_string(), ty.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                bind_python_target_type(elt, ty, scope);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                bind_python_target_type(elt, ty, scope);
            }
        }
        Expr::Starred(starred) => bind_python_target_type(&starred.value, ty, scope),
        _ => {}
    }
}

fn record_type_annotation(annotation: &Expr, stats: &mut CodeStats) {
    stats.type_annotated_binding_count += 1;
    record_type_annotation_inner(annotation, stats);
}

fn record_type_annotation_inner(annotation: &Expr, stats: &mut CodeStats) {
    match annotation {
        Expr::Name(name) if name.id.as_str() == "Any" => {
            stats.type_any_like_count += 1;
        }
        Expr::Name(_) => {}
        Expr::Attribute(attribute) => {
            if attribute.attr.as_str() == "Any" {
                stats.type_any_like_count += 1;
            }
            record_type_annotation_inner(&attribute.value, stats);
        }
        Expr::Subscript(subscript) => {
            stats.type_generic_count += 1;
            if annotation_is_union_like(&subscript.value) {
                stats.type_union_optional_count += 1;
            }
            record_type_annotation_inner(&subscript.value, stats);
            record_type_annotation_inner(&subscript.slice, stats);
        }
        Expr::BinOp(bin_op) => {
            stats.type_union_optional_count += 1;
            record_type_annotation_inner(&bin_op.left, stats);
            record_type_annotation_inner(&bin_op.right, stats);
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                record_type_annotation_inner(elt, stats);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                record_type_annotation_inner(elt, stats);
            }
        }
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_) => {}
        _ => {}
    }
}

fn annotation_is_union_like(expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => matches!(name.id.as_str(), "Union" | "Optional"),
        Expr::Attribute(attribute) => matches!(attribute.attr.as_str(), "Union" | "Optional"),
        _ => false,
    }
}

fn annotation_label(annotation: &Expr) -> String {
    match annotation {
        Expr::Name(name) => name.id.as_str().to_string(),
        Expr::Attribute(attribute) => {
            let prefix = annotation_label(&attribute.value);
            if prefix.is_empty() {
                attribute.attr.as_str().to_string()
            } else {
                format!("{prefix}.{}", attribute.attr.as_str())
            }
        }
        Expr::Subscript(subscript) => format!("{}[]", annotation_label(&subscript.value)),
        Expr::BinOp(_) => "union".to_string(),
        Expr::Tuple(_) => "tuple".to_string(),
        Expr::List(_) => "list".to_string(),
        Expr::StringLiteral(_) => "forward_ref".to_string(),
        Expr::NoneLiteral(_) => "None".to_string(),
        _ => "unknown".to_string(),
    }
}

fn annotation_text_is_any_like(text: &str) -> bool {
    text == "Any" || text.ends_with(".Any") || text == "unknown"
}

fn signature_has_any(signature: &PythonTypeSignature) -> bool {
    signature
        .parameter_annotations
        .iter()
        .flatten()
        .any(|annotation| annotation_text_is_any_like(annotation))
        || signature
            .return_annotation
            .as_deref()
            .is_some_and(annotation_text_is_any_like)
}

fn literal_type_kind(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NumberLiteral(_) => Some("number"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("none"),
        Expr::List(_) | Expr::ListComp(_) => Some("list"),
        Expr::Tuple(_) => Some("tuple"),
        Expr::Dict(_) | Expr::DictComp(_) => Some("dict"),
        Expr::Set(_) | Expr::SetComp(_) => Some("set"),
        _ => None,
    }
}

fn add_hashed_pair_to_counts(kind: &str, stats: &mut CodeStats) {
    if kind == "none" || kind == "bool" {
        stats.type_union_optional_count += usize::from(kind == "none");
    }
}

fn count_python_identifiers(source: &str) -> usize {
    source
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| {
            !token.is_empty()
                && token
                    .chars()
                    .next()
                    .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        })
        .count()
}

fn is_python_builtin(name: &str) -> bool {
    PYTHON_BUILTINS.contains(&name)
}

const PYTHON_BUILTINS: &[&str] = &[
    "Any",
    "BaseException",
    "Exception",
    "False",
    "None",
    "NotImplemented",
    "Optional",
    "True",
    "Union",
    "__debug__",
    "abs",
    "all",
    "any",
    "bool",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "dict",
    "enumerate",
    "filter",
    "float",
    "getattr",
    "hasattr",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "list",
    "map",
    "max",
    "min",
    "next",
    "object",
    "open",
    "print",
    "property",
    "range",
    "repr",
    "reversed",
    "set",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "zip",
];

fn count_words(source: &str, words: &[&str]) -> usize {
    source
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .filter(|token| words.contains(token))
        .count()
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
    if stats.analyzer == CodeAnalyzerKind::PythonRuffSemanticV1 {
        encode_python_semantic_channel(&mut out, stats, slot);
    }
    add_hashed_token_features(&mut out[32..], &input.source, 0.25);
    add_hashed_pair(&mut out, &input.path, 1.0, 16, 16);
    add_hashed_pair(&mut out, channel, 2.0, slot.dim().saturating_sub(32), 32);
    normalize_l2(&mut out);
    validate_finite_output("e_code.output", &out)?;
    Ok(out)
}

fn encode_python_semantic_channel(out: &mut [f32], stats: &CodeStats, slot: InstrumentSlot) {
    let values: &[f32] = match slot {
        InstrumentSlot::EAst => &[
            bounded_ratio(stats.ast_stmt_count as f32, 100_000.0),
            bounded_ratio(stats.ast_expr_count as f32, 250_000.0),
            bounded_ratio(stats.decorator_count as f32, 10_000.0),
            bounded_ratio(stats.comprehension_count as f32, 10_000.0),
            bounded_ratio(stats.match_case_count as f32, 10_000.0),
            bounded_ratio(stats.try_count as f32, 10_000.0),
            bounded_ratio(stats.with_count as f32, 10_000.0),
            bounded_ratio(stats.await_count as f32, 10_000.0),
            bounded_ratio(stats.yield_count as f32, 10_000.0),
        ],
        InstrumentSlot::ECfg => &[
            bounded_ratio(stats.cfg_block_count as f32, 100_000.0),
            bounded_ratio(stats.cfg_edge_count as f32, 250_000.0),
            bounded_ratio(stats.cfg_exit_count as f32, 10_000.0),
            bounded_ratio(stats.cfg_branch_count as f32, 10_000.0),
            bounded_ratio(stats.cfg_loop_back_edge_count as f32, 10_000.0),
            bounded_ratio(stats.cfg_unreachable_stmt_count as f32, 10_000.0),
            bounded_ratio(stats.cfg_exception_edge_count as f32, 10_000.0),
            cfg_cyclomatic_ratio(stats),
        ],
        InstrumentSlot::EDataFlow => &[
            bounded_ratio(stats.data_def_count as f32, 100_000.0),
            bounded_ratio(stats.data_use_count as f32, 250_000.0),
            bounded_ratio(stats.data_def_use_edge_count as f32, 250_000.0),
            bounded_ratio(stats.data_param_source_count as f32, 10_000.0),
            bounded_ratio(stats.data_call_sink_count as f32, 50_000.0),
            bounded_ratio(stats.data_undefined_read_count as f32, 10_000.0),
            bounded_ratio(stats.data_attribute_flow_count as f32, 50_000.0),
            bounded_ratio(stats.data_subscript_flow_count as f32, 50_000.0),
            bounded_ratio(stats.data_unused_def_count as f32, 50_000.0),
        ],
        InstrumentSlot::ETypeGraph => &[
            bounded_ratio(stats.type_annotated_binding_count as f32, 50_000.0),
            bounded_ratio(stats.type_unannotated_binding_count as f32, 50_000.0),
            bounded_ratio(stats.type_return_annotation_count as f32, 10_000.0),
            bounded_ratio(stats.type_class_base_count as f32, 10_000.0),
            bounded_ratio(stats.type_generic_count as f32, 10_000.0),
            bounded_ratio(stats.type_union_optional_count as f32, 10_000.0),
            bounded_ratio(stats.type_literal_inference_count as f32, 50_000.0),
            bounded_ratio(stats.type_any_like_count as f32, 10_000.0),
            bounded_ratio(stats.type_signature_count as f32, 50_000.0),
            bounded_ratio(stats.type_call_site_count as f32, 100_000.0),
            bounded_ratio(stats.type_known_call_site_count as f32, 100_000.0),
            bounded_ratio(stats.type_unknown_call_site_count as f32, 100_000.0),
            bounded_ratio(stats.type_typed_argument_count as f32, 250_000.0),
            bounded_ratio(stats.type_untyped_argument_count as f32, 250_000.0),
            bounded_ratio(stats.type_return_edge_count as f32, 100_000.0),
            bounded_ratio(stats.type_any_call_site_count as f32, 100_000.0),
            bounded_ratio(stats.type_arity_mismatch_count as f32, 50_000.0),
        ],
        _ => &[],
    };
    for (idx, value) in values.iter().enumerate() {
        let offset = 16 + idx;
        if offset < out.len() {
            out[offset] = *value;
        }
    }
}

fn cfg_cyclomatic_ratio(stats: &CodeStats) -> f32 {
    if stats.cfg_block_count == 0 {
        return 0.0;
    }
    let complexity = stats
        .cfg_edge_count
        .saturating_add(2)
        .saturating_sub(stats.cfg_block_count);
    bounded_ratio(complexity as f32, 10_000.0)
}

fn signed_delta(after: usize, before: usize, denom: f32) -> f32 {
    ((after as f32 - before as f32) / denom).clamp(-1.0, 1.0)
}
