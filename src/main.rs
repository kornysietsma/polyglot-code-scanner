#![warn(clippy::all)]

extern crate clap;
extern crate clap_log_flag;
extern crate clap_verbosity_flag;
extern crate failure_tools;
extern crate lati_explorer_server;

use failure::bail;
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
    /// Output file, stdout if not present, or not used if sending to web server
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: Option<PathBuf>,
    /// Root directory, current dir if not present
    #[structopt(parse(from_os_str))]
    root: Option<PathBuf>,
    #[structopt(short = "s", long = "server")]
    server: bool,
}

fn real_main() -> Result<(), Error> {
    let args = Cli::from_args();
    args.log.log_all(Some(args.verbose.log_level()))?;
    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    if args.output.is_some() && args.server {
        bail!("Can't select server mode and specify output file!");
    }

    if args.server {
        let mut out = Vec::new();
        lati_scanner::run(root, vec!["loc", "git"], &mut out)?;
        let json_output = String::from_utf8(out)?;
        lati_explorer_server::serve(&PathBuf::from("../lati-explorer/docs"), &json_output)?
    } else {
        let mut out: Box<dyn io::Write> = if let Some(output) = args.output {
            Box::new(File::create(output)?)
        } else {
            Box::new(io::stdout())
        };

        lati_scanner::run(root, vec!["loc", "git"], &mut out)?;
    }

    Ok(())
}

fn main() {
    failure_tools::ok_or_exit(real_main());
}
