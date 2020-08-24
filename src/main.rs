#![warn(clippy::all)]

extern crate chrono;
extern crate clap;
extern crate clap_verbosity_flag;
extern crate failure_tools;
extern crate fern;
extern crate indicatif;
extern crate log;
extern crate structopt;

use failure::Error;
use polyglot_code_scanner::coupling::CouplingConfig;
use polyglot_code_scanner::CalculatorConfig;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt()]
/// Polyglot Code Scanner
///
/// Scans source code and generates indicators that may (or may not) show toxic code.
/// Ignores files specified by `.gitignore` or `.polyglot_code_scanner_ignore` files
/// See https://polyglot.korny.info for details
struct Cli {
    #[structopt(
        short = "v",
        long = "verbose",
        parse(from_occurrences),
        multiple = true
    )]
    /// Logging verbosity, v = error, vv = warn, vvv = info (default), vvvv = debug, vvvvv = trace
    verbose: u64,
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
    #[structopt(short = "c", long = "coupling")]
    /// include temporal coupling data
    coupling: bool,
    #[structopt(long = "coupling-bucket-days", default_value = "91")]
    /// how many days are reviewed for one "bucket" of temporal coupling
    bucket_days: u64,
    #[structopt(long = "coupling-source-days", default_value = "10")]
    /// how many days should a file change in a bucket for it to generate coupling stats
    min_source_days: u64,
    #[structopt(long = "coupling-min-ratio", default_value = "0.25")]
    /// what is the minimum ratio of (other file changes)/(this file changes) to include a file in coupling stats
    min_coupling_ratio: f64,
}

// very basic logging - just so I can have a nice default, and hide verbose tokei logs
fn setup_logging(verbosity: u64) -> Result<(), fern::InitError> {
    let mut base_config = fern::Dispatch::new();

    base_config = match verbosity {
        0 => base_config.level(log::LevelFilter::Info),
        1 => base_config.level(log::LevelFilter::Error),
        2 => base_config.level(log::LevelFilter::Warn),
        3 => base_config.level(log::LevelFilter::Info),
        4 => base_config.level(log::LevelFilter::Debug),
        _5_or_more => base_config.level(log::LevelFilter::Trace),
    };

    // Tokei warns whenever we scan a language type we don't know - but I catch that error!
    base_config = base_config.level_for("tokei::language::language_type", log::LevelFilter::Error);

    let stdout_config = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%H:%M"),
                record.target(),
                record.level(),
                message
            ))
        })
        .chain(io::stderr());

    base_config.chain(stdout_config).apply()?;

    Ok(())
}

fn real_main() -> Result<(), Error> {
    let args = Cli::from_args();

    setup_logging(args.verbose)?;

    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    let calculator_config = CalculatorConfig {
        git_years: args.git_years,
        detailed: !args.no_detailed_git,
    };

    let coupling_config = if args.coupling {
        Some(CouplingConfig::new(
            args.bucket_days,
            args.min_source_days,
            args.min_coupling_ratio,
        ))
    } else {
        None
    };

    let mut out: Box<dyn io::Write> = if let Some(output) = args.output {
        Box::new(File::create(output)?)
    } else {
        Box::new(io::stdout())
    };

    polyglot_code_scanner::run(
        root,
        calculator_config,
        coupling_config,
        vec!["loc", "git", "indentation"],
        &mut out,
    )?;

    Ok(())
}

fn main() {
    failure_tools::ok_or_exit(real_main());
}
