use tree_sitter::Node;

use super::{EntityType, Language};

pub(crate) fn classify_node(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
) -> Option<EntityType> {
    if !node.is_named() {
        return None;
    }
    let kind = node.kind();
    if is_doc_comment(node, source) {
        return Some(EntityType::Docstring);
    }
    if matches!(kind, "comment" | "line_comment" | "block_comment") {
        return Some(EntityType::CommentBlock);
    }
    match language {
        Language::Rust => classify_rust(node, source, path),
        Language::Python => classify_python(node, source, path),
        Language::JavaScript => classify_javascript(node, source, path),
        Language::TypeScript => classify_typescript(node, source, path),
        Language::Go => classify_go(node, source, path),
        Language::Java => classify_java(node, source, path),
        Language::C => classify_c(node),
        Language::Cpp => classify_cpp(node, source),
        Language::CSharp => classify_csharp(node, source, path),
        Language::Ruby => classify_ruby(node, source, path),
        Language::Php => classify_php(node, source, path),
    }
}

pub(crate) fn node_name(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
    match language {
        Language::JavaScript | Language::TypeScript => js_like_name(node, source),
        Language::Cpp => cpp_name(node, source).or_else(|| generic_name(node, source)),
        Language::Php => php_name(node, source).or_else(|| generic_name(node, source)),
        Language::Ruby => ruby_name(node, source).or_else(|| generic_name(node, source)),
        Language::CSharp => csharp_name(node, source).or_else(|| generic_name(node, source)),
        _ => generic_name(node, source),
    }
}

fn classify_rust(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "function_item" => {
            let _ = path;
            if has_rust_test_attribute(node, source) {
                Some(EntityType::TestFunction)
            } else if has_ancestor(node, "impl_item") || has_ancestor(node, "trait_item") {
                Some(EntityType::Method)
            } else {
                Some(EntityType::Function)
            }
        }
        "struct_item" => Some(EntityType::Struct),
        "enum_item" => Some(EntityType::Enum),
        "trait_item" => Some(EntityType::TraitOrInterface),
        "impl_item" => Some(EntityType::Impl),
        "mod_item" => Some(EntityType::Module),
        "use_declaration" => Some(EntityType::Import),
        _ => None,
    }
}

fn classify_python(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "function_definition" => {
            let _ = path;
            if has_ancestor(node, "class_definition") {
                if generic_name(node, source).is_some_and(|name| is_test_function_name(&name)) {
                    Some(EntityType::TestFunction)
                } else {
                    Some(EntityType::Method)
                }
            } else if generic_name(node, source).is_some_and(|name| is_test_function_name(&name)) {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Function)
            }
        }
        "class_definition" => Some(EntityType::Class),
        "import_statement" | "import_from_statement" => Some(EntityType::Import),
        _ => None,
    }
}

fn classify_javascript(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_expression"
        | "function"
        | "generator_function"
        | "arrow_function" => {
            let _ = path;
            let name = js_like_name(node, source)?;
            if is_test_function_name(&name) {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Function)
            }
        }
        "method_definition" => Some(EntityType::Method),
        "class_declaration" | "class_expression" => Some(EntityType::Class),
        "import_statement" => Some(EntityType::Import),
        "program" => Some(EntityType::Module),
        _ => None,
    }
}

fn classify_typescript(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "interface_declaration" | "type_alias_declaration" => Some(EntityType::TraitOrInterface),
        "enum_declaration" => Some(EntityType::Enum),
        "internal_module" | "namespace_declaration" => Some(EntityType::Namespace),
        "abstract_class_declaration" => Some(EntityType::Class),
        _ => classify_javascript(node, source, path),
    }
}

fn classify_go(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "function_declaration" => {
            if is_go_test(path, node, source) {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Function)
            }
        }
        "method_declaration" => Some(EntityType::Method),
        "type_declaration" => go_type_entity(node, source),
        "package_clause" => Some(EntityType::Namespace),
        "import_declaration" | "import_spec" => Some(EntityType::Import),
        "source_file" => Some(EntityType::Module),
        _ => None,
    }
}

