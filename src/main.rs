mod commands;
mod ir;
mod template;

use clap::{Parser, Subcommand};
use std::collections::HashMap;

#[derive(Parser)]
#[command(
    name = "culebra",
    version,
    about = "Compiler forge — ABI, IR, binary, and bootstrap diagnostics for self-hosting compilers"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate [N x i8] c"..." byte counts in an LLVM IR file
    Strings {
        /// Path to .ll file
        file: String,
        /// Show duplicate string details
        #[arg(short, long)]
        verbose: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Audit LLVM IR for known pathologies (empty switch, ret mismatch, alloca alias, etc.)
    Audit {
        /// Path to .ll file
        file: String,
        /// Filter functions by substring
        #[arg(long)]
        only: Option<String>,
        /// Save baseline for delta tracking
        #[arg(long)]
        baseline: Option<String>,
    },

    /// Validate IR file with llvm-as
    Check {
        /// Path to .ll file
        file: String,
    },

    /// Validate a transform script preserves IR structure
    PhiCheck {
        /// Path to .ll file (before transform)
        file: String,
        /// Transform command (receives IR on stdin, outputs on stdout)
        #[arg(long, default_value = "python3 scripts/fix_stage2_phis.py -")]
        fix_cmd: String,
    },

    /// Per-function structural diff between two IR files
    Diff {
        /// First .ll file
        file_a: String,
        /// Second .ll file
        file_b: String,
        /// Show per-instruction diffs for divergent functions
        #[arg(short, long)]
        verbose: bool,
    },

    /// Extract one function's IR from a .ll file
    Extract {
        /// Path to .ll file
        file: String,
        /// Function name (exact or substring)
        func_name: String,
    },

    /// Per-function metrics table (instructions, allocas, calls, etc.)
    Table {
        /// Path to .ll file
        file: String,
        /// Show top N functions only
        #[arg(long)]
        top: Option<usize>,
        /// Sort column
        #[arg(long, default_value = "instructions")]
        sort_by: String,
    },

    /// Validate struct layouts match between IR and C headers
    Abi {
        /// Path to .ll file
        file: String,
        /// Path to C header or source file
        #[arg(long)]
        header: Option<String>,
    },

    /// Inspect binary and cross-reference string addresses against IR
    Binary {
        /// Path to ELF/PE binary or .o file
        file: String,
        /// Path to .ll file for cross-referencing GEP targets
        #[arg(long)]
        ir: Option<String>,
        /// Verify a specific string exists at correct address
        #[arg(long)]
        find: Option<String>,
    },

    /// Compile a .mn program through a compiler, run the binary, check output
    Run {
        /// Compiler binary to use
        compiler: String,
        /// Source file to compile
        source: String,
        /// Expected stdout output (fail if different)
        #[arg(long)]
        expect: Option<String>,
        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
        /// Extra flags passed to clang when linking
        #[arg(long)]
        clang_flags: Option<String>,
        /// Path to C runtime to link
        #[arg(long)]
        runtime: Option<String>,
    },

    /// Run all [[tests]] from culebra.toml — compile, execute, diff output
    Test {
        /// Culebra project config file
        #[arg(short, long, default_value = "culebra.toml")]
        config: String,
        /// Filter tests by name substring
        #[arg(long)]
        filter: Option<String>,
        /// Timeout per test in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Watch files and re-run a command on change
    Watch {
        /// Glob patterns to watch (comma-separated)
        #[arg(short, long, default_value = "*.ll,*.mn")]
        patterns: String,
        /// Directory to watch
        #[arg(short, long, default_value = ".")]
        dir: String,
        /// Command to run on change
        cmd: Vec<String>,
    },

    /// Build and test a full stage pipeline end-to-end
    Pipeline {
        /// Culebra project config file
        #[arg(short, long, default_value = "culebra.toml")]
        config: String,
        /// Per-step timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Show bootstrap self-hosting progress
    Status {
        /// Culebra project config file
        #[arg(short, long, default_value = "culebra.toml")]
        config: String,
    },

    /// Detect fixed-point: run stage N output through itself, check if output stabilizes
    Fixedpoint {
        /// Compiler binary (stage N)
        compiler: String,
        /// Source file to compile (usually the compiler's own source)
        source: String,
        /// Max iterations before giving up
        #[arg(long, default_value = "3")]
        max_iters: usize,
        /// Timeout per compilation in seconds
        #[arg(long, default_value = "120")]
        timeout: u64,
        /// Path to C runtime to link
        #[arg(long)]
        runtime: Option<String>,
    },

    /// Initialize a culebra.toml config for a compiler project
    Init,

    /// Scan IR files with pattern templates (Nuclei-style)
    Scan {
        /// Path to .ll file
        file: String,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Filter by severity (comma-separated: critical,high,medium,low,info)
        #[arg(long)]
        severity: Option<String>,
        /// Run a specific template by ID
        #[arg(long)]
        id: Option<String>,
        /// Path to custom template file or directory
        #[arg(long)]
        template: Option<String>,
        /// Path to C header for cross-reference checks
        #[arg(long)]
        header: Option<String>,
        /// Output format: text, json, sarif, markdown
        #[arg(long, default_value = "text")]
        format: String,
        /// Apply auto-fixes
        #[arg(long)]
        autofix: bool,
        /// Show fixes without applying (use with --autofix)
        #[arg(long)]
        dry_run: bool,
    },

    /// Show a diagnostic map of templates matching a symptom or keyword
    Map {
        /// Search query (symptom, keyword, or tag — e.g. "segfault", "type mismatch", "phi")
        query: Vec<String>,
        /// Show all matches (default: top 12)
        #[arg(long)]
        all: bool,
    },

    /// Drain a queue file — run dynamically-queued templates against their targets
    Drain {
        /// Path to .culebra-queue.yaml
        #[arg(default_value = ".culebra-queue.yaml")]
        queue_file: String,
        /// Output format: text, json, sarif, markdown
        #[arg(long, default_value = "text")]
        format: String,
        /// Apply auto-fixes
        #[arg(long)]
        autofix: bool,
        /// Show fixes without applying (use with --autofix)
        #[arg(long)]
        dry_run: bool,
        /// Clear the queue file after draining
        #[arg(long)]
        clear: bool,
    },

    /// Triage: group findings by root cause, deduplicate, show actionable summary
    Triage {
        /// Path to .ll file
        file: String,
        /// Output format: text, json
        #[arg(long, default_value = "text")]
        format: String,
        /// One-line summary (minimal tokens for AI)
        #[arg(long)]
        brief: bool,
    },

    /// Compare per-function metrics between two IR files, flag significant drops
    Compare {
        /// First .ll file (reference/stage1)
        file_a: String,
        /// Second .ll file (test/stage2)
        file_b: String,
        /// Metric to compare: instructions, blocks, calls, pushes, allocas, stores, loads, rets
        #[arg(long, default_value = "instructions")]
        metric: String,
        /// Drop threshold (0.0-1.0) — flag functions with drops above this
        #[arg(long, default_value = "0.3")]
        threshold: f64,
    },

    /// Explain a finding: show matched IR in context with description + remediation
    Explain {
        /// Path to .ll file
        file: String,
        /// Template ID (e.g. "return-type-divergence")
        id: String,
        /// Filter to a specific function
        #[arg(long)]
        function: Option<String>,
    },

    /// Bisect: find divergent functions between stages, ranked by impact
    Bisect {
        /// First .ll file (reference/stage1)
        file_a: String,
        /// Second .ll file (test/stage2)
        file_b: String,
        /// Show top N functions
        #[arg(long, default_value = "15")]
        top: usize,
    },

    /// Verify a specific fix: re-scan for one finding to confirm it's gone
    Verify {
        /// Path to .ll file
        file: String,
        /// Template ID to check
        id: String,
        /// Filter to a specific function
        #[arg(long)]
        function: Option<String>,
    },

    /// Map a crash offset to a struct field — "what's at byte 0x20?"
    Crashmap {
        /// Path to .ll file (for struct type definitions)
        file: String,
        /// Byte offset to map (decimal or 0x hex)
        #[arg(long, default_value = "0", value_parser = parse_offset)]
        offset: usize,
        /// Struct name to map into (e.g. FnDefData, LowerState)
        #[arg(long, name = "struct")]
        struct_name: Option<String>,
    },

    /// Trace a variable through a function — show every load/store/phi/call
    Trace {
        /// Path to .ll file
        file: String,
        /// Function name (exact or substring)
        #[arg(long)]
        function: String,
        /// Variable name to trace (e.g. %state, %result)
        #[arg(long)]
        var: String,
    },

    /// Struct health check — find PHI zeroinit, type-pun, null loads
    Health {
        /// Path to .ll file
        file: String,
        /// Struct name to check (omit for all structs)
        #[arg(long, name = "struct")]
        struct_name: Option<String>,
    },

    /// Suggest fixes for a function based on its findings
    Suggest {
        /// Path to .ll file
        file: String,
        /// Function name
        #[arg(long)]
        function: String,
    },

    /// Save or diff a scan baseline — track progress across fix iterations
    Baseline {
        #[command(subcommand)]
        action: BaselineAction,
    },

    /// Assert a template fires (--expect) or doesn't fire (--reject) on a file
    LintTemplate {
        /// Path to .ll file
        file: String,
        /// Template ID to check
        id: String,
        /// Template MUST fire (fail if 0 matches)
        #[arg(long)]
        expect: bool,
        /// Template must NOT fire (fail if any matches)
        #[arg(long)]
        reject: bool,
    },

    /// List or show available scan templates
    Templates {
        #[command(subcommand)]
        action: TemplatesAction,
    },

    /// Run a multi-step scan workflow
    Workflow {
        /// Workflow ID to run
        workflow_id: String,
        /// Input files as key=value pairs (e.g. --input stage1=file.ll)
        #[arg(long = "input", value_parser = parse_kv)]
        inputs: Vec<(String, String)>,
        /// Output format: text, json, sarif, markdown
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum TemplatesAction {
    /// List all available templates
    List {
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
    /// Show details of a specific template
    Show {
        /// Template ID
        id: String,
    },
}

#[derive(Subcommand)]
enum BaselineAction {
    /// Save current scan findings as baseline
    Save {
        /// Path to .ll file to scan
        file: String,
        /// Output path for baseline file
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Compare current scan against saved baseline
    Diff {
        /// Path to .ll file to scan
        file: String,
        /// Path to baseline file
        #[arg(long, short)]
        baseline: Option<String>,
    },
}

fn parse_offset(s: &str) -> Result<usize, String> {
    if s.starts_with("0x") || s.starts_with("0X") {
        usize::from_str_radix(&s[2..], 16).map_err(|e| format!("invalid hex offset: {}", e))
    } else {
        s.parse::<usize>().map_err(|e| format!("invalid offset: {}", e))
    }
}

fn parse_kv(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        Err(format!("expected key=value, got '{}'", s))
    } else {
        Ok((parts[0].to_string(), parts[1].to_string()))
    }
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Strings { file, verbose, json } => commands::strings::run(&file, verbose, json),
        Commands::Audit { file, only, baseline } => {
            commands::audit::run(&file, only.as_deref(), baseline.as_deref())
        }
        Commands::Check { file } => commands::check::run(&file),
        Commands::PhiCheck { file, fix_cmd } => commands::phi_check::run(&file, &fix_cmd),
        Commands::Diff {
            file_a,
            file_b,
            verbose,
        } => commands::diff::run(&file_a, &file_b, verbose),
        Commands::Extract { file, func_name } => commands::extract::run(&file, &func_name),
        Commands::Table { file, top, sort_by } => commands::table::run(&file, top, &sort_by),
        Commands::Abi { file, header } => commands::abi::run(&file, header.as_deref()),
        Commands::Binary { file, ir, find } => {
            commands::binary::run(&file, ir.as_deref(), find.as_deref())
        }
        Commands::Run {
            compiler,
            source,
            expect,
            timeout,
            clang_flags,
            runtime,
        } => commands::run::run(
            &compiler,
            &source,
            expect.as_deref(),
            timeout,
            clang_flags.as_deref(),
            runtime.as_deref(),
        ),
        Commands::Test {
            config,
            filter,
            timeout,
        } => commands::test::run(&config, filter.as_deref(), timeout),
        Commands::Watch { patterns, dir, cmd } => commands::watch::run(&patterns, &dir, &cmd),
        Commands::Pipeline { config, timeout } => commands::pipeline::run(&config, timeout),
        Commands::Status { config } => commands::status::run(&config),
        Commands::Fixedpoint {
            compiler,
            source,
            max_iters,
            timeout,
            runtime,
        } => commands::fixedpoint::run(&compiler, &source, max_iters, timeout, runtime.as_deref()),
        Commands::Init => commands::init::run(),
        Commands::Map { query, all } => {
            let q = query.join(" ");
            commands::map::run(&q, all)
        }
        Commands::Drain {
            queue_file,
            format,
            autofix,
            dry_run,
            clear,
        } => commands::drain::run(&queue_file, &format, autofix, dry_run, clear),
        Commands::Scan {
            file,
            tags,
            severity,
            id,
            template,
            header,
            format,
            autofix,
            dry_run,
        } => {
            let tag_list: Vec<String> = tags
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let sev_list: Vec<String> = severity
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let id_list: Vec<String> = id.into_iter().collect();
            commands::scan::run(
                &file,
                &tag_list,
                &sev_list,
                &id_list,
                template.as_deref(),
                header.as_deref(),
                &format,
                autofix,
                dry_run,
            )
        }
        Commands::Triage { file, format, brief } => commands::triage::run(&file, &format, brief),
        Commands::Compare {
            file_a,
            file_b,
            metric,
            threshold,
        } => commands::compare::run(&file_a, &file_b, &metric, threshold),
        Commands::Explain { file, id, function } => {
            commands::explain::run(&file, &id, function.as_deref())
        }
        Commands::Bisect { file_a, file_b, top } => commands::bisect::run(&file_a, &file_b, top),
        Commands::Verify { file, id, function } => {
            commands::verify::run(&file, &id, function.as_deref())
        }
        Commands::Crashmap { file, offset, struct_name } => {
            commands::crashmap::run(&file, offset, struct_name.as_deref())
        }
        Commands::Trace { file, function, var } => {
            commands::trace::run(&file, &function, &var)
        }
        Commands::Health { file, struct_name } => {
            commands::health::run(&file, struct_name.as_deref())
        }
        Commands::Suggest { file, function } => {
            commands::suggest::run(&file, &function)
        }
        Commands::Baseline { action } => match action {
            BaselineAction::Save { file, output } => {
                commands::baseline::run_save(&file, output.as_deref())
            }
            BaselineAction::Diff { file, baseline } => {
                commands::baseline::run_diff(&file, baseline.as_deref())
            }
        },
        Commands::LintTemplate { file, id, expect, reject } => {
            commands::lint_template::run(&file, &id, expect, reject)
        }
        Commands::Templates { action } => match action {
            TemplatesAction::List { tags } => {
                let tag_list: Vec<String> = tags
                    .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default();
                commands::templates_cmd::run_list(&tag_list)
            }
            TemplatesAction::Show { id } => commands::templates_cmd::run_show(&id),
        },
        Commands::Workflow {
            workflow_id,
            inputs,
            format,
        } => {
            let input_map: HashMap<String, String> = inputs.into_iter().collect();
            commands::workflow::run(&workflow_id, &input_map, &format)
        }
    };

    std::process::exit(exit_code);
}
