use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// C data structures (mirrors ir.rs for C files)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CFunction {
    pub name: String,
    pub return_type: String,
    pub params: Vec<String>,
    pub body: String,
    pub line_start: usize,
    pub line_end: usize,
    pub metrics: CFnMetrics,
    pub body_hash: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CFnMetrics {
    pub lines: usize,
    pub statements: usize,
    pub locals: usize,
    pub calls: usize,
    pub ifs: usize,
    pub switches: usize,
    pub gotos: usize,
    pub returns: usize,
    pub loops: usize,
    pub mallocs: usize,
    pub frees: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CStruct {
    pub name: String,
    pub fields: Vec<CField>,
    pub line: usize,
    pub is_typedef: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CField {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CEnum {
    pub name: String,
    pub variants: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct CModule {
    pub source: String,
    pub functions: HashMap<String, CFunction>,
    pub structs: Vec<CStruct>,
    pub enums: Vec<CEnum>,
    pub typedefs: Vec<String>,
    pub includes: Vec<String>,
    pub forward_decls: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

static FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Match C function definitions: type name(params) {
    // Handles: static, inline, MN_EXPORT, void, pointer returns, struct returns
    Regex::new(
        r"(?m)^(?:static\s+|inline\s+|MN_EXPORT\s+|extern\s+)*([\w\s*]+?)\s+(\w+)\s*\(([^)]*)\)\s*\{"
    ).unwrap()
});

static STRUCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(?:typedef\s+)?struct\s+(\w+)?\s*\{").unwrap()
});

static TYPEDEF_STRUCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\}\s*(\w+)\s*;").unwrap()
});

static ENUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(?:typedef\s+)?enum\s+(\w+)?\s*\{").unwrap()
});

static INCLUDE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^#include\s+[<"](.+)[>"]"#).unwrap()
});

static FORWARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(?:extern\s+)?\w[\w\s*]*\s+\w+\s*\([^)]*\)\s*;").unwrap()
});

pub fn parse_c(text: &str) -> CModule {
    let mut module = CModule {
        source: text.to_string(),
        functions: HashMap::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        typedefs: Vec::new(),
        includes: Vec::new(),
        forward_decls: Vec::new(),
    };

    if text.trim().is_empty() {
        return module;
    }

    // Includes
    for caps in INCLUDE_RE.captures_iter(text) {
        module.includes.push(caps[1].to_string());
    }

    // Forward declarations
    for m in FORWARD_RE.find_iter(text) {
        let decl = m.as_str().to_string();
        if !decl.contains('{') {
            module.forward_decls.push(decl);
        }
    }

    // Structs
    parse_structs(text, &mut module);

    // Enums
    parse_enums(text, &mut module);

    // Functions — brace matching
    for caps in FN_RE.captures_iter(text) {
        let ret_type = caps[1].trim().to_string();
        let fn_name = caps[2].to_string();
        let params_str = caps[3].to_string();
        let start = caps.get(0).unwrap().end();

        // Skip if this looks like a forward decl or macro
        if fn_name == "if" || fn_name == "for" || fn_name == "while" || fn_name == "switch" {
            continue;
        }

        let mut depth: i32 = 1;
        let mut pos = start;
        let bytes = text.as_bytes();
        let max_scan = (start + 50_000_000).min(bytes.len());

        while pos < max_scan && depth > 0 {
            match bytes[pos] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' => {
                    // Skip line comment
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                b'"' => {
                    // Skip string literal
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != b'"' {
                        if bytes[pos] == b'\\' { pos += 1; }
                        pos += 1;
                    }
                }
                _ => {}
            }
            pos += 1;
        }

        if depth != 0 {
            continue; // Truncated function
        }

        let body = &text[start..pos.saturating_sub(1)];
        let line_start = text[..caps.get(0).unwrap().start()].matches('\n').count() + 1;
        let line_end = text[..pos].matches('\n').count() + 1;

        let params: Vec<String> = if params_str.trim() == "void" || params_str.trim().is_empty() {
            Vec::new()
        } else {
            params_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        let metrics = analyze_c_function(body);
        let hash = structural_hash_c(body);

        module.functions.insert(
            fn_name.clone(),
            CFunction {
                name: fn_name,
                return_type: ret_type,
                params,
                body: body.to_string(),
                line_start,
                line_end,
                metrics,
                body_hash: hash,
            },
        );
    }

    module
}

