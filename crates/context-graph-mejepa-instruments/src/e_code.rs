use context_graph_core::memory::ast::{
    chunk_with_options, AstChunk, AstChunkOptions, AstChunkerError, EntityType, Language,
};
use serde::{Deserialize, Serialize};

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
    let chunks = chunk_with_options(
        input.source.as_bytes(),
        language,
        &AstChunkOptions::for_path(&input.path),
    )
    .map_err(map_chunker_error)?;
    stats_from_chunks(&chunks, &input.source)
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
    add_hashed_token_features(&mut out[32..], &input.source, 0.25);
    add_hashed_pair(&mut out, &input.path, 1.0, 16, 16);
    add_hashed_pair(&mut out, channel, 2.0, slot.dim().saturating_sub(32), 32);
    normalize_l2(&mut out);
    validate_finite_output("e_code.output", &out)?;
    Ok(out)
}

fn signed_delta(after: usize, before: usize, denom: f32) -> f32 {
    ((after as f32 - before as f32) / denom).clamp(-1.0, 1.0)
}
