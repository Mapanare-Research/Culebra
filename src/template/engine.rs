use regex::Regex;
use std::collections::HashMap;

use crate::ir::IRModule;
use super::schema::*;

// ---------------------------------------------------------------------------
// Finding — a single match result from a template
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Finding {
    pub template_id: String,
    pub template_name: String,
    pub severity: Severity,
    pub tags: Vec<String>,
    pub matched_text: String,
    pub line: usize,
    pub function: Option<String>,
    pub extractions: HashMap<String, String>,
    pub report_format: String,
    pub suggestion: String,
    pub description: String,
    pub impact: String,
    pub cwe: String,
    pub autofix: Option<Autofix>,
}

// ---------------------------------------------------------------------------
// Engine — runs templates against an IR module
// ---------------------------------------------------------------------------

pub fn run_template(template: &Template, module: &IRModule) -> Vec<Finding> {
    if template.scope.file_type != FileType::LlvmIr {
        return Vec::new();
    }

    let mut findings = Vec::new();

    match &template.match_block {
        MatchBlock::Matchers { matchers, condition } => {
            run_matchers(template, module, matchers, condition, &mut findings);
        }
        MatchBlock::Sequence { steps, condition, .. } => {
            run_sequence(template, module, steps, condition, &mut findings);
        }
        MatchBlock::CrossReference { steps, .. } => {
            run_cross_reference(template, module, steps, &mut findings);
        }
    }

    // Run extractors on findings to populate extractions
    for finding in &mut findings {
        run_extractors(template, &finding.matched_text, &mut finding.extractions);
    }

    // Format report messages
    let report_fmt = template
        .report
        .as_ref()
        .map(|r| r.format.clone())
        .unwrap_or_default();
    for finding in &mut findings {
        finding.report_format = interpolate(&report_fmt, &finding.extractions);
    }

    findings
}

// ---------------------------------------------------------------------------
// Matcher-based matching (regex on globals/functions/all)
// ---------------------------------------------------------------------------

fn run_matchers(
    template: &Template,
    module: &IRModule,
    matchers: &[Matcher],
    condition: &Condition,
    findings: &mut Vec<Finding>,
) {
    let lines = get_target_lines(template, module);

    for (line_num, line_text) in &lines {
        let results: Vec<bool> = matchers.iter().map(|m| match_single(m, line_text)).collect();

        let matched = match condition {
            Condition::Or => results.iter().any(|&r| r),
            Condition::And | Condition::All => results.iter().all(|&r| r),
        };

        if matched {
            let mut extractions = HashMap::new();
            // Run inline matcher extractors
            for m in matchers {
                if let Some(ext) = &m.extractor {
                    for pat in &m.pattern {
                        if let Ok(re) = Regex::new(pat) {
                            if let Some(caps) = re.captures(line_text) {
                                if let Some(val) = caps.get(ext.group) {
                                    extractions
                                        .insert(ext.name.clone(), val.as_str().to_string());
                                }
                            }
                        }
                    }
                }
            }

            findings.push(make_finding(
                template,
                line_text.to_string(),
                *line_num,
                None,
                extractions,
            ));
        }
    }
}

fn match_single(matcher: &Matcher, text: &str) -> bool {
    match matcher.matcher_type {
        MatcherType::Regex => {
            let negated = matcher.condition == Some(MatcherCondition::NotContains);
            let matched = matcher.pattern.iter().any(|pat| {
                Regex::new(pat).map(|re| re.is_match(text)).unwrap_or(false)
            });
            if negated { !matched } else { matched }
        }
        MatcherType::Contains => {
            let negated = matcher.condition == Some(MatcherCondition::NotContains);
            let matched = matcher
                .value
                .as_ref()
                .map(|v| text.contains(v.as_str()))
                .unwrap_or(false);
            if negated { !matched } else { matched }
        }
        MatcherType::ByteScan => {
            if let Some(range) = &matcher.byte_range {
                if range.len() == 2 {
                    let (lo, hi) = (range[0], range[1]);
                    let exclude = matcher.exclude.as_deref().unwrap_or(&[]);
                    for byte in text.as_bytes() {
                        if *byte >= lo && *byte <= hi && !exclude.contains(byte) {
                            return true;
                        }
                    }
                }
            }
            false
        }
    }
}

