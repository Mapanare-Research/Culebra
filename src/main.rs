mod commands;
mod ir;

use clap::{Parser, Subcommand};

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

    /// Inspect string constants and symbols in a compiled binary
    Binary {
        /// Path to ELF/PE binary or .o file
        file: String,
        /// Verify a specific string exists at correct address
        #[arg(long)]
        find: Option<String>,
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

    /// Initialize a culebra.toml config for a compiler project
    Init,
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
        Commands::Binary { file, find } => commands::binary::run(&file, find.as_deref()),
        Commands::Pipeline { config, timeout } => commands::pipeline::run(&config, timeout),
        Commands::Status { config } => commands::status::run(&config),
        Commands::Init => commands::init::run(),
    };

    std::process::exit(exit_code);
}
