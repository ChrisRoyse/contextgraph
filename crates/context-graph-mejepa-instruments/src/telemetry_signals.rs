use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_TELEMETRY_SIGNALS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use ruff_python_ast::{Arguments, Comprehension, ElifElseClause, Expr, Stmt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::e_runtime::{ERuntimeInstrument, RuntimeInput};
use crate::features::fnv1a64;
use crate::{Instrument, InstrumentError, InstrumentResult};

const TELEMETRY_SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 10_000_000;
const MAX_SYMBOL_OBSERVATIONS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetrySource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub language: String,
    pub path: String,
    pub before_source: String,
    pub after_source: String,
    pub baseline_latency_ms: u64,
    pub baseline_vram_bytes: u64,
    pub baseline_cost_micro_usd: u64,
    pub symbol_observations: Vec<SymbolCallObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolCallObservation {
    pub symbol: String,
    pub calls_before: u64,
    pub calls_after: u64,
    pub inclusive_time_ms_before: f32,
    pub inclusive_time_ms_after: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BigOClass {
    O1,
    OLogN,
    ON,
    ONLogN,
    ON2,
    ON3OrWorse,
}

impl BigOClass {
    fn rank(self) -> i32 {
        match self {
            Self::O1 => 0,
            Self::OLogN => 1,
            Self::ON => 2,
            Self::ONLogN => 3,
            Self::ON2 => 4,
            Self::ON3OrWorse => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BigOShift {
    Improved,
    Unchanged,
    Regressed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub baseline_latency_ms: u64,
    pub predicted_latency_delta_ms: i64,
    pub predicted_latency_ms: u64,
    pub baseline_vram_bytes: u64,
    pub predicted_vram_delta_bytes: i64,
    pub predicted_vram_bytes: u64,
    pub baseline_cost_micro_usd: u64,
    pub predicted_cost_delta_micro_usd: i64,
    pub predicted_cost_micro_usd: u64,
    pub before_big_o: BigOClass,
    pub after_big_o: BigOClass,
    pub big_o_shift: BigOShift,
    pub before_call_site_count: u32,
    pub after_call_site_count: u32,
    pub call_site_delta: i32,
    pub before_function_count: u32,
    pub after_function_count: u32,
    pub symbol_profile_count: u32,
    pub total_observed_calls_before: u64,
    pub total_observed_calls_after: u64,
    pub hot_symbol: Option<String>,
    pub max_call_frequency_fraction: f32,
    pub runtime_vector_dim: u32,
    pub runtime_vector_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerSymbolTelemetry {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub symbol: String,
    pub static_call_sites_before: u32,
    pub static_call_sites_after: u32,
    pub observed_calls_before: u64,
    pub observed_calls_after: u64,
    pub observed_call_delta: i64,
    pub inclusive_time_ms_delta: f32,
    pub frequency_fraction_after: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum TelemetrySignalRecord {
    Label(TelemetryLabel),
    SymbolProfile(PerSymbolTelemetry),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedTelemetrySignal {
    pub schema_version: u32,
    pub signal: TelemetrySignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TelemetryExtraction {
    pub labels: Vec<TelemetryLabel>,
    pub symbols: Vec<PerSymbolTelemetry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub telemetry_json: PathBuf,
}

impl TelemetryExtraction {
    pub fn persisted_records(&self) -> Vec<PersistedTelemetrySignal> {
        self.labels
            .iter()
            .cloned()
            .map(|label| PersistedTelemetrySignal {
                schema_version: TELEMETRY_SCHEMA_VERSION,
                signal: TelemetrySignalRecord::Label(label),
            })
            .chain(
                self.symbols
                    .iter()
                    .cloned()
                    .map(|symbol| PersistedTelemetrySignal {
                        schema_version: TELEMETRY_SCHEMA_VERSION,
                        signal: TelemetrySignalRecord::SymbolProfile(symbol),
                    }),
            )
            .collect()
    }
}

impl TelemetrySignalRecord {
    fn identity_parts(&self) -> (&str, &str, &str) {
        match self {
            Self::Label(label) => (&label.corpus_row_id, &label.chunk_id, &label.path),
            Self::SymbolProfile(symbol) => {
                (&symbol.corpus_row_id, &symbol.chunk_id, &symbol.symbol)
            }
        }
    }
}

pub fn extract_telemetry_signals(
    source: &TelemetrySource,
) -> InstrumentResult<TelemetryExtraction> {
    validate_source(source)?;
    let before = analyze_source(&source.before_source)?;
    let after = analyze_source(&source.after_source)?;
    let symbols = build_symbol_profile(source, &before, &after)?;
    let label = build_label(source, &before, &after, &symbols)?;
    Ok(TelemetryExtraction {
        labels: vec![label],
        symbols,
    })
}

pub fn write_telemetry_raw_output(
    root: impl AsRef<Path>,
    row_id: &str,
    extraction: &TelemetryExtraction,
) -> InstrumentResult<TelemetryRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-telemetry-signals-v1",
            err.to_string(),
            "ensure the D-root telemetry raw-output directory is writable",
        )
    })?;
    let paths = TelemetryRawOutputPaths {
        row_id: row_id.to_string(),
        root: root.clone(),
        telemetry_json: root.join("telemetry.json"),
    };
    write_json_readback(&paths.telemetry_json, extraction)?;
    Ok(paths)
}

pub struct TelemetrySignalStore {
    db: DB,
}

impl TelemetrySignalStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_TELEMETRY_SIGNALS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_TELEMETRY_SIGNALS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        if db.cf_handle(CF_MEJEPA_TELEMETRY_SIGNALS).is_none() {
            return Err(InstrumentError::store(
                "cf_handle",
                CF_MEJEPA_TELEMETRY_SIGNALS,
                "column family missing after RocksDB open",
                "open telemetry signal storage with the canonical CF descriptor",
            ));
        }
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &TelemetryExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for record in extraction.persisted_records() {
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedTelemetrySignal)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB iterator state and telemetry CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    err.to_string(),
                    "telemetry signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    err.to_string(),
                    "only mutate telemetry rows through TelemetrySignalStore",
                )
            })?;
            rows.push((key, record));
        }
        Ok(rows)
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_TELEMETRY_SIGNALS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedTelemetrySignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_TELEMETRY_SIGNALS,
                err.to_string(),
                "ensure telemetry signal records remain JSON-serializable",
            )
        })?;
        let key = telemetry_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self
            .db
            .get_cf(self.cf()?, key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get_cf",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
            .ok_or_else(|| {
                InstrumentError::store(
                    "read_after_write",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    "telemetry row missing after put_cf",
                    "do not advance reward-signal checkpoints until the CF row is readable",
                )
            })?;
        if readback != value {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_TELEMETRY_SIGNALS,
                "readback bytes differ from written telemetry payload",
                "inspect RocksDB WAL/device integrity before continuing",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_TELEMETRY_SIGNALS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_TELEMETRY_SIGNALS,
                    "column-family handle not found",
                    "open the store through TelemetrySignalStore::open",
                )
            })
    }
}

