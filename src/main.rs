#![warn(clippy::all)]

extern crate ignore;
extern crate tokei;
#[macro_use]
extern crate failure;
extern crate clap;
#[macro_use]
extern crate log;
extern crate clap_log_flag;
extern crate clap_verbosity_flag;

use std::path::PathBuf;
use structopt::StructOpt;
use failure::Error;
use std::fs::File;
use std::io::{self, Write};

mod file_walker;
mod flare;
mod loc;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cloc2flare",
    about = "Collects software stats as flare files for display in D3"
)]
struct Cli {
    #[structopt(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[structopt(flatten)]
    log: clap_log_flag::Log,
    /// Output file, stdout if not present
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: Option<PathBuf>,
    /// Root directory, current dir if not present
    #[structopt(parse(from_os_str))]
    root: Option<PathBuf>,
}

fn main() -> Result<(), Error> {
    let args = Cli::from_args();
    args.log.log_all(Some(args.verbose.log_level()))?;
    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    let tree = file_walker::walk_directory(&root, vec![Box::new(loc::LocMetricCalculator {})])?;

    let out: Box<Write> = if let Some(output) = args.output {
        Box::new(File::create(output)?)
    } else {
        Box::new(io::stdout())
    };

    serde_json::to_writer_pretty(out, &tree)?;
    // TODO: why doesn't the previous line work as return value? Maybe can fix this when error handling is improved
    Ok(())
}