fn classify_java(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "method_declaration" | "constructor_declaration" => {
            let _ = path;
            if source_text(node, source).contains("@Test")
                || generic_name(node, source).is_some_and(|name| is_test_function_name(&name))
            {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Method)
            }
        }
        "class_declaration" | "record_declaration" => Some(EntityType::Class),
        "interface_declaration" | "annotation_type_declaration" => {
            Some(EntityType::TraitOrInterface)
        }
        "enum_declaration" => Some(EntityType::Enum),
        "package_declaration" => Some(EntityType::Namespace),
        "import_declaration" => Some(EntityType::Import),
        _ => None,
    }
}

fn classify_c(node: Node<'_>) -> Option<EntityType> {
    match node.kind() {
        "function_definition" => Some(EntityType::Function),
        "struct_specifier" | "union_specifier" if node.child_by_field_name("body").is_some() => {
            Some(EntityType::Struct)
        }
        "enum_specifier" if node.child_by_field_name("body").is_some() => Some(EntityType::Enum),
        "preproc_include" => Some(EntityType::Import),
        "translation_unit" => Some(EntityType::Module),
        _ => None,
    }
}

fn classify_cpp(node: Node<'_>, source: &[u8]) -> Option<EntityType> {
    match node.kind() {
        "function_definition" => {
            if source_text(node, source).contains("::") || has_ancestor(node, "class_specifier") {
                Some(EntityType::Method)
            } else {
                Some(EntityType::Function)
            }
        }
        "class_specifier" => Some(EntityType::Class),
        "struct_specifier" | "union_specifier" => Some(EntityType::Struct),
        "enum_specifier" => Some(EntityType::Enum),
        "namespace_definition" => Some(EntityType::Namespace),
        "preproc_include" | "using_declaration" | "using_directive" | "alias_declaration" => {
            Some(EntityType::Import)
        }
        "translation_unit" => Some(EntityType::Module),
        _ => None,
    }
}

fn classify_csharp(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "method_declaration"
        | "constructor_declaration"
        | "local_function_statement"
        | "destructor_declaration"
        | "operator_declaration"
        | "conversion_operator_declaration" => {
            let _ = path;
            if source_text(node, source).contains("[Test")
                || csharp_name(node, source).is_some_and(|name| is_test_function_name(&name))
            {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Method)
            }
        }
        "class_declaration" | "record_declaration" => Some(EntityType::Class),
        "struct_declaration" => Some(EntityType::Struct),
        "interface_declaration" => Some(EntityType::TraitOrInterface),
        "enum_declaration" => Some(EntityType::Enum),
        "namespace_declaration" | "file_scoped_namespace_declaration" => {
            Some(EntityType::Namespace)
        }
        "using_directive" => Some(EntityType::Import),
        _ => None,
    }
}

fn classify_ruby(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "method" | "singleton_method" => {
            if ruby_name(node, source).is_some_and(|name| name.starts_with("test_")) {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Method)
            }
        }
        "class" => Some(EntityType::Class),
        "module" => Some(EntityType::Module),
        "require" | "require_relative" => Some(EntityType::Import),
        "call" | "command" if is_ruby_test_dsl(node, source, path) => {
            Some(EntityType::TestFunction)
        }
        _ => None,
    }
}

fn classify_php(node: Node<'_>, source: &[u8], path: &str) -> Option<EntityType> {
    match node.kind() {
        "function_definition" => {
            if is_php_test(node, source, path) {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Function)
            }
        }
        "method_declaration" => {
            if is_php_test(node, source, path) {
                Some(EntityType::TestFunction)
            } else {
                Some(EntityType::Method)
            }
        }
        "class_declaration" => Some(EntityType::Class),
        "interface_declaration" | "trait_declaration" => Some(EntityType::TraitOrInterface),
        "enum_declaration" => Some(EntityType::Enum),
        "namespace_definition" => Some(EntityType::Namespace),
        "namespace_use_declaration" | "require_expression" | "include_expression" => {
            Some(EntityType::Import)
        }
        _ => None,
    }
}

fn generic_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(named) = node
        .child_by_field_name("name")
        .and_then(|child| text(child, source))
    {
        return Some(named);
    }
    node.child_by_field_name("declarator")
        .and_then(|child| first_identifier(child, source))
        .or_else(|| first_identifier(node, source))
}

