use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// IR data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct IRFunction {
    pub name: String,
    pub signature: String,
    pub body: String,
    pub line_start: usize,
    pub line_end: usize,
    pub metrics: FnMetrics,
    pub body_hash: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct FnMetrics {
    pub instructions: usize,
    pub basic_blocks: usize,
    pub allocas: usize,
    pub stores: usize,
    pub loads: usize,
    pub calls: usize,
    pub switches: usize,
    pub phis: usize,
    pub branches: usize,
    pub rets: usize,
    pub geps: usize,
    pub list_pushes: usize,
    pub insertvalues: usize,
    pub extractvalues: usize,
    pub alloca_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StringConstant {
    pub name: String,
    pub declared_size: usize,
    pub actual_size: usize,
    pub content: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructType {
    pub name: String,
    pub definition: String,
    pub fields: Vec<String>,
    pub estimated_size: usize,
}

#[derive(Debug, Clone)]
pub struct IRModule {
    pub source: String,
    pub functions: HashMap<String, IRFunction>,
    pub declares: Vec<String>,
    pub globals: Vec<String>,
    pub struct_types: Vec<StructType>,
    pub string_constants: Vec<StringConstant>,
}

// ---------------------------------------------------------------------------
// Pathology — a detected issue in the IR
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Pathology {
    pub severity: String,
    pub code: String,
    pub function: String,
    pub line: usize,
    pub message: String,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

static FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^(define\s+(?:internal\s+)?(?:dso_local\s+)?[\w{}<>*,%\s]+?\s+@([^\s("]+)\s*\(.*?\))(?:\s*#\d+)?\s*\{"#
    ).unwrap()
});

static STRUCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(%[\w.]+)\s*=\s*type\s+(\{.+\})").unwrap()
});

static STRING_CONST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^(@[\w.]+)\s*=\s*(?:private\s+|internal\s+)?(?:unnamed_addr\s+)?constant\s+\[(\d+)\s+x\s+i8\]\s+c"(.*?)""#
    ).unwrap()
});

static DECLARE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^declare\s+.+").unwrap()
});

static GLOBAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^@[\w.]+\s*=\s*(?:private\s+|internal\s+)?(?:unnamed_addr\s+)?(?:constant|global)\s+.+"
    ).unwrap()
});

pub fn count_actual_bytes(content: &str) -> usize {
    let bytes = content.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            count += 1;
            i += 3;
        } else {
            count += 1;
            i += 1;
        }
    }
    count
}

fn estimate_type_size(ty: &str) -> usize {
    let ty = ty.trim();
    if ty.starts_with("i1") {
        return 1;
    }
    if ty.starts_with("i8") && !ty.starts_with("i8*") {
        return 1;
    }
    if ty.starts_with("i32") {
        return 4;
    }
    if ty.starts_with("i64") || ty == "double" || ty.ends_with('*') || ty == "ptr" {
        return 8;
    }
    if ty == "float" {
        return 4;
    }
    if ty.starts_with('{') {
        return (ty.matches(',').count() + 1) * 8;
    }
    let arr_re = Regex::new(r"^\[(\d+)\s*x\s*(.+)\]$").unwrap();
    if let Some(caps) = arr_re.captures(ty) {
        let n: usize = caps[1].parse().unwrap_or(1);
        return n * estimate_type_size(&caps[2]);
    }
    8
}

fn structural_hash(body: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let reg_re = Regex::new(r"%[a-zA-Z_][\w.]*").unwrap();
    let str_re = Regex::new(r"@\.?str\.\d+").unwrap();
    let label_re = Regex::new(r"label\s+%[\w.]+").unwrap();
    let block_re = Regex::new(r"(?m)^[\w.]+:").unwrap();

    let norm = reg_re.replace_all(body, "%R");
    let norm = str_re.replace_all(&norm, "@S");
    let norm = label_re.replace_all(&norm, "label %L");
    let norm = block_re.replace_all(&norm, "L:");

    let norm: String = norm
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let mut hasher = DefaultHasher::new();
    norm.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn analyze_function(body: &str) -> FnMetrics {
    let alloca_re = Regex::new(r"=\s*alloca\s+(.+?)(?:\s*,|\s*$)").unwrap();
    let mut m = FnMetrics::default();

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.ends_with(':') {
            continue;
        }
        m.instructions += 1;
    }

    m.basic_blocks = Regex::new(r"(?m)^\w[\w.]*:")
        .unwrap()
        .find_iter(body)
        .count()
        + 1;
    m.allocas = body.matches("= alloca ").count();
    m.stores = body.matches("store ").count();
    m.loads = body.matches("= load ").count();
    m.calls = Regex::new(r"(?:= )?call ").unwrap().find_iter(body).count();
    m.switches = body.matches("switch i64").count();
    m.phis = body.matches(" = phi ").count();
    m.branches = body.matches("br ").count();
    m.rets = Regex::new(r"(?m)^\s*ret\s")
        .unwrap()
        .find_iter(body)
        .count();
    m.geps = body.matches("getelementptr").count();
    m.list_pushes = body.matches("@__mn_list_push").count();
    m.insertvalues = body.matches("insertvalue").count();
    m.extractvalues = body.matches("extractvalue").count();

    for caps in alloca_re.captures_iter(body) {
        m.alloca_bytes += estimate_type_size(&caps[1]);
    }

    m
}

