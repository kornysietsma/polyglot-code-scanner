#![warn(clippy::all)]

extern crate chrono;
extern crate clap;
extern crate clap_log_flag;
extern crate clap_verbosity_flag;
extern crate failure_tools;
extern crate indicatif;
extern crate log;
extern crate structopt;

use failure::{bail, format_err, Error};
use polyglot_code_scanner::CalculatorConfig;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt()]
/// Language-agnostic Toxicity Indicators scanner
///
/// Scans source code and generates indicators that may (or may not) show toxic code.
/// Ignores files specified by `.gitignore` or `.polyglot_code_scanner_ignore` files
/// See https://github.com/kornysietsma/polyglot-code-scanner for details
struct Cli {
    #[structopt(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[structopt(flatten)]
    log: clap_log_flag::Log,
    /// Output file, stdout if not present, or not used if sending to web server
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: Option<PathBuf>,
    /// Root directory, current dir if not present
    #[structopt(parse(from_os_str))]
    root: Option<PathBuf>,
    #[structopt(long = "years", default_value = "3")]
    /// how many years of git history to parse - default only scan the last 3 years (from now, not git head)
    git_years: u64,
    #[structopt(long = "no-detailed-git")]
    /// Don't include detailed git information - output may be big!
    no_detailed_git: bool,
}

fn real_main() -> Result<(), Error> {
    let args = Cli::from_args();
    args.log.log_all(Some(args.verbose.log_level()))?;
    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    let calculator_config = CalculatorConfig {
        git_years: args.git_years,
        detailed: !args.no_detailed_git,
    };

    let mut out: Box<dyn io::Write> = if let Some(output) = args.output {
        Box::new(File::create(output)?)
    } else {
        Box::new(io::stdout())
    };

    polyglot_code_scanner::run(
        root,
        calculator_config,
        vec!["loc", "git", "indentation"],
        &mut out,
    )?;

    Ok(())
}

fn main() {
    failure_tools::ok_or_exit(real_main());
}