/// Get lines from the appropriate section of the module
fn get_target_lines(template: &Template, module: &IRModule) -> Vec<(usize, String)> {
    let source = &module.source;
    let section = &template.scope.section;

    match section {
        Section::Globals => {
            // Return global/constant lines
            source
                .lines()
                .enumerate()
                .filter(|(_, line)| {
                    let trimmed = line.trim();
                    trimmed.starts_with('@')
                        && (trimmed.contains("constant") || trimmed.contains("global"))
                })
                .map(|(i, l)| (i + 1, l.to_string()))
                .collect()
        }
        Section::Declarations => {
            source
                .lines()
                .enumerate()
                .filter(|(_, line)| line.trim().starts_with("declare"))
                .map(|(i, l)| (i + 1, l.to_string()))
                .collect()
        }
        Section::Functions => {
            // Return all lines within function bodies
            let mut result = Vec::new();
            for func in module.functions.values() {
                for (offset, line) in func.body.lines().enumerate() {
                    result.push((func.line_start + offset, line.to_string()));
                }
            }
            result
        }
        Section::Metadata => {
            source
                .lines()
                .enumerate()
                .filter(|(_, line)| line.trim().starts_with('!'))
                .map(|(i, l)| (i + 1, l.to_string()))
                .collect()
        }
        Section::All => {
            source
                .lines()
                .enumerate()
                .map(|(i, l)| (i + 1, l.to_string()))
                .collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Sequence matching — multi-step patterns within function bodies
// ---------------------------------------------------------------------------

fn run_sequence(
    template: &Template,
    module: &IRModule,
    steps: &[SequenceStep],
    _condition: &Condition,
    findings: &mut Vec<Finding>,
) {
    for func in module.functions.values() {
        let body_lines: Vec<&str> = func.body.lines().collect();
        if let Some(finding) = match_sequence_in_body(template, steps, &body_lines, func) {
            findings.push(finding);
        }
    }
}

fn match_sequence_in_body(
    template: &Template,
    steps: &[SequenceStep],
    lines: &[&str],
    func: &crate::ir::IRFunction,
) -> Option<Finding> {
    let mut captures: HashMap<String, String> = HashMap::new();
    let mut step_positions: HashMap<String, usize> = HashMap::new();
    let mut first_match_line = 0usize;
    let mut matched_text = String::new();

    for step in steps {
        let is_absent = step.step_type.as_deref() == Some("absent");

        // Determine search start line
        let start_line = if let Some(ref after_id) = step.after {
            step_positions.get(after_id).copied().unwrap_or(0)
        } else {
            // If no after, start after the previous step's position
            step_positions.values().copied().max().unwrap_or(0)
        };

        let end_line = if let Some(within) = step.within_lines {
            (start_line + within).min(lines.len())
        } else {
            lines.len()
        };

        // Interpolate captures into pattern
        let pattern = interpolate(&step.pattern, &captures);

        let re = match Regex::new(&pattern) {
            Ok(r) => r,
            Err(_) => return None,
        };

        if is_absent {
            // Pattern must NOT appear in the range
            let found = (start_line..end_line).any(|i| re.is_match(lines[i]));
            if found {
                return None; // Pattern was found, so this sequence doesn't match
            }
            step_positions.insert(step.id.clone(), start_line);
        } else {
            // Pattern must appear in the range
            let mut found = false;
            for i in start_line..end_line {
                if let Some(caps) = re.captures(lines[i]) {
                    // Capture named groups
                    for (name, &group) in &step.capture {
                        if let Some(val) = caps.get(group) {
                            captures.insert(name.clone(), regex::escape(val.as_str()));
                        }
                    }
                    step_positions.insert(step.id.clone(), i + 1);
                    if first_match_line == 0 {
                        first_match_line = i;
                        matched_text = lines[i].to_string();
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
        }
    }

    Some(make_finding(
        template,
        matched_text,
        func.line_start + first_match_line,
        Some(func.name.clone()),
        captures,
    ))
}

// ---------------------------------------------------------------------------
// Cross-reference matching (stub — logs that it needs two files)
// ---------------------------------------------------------------------------

fn run_cross_reference(
    template: &Template,
    _module: &IRModule,
    _steps: &[CrossRefStep],
    _findings: &mut Vec<Finding>,
) {
    // Cross-reference matching requires two files (IR + C header).
    // This is handled at the scan command level by passing both files.
    // When run in single-file mode, cross-ref templates are skipped.
    eprintln!(
        "  note: template '{}' requires cross-reference inputs (--ir + --header)",
        template.id
    );
}

/// Run cross-reference matching with two file contents
pub fn run_cross_reference_with_files(
    template: &Template,
    ir_content: &str,
    header_content: &str,
) -> Vec<Finding> {
    let steps = match &template.match_block {
        MatchBlock::CrossReference { steps, .. } => steps,
        _ => return Vec::new(),
    };

    let mut ir_captures: HashMap<String, String> = HashMap::new();
    let mut header_captures: HashMap<String, String> = HashMap::new();
    let mut findings = Vec::new();

    // Extract from IR file
    for step in steps {
        if step.step_type.as_deref() == Some("layout_compare") {
            continue;
        }
        let Some(ref pattern) = step.pattern else {
            continue;
        };
        let re = match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let content = match step.file.as_deref() {
            Some("ir_file") => ir_content,
            Some("c_header") => header_content,
            _ => continue,
        };

        for caps in re.captures_iter(content) {
            for (name, &group) in &step.capture {
                if let Some(val) = caps.get(group) {
                    let target = if step.file.as_deref() == Some("ir_file") {
                        &mut ir_captures
                    } else {
                        &mut header_captures
                    };
                    target.insert(name.clone(), val.as_str().to_string());
                }
            }
        }
    }

    // Check for layout comparison step
    for step in steps {
        if step.step_type.as_deref() == Some("layout_compare") {
            // Compare field counts as a basic check
            let ir_fields = ir_captures
                .values()
                .next()
                .map(|v| v.split(',').count())
                .unwrap_or(0);
            let c_fields = header_captures
                .values()
                .next()
                .map(|v| v.split(';').filter(|s| !s.trim().is_empty()).count())
                .unwrap_or(0);

            if ir_fields != c_fields && ir_fields > 0 && c_fields > 0 {
                let mut extractions = HashMap::new();
                extractions.insert("ir_field_count".into(), ir_fields.to_string());
                extractions.insert("c_field_count".into(), c_fields.to_string());
                for (k, v) in &ir_captures {
                    extractions.insert(k.clone(), v.clone());
                }
                for (k, v) in &header_captures {
                    extractions.insert(k.clone(), v.clone());
                }

                findings.push(make_finding(
                    template,
                    format!("IR has {} fields, C has {} fields", ir_fields, c_fields),
                    0,
                    None,
                    extractions,
                ));
            }
        }
    }

    findings
}

// ---------------------------------------------------------------------------
// Extractors
// ---------------------------------------------------------------------------

fn run_extractors(
    template: &Template,
    matched_text: &str,
    extractions: &mut HashMap<String, String>,
) {
    for ext in &template.extractors {
        match ext.extractor_type {
            ExtractorType::Regex => {
                if let Some(ref pat) = ext.pattern {
                    if let Ok(re) = Regex::new(pat) {
                        if let Some(caps) = re.captures(matched_text) {
                            let group = ext.group.unwrap_or(1);
                            if let Some(val) = caps.get(group) {
                                extractions.insert(ext.name.clone(), val.as_str().to_string());
                            }
                        }
                    }
                }
            }
            ExtractorType::Computed => {
                // Computed extractors would need engine-level logic
                // Stub: insert placeholder
                extractions.insert(ext.name.clone(), "(computed)".into());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Autofix engine
// ---------------------------------------------------------------------------

/// Apply autofixes from findings to the source text. Returns the modified text.
pub fn apply_autofixes(source: &str, findings: &[Finding]) -> String {
    let mut result = source.to_string();
    for finding in findings {
        if let Some(ref autofix) = finding.autofix {
            if autofix.fix_type == "line_replace" {
                if let Ok(re) = Regex::new(&autofix.match_pattern) {
                    result = re.replace_all(&result, autofix.replace.as_str()).to_string();
                }
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_finding(
    template: &Template,
    matched_text: String,
    line: usize,
    function: Option<String>,
    extractions: HashMap<String, String>,
) -> Finding {
    Finding {
        template_id: template.id.clone(),
        template_name: template.info.name.clone(),
        severity: template.info.severity.clone(),
        tags: template.info.tags.clone(),
        matched_text,
        line,
        function,
        extractions,
        report_format: String::new(),
        suggestion: template
            .remediation
            .as_ref()
            .map(|r| r.suggestion.clone())
            .unwrap_or_default(),
        description: template.info.description.clone(),
        impact: template.info.impact.clone(),
        cwe: template.info.cwe.clone(),
        autofix: template.remediation.as_ref().and_then(|r| r.autofix.clone()),
    }
}

/// Interpolate {variable} references in a string
fn interpolate(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}