#[derive(Debug, Default)]
struct SourceTelemetryStats {
    function_count: u32,
    call_site_count: u32,
    max_loop_depth: u32,
    sort_call_count: u32,
    comprehension_count: u32,
    recursive_call_count: u32,
    calls_by_symbol: BTreeMap<String, u32>,
}

fn analyze_source(source: &str) -> InstrumentResult<SourceTelemetryStats> {
    let parsed = ruff_python_parser::parse_module(source).map_err(|err| {
        InstrumentError::invalid(
            "source.python",
            format!("ruff-python-parser rejected source: {err}"),
            "fix Python syntax before extracting telemetry reward signals",
        )
    })?;
    let module = parsed.into_syntax();
    let mut stats = SourceTelemetryStats::default();
    collect_suite(&module.body, &mut stats, 0, None);
    Ok(stats)
}

fn collect_suite(
    suite: &[Stmt],
    stats: &mut SourceTelemetryStats,
    loop_depth: u32,
    current_function: Option<&str>,
) {
    for stmt in suite {
        collect_stmt(stmt, stats, loop_depth, current_function);
    }
}

fn collect_stmt(
    stmt: &Stmt,
    stats: &mut SourceTelemetryStats,
    loop_depth: u32,
    current_function: Option<&str>,
) {
    match stmt {
        Stmt::FunctionDef(function) => {
            stats.function_count += 1;
            let name = function.name.as_str();
            for decorator in &function.decorator_list {
                collect_expr(&decorator.expression, stats, loop_depth, current_function);
            }
            collect_suite(&function.body, stats, loop_depth, Some(name));
        }
        Stmt::ClassDef(class_def) => {
            for decorator in &class_def.decorator_list {
                collect_expr(&decorator.expression, stats, loop_depth, current_function);
            }
            if let Some(arguments) = &class_def.arguments {
                collect_arguments(arguments, stats, loop_depth, current_function);
            }
            collect_suite(&class_def.body, stats, loop_depth, current_function);
        }
        Stmt::Return(stmt_return) => {
            if let Some(value) = &stmt_return.value {
                collect_expr(value, stats, loop_depth, current_function);
            }
        }
        Stmt::Assign(assign) => {
            collect_expr(&assign.value, stats, loop_depth, current_function);
            for target in &assign.targets {
                collect_expr(target, stats, loop_depth, current_function);
            }
        }
        Stmt::AnnAssign(ann_assign) => {
            if let Some(value) = &ann_assign.value {
                collect_expr(value, stats, loop_depth, current_function);
            }
            collect_expr(&ann_assign.target, stats, loop_depth, current_function);
        }
        Stmt::AugAssign(aug_assign) => {
            collect_expr(&aug_assign.target, stats, loop_depth, current_function);
            collect_expr(&aug_assign.value, stats, loop_depth, current_function);
        }
        Stmt::For(stmt_for) => {
            let next_depth = loop_depth + 1;
            stats.max_loop_depth = stats.max_loop_depth.max(next_depth);
            collect_expr(&stmt_for.iter, stats, loop_depth, current_function);
            collect_expr(&stmt_for.target, stats, loop_depth, current_function);
            collect_suite(&stmt_for.body, stats, next_depth, current_function);
            collect_suite(&stmt_for.orelse, stats, loop_depth, current_function);
        }
        Stmt::While(stmt_while) => {
            let next_depth = loop_depth + 1;
            stats.max_loop_depth = stats.max_loop_depth.max(next_depth);
            collect_expr(&stmt_while.test, stats, loop_depth, current_function);
            collect_suite(&stmt_while.body, stats, next_depth, current_function);
            collect_suite(&stmt_while.orelse, stats, loop_depth, current_function);
        }
        Stmt::If(stmt_if) => {
            collect_expr(&stmt_if.test, stats, loop_depth, current_function);
            collect_suite(&stmt_if.body, stats, loop_depth, current_function);
            for clause in &stmt_if.elif_else_clauses {
                collect_elif_else(clause, stats, loop_depth, current_function);
            }
        }
        Stmt::With(stmt_with) => {
            for item in &stmt_with.items {
                collect_expr(&item.context_expr, stats, loop_depth, current_function);
                if let Some(optional_vars) = &item.optional_vars {
                    collect_expr(optional_vars, stats, loop_depth, current_function);
                }
            }
            collect_suite(&stmt_with.body, stats, loop_depth, current_function);
        }
        Stmt::Try(stmt_try) => {
            collect_suite(&stmt_try.body, stats, loop_depth, current_function);
            for handler in &stmt_try.handlers {
                let ruff_python_ast::ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = &handler.type_ {
                    collect_expr(type_, stats, loop_depth, current_function);
                }
                collect_suite(&handler.body, stats, loop_depth, current_function);
            }
            collect_suite(&stmt_try.orelse, stats, loop_depth, current_function);
            collect_suite(&stmt_try.finalbody, stats, loop_depth, current_function);
        }
        Stmt::Match(stmt_match) => {
            collect_expr(&stmt_match.subject, stats, loop_depth, current_function);
            for case in &stmt_match.cases {
                if let Some(guard) = &case.guard {
                    collect_expr(guard, stats, loop_depth, current_function);
                }
                collect_suite(&case.body, stats, loop_depth, current_function);
            }
        }
        Stmt::Expr(stmt_expr) => {
            collect_expr(&stmt_expr.value, stats, loop_depth, current_function)
        }
        Stmt::Delete(stmt_delete) => {
            for target in &stmt_delete.targets {
                collect_expr(target, stats, loop_depth, current_function);
            }
        }
        Stmt::Assert(stmt_assert) => {
            collect_expr(&stmt_assert.test, stats, loop_depth, current_function);
            if let Some(msg) = &stmt_assert.msg {
                collect_expr(msg, stats, loop_depth, current_function);
            }
        }
        Stmt::Raise(stmt_raise) => {
            if let Some(exc) = &stmt_raise.exc {
                collect_expr(exc, stats, loop_depth, current_function);
            }
            if let Some(cause) = &stmt_raise.cause {
                collect_expr(cause, stats, loop_depth, current_function);
            }
        }
        Stmt::TypeAlias(type_alias) => {
            collect_expr(&type_alias.value, stats, loop_depth, current_function)
        }
        Stmt::Import(_)
        | Stmt::ImportFrom(_)
        | Stmt::Global(_)
        | Stmt::Nonlocal(_)
        | Stmt::Pass(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::IpyEscapeCommand(_) => {}
    }
}

fn collect_expr(
    expr: &Expr,
    stats: &mut SourceTelemetryStats,
    loop_depth: u32,
    current_function: Option<&str>,
) {
    match expr {
        Expr::Call(call) => {
            stats.call_site_count += 1;
            if let Some(target) = call_target_name(&call.func) {
                *stats.calls_by_symbol.entry(target.clone()).or_default() += 1;
                if target == "sorted" || target.ends_with(".sort") {
                    stats.sort_call_count += 1;
                }
                if current_function.is_some_and(|name| target.rsplit('.').next() == Some(name)) {
                    stats.recursive_call_count += 1;
                }
            }
            collect_expr(&call.func, stats, loop_depth, current_function);
            collect_arguments(&call.arguments, stats, loop_depth, current_function);
        }
        Expr::BoolOp(bool_op) => {
            for value in &bool_op.values {
                collect_expr(value, stats, loop_depth, current_function);
            }
        }
        Expr::Named(named) => {
            collect_expr(&named.target, stats, loop_depth, current_function);
            collect_expr(&named.value, stats, loop_depth, current_function);
        }
        Expr::BinOp(bin_op) => {
            collect_expr(&bin_op.left, stats, loop_depth, current_function);
            collect_expr(&bin_op.right, stats, loop_depth, current_function);
        }
        Expr::UnaryOp(unary_op) => {
            collect_expr(&unary_op.operand, stats, loop_depth, current_function)
        }
        Expr::Lambda(lambda) => collect_expr(&lambda.body, stats, loop_depth, current_function),
        Expr::If(expr_if) => {
            collect_expr(&expr_if.test, stats, loop_depth, current_function);
            collect_expr(&expr_if.body, stats, loop_depth, current_function);
            collect_expr(&expr_if.orelse, stats, loop_depth, current_function);
        }
        Expr::Dict(dict) => {
            for item in &dict.items {
                if let Some(key) = &item.key {
                    collect_expr(key, stats, loop_depth, current_function);
                }
                collect_expr(&item.value, stats, loop_depth, current_function);
            }
        }
        Expr::Set(set) => {
            for elt in &set.elts {
                collect_expr(elt, stats, loop_depth, current_function);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_expr(elt, stats, loop_depth, current_function);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_expr(elt, stats, loop_depth, current_function);
            }
        }
        Expr::ListComp(comp) => {
            collect_comprehensions(&comp.generators, stats, loop_depth, current_function);
            collect_expr(&comp.elt, stats, loop_depth, current_function);
        }
        Expr::SetComp(comp) => {
            collect_comprehensions(&comp.generators, stats, loop_depth, current_function);
            collect_expr(&comp.elt, stats, loop_depth, current_function);
        }
        Expr::DictComp(comp) => {
            collect_comprehensions(&comp.generators, stats, loop_depth, current_function);
            collect_expr(&comp.key, stats, loop_depth, current_function);
            collect_expr(&comp.value, stats, loop_depth, current_function);
        }
        Expr::Generator(generator) => {
            collect_comprehensions(&generator.generators, stats, loop_depth, current_function);
            collect_expr(&generator.elt, stats, loop_depth, current_function);
        }
        Expr::Await(await_expr) => {
            collect_expr(&await_expr.value, stats, loop_depth, current_function)
        }
        Expr::Yield(yield_expr) => {
            if let Some(value) = &yield_expr.value {
                collect_expr(value, stats, loop_depth, current_function);
            }
        }
        Expr::YieldFrom(yield_from) => {
            collect_expr(&yield_from.value, stats, loop_depth, current_function)
        }
        Expr::Compare(compare) => {
            collect_expr(&compare.left, stats, loop_depth, current_function);
            for comparator in &compare.comparators {
                collect_expr(comparator, stats, loop_depth, current_function);
            }
        }
        Expr::Attribute(attribute) => {
            collect_expr(&attribute.value, stats, loop_depth, current_function)
        }
        Expr::Subscript(subscript) => {
            collect_expr(&subscript.value, stats, loop_depth, current_function);
            collect_expr(&subscript.slice, stats, loop_depth, current_function);
        }
        Expr::Starred(starred) => collect_expr(&starred.value, stats, loop_depth, current_function),
        Expr::Slice(slice) => {
            if let Some(lower) = &slice.lower {
                collect_expr(lower, stats, loop_depth, current_function);
            }
            if let Some(upper) = &slice.upper {
                collect_expr(upper, stats, loop_depth, current_function);
            }
            if let Some(step) = &slice.step {
                collect_expr(step, stats, loop_depth, current_function);
            }
        }
        Expr::Name(_)
        | Expr::FString(_)
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

fn collect_arguments(
    arguments: &Arguments,
    stats: &mut SourceTelemetryStats,
    loop_depth: u32,
    current_function: Option<&str>,
) {
    for arg in &arguments.args {
        collect_expr(arg, stats, loop_depth, current_function);
    }
    for keyword in &arguments.keywords {
        collect_expr(&keyword.value, stats, loop_depth, current_function);
    }
}

fn collect_comprehensions(
    comprehensions: &[Comprehension],
    stats: &mut SourceTelemetryStats,
    loop_depth: u32,
    current_function: Option<&str>,
) {
    stats.comprehension_count += comprehensions.len() as u32;
    let depth = loop_depth + comprehensions.len() as u32;
    stats.max_loop_depth = stats.max_loop_depth.max(depth);
    for comprehension in comprehensions {
        collect_expr(&comprehension.iter, stats, loop_depth, current_function);
        collect_expr(&comprehension.target, stats, loop_depth, current_function);
        for if_expr in &comprehension.ifs {
            collect_expr(if_expr, stats, loop_depth, current_function);
        }
    }
}

fn collect_elif_else(
    clause: &ElifElseClause,
    stats: &mut SourceTelemetryStats,
    loop_depth: u32,
    current_function: Option<&str>,
) {
    if let Some(test) = &clause.test {
        collect_expr(test, stats, loop_depth, current_function);
    }
    collect_suite(&clause.body, stats, loop_depth, current_function);
}

fn call_target_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str().to_string()),
        Expr::Attribute(attribute) => call_target_name(&attribute.value)
            .map(|prefix| format!("{prefix}.{}", attribute.attr.as_str()))
            .or_else(|| Some(attribute.attr.as_str().to_string())),
        _ => None,
    }
}

