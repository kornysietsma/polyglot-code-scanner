#![forbid(unsafe_code)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(rust_2018_idioms)]

use anyhow::Error;
use clap::{CommandFactory, ErrorKind, Parser};
use polyglot_code_scanner::coupling::CouplingConfig;
use polyglot_code_scanner::{FeatureFlags, ScannerConfig};
use std::fs::File;
use std::io;
use std::path::PathBuf;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[clap(author, version)]
/// Polyglot Code Scanner
///
/// Scans source code and generates indicators that may (or may not) show toxic code.
/// Ignores files specified by `.gitignore` or `.polyglot_code_scanner_ignore` files
/// See <https://polyglot.korny.info> for details
struct Cli {
    #[clap(
        short = 'v',
        long = "verbose",
        action = clap::ArgAction::Count
    )]
    /// Logging verbosity, v = error, vv = warn, vvv = info (default), vvvv = debug, vvvvv = trace
    verbose: u8,
    /// Output file, stdout if not present, or not used if sending to web server
    #[clap(short = 'o', long = "output", parse(from_os_str))]
    output: Option<PathBuf>,
    /// project name - identifies the selected data for display and state storage
    #[clap(value_parser, short = 'n', long = "name")]
    name: String,

    /// data file ID - used to identify unique data files for browser storage, generates a UUID if not specified
    #[clap(value_parser, long = "id")]
    id: Option<String>,
    /// Root directory, current dir if not present
    #[clap(parse(from_os_str))]
    root: Option<PathBuf>,

    // global indicator flags
    #[clap(value_parser, long = "no-git")]
    /// Do not scan for git repositories
    no_git: bool,
    #[clap(value_parser, short = 'c', long = "coupling")]
    /// include temporal coupling data
    coupling: bool,
    #[clap(value_parser, long = "no-detailed-git")]
    /// Don't include detailed git information - output may be big!
    no_detailed_git: bool,
    #[clap(value_parser, long = "no-file-stats")]
    /// Do not scan for file stats - mainly an option as this is very hard to unit test
    no_file_stats: bool,

    #[clap(value_parser, long = "years", default_value = "3")]
    /// how many years of git history to parse - default only scan the last 3 years (from now, not git head)
    git_years: u64,
    #[clap(value_parser, long = "follow-symlinks")]
    /// Follow symbolic links when traversing directories
    follow_symlinks: bool,
    #[clap(value_parser, long = "coupling-bucket-days", default_value = "91")]
    /// Number of days in a single "bucket" of coupling activity
    bucket_days: u64,
    #[clap(value_parser, long = "coupling-min-bursts", default_value = "10")]
    /// If a file has fewer bursts of change than this in a bucket, don't measure coupling from it
    min_activity_bursts: u64,
    #[clap(value_parser, long = "coupling-min-ratio", default_value = "0.8")]
    /// The minimum ratio of (other file changes)/(this file changes) to include a file in coupling stats
    min_coupling_ratio: f64,
    #[clap(
        value_parser,
        long = "coupling-min-activity-gap-minutes",
        default_value = "60"
    )]
    /// what is the minimum gap between activities in a burst? a sequence of commits with no gaps this long is treated as one burst
    min_activity_gap_minutes: u64,
    #[clap(
        value_parser,
        long = "coupling-time-overlap-minutes",
        default_value = "60"
    )]
    /// how far before/after an activity burst is included for coupling? e.g. if I commit Foo.c at 1am, and Bar.c at 2am, they are coupled if an overlap of 60 minutes or longer is specified
    min_overlap_minutes: u64,
    #[clap(value_parser, long = "coupling-min-distance", default_value = "3")]
    /// The minimum distance between nodes to include in coupling
    /// 0 is all, 1 is siblings, 2 is cousins and so on.
    /// so if you set this to 3, cousins "foo/src/a.rs" and "foo/test/a_test.rs" won't be counted as their distance is 2
    coupling_min_distance: usize,
    #[clap(value_parser, long = "coupling-max-common-roots")]
    /// The maximum number of common ancestors to include in coupling
    /// e.g. "foo/src/controller/a.c" and "foo/src/service/b.c" have two common ancestors, if you
    /// set this value to 3 they won't show as coupled.
    coupling_max_common_roots: Option<usize>,
}

// very basic logging - just so I can have a nice default, and hide verbose tokei logs
fn setup_logging(verbosity: u8) -> Result<(), fern::InitError> {
    let mut base_config = fern::Dispatch::new();

    base_config = match verbosity {
        0 | 3 => base_config.level(log::LevelFilter::Info),
        1 => base_config.level(log::LevelFilter::Error),
        2 => base_config.level(log::LevelFilter::Warn),
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
            ));
        })
        .chain(io::stderr());

    base_config.chain(stdout_config).apply()?;

    Ok(())
}

fn custom_validation_conflict(message: &str) {
    let mut cmd = Cli::command();
    cmd.error(ErrorKind::ArgumentConflict, message).exit()
}

fn main() -> Result<(), Error> {
    let args = Cli::from_args();

    // custom validation - easier than trying to wrangle clap to do this!
    if args.no_git {
        if args.coupling {
            custom_validation_conflict("Can't enable coupling when git is disabled!");
        }
        if args.no_detailed_git {
            custom_validation_conflict("Can't specify no_detailed_git when git is disabled!");
        }
    }

    setup_logging(args.verbose)?;

    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    let features = FeatureFlags {
        git: !args.no_git,
        coupling: args.coupling,
        git_details: !(args.no_detailed_git || args.no_git),
        file_stats: !args.no_file_stats,
    };

    let scanner_config = ScannerConfig {
        git_years: Some(args.git_years),
        data_id: args.id,
        name: args.name,
        follow_symlinks: args.follow_symlinks,
        features,
    };

    let coupling_config = if args.coupling {
        Some(CouplingConfig::new(
            args.bucket_days,
            args.min_activity_bursts,
            args.min_coupling_ratio,
            args.min_activity_gap_minutes * 60,
            args.min_overlap_minutes * 60,
            args.coupling_min_distance,
            args.coupling_max_common_roots,
        ))
    } else {
        None
    };

    let mut out: Box<dyn io::Write> = if let Some(output) = args.output {
        Box::new(File::create(output)?)
    } else {
        Box::new(io::stdout())
    };

    let mut calculator_names: Vec<&str> = vec!["loc", "indentation"];
    if !args.no_git {
        calculator_names.push("git");
    }
    if !args.no_file_stats {
        calculator_names.push("file_stats");
    }

    polyglot_code_scanner::run(
        &root,
        &scanner_config,
        coupling_config,
        &calculator_names,
        &mut out,
    )?;

    Ok(())
}
