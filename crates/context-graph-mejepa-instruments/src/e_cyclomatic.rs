// TASK-RWD-201: deterministic Python static-quality complexity signals.
// This additive instrument feeds the existing E_StaticAnalysis panel slot.

use ruff_python_ast::{Arguments, Comprehension, ElifElseClause, ExceptHandler, Expr, Stmt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::features::{bounded_ratio, fnv1a64, validate_finite_output};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_SOURCE_BYTES: usize = 5_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComplexityInput {
    pub language: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionComplexity {
    pub qualified_name: String,
    pub cyclomatic_complexity: u32,
    pub cognitive_complexity: u32,
    pub max_nesting_depth: u32,
    pub halstead_operators: u32,
    pub halstead_operands: u32,
    pub halstead_unique_operators: u32,
    pub halstead_unique_operands: u32,
    pub halstead_volume: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComplexityMetrics {
    pub analyzer: String,
    pub language: String,
    pub path: String,
    pub function_count: u32,
    pub max_cyclomatic_complexity: u32,
    pub mean_cyclomatic_complexity: f32,
    pub total_cyclomatic_complexity: u32,
    pub max_cognitive_complexity: u32,
    pub total_cognitive_complexity: u32,
    pub max_nesting_depth: u32,
    pub halstead_operators: u32,
    pub halstead_operands: u32,
    pub halstead_unique_operators: u32,
    pub halstead_unique_operands: u32,
    pub halstead_volume: f32,
    pub functions: Vec<FunctionComplexity>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ECyclomaticInstrument;

impl Instrument for ECyclomaticInstrument {
    type Input = ComplexityInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EStaticAnalysis
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let metrics = analyze_complexity(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::EStaticAnalysis.dim()];
        out[0] = bounded_ratio(metrics.max_cyclomatic_complexity as f32, 100.0);
        out[1] = bounded_ratio(metrics.mean_cyclomatic_complexity, 100.0);
        out[2] = bounded_ratio(metrics.total_cyclomatic_complexity as f32, 10_000.0);
        out[3] = bounded_ratio(metrics.max_cognitive_complexity as f32, 1_000.0);
        out[4] = bounded_ratio(metrics.total_cognitive_complexity as f32, 100_000.0);
        out[5] = bounded_ratio(metrics.max_nesting_depth as f32, 100.0);
        out[6] = bounded_ratio(metrics.function_count as f32, 10_000.0);
        out[7] = bounded_ratio(metrics.halstead_volume, 1_000_000.0);
        out[8] = bounded_ratio(metrics.halstead_operators as f32, 1_000_000.0);
        out[9] = bounded_ratio(metrics.halstead_operands as f32, 1_000_000.0);
        out[10] = bounded_ratio(metrics.halstead_unique_operators as f32, 10_000.0);
        out[11] = bounded_ratio(metrics.halstead_unique_operands as f32, 100_000.0);

        for function in &metrics.functions {
            let weight = bounded_ratio(function.cyclomatic_complexity as f32, 100.0);
            let idx = 32 + (fnv1a64(function.qualified_name.as_bytes()) as usize % 64);
            out[idx] += weight;
        }

        for (idx, value) in out.iter_mut().enumerate().skip(128) {
            let seed = (idx as u64)
                ^ ((metrics.total_cyclomatic_complexity as u64) << 11)
                ^ ((metrics.total_cognitive_complexity as u64) << 23)
                ^ ((metrics.halstead_operators as u64) << 37)
                ^ fnv1a64(metrics.path.as_bytes());
            *value = (seed.rotate_left((idx % 61) as u32) as f64 / u64::MAX as f64) as f32;
        }

        validate_finite_output("e_cyclomatic.output", &out)?;
        Ok(out)
    }
}

pub fn analyze_complexity(input: &ComplexityInput) -> InstrumentResult<ComplexityMetrics> {
    validate_input(input)?;
    let parsed = ruff_python_parser::parse_module(&input.source).map_err(|err| {
        InstrumentError::invalid(
            "input.source",
            format!("ruff-python-parser rejected source: {err}"),
            "fix Python syntax before computing TASK-RWD-201 complexity signals",
        )
    })?;
    let module = parsed.into_syntax();
    let mut collector = ComplexityCollector::default();
    collector.collect_suite("module", &module.body, 0);
    let mut functions = collector.functions;
    if functions.is_empty() {
        let mut module_acc = ScopeAccumulator::new("module");
        analyze_suite(&module.body, &mut module_acc, 0);
        functions.push(module_acc.finish());
    }
    build_metrics(input, functions)
}

fn validate_input(input: &ComplexityInput) -> InstrumentResult<()> {
    if !input.language.trim().eq_ignore_ascii_case("python") {
        return Err(InstrumentError::invalid(
            "input.language",
            format!(
                "E_Cyclomatic currently supports python only, got {}",
                input.language
            ),
            "route non-Python complexity through a language-specific TASK-RWD instrument",
        ));
    }
    if input.path.trim().is_empty() {
        return Err(InstrumentError::invalid(
            "input.path",
            "complexity input path is empty",
            "persist the analyzed file path with each complexity signal",
        ));
    }
    if input.source.trim().is_empty() {
        return Err(InstrumentError::invalid(
            "input.source",
            "complexity input source is empty",
            "capture source text before computing complexity signals",
        ));
    }
    if input.source.len() > MAX_SOURCE_BYTES {
        return Err(InstrumentError::invalid(
            "input.source",
            format!(
                "source length {} exceeds max {} bytes",
                input.source.len(),
                MAX_SOURCE_BYTES
            ),
            "shard very large files before computing TASK-RWD-201 signals",
        ));
    }
    if input.source.chars().any(|ch| ch == '\0') {
        return Err(InstrumentError::invalid(
            "input.source",
            "source contains a NUL byte",
            "store parser-facing UTF-8 source without embedded NUL bytes",
        ));
    }
    Ok(())
}

fn build_metrics(
    input: &ComplexityInput,
    functions: Vec<FunctionComplexity>,
) -> InstrumentResult<ComplexityMetrics> {
    let function_count = functions.len() as u32;
    let total_cyclomatic_complexity = functions
        .iter()
        .map(|function| function.cyclomatic_complexity)
        .sum::<u32>();
    let total_cognitive_complexity = functions
        .iter()
        .map(|function| function.cognitive_complexity)
        .sum::<u32>();
    let max_cyclomatic_complexity = functions
        .iter()
        .map(|function| function.cyclomatic_complexity)
        .max()
        .unwrap_or(0);
    let max_cognitive_complexity = functions
        .iter()
        .map(|function| function.cognitive_complexity)
        .max()
        .unwrap_or(0);
    let max_nesting_depth = functions
        .iter()
        .map(|function| function.max_nesting_depth)
        .max()
        .unwrap_or(0);
    let halstead_operators = functions
        .iter()
        .map(|function| function.halstead_operators)
        .sum::<u32>();
    let halstead_operands = functions
        .iter()
        .map(|function| function.halstead_operands)
        .sum::<u32>();
    let halstead_unique_operators = functions
        .iter()
        .map(|function| function.halstead_unique_operators)
        .sum::<u32>();
    let halstead_unique_operands = functions
        .iter()
        .map(|function| function.halstead_unique_operands)
        .sum::<u32>();
    let halstead_volume = functions
        .iter()
        .map(|function| function.halstead_volume)
        .sum::<f32>();
    let mean_cyclomatic_complexity = if function_count == 0 {
        0.0
    } else {
        total_cyclomatic_complexity as f32 / function_count as f32
    };
    let metrics = ComplexityMetrics {
        analyzer: "contextgraph_python_complexity_v1".to_string(),
        language: input.language.clone(),
        path: input.path.clone(),
        function_count,
        max_cyclomatic_complexity,
        mean_cyclomatic_complexity,
        total_cyclomatic_complexity,
        max_cognitive_complexity,
        total_cognitive_complexity,
        max_nesting_depth,
        halstead_operators,
        halstead_operands,
        halstead_unique_operators,
        halstead_unique_operands,
        halstead_volume,
        functions,
    };
    validate_metrics(&metrics)?;
    Ok(metrics)
}

fn validate_metrics(metrics: &ComplexityMetrics) -> InstrumentResult<()> {
    for value in [metrics.mean_cyclomatic_complexity, metrics.halstead_volume] {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "complexity.metrics",
                format!("metric is non-finite: {value}"),
                "inspect TASK-RWD-201 metric aggregation",
            ));
        }
    }
    for function in &metrics.functions {
        if !function.halstead_volume.is_finite() {
            return Err(InstrumentError::invariant(
                "complexity.functions.halstead_volume",
                format!(
                    "{} has non-finite Halstead volume {}",
                    function.qualified_name, function.halstead_volume
                ),
                "inspect TASK-RWD-201 Halstead aggregation",
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct ComplexityCollector {
    functions: Vec<FunctionComplexity>,
}

impl ComplexityCollector {
    fn collect_suite(&mut self, prefix: &str, suite: &[Stmt], nesting: u32) {
        for stmt in suite {
            match stmt {
                Stmt::FunctionDef(function) => {
                    let qualified = qualify(prefix, function.name.as_str());
                    let mut acc = ScopeAccumulator::new(&qualified);
                    add_parameters(&function.parameters, &mut acc);
                    analyze_suite(&function.body, &mut acc, nesting);
                    self.functions.push(acc.finish());
                    self.collect_suite(&qualified, &function.body, nesting + 1);
                }
                Stmt::ClassDef(class_def) => {
                    let qualified = qualify(prefix, class_def.name.as_str());
                    self.collect_suite(&qualified, &class_def.body, nesting + 1);
                }
                _ => {}
            }
        }
    }
}

fn qualify(prefix: &str, name: &str) -> String {
    if prefix == "module" {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

#[derive(Debug)]
struct ScopeAccumulator {
    qualified_name: String,
    cyclomatic_decisions: u32,
    cognitive_complexity: u32,
    max_nesting_depth: u32,
    operators: u32,
    operands: u32,
    unique_operators: BTreeSet<String>,
    unique_operands: BTreeSet<String>,
}

impl ScopeAccumulator {
    fn new(qualified_name: &str) -> Self {
        let mut acc = Self {
            qualified_name: qualified_name.to_string(),
            cyclomatic_decisions: 0,
            cognitive_complexity: 0,
            max_nesting_depth: 0,
            operators: 0,
            operands: 0,
            unique_operators: BTreeSet::new(),
            unique_operands: BTreeSet::new(),
        };
        acc.operator("scope");
        acc.operand(qualified_name);
        acc
    }

    fn decision(&mut self, operator: &str, count: u32, nesting: u32) {
        self.cyclomatic_decisions = self.cyclomatic_decisions.saturating_add(count);
        self.cognitive_complexity = self
            .cognitive_complexity
            .saturating_add(count.saturating_mul(1 + nesting));
        self.operator(operator);
    }

    fn operator(&mut self, operator: &str) {
        self.operators = self.operators.saturating_add(1);
        self.unique_operators.insert(operator.to_string());
    }

    fn operand(&mut self, operand: &str) {
        self.operands = self.operands.saturating_add(1);
        self.unique_operands.insert(operand.to_string());
    }

    fn finish(self) -> FunctionComplexity {
        let halstead_unique_operators = self.unique_operators.len() as u32;
        let halstead_unique_operands = self.unique_operands.len() as u32;
        let halstead_operators = self.operators;
        let halstead_operands = self.operands;
        let vocabulary = halstead_unique_operators + halstead_unique_operands;
        let length = halstead_operators + halstead_operands;
        let halstead_volume = if vocabulary <= 1 || length == 0 {
            0.0
        } else {
            length as f32 * (vocabulary as f32).log2()
        };
        FunctionComplexity {
            qualified_name: self.qualified_name,
            cyclomatic_complexity: 1 + self.cyclomatic_decisions,
            cognitive_complexity: self.cognitive_complexity,
            max_nesting_depth: self.max_nesting_depth,
            halstead_operators,
            halstead_operands,
            halstead_unique_operators,
            halstead_unique_operands,
            halstead_volume,
        }
    }
}

fn analyze_suite(suite: &[Stmt], acc: &mut ScopeAccumulator, nesting: u32) {
    acc.max_nesting_depth = acc.max_nesting_depth.max(nesting);
    for stmt in suite {
        analyze_stmt(stmt, acc, nesting);
    }
}

fn analyze_stmt(stmt: &Stmt, acc: &mut ScopeAccumulator, nesting: u32) {
    match stmt {
        Stmt::FunctionDef(function) => {
            acc.operator("function_def");
            acc.operand(function.name.as_str());
            add_parameters(&function.parameters, acc);
            for decorator in &function.decorator_list {
                analyze_expr(&decorator.expression, acc, nesting + 1);
            }
            if let Some(returns) = &function.returns {
                analyze_expr(returns, acc, nesting + 1);
            }
        }
        Stmt::ClassDef(class_def) => {
            acc.operator("class_def");
            acc.operand(class_def.name.as_str());
            for decorator in &class_def.decorator_list {
                analyze_expr(&decorator.expression, acc, nesting + 1);
            }
            if let Some(arguments) = &class_def.arguments {
                analyze_arguments(arguments, acc, nesting + 1);
            }
        }
        Stmt::Return(stmt_return) => {
            acc.operator("return");
            if let Some(value) = &stmt_return.value {
                analyze_expr(value, acc, nesting + 1);
            }
        }
        Stmt::Delete(stmt_delete) => {
            acc.operator("delete");
            for target in &stmt_delete.targets {
                analyze_expr(target, acc, nesting + 1);
            }
        }
        Stmt::TypeAlias(type_alias) => {
            acc.operator("type_alias");
            analyze_expr(&type_alias.name, acc, nesting + 1);
            analyze_expr(&type_alias.value, acc, nesting + 1);
        }
        Stmt::Assign(assign) => {
            acc.operator("assign");
            analyze_expr(&assign.value, acc, nesting + 1);
            for target in &assign.targets {
                analyze_expr(target, acc, nesting + 1);
            }
        }
        Stmt::AugAssign(aug_assign) => {
            acc.operator("aug_assign");
            analyze_expr(&aug_assign.target, acc, nesting + 1);
            analyze_expr(&aug_assign.value, acc, nesting + 1);
        }
        Stmt::AnnAssign(ann_assign) => {
            acc.operator("ann_assign");
            analyze_expr(&ann_assign.annotation, acc, nesting + 1);
            if let Some(value) = &ann_assign.value {
                analyze_expr(value, acc, nesting + 1);
            }
            analyze_expr(&ann_assign.target, acc, nesting + 1);
        }
        Stmt::For(stmt_for) => {
            acc.decision("for", 1, nesting);
            analyze_expr(&stmt_for.iter, acc, nesting + 1);
            analyze_expr(&stmt_for.target, acc, nesting + 1);
            analyze_suite(&stmt_for.body, acc, nesting + 1);
            analyze_suite(&stmt_for.orelse, acc, nesting + 1);
        }
        Stmt::While(stmt_while) => {
            acc.decision("while", 1, nesting);
            analyze_expr(&stmt_while.test, acc, nesting + 1);
            analyze_suite(&stmt_while.body, acc, nesting + 1);
            analyze_suite(&stmt_while.orelse, acc, nesting + 1);
        }
        Stmt::If(stmt_if) => {
            let elif_count = stmt_if
                .elif_else_clauses
                .iter()
                .filter(|clause| clause.test.is_some())
                .count() as u32;
            acc.decision("if", 1 + elif_count, nesting);
            analyze_expr(&stmt_if.test, acc, nesting + 1);
            analyze_suite(&stmt_if.body, acc, nesting + 1);
            for clause in &stmt_if.elif_else_clauses {
                analyze_elif_else_clause(clause, acc, nesting + 1);
            }
        }
        Stmt::With(stmt_with) => {
            acc.operator("with");
            for item in &stmt_with.items {
                analyze_expr(&item.context_expr, acc, nesting + 1);
                if let Some(optional_vars) = &item.optional_vars {
                    analyze_expr(optional_vars, acc, nesting + 1);
                }
            }
            analyze_suite(&stmt_with.body, acc, nesting + 1);
        }
        Stmt::Match(stmt_match) => {
            let cases = stmt_match.cases.len().max(1) as u32;
            acc.decision("match", cases, nesting);
            analyze_expr(&stmt_match.subject, acc, nesting + 1);
            for case in &stmt_match.cases {
                if let Some(guard) = &case.guard {
                    analyze_expr(guard, acc, nesting + 1);
                }
                analyze_suite(&case.body, acc, nesting + 1);
            }
        }
        Stmt::Raise(stmt_raise) => {
            acc.operator("raise");
            if let Some(exc) = &stmt_raise.exc {
                analyze_expr(exc, acc, nesting + 1);
            }
            if let Some(cause) = &stmt_raise.cause {
                analyze_expr(cause, acc, nesting + 1);
            }
        }
        Stmt::Try(stmt_try) => {
            acc.decision("try_except", stmt_try.handlers.len().max(1) as u32, nesting);
            analyze_suite(&stmt_try.body, acc, nesting + 1);
            for handler in &stmt_try.handlers {
                let ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = &handler.type_ {
                    analyze_expr(type_, acc, nesting + 1);
                }
                if let Some(name) = &handler.name {
                    acc.operand(name.as_str());
                }
                analyze_suite(&handler.body, acc, nesting + 1);
            }
            analyze_suite(&stmt_try.orelse, acc, nesting + 1);
            analyze_suite(&stmt_try.finalbody, acc, nesting + 1);
        }
        Stmt::Assert(stmt_assert) => {
            acc.operator("assert");
            analyze_expr(&stmt_assert.test, acc, nesting + 1);
            if let Some(msg) = &stmt_assert.msg {
                analyze_expr(msg, acc, nesting + 1);
            }
        }
        Stmt::Import(stmt_import) => {
            acc.operator("import");
            for alias in &stmt_import.names {
                acc.operand(alias.name.as_str());
            }
        }
        Stmt::ImportFrom(stmt_import_from) => {
            acc.operator("import_from");
            if let Some(module) = &stmt_import_from.module {
                acc.operand(module.as_str());
            }
            for alias in &stmt_import_from.names {
                acc.operand(alias.name.as_str());
            }
        }
        Stmt::Expr(stmt_expr) => analyze_expr(&stmt_expr.value, acc, nesting + 1),
        Stmt::Global(_)
        | Stmt::Nonlocal(_)
        | Stmt::Pass(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::IpyEscapeCommand(_) => {
            acc.operator("control");
        }
    }
}

fn analyze_elif_else_clause(clause: &ElifElseClause, acc: &mut ScopeAccumulator, nesting: u32) {
    if let Some(test) = &clause.test {
        analyze_expr(test, acc, nesting + 1);
    }
    analyze_suite(&clause.body, acc, nesting);
}

fn analyze_expr(expr: &Expr, acc: &mut ScopeAccumulator, nesting: u32) {
    match expr {
        Expr::BoolOp(bool_op) => {
            let extra_decisions = bool_op.values.len().saturating_sub(1) as u32;
            if extra_decisions > 0 {
                acc.decision("bool_op", extra_decisions, nesting);
            } else {
                acc.operator("bool_op");
            }
            for value in &bool_op.values {
                analyze_expr(value, acc, nesting + 1);
            }
        }
        Expr::Named(named) => {
            acc.operator("named_expr");
            analyze_expr(&named.value, acc, nesting + 1);
            analyze_expr(&named.target, acc, nesting + 1);
        }
        Expr::BinOp(bin_op) => {
            acc.operator("bin_op");
            analyze_expr(&bin_op.left, acc, nesting + 1);
            analyze_expr(&bin_op.right, acc, nesting + 1);
        }
        Expr::UnaryOp(unary_op) => {
            acc.operator("unary_op");
            analyze_expr(&unary_op.operand, acc, nesting + 1);
        }
        Expr::Lambda(lambda) => {
            acc.decision("lambda", 1, nesting);
            if let Some(parameters) = &lambda.parameters {
                add_parameters(parameters, acc);
            }
            analyze_expr(&lambda.body, acc, nesting + 1);
        }
        Expr::If(expr_if) => {
            acc.decision("if_expr", 1, nesting);
            analyze_expr(&expr_if.test, acc, nesting + 1);
            analyze_expr(&expr_if.body, acc, nesting + 1);
            analyze_expr(&expr_if.orelse, acc, nesting + 1);
        }
        Expr::Dict(dict) => {
            acc.operator("dict");
            for item in &dict.items {
                if let Some(key) = &item.key {
                    analyze_expr(key, acc, nesting + 1);
                }
                analyze_expr(&item.value, acc, nesting + 1);
            }
        }
        Expr::Set(set) => {
            acc.operator("set");
            for elt in &set.elts {
                analyze_expr(elt, acc, nesting + 1);
            }
        }
        Expr::ListComp(comp) => {
            analyze_comprehensions(&comp.generators, acc, nesting + 1);
            analyze_expr(&comp.elt, acc, nesting + 1);
        }
        Expr::SetComp(comp) => {
            analyze_comprehensions(&comp.generators, acc, nesting + 1);
            analyze_expr(&comp.elt, acc, nesting + 1);
        }
        Expr::DictComp(comp) => {
            analyze_comprehensions(&comp.generators, acc, nesting + 1);
            analyze_expr(&comp.key, acc, nesting + 1);
            analyze_expr(&comp.value, acc, nesting + 1);
        }
        Expr::Generator(generator) => {
            analyze_comprehensions(&generator.generators, acc, nesting + 1);
            analyze_expr(&generator.elt, acc, nesting + 1);
        }
        Expr::Await(await_expr) => {
            acc.operator("await");
            analyze_expr(&await_expr.value, acc, nesting + 1);
        }
        Expr::Yield(yield_expr) => {
            acc.operator("yield");
            if let Some(value) = &yield_expr.value {
                analyze_expr(value, acc, nesting + 1);
            }
        }
        Expr::YieldFrom(yield_from) => {
            acc.operator("yield_from");
            analyze_expr(&yield_from.value, acc, nesting + 1);
        }
        Expr::Compare(compare) => {
            acc.decision(
                "compare",
                compare.ops.len().saturating_sub(1) as u32,
                nesting,
            );
            acc.operator("compare");
            analyze_expr(&compare.left, acc, nesting + 1);
            for comparator in &compare.comparators {
                analyze_expr(comparator, acc, nesting + 1);
            }
        }
        Expr::Call(call) => {
            acc.operator("call");
            analyze_expr(&call.func, acc, nesting + 1);
            analyze_arguments(&call.arguments, acc, nesting + 1);
        }
        Expr::Attribute(attribute) => {
            acc.operator("attribute");
            analyze_expr(&attribute.value, acc, nesting + 1);
        }
        Expr::Subscript(subscript) => {
            acc.operator("subscript");
            analyze_expr(&subscript.value, acc, nesting + 1);
            analyze_expr(&subscript.slice, acc, nesting + 1);
        }
        Expr::Starred(starred) => analyze_expr(&starred.value, acc, nesting + 1),
        Expr::Name(name) => acc.operand(name.id.as_str()),
        Expr::List(list) => {
            acc.operator("list");
            for elt in &list.elts {
                analyze_expr(elt, acc, nesting + 1);
            }
        }
        Expr::Tuple(tuple) => {
            acc.operator("tuple");
            for elt in &tuple.elts {
                analyze_expr(elt, acc, nesting + 1);
            }
        }
        Expr::Slice(slice) => {
            acc.operator("slice");
            if let Some(lower) = &slice.lower {
                analyze_expr(lower, acc, nesting + 1);
            }
            if let Some(upper) = &slice.upper {
                analyze_expr(upper, acc, nesting + 1);
            }
            if let Some(step) = &slice.step {
                analyze_expr(step, acc, nesting + 1);
            }
        }
        Expr::FString(_) => acc.operand("literal:f_string"),
        Expr::TString(_) => acc.operand("literal:t_string"),
        Expr::StringLiteral(_) => acc.operand("literal:string"),
        Expr::BytesLiteral(_) => acc.operand("literal:bytes"),
        Expr::NumberLiteral(_) => acc.operand("literal:number"),
        Expr::BooleanLiteral(_) => acc.operand("literal:boolean"),
        Expr::NoneLiteral(_) => acc.operand("literal:none"),
        Expr::EllipsisLiteral(_) => acc.operand("literal:ellipsis"),
        Expr::IpyEscapeCommand(_) => acc.operator("ipy_escape"),
    }
}

fn analyze_comprehensions(
    comprehensions: &[Comprehension],
    acc: &mut ScopeAccumulator,
    nesting: u32,
) {
    for comprehension in comprehensions {
        acc.decision("comprehension", 1 + comprehension.ifs.len() as u32, nesting);
        analyze_expr(&comprehension.iter, acc, nesting + 1);
        analyze_expr(&comprehension.target, acc, nesting + 1);
        for if_expr in &comprehension.ifs {
            analyze_expr(if_expr, acc, nesting + 1);
        }
    }
}

fn analyze_arguments(arguments: &Arguments, acc: &mut ScopeAccumulator, nesting: u32) {
    for arg in &arguments.args {
        analyze_expr(arg, acc, nesting + 1);
    }
    for keyword in &arguments.keywords {
        analyze_expr(&keyword.value, acc, nesting + 1);
    }
}

fn add_parameters(parameters: &ruff_python_ast::Parameters, acc: &mut ScopeAccumulator) {
    for param in parameters.iter() {
        acc.operand(param.name().as_str());
        if let Some(annotation) = param.annotation() {
            analyze_expr(annotation, acc, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(source: &str) -> ComplexityInput {
        ComplexityInput {
            language: "python".to_string(),
            path: "sample.py".to_string(),
            source: source.to_string(),
        }
    }

    fn ladder_source() -> &'static str {
        r#"def classify(x):
    if x == 0:
        return "zero"
    elif x == 1:
        return "one"
    elif x == 2:
        return "two"
    elif x == 3:
        return "three"
    elif x == 4:
        return "four"
    elif x == 5:
        return "five"
    else:
        return "many"
"#
    }

    #[test]
    fn ladder_has_expected_cyclomatic_complexity() {
        let metrics = analyze_complexity(&input(ladder_source())).unwrap();
        assert_eq!(metrics.function_count, 1);
        assert_eq!(metrics.max_cyclomatic_complexity, 7);
        assert_eq!(metrics.functions[0].cyclomatic_complexity, 7);
        assert!(metrics.functions[0].halstead_volume > 0.0);
    }

    #[test]
    fn nested_control_flow_records_depth_and_cognitive_complexity() {
        let source = r#"def nested(values):
    total = 0
    for row in values:
        if row:
            while total < 10:
                if row[0] > total and row[0] < 99:
                    total += row[0]
    return total
"#;
        let metrics = analyze_complexity(&input(source)).unwrap();
        let function = &metrics.functions[0];
        assert!(function.cyclomatic_complexity >= 6);
        assert!(function.cognitive_complexity > function.cyclomatic_complexity);
        assert!(function.max_nesting_depth >= 4);
    }

    #[test]
    fn instrument_encodes_static_analysis_slot_vector() {
        let inst = ECyclomaticInstrument;
        let encoded = inst.encode(&input(ladder_source())).unwrap();
        assert_eq!(encoded.len(), InstrumentSlot::EStaticAnalysis.dim());
        assert!(encoded.iter().all(|value| value.is_finite()));
        assert!(encoded.iter().any(|value| *value > 0.0));
        assert!(encoded[0] > 0.0);
    }

    #[test]
    fn invalid_sources_fail_closed() {
        let inst = ECyclomaticInstrument;
        let mut bad = input("");
        assert_eq!(
            inst.encode(&bad).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        bad = input("def broken(:\n");
        assert_eq!(
            inst.encode(&bad).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        bad = input("x = 1\n");
        bad.language = "javascript".to_string();
        assert_eq!(
            inst.encode(&bad).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}