fn build_symbol_profile(
    source: &TelemetrySource,
    before: &SourceTelemetryStats,
    after: &SourceTelemetryStats,
) -> InstrumentResult<Vec<PerSymbolTelemetry>> {
    let mut observations = BTreeMap::new();
    for observation in &source.symbol_observations {
        observations.insert(observation.symbol.clone(), observation);
    }
    let mut symbols = BTreeSet::new();
    symbols.extend(before.calls_by_symbol.keys().cloned());
    symbols.extend(after.calls_by_symbol.keys().cloned());
    symbols.extend(observations.keys().cloned());
    let total_after: u64 = source
        .symbol_observations
        .iter()
        .map(|observation| observation.calls_after)
        .sum();
    let mut rows = Vec::new();
    for symbol in symbols {
        let observation = observations.get(&symbol);
        let calls_before = observation.map_or(0, |value| value.calls_before);
        let calls_after = observation.map_or(0, |value| value.calls_after);
        let time_before = observation.map_or(0.0, |value| value.inclusive_time_ms_before);
        let time_after = observation.map_or(0.0, |value| value.inclusive_time_ms_after);
        rows.push(PerSymbolTelemetry {
            corpus_row_id: source.corpus_row_id.clone(),
            chunk_id: source.chunk_id.clone(),
            path: source.path.clone(),
            static_call_sites_before: before.calls_by_symbol.get(&symbol).copied().unwrap_or(0),
            static_call_sites_after: after.calls_by_symbol.get(&symbol).copied().unwrap_or(0),
            observed_call_delta: i64_delta(calls_before, calls_after)?,
            inclusive_time_ms_delta: time_after - time_before,
            frequency_fraction_after: if total_after == 0 {
                0.0
            } else {
                calls_after as f32 / total_after as f32
            },
            symbol,
            observed_calls_before: calls_before,
            observed_calls_after: calls_after,
        });
    }
    Ok(rows)
}