pub fn parse_ir(text: &str) -> IRModule {
    let mut module = IRModule {
        source: text.to_string(),
        functions: HashMap::new(),
        declares: Vec::new(),
        globals: Vec::new(),
        struct_types: Vec::new(),
        string_constants: Vec::new(),
    };

    // Struct types
    for caps in STRUCT_RE.captures_iter(text) {
        let name = caps[1].to_string();
        let def = caps[2].to_string();
        let fields: Vec<String> = def
            .trim_matches(|c| c == '{' || c == '}')
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let size = fields.iter().map(|f| estimate_type_size(f)).sum();
        module.struct_types.push(StructType {
            name,
            definition: def,
            fields,
            estimated_size: size,
        });
    }

    // Declares
    for m in DECLARE_RE.find_iter(text) {
        module.declares.push(m.as_str().to_string());
    }

    // Globals
    for m in GLOBAL_RE.find_iter(text) {
        module.globals.push(m.as_str().to_string());
    }

    // String constants
    for caps in STRING_CONST_RE.captures_iter(text) {
        let name = caps[1].to_string();
        let declared: usize = caps[2].parse().unwrap_or(0);
        let content = caps[3].to_string();
        let actual = count_actual_bytes(&content);
        let line = text[..caps.get(0).unwrap().start()].matches('\n').count() + 1;
        module.string_constants.push(StringConstant {
            name,
            declared_size: declared,
            actual_size: actual,
            content,
            line,
        });
    }

    // Functions — find matching braces
    for caps in FN_RE.captures_iter(text) {
        let sig = caps[1].to_string();
        let fn_name = caps[2].to_string();
        let start = caps.get(0).unwrap().end();

        let mut depth: i32 = 1;
        let mut pos = start;
        let text_bytes = text.as_bytes();
        while pos < text_bytes.len() && depth > 0 {
            match text_bytes[pos] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            pos += 1;
        }

        let body = &text[start..pos.saturating_sub(1)];
        let line_start = text[..caps.get(0).unwrap().start()]
            .matches('\n')
            .count()
            + 1;
        let line_end = text[..pos].matches('\n').count() + 1;

        let metrics = analyze_function(body);
        let hash = structural_hash(body);

        module.functions.insert(
            fn_name.clone(),
            IRFunction {
                name: fn_name,
                signature: sig,
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

// ---------------------------------------------------------------------------
// Pathology detectors
// ---------------------------------------------------------------------------

pub fn detect_empty_switches(module: &IRModule) -> Vec<Pathology> {
    let re = Regex::new(r"switch\s+i64\s+%[\w.]+,\s*label\s+%[\w.]+\s*\[\s*\]").unwrap();
    let mut results = Vec::new();
    for func in module.functions.values() {
        for _ in re.find_iter(&func.body) {
            results.push(Pathology {
                severity: "error".into(),
                code: "EMPTY_SWITCH".into(),
                function: func.name.clone(),
                line: func.line_start,
                message: "Switch with 0 cases — match arms not generated".into(),
                detail: String::new(),
            });
        }
    }
    results
}

pub fn detect_ret_type_mismatch(module: &IRModule) -> Vec<Pathology> {
    let sig_re = Regex::new(r"define\s+(?:internal\s+)?(?:dso_local\s+)?(.+?)\s+@").unwrap();
    let ret_re = Regex::new(r"(?m)^\s*ret\s+(.*)").unwrap();
    let mut results = Vec::new();

    for func in module.functions.values() {
        let Some(sig_caps) = sig_re.captures(&func.signature) else {
            continue;
        };
        let declared_ret = sig_caps[1].trim();
        if declared_ret == "void" {
            continue;
        }

        for ret_caps in ret_re.captures_iter(&func.body) {
            let rest = ret_caps[1].trim();
            if rest == "void" {
                continue;
            }
            let ret_ty = if rest.starts_with('{') {
                let mut depth = 0;
                let mut end = rest.len();
                for (i, ch) in rest.chars().enumerate() {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                &rest[..end]
            } else {
                rest.split_whitespace().next().unwrap_or(rest)
            };

            if ret_ty != declared_ret && ret_ty != "void" {
                results.push(Pathology {
                    severity: "error".into(),
                    code: "RET_TYPE_MISMATCH".into(),
                    function: func.name.clone(),
                    line: func.line_start,
                    message: format!("ret {ret_ty} but function declares {declared_ret}"),
                    detail: String::new(),
                });
                break;
            }
        }
    }
    results
}

pub fn detect_missing_percent(module: &IRModule) -> Vec<Pathology> {
    let re = Regex::new(r"(?m)(?:load|store|alloca|getelementptr|call)\s.*\s([a-zA-Z_]\w*\.addr|[a-zA-Z_]\w*\.ptr)\b").unwrap();
    let mut results = Vec::new();
    for func in module.functions.values() {
        for caps in re.captures_iter(&func.body) {
            let bare = &caps[1];
            if !bare.starts_with('%') && !bare.starts_with('@') {
                results.push(Pathology {
                    severity: "warning".into(),
                    code: "MISSING_PERCENT".into(),
                    function: func.name.clone(),
                    line: func.line_start,
                    message: format!("Bare identifier '{bare}' — missing % prefix?"),
                    detail: String::new(),
                });
            }
        }
    }
    results
}

pub fn detect_duplicate_switch_cases(module: &IRModule) -> Vec<Pathology> {
    let switch_re = Regex::new(r"switch\s+i64\s+%[\w.]+,\s*label\s+%[\w.]+\s*\[([^\]]+)\]").unwrap();
    let case_re = Regex::new(r"i64\s+(\d+),").unwrap();
    let mut results = Vec::new();

    for func in module.functions.values() {
        for sw_caps in switch_re.captures_iter(&func.body) {
            let cases_text = &sw_caps[1];
            let mut seen = std::collections::HashSet::new();
            for case_caps in case_re.captures_iter(cases_text) {
                let val = &case_caps[1];
                if !seen.insert(val.to_string()) {
                    results.push(Pathology {
                        severity: "error".into(),
                        code: "DUPLICATE_CASE".into(),
                        function: func.name.clone(),
                        line: func.line_start,
                        message: format!("Duplicate switch case value: {val}"),
                        detail: String::new(),
                    });
                }
            }
        }
    }
    results
}

pub fn run_all_detectors(module: &IRModule) -> Vec<Pathology> {
    let mut all = Vec::new();
    all.extend(detect_empty_switches(module));
    all.extend(detect_ret_type_mismatch(module));
    all.extend(detect_missing_percent(module));
    all.extend(detect_duplicate_switch_cases(module));
    all.sort_by(|a, b| {
        let sev_ord = |s: &str| match s {
            "error" => 0,
            "warning" => 1,
            _ => 2,
        };
        sev_ord(&a.severity)
            .cmp(&sev_ord(&b.severity))
            .then(a.function.cmp(&b.function))
    });
    all
}

// ---------------------------------------------------------------------------
// llvm-as validation
// ---------------------------------------------------------------------------

pub fn find_llvm_as() -> Option<String> {
    for name in &["llvm-as", "llvm-as-18", "llvm-as-17", "llvm-as-16", "llvm-as-15"] {
        if std::process::Command::new(name)
            .arg("--version")
            .output()
            .is_ok()
        {
            return Some(name.to_string());
        }
    }
    None
}

pub fn validate_with_llvm_as(ir_text: &str) -> (bool, String) {
    let Some(llvm_as) = find_llvm_as() else {
        return (true, "(llvm-as not found, skipped)".into());
    };

    let tmp = std::env::temp_dir().join("culebra_check.ll");
    if std::fs::write(&tmp, ir_text).is_err() {
        return (false, "failed to write temp file".into());
    }

    let result = std::process::Command::new(&llvm_as)
        .arg(tmp.to_str().unwrap())
        .arg("-o")
        .arg("/dev/null")
        .output();

    let _ = std::fs::remove_file(&tmp);

    match result {
        Ok(output) => {
            if output.status.success() {
                (true, String::new())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let first_line = stderr.lines().next().unwrap_or("unknown error");
                (false, first_line.to_string())
            }
        }
        Err(e) => (false, format!("failed to run llvm-as: {e}")),
    }
}