fn js_like_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    generic_name(node, source).or_else(|| {
        let mut current = node.parent();
        while let Some(parent) = current {
            if matches!(
                parent.kind(),
                "variable_declarator" | "assignment_expression" | "pair"
            ) {
                if let Some(name) = parent
                    .child_by_field_name("name")
                    .or_else(|| parent.child_by_field_name("left"))
                    .or_else(|| parent.child_by_field_name("key"))
                    .and_then(|child| text(child, source))
                {
                    return Some(name);
                }
            }
            current = parent.parent();
        }
        None
    })
}

fn csharp_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    generic_name(node, source)
}

fn cpp_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("declarator")
        .and_then(|child| first_identifier(child, source))
}

fn ruby_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .or_else(|| node.child_by_field_name("method"))
        .and_then(|child| text(child, source))
        .or_else(|| first_identifier(node, source))
}

fn php_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|child| text(child, source))
        .map(|name| {
            name.trim_start_matches('\\')
                .rsplit('\\')
                .next()
                .unwrap_or("")
                .to_string()
        })
}

fn go_type_entity(node: Node<'_>, source: &[u8]) -> Option<EntityType> {
    let text = source_text(node, source);
    if text.contains(" struct") || text.contains("struct {") {
        Some(EntityType::Struct)
    } else if text.contains(" interface") || text.contains("interface {") {
        Some(EntityType::TraitOrInterface)
    } else {
        Some(EntityType::Class)
    }
}

fn is_go_test(path: &str, node: Node<'_>, source: &[u8]) -> bool {
    if !path.ends_with("_test.go") {
        return false;
    }
    let Some(name) = generic_name(node, source) else {
        return false;
    };
    if name.starts_with("Example") {
        return true;
    }
    let node_text = source_text(node, source);
    if !(node_text.contains("*testing.T")
        || node_text.contains("*testing.B")
        || node_text.contains("*testing.F"))
    {
        return false;
    }
    ["Test", "Benchmark", "Fuzz"].iter().any(|prefix| {
        name.strip_prefix(prefix)
            .and_then(|suffix| suffix.chars().next())
            .is_some_and(|ch| !ch.is_ascii_lowercase())
    })
}

fn is_test_function_name(name: &str) -> bool {
    is_test_prefix(name, "test") || is_test_prefix(name, "Test")
}

fn is_test_prefix(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    suffix.is_empty()
        || suffix.starts_with('_')
        || suffix
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn is_php_test(node: Node<'_>, source: &[u8], path: &str) -> bool {
    let path_marks = is_test_path(path) || path.ends_with("Test.php") || path.contains(".test.");
    php_name(node, source)
        .is_some_and(|name| is_test_function_name(&name) || name.starts_with("it_"))
        || (path_marks && source_text(node, source).contains("@test"))
}

fn is_ruby_test_dsl(node: Node<'_>, source: &[u8], path: &str) -> bool {
    let Some(name) = ruby_name(node, source) else {
        return false;
    };
    is_test_path(path)
        && matches!(
            name.as_str(),
            "describe" | "context" | "it" | "specify" | "test"
        )
}

fn is_doc_comment(node: Node<'_>, source: &[u8]) -> bool {
    if !matches!(node.kind(), "comment" | "line_comment" | "block_comment") {
        return false;
    }
    let text = source_text(node, source);
    text.trim_start().starts_with("///")
        || text.trim_start().starts_with("//!")
        || text.trim_start().starts_with("/**")
        || text.trim_start().starts_with("=begin")
}

fn has_rust_test_attribute(node: Node<'_>, source: &[u8]) -> bool {
    source
        .get(..node.start_byte())
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|prefix| prefix.lines().rev().find(|line| !line.trim().is_empty()))
        .is_some_and(|line| line.contains("#[test]") || line.contains("tokio::test"))
}

fn is_test_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.contains("/test/")
        || normalized.contains("/tests/")
        || normalized.contains("_test.")
        || normalized.contains(".test.")
        || normalized.contains("_spec.")
        || normalized.contains(".spec.")
}

fn has_ancestor(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }
    false
}

fn first_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "identifier" | "field_identifier" | "property_identifier" | "type_identifier" | "name"
    ) {
        return text(node, source);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = first_identifier(child, source) {
            return Some(found);
        }
    }
    None
}

fn text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source)
        .ok()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn source_text(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}