fn build_label(
    source: &TelemetrySource,
    before: &SourceTelemetryStats,
    after: &SourceTelemetryStats,
    symbols: &[PerSymbolTelemetry],
) -> InstrumentResult<TelemetryLabel> {
    let before_big_o = infer_big_o(before);
    let after_big_o = infer_big_o(after);
    let big_o_shift = match after_big_o.rank().cmp(&before_big_o.rank()) {
        std::cmp::Ordering::Less => BigOShift::Improved,
        std::cmp::Ordering::Equal => BigOShift::Unchanged,
        std::cmp::Ordering::Greater => BigOShift::Regressed,
    };
    let total_before: u64 = symbols
        .iter()
        .map(|symbol| symbol.observed_calls_before)
        .sum();
    let total_after: u64 = symbols
        .iter()
        .map(|symbol| symbol.observed_calls_after)
        .sum();
    let profile_time_delta: f32 = symbols
        .iter()
        .map(|symbol| symbol.inclusive_time_ms_delta)
        .sum();
    let call_site_delta = u32_delta(before.call_site_count, after.call_site_count)?;
    let function_delta = u32_delta(before.function_count, after.function_count)?;
    let big_o_delta = after_big_o.rank() - before_big_o.rank();
    let latency_delta = predict_latency_delta(
        source,
        call_site_delta,
        function_delta,
        big_o_delta,
        profile_time_delta,
    );
    let vram_delta = predict_vram_delta(source, call_site_delta, function_delta, big_o_delta)?;
    let cost_delta = predict_cost_delta(latency_delta, vram_delta, total_before, total_after)?;
    let predicted_latency_ms = add_signed_floor_one(source.baseline_latency_ms, latency_delta);
    let predicted_vram_bytes = add_signed_floor_one(source.baseline_vram_bytes, vram_delta);
    let predicted_cost_micro_usd =
        add_signed_floor_zero(source.baseline_cost_micro_usd, cost_delta);
    let runtime_input = RuntimeInput {
        wall_time_ms: predicted_latency_ms,
        peak_rss_bytes: predicted_vram_bytes,
        exit_code: 0,
        timed_out: false,
        coverage_percent: None,
        network_events: 0,
        filesystem_writes: after.call_site_count,
    };
    let vector = ERuntimeInstrument.encode(&runtime_input)?;
    let hot_symbol = symbols
        .iter()
        .max_by_key(|symbol| symbol.observed_calls_after)
        .filter(|symbol| symbol.observed_calls_after > 0)
        .map(|symbol| symbol.symbol.clone());
    let max_fraction = symbols
        .iter()
        .map(|symbol| symbol.frequency_fraction_after)
        .fold(0.0_f32, f32::max);
    Ok(TelemetryLabel {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        path: source.path.clone(),
        baseline_latency_ms: source.baseline_latency_ms,
        predicted_latency_delta_ms: latency_delta,
        predicted_latency_ms,
        baseline_vram_bytes: source.baseline_vram_bytes,
        predicted_vram_delta_bytes: vram_delta,
        predicted_vram_bytes,
        baseline_cost_micro_usd: source.baseline_cost_micro_usd,
        predicted_cost_delta_micro_usd: cost_delta,
        predicted_cost_micro_usd,
        before_big_o,
        after_big_o,
        big_o_shift,
        before_call_site_count: before.call_site_count,
        after_call_site_count: after.call_site_count,
        call_site_delta,
        before_function_count: before.function_count,
        after_function_count: after.function_count,
        symbol_profile_count: to_u32("symbol_profile_count", symbols.len())?,
        total_observed_calls_before: total_before,
        total_observed_calls_after: total_after,
        hot_symbol,
        max_call_frequency_fraction: max_fraction,
        runtime_vector_dim: to_u32("runtime_vector_dim", vector.len())?,
        runtime_vector_sha256: sha256_f32_vector(&vector),
    })
}

