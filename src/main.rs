#![warn(clippy::all)]

extern crate clap;
extern crate clap_log_flag;
extern crate clap_verbosity_flag;
extern crate failure_tools;

use failure::Error;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt()]
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

fn real_main() -> Result<(), Error> {
    let args = Cli::from_args();
    args.log.log_all(Some(args.verbose.log_level()))?;
    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    let mut out: Box<dyn io::Write> = if let Some(output) = args.output {
        Box::new(File::create(output)?)
    } else {
        Box::new(io::stdout())
    };

    lati_scanner::run(root, vec!["loc".to_string()], &mut out)?;

    Ok(())
}

fn main() {
    failure_tools::ok_or_exit(real_main());
}
