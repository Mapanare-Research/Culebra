use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Top-level template
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Template {
    pub id: String,
    pub info: TemplateInfo,
    #[serde(default)]
    pub scope: Scope,
    #[serde(rename = "match")]
    pub match_block: MatchBlock,
    #[serde(default)]
    pub extractors: Vec<Extractor>,
    #[serde(default)]
    pub report: Option<Report>,
    #[serde(default)]
    pub remediation: Option<Remediation>,
    #[serde(default)]
    pub related: Vec<String>,
}

// ---------------------------------------------------------------------------
// Info section
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    #[serde(default = "default_severity")]
    pub severity: Severity,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub impact: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub cwe: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

fn default_severity() -> Severity {
    Severity::Medium
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "critical"),
            Severity::High => write!(f, "high"),
            Severity::Medium => write!(f, "medium"),
            Severity::Low => write!(f, "low"),
            Severity::Info => write!(f, "info"),
        }
    }
}

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Scope {
    #[serde(default = "default_file_type")]
    pub file_type: FileType,
    #[serde(default = "default_section")]
    pub section: Section,
    #[serde(default)]
    pub inputs: HashMap<String, String>,
}

impl Default for Scope {
    fn default() -> Self {
        Scope {
            file_type: FileType::LlvmIr,
            section: Section::All,
            inputs: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FileType {
    LlvmIr,
    CSource,
    ElfBinary,
    CHeader,
    CrossReference,
}

fn default_file_type() -> FileType {
    FileType::LlvmIr
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Globals,
    Functions,
    Declarations,
    Metadata,
    All,
}

fn default_section() -> Section {
    Section::All
}

// ---------------------------------------------------------------------------
// Match block — supports multiple forms
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MatchBlock {
    Matchers {
        matchers: Vec<Matcher>,
        #[serde(default = "default_condition")]
        condition: Condition,
    },
    Sequence {
        #[serde(rename = "type")]
        match_type: String, // "sequence"
        steps: Vec<SequenceStep>,
        #[serde(default = "default_condition_all")]
        condition: Condition,
    },
    CrossReference {
        #[serde(rename = "type")]
        match_type: String, // "cross_reference"
        steps: Vec<CrossRefStep>,
    },
}

fn default_condition() -> Condition {
    Condition::Or
}

fn default_condition_all() -> Condition {
    Condition::All
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Condition {
    Or,
    And,
    All,
}

// ---------------------------------------------------------------------------
// Matchers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Matcher {
    #[serde(rename = "type")]
    pub matcher_type: MatcherType,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub pattern: Vec<String>,
    #[serde(default)]
    pub condition: Option<MatcherCondition>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub byte_range: Option<Vec<u8>>,
    #[serde(default)]
    pub exclude: Option<Vec<u8>>,
    #[serde(default)]
    pub extractor: Option<MatcherExtractor>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MatcherType {
    Regex,
    ByteScan,
    Contains,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MatcherCondition {
    Contains,
    NotContains,
    ContainsRawBytes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatcherExtractor {
    pub name: String,
    pub group: usize,
}

// ---------------------------------------------------------------------------
// Sequence steps
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct SequenceStep {
    pub id: String,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub capture: HashMap<String, usize>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub within_lines: Option<usize>,
    /// "absent" means this pattern must NOT appear
    #[serde(rename = "type", default)]
    pub step_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Cross-reference steps
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct CrossRefStep {
    pub id: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub capture: HashMap<String, usize>,
    #[serde(rename = "type", default)]
    pub step_type: Option<String>,
    #[serde(default)]
    pub type_map: Option<HashMap<String, Vec<String>>>,
}

// ---------------------------------------------------------------------------
// Extractors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Extractor {
    #[serde(rename = "type")]
    pub extractor_type: ExtractorType,
    pub name: String,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub group: Option<usize>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExtractorType {
    Regex,
    Computed,
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub evidence: Option<Evidence>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Evidence {
    #[serde(default)]
    pub show_line: bool,
    #[serde(default = "default_context")]
    pub show_context: usize,
}

fn default_context() -> usize {
    0
}

// ---------------------------------------------------------------------------
// Remediation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Remediation {
    #[serde(default)]
    pub suggestion: String,
    #[serde(default)]
    pub autofix: Option<Autofix>,
    #[serde(default)]
    pub difficulty: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Autofix {
    #[serde(rename = "type")]
    pub fix_type: String,
    #[serde(rename = "match")]
    pub match_pattern: String,
    pub replace: String,
}

// ---------------------------------------------------------------------------
// Drain queue — dynamic template queue written by external tools (e.g. Mapanare)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct DrainQueue {
    pub queued: Vec<DrainEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DrainEntry {
    /// Run a specific template by ID (e.g. "abi/return-type-divergence")
    #[serde(default)]
    pub template: Option<String>,
    /// Or select templates by tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Target file to scan
    pub target: String,
    /// Optional C header for cross-reference templates
    #[serde(default)]
    pub header: Option<String>,
    /// Human-readable reason this entry was queued (for reporting)
    #[serde(default)]
    pub reason: Option<String>,
    /// Stop the entire drain if this entry produces findings at this severity or above
    #[serde(default)]
    pub stop_on: Option<Severity>,
}

// ---------------------------------------------------------------------------
// Workflow (separate template type)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub info: TemplateInfo,
    pub workflow: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowStep {
    pub templates: WorkflowTemplateSelector,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(default)]
    pub stop_on: Option<Severity>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowTemplateSelector {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ids: Vec<String>,
}