fn infer_big_o(stats: &SourceTelemetryStats) -> BigOClass {
    if stats.max_loop_depth >= 3 || stats.recursive_call_count > 0 {
        BigOClass::ON3OrWorse
    } else if stats.max_loop_depth >= 2 {
        BigOClass::ON2
    } else if stats.sort_call_count > 0 {
        BigOClass::ONLogN
    } else if stats.max_loop_depth == 1 || stats.comprehension_count > 0 {
        BigOClass::ON
    } else {
        BigOClass::O1
    }
}

fn predict_latency_delta(
    source: &TelemetrySource,
    call_site_delta: i32,
    function_delta: i32,
    big_o_delta: i32,
    profile_time_delta: f32,
) -> i64 {
    let source_bytes_delta = source.after_source.len() as i64 - source.before_source.len() as i64;
    let estimate = (source_bytes_delta as f32 * 0.002)
        + (call_site_delta as f32 * 0.75)
        + (function_delta as f32 * 1.5)
        + (big_o_delta as f32 * source.baseline_latency_ms as f32 * 0.20)
        + (profile_time_delta * 0.25);
    estimate.round() as i64
}

fn predict_vram_delta(
    source: &TelemetrySource,
    call_site_delta: i32,
    function_delta: i32,
    big_o_delta: i32,
) -> InstrumentResult<i64> {
    let source_bytes_delta = i64::try_from(source.after_source.len())
        .and_then(|after| i64::try_from(source.before_source.len()).map(|before| after - before))
        .map_err(|_| {
            InstrumentError::invalid(
                "source.length",
                "source length delta exceeds i64",
                "shard telemetry extraction before source-length overflow",
            )
        })?;
    Ok(source_bytes_delta
        .saturating_mul(64)
        .saturating_add((call_site_delta as i64).saturating_mul(2_048))
        .saturating_add((function_delta as i64).saturating_mul(8_192))
        .saturating_add((big_o_delta as i64).saturating_mul(1_048_576)))
}