fn parse_structs(text: &str, module: &mut CModule) {
    let field_re = Regex::new(r"(?m)^\s+([\w\s*]+?)\s+(\w+)\s*(?:\[[\w\s]*\])?\s*;").unwrap();

    for caps in STRUCT_RE.captures_iter(text) {
        let name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let start = caps.get(0).unwrap().end();
        let line = text[..caps.get(0).unwrap().start()].matches('\n').count() + 1;

        // Find closing brace
        let mut depth = 1i32;
        let mut pos = start;
        let bytes = text.as_bytes();
        while pos < bytes.len() && depth > 0 {
            if bytes[pos] == b'{' { depth += 1; }
            if bytes[pos] == b'}' { depth -= 1; }
            pos += 1;
        }

        let body = &text[start..pos.saturating_sub(1)];

        // Check for typedef name after closing brace
        let after_brace = &text[pos..text.len().min(pos + 50)];
        let typedef_name = TYPEDEF_STRUCT_RE.captures(after_brace)
            .map(|c| c[1].to_string());

        let actual_name = typedef_name.unwrap_or(name);
        if actual_name.is_empty() { continue; }

        let mut fields = Vec::new();
        for fcaps in field_re.captures_iter(body) {
            fields.push(CField {
                type_name: fcaps[1].trim().to_string(),
                name: fcaps[2].to_string(),
            });
        }

        module.structs.push(CStruct {
            name: actual_name,
            fields,
            line,
            is_typedef: text[..caps.get(0).unwrap().start()].ends_with("typedef ") ||
                text[caps.get(0).unwrap().start()..].starts_with("typedef"),
        });
    }
}

fn parse_enums(text: &str, module: &mut CModule) {
    let variant_re = Regex::new(r"(\w+)\s*(?:=\s*\d+)?\s*[,}]").unwrap();

    for caps in ENUM_RE.captures_iter(text) {
        let name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let start = caps.get(0).unwrap().end();
        let line = text[..caps.get(0).unwrap().start()].matches('\n').count() + 1;

        let mut depth = 1i32;
        let mut pos = start;
        let bytes = text.as_bytes();
        while pos < bytes.len() && depth > 0 {
            if bytes[pos] == b'{' { depth += 1; }
            if bytes[pos] == b'}' { depth -= 1; }
            pos += 1;
        }

        let body = &text[start..pos.saturating_sub(1)];

        let variants: Vec<String> = variant_re.captures_iter(body)
            .map(|c| c[1].to_string())
            .collect();

        if !name.is_empty() || !variants.is_empty() {
            module.enums.push(CEnum {
                name: if name.is_empty() { "(anonymous)".to_string() } else { name },
                variants,
                line,
            });
        }
    }
}

fn analyze_c_function(body: &str) -> CFnMetrics {
    let mut m = CFnMetrics::default();

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        m.lines += 1;
        if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
            m.statements += 1;
        }
    }

    let call_re = Regex::new(r"\w+\s*\(").unwrap();
    let keywords = ["if", "for", "while", "switch", "return", "goto", "else"];

    m.calls = call_re.find_iter(body).filter(|m| {
        let word = m.as_str().trim_end_matches('(').trim();
        !keywords.contains(&word)
    }).count();

    m.locals = Regex::new(r"(?m)^\s+\w[\w\s*]*\s+\w+\s*[=;]").unwrap().find_iter(body).count();
    m.ifs = body.matches(" if ").count() + body.matches("\tif ").count() + body.matches("\nif ").count();
    m.switches = body.matches("switch ").count() + body.matches("switch(").count();
    m.gotos = Regex::new(r"\bgoto\s+\w+").unwrap().find_iter(body).count();
    m.returns = Regex::new(r"\breturn\b").unwrap().find_iter(body).count();
    m.loops = body.matches("for ").count() + body.matches("for(").count()
        + body.matches("while ").count() + body.matches("while(").count();
    m.mallocs = body.matches("malloc(").count() + body.matches("calloc(").count();
    m.frees = body.matches("free(").count();

    m
}

fn structural_hash_c(body: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let var_re = Regex::new(r"\b[a-z_]\w*\b").unwrap();

    let norm = var_re.replace_all(body, "V");
    let norm: String = norm.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut hasher = DefaultHasher::new();
    norm.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