fn predict_cost_delta(
    latency_delta: i64,
    vram_delta: i64,
    calls_before: u64,
    calls_after: u64,
) -> InstrumentResult<i64> {
    let call_delta = i64_delta(calls_before, calls_after)?;
    let vram_mib_delta = vram_delta as f32 / (1024.0 * 1024.0);
    let estimate =
        (latency_delta as f32 * 0.30) + (vram_mib_delta * 0.20) + (call_delta as f32 * 0.02);
    Ok(estimate.round() as i64)
}

fn validate_source(source: &TelemetrySource) -> InstrumentResult<()> {
    validate_single_line("source.corpus_row_id", &source.corpus_row_id)?;
    validate_single_line("source.chunk_id", &source.chunk_id)?;
    validate_single_line("source.language", &source.language)?;
    if !source.language.eq_ignore_ascii_case("python") {
        return invalid(
            "source.language",
            format!(
                "E_Telemetry currently supports Python source, got {}",
                source.language
            ),
            "route Python runtime-telemetry extraction through this instrument",
        );
    }
    validate_no_nul("source.path", &source.path)?;
    if source.path.trim().is_empty() {
        return invalid(
            "source.path",
            "telemetry source path is empty",
            "persist the analyzed Python file path with every telemetry label",
        );
    }
    validate_source_text("source.before_source", &source.before_source)?;
    validate_source_text("source.after_source", &source.after_source)?;
    if source.baseline_latency_ms == 0 {
        return invalid(
            "source.baseline_latency_ms",
            "baseline latency must be positive",
            "capture a non-zero baseline runtime before predicting latency delta",
        );
    }
    if source.baseline_vram_bytes == 0 {
        return invalid(
            "source.baseline_vram_bytes",
            "baseline VRAM must be positive",
            "capture a non-zero baseline memory footprint before predicting VRAM delta",
        );
    }
    if source.symbol_observations.len() > MAX_SYMBOL_OBSERVATIONS {
        return invalid(
            "source.symbol_observations",
            format!(
                "symbol observation count {} exceeds max {MAX_SYMBOL_OBSERVATIONS}",
                source.symbol_observations.len()
            ),
            "aggregate or shard extremely large profiler call-frequency outputs",
        );
    }
    let mut seen = BTreeSet::new();
    for (idx, observation) in source.symbol_observations.iter().enumerate() {
        validate_single_line("source.symbol_observations[i].symbol", &observation.symbol)?;
        if !seen.insert(observation.symbol.clone()) {
            return invalid(
                "source.symbol_observations[i].symbol",
                format!(
                    "duplicate symbol observation at index {idx}: {}",
                    observation.symbol
                ),
                "merge profiler observations by symbol before telemetry extraction",
            );
        }
        validate_finite_nonnegative(
            "source.symbol_observations[i].inclusive_time_ms_before",
            observation.inclusive_time_ms_before,
        )?;
        validate_finite_nonnegative(
            "source.symbol_observations[i].inclusive_time_ms_after",
            observation.inclusive_time_ms_after,
        )?;
    }
    Ok(())
}

fn validate_source_text(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(
            field,
            "telemetry source text is empty",
            "capture parser-facing source before telemetry signal extraction",
        );
    }
    if value.len() > MAX_SOURCE_BYTES {
        return invalid(
            field,
            format!(
                "source text is {} bytes, exceeding max {MAX_SOURCE_BYTES}",
                value.len()
            ),
            "shard very large Python files before telemetry signal extraction",
        );
    }
    validate_no_nul(field, value)
}

fn telemetry_record_key(record: &PersistedTelemetrySignal, value: &[u8]) -> String {
    let (row_id, chunk_id, discriminator) = record.signal.identity_parts();
    format!(
        "telemetry::{row_id}::{chunk_id}::{discriminator}::{:016x}",
        fnv1a64(value)
    )
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_single_line(field, value)?;
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return invalid(
            field,
            format!("{field} must be a single safe path component, got {value:?}"),
            "use a stable row id without path separators for raw-output directories",
        );
    }
    Ok(())
}

fn validate_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_no_nul(field, value)?;
    if value.trim().is_empty() || value.contains('\n') || value.contains('\r') {
        return invalid(
            field,
            format!("{field} must be non-empty single-line text"),
            "persist canonical single-line identifiers in telemetry rows",
        );
    }
    Ok(())
}

fn validate_no_nul(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.chars().any(|ch| ch == '\0') {
        return invalid(
            field,
            format!("{field} contains a NUL byte"),
            "reject analyzer text with embedded NUL bytes before persistence",
        );
    }
    Ok(())
}

fn validate_finite_nonnegative(field: &'static str, value: f32) -> InstrumentResult<()> {
    if !value.is_finite() || value < 0.0 {
        return invalid(
            field,
            format!("{field} must be finite and non-negative, got {value}"),
            "persist profiler durations as finite milliseconds",
        );
    }
    Ok(())
}

fn u32_delta(before: u32, after: u32) -> InstrumentResult<i32> {
    i32::try_from(after as i64 - before as i64).map_err(|_| {
        InstrumentError::invalid(
            "delta.u32",
            "u32 delta exceeds i32",
            "shard telemetry extraction before delta overflow",
        )
    })
}

fn i64_delta(before: u64, after: u64) -> InstrumentResult<i64> {
    i64::try_from(after as i128 - before as i128).map_err(|_| {
        InstrumentError::invalid(
            "delta.u64",
            "u64 delta exceeds i64",
            "shard telemetry extraction before counter-delta overflow",
        )
    })
}

fn add_signed_floor_one(base: u64, delta: i64) -> u64 {
    let value = base as i128 + delta as i128;
    value.max(1) as u64
}

fn add_signed_floor_zero(base: u64, delta: i64) -> u64 {
    let value = base as i128 + delta as i128;
    value.max(0) as u64
}

fn to_u32(field: &'static str, value: usize) -> InstrumentResult<u32> {
    u32::try_from(value).map_err(|_| {
        InstrumentError::invalid(
            field,
            format!("{field} value {value} exceeds u32"),
            "shard telemetry extraction before counter overflow",
        )
    })
}

fn sha256_f32_vector(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn write_json_readback<T: Serialize>(path: &Path, value: &T) -> InstrumentResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| {
        InstrumentError::store(
            "serialize",
            "python-telemetry-signals-v1",
            err.to_string(),
            "ensure telemetry raw output remains JSON-serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write",
            "python-telemetry-signals-v1",
            err.to_string(),
            "ensure the raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_after_write",
            "python-telemetry-signals-v1",
            err.to_string(),
            "verify telemetry raw-output bytes after writing",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write",
            "python-telemetry-signals-v1",
            "raw-output readback bytes differ from written payload",
            "do not advance reward-signal checkpoints until raw bytes match",
        ));
    }
    Ok(())
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_extracts_big_o_and_symbol_profile() {
        let extraction = extract_telemetry_signals(&sample_source()).unwrap();
        let label = extraction.labels.first().unwrap();
        assert_eq!(label.before_big_o, BigOClass::ON);
        assert_eq!(label.after_big_o, BigOClass::ON2);
        assert_eq!(label.big_o_shift, BigOShift::Regressed);
        assert!(label.predicted_latency_delta_ms > 0);
        assert!(label.predicted_vram_delta_bytes > 0);
        assert!(label.predicted_cost_delta_micro_usd > 0);
        assert!(label.runtime_vector_sha256.starts_with("sha256:"));
        assert!(extraction
            .symbols
            .iter()
            .any(|symbol| symbol.symbol == "score_pair" && symbol.observed_calls_after == 900));
    }

    #[test]
    fn telemetry_rejects_invalid_baseline() {
        let mut source = sample_source();
        source.baseline_latency_ms = 0;
        let err = extract_telemetry_signals(&source).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }

    fn sample_source() -> TelemetrySource {
        TelemetrySource {
            corpus_row_id: "row-telemetry".to_string(),
            chunk_id: "chunk-telemetry".to_string(),
            language: "python".to_string(),
            path: "pkg/telemetry.py".to_string(),
            before_source: "def score(values):\n    total = 0\n    for value in values:\n        total += normalize(value)\n    return total\n".to_string(),
            after_source: "def score(values):\n    total = 0\n    ordered = sorted(values)\n    for left in ordered:\n        for right in ordered:\n            total += score_pair(left, right)\n    return total\n".to_string(),
            baseline_latency_ms: 120,
            baseline_vram_bytes: 512 * 1024 * 1024,
            baseline_cost_micro_usd: 900,
            symbol_observations: vec![
                SymbolCallObservation {
                    symbol: "normalize".to_string(),
                    calls_before: 100,
                    calls_after: 0,
                    inclusive_time_ms_before: 15.0,
                    inclusive_time_ms_after: 0.0,
                },
                SymbolCallObservation {
                    symbol: "score_pair".to_string(),
                    calls_before: 0,
                    calls_after: 900,
                    inclusive_time_ms_before: 0.0,
                    inclusive_time_ms_after: 260.0,
                },
            ],
        }
    }
}
