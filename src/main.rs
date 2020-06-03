#![warn(clippy::all)]

extern crate clap;
extern crate clap_log_flag;
extern crate clap_verbosity_flag;
extern crate failure_tools;
extern crate indicatif;
extern crate lati_explorer_server;
extern crate log;
extern crate structopt;
extern crate chrono;

use failure::{bail, format_err, Error};
use lati_scanner::CalculatorConfig;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt()]
/// Language-agnostic Toxicity Indicators scanner
///
/// Scans source code and generates indicators that may (or may not) show toxic code.
/// Ignores files specified by `.gitignore` or `.lati_ignore` files
/// See https://github.com/kornysietsma/lati-scanner for details
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
    /// Run a web server to display the lati-explorer visualisation
    /// Requires "-e" to indicate where to find the lati-explorer code
    /// Download the code from https://github.com/kornysietsma/lati-explorer if you want to see pretty visualisations
    server: bool,
    #[structopt(short = "p", long = "port", default_value = "3000")]
    /// The web server port
    port: u32,
    #[structopt(short = "e", long = "explorer", parse(from_os_str))]
    /// The location of the lati-explorer code, needed for server mode
    /// Download the code from https://github.com/kornysietsma/lati-explorer and then pass the file location of the "docs" subdirectory containing the "index.html" file
    explorer_location: Option<PathBuf>,
    #[structopt(long = "years", default_value = "3")]
    /// how many years of git history to parse - default only scan the last 3 years (from now, not git head)
    git_years: u64,
}

fn real_main() -> Result<(), Error> {
    let args = Cli::from_args();
    args.log.log_all(Some(args.verbose.log_level()))?;
    let root = args.root.unwrap_or_else(|| PathBuf::from("."));

    if args.output.is_some() && args.server {
        bail!("Can't select server mode and specify output file!");
    }
    let calculator_config = CalculatorConfig {
        git_years: args.git_years,
    };

    if args.server {
        if args.output.is_some() {
            bail!("Can't select server mode and specify output file!");
        }
        if args.explorer_location.is_none() {
            bail!("Server mode requires the 'explorer' path to contain a local copy of the lati-explorer files");
        }
        let explorer_location = args.explorer_location.unwrap();
        let index_file = explorer_location.join("docs/index.html");
        if !index_file.is_file() {
            return Err(format_err!("Server mode requires the 'explorer' path to contain a local copy of the lati-explorer files - can't find {}", index_file.to_str().unwrap()));
        }

        let mut out = Vec::new();
        lati_scanner::run(
            root,
            calculator_config,
            vec!["loc", "git", "indentation"],
            &mut out,
        )?;
        let json_output = String::from_utf8(out)?;
        let docs_dir = explorer_location.join("docs");
        lati_explorer_server::serve(&docs_dir, args.port, &json_output)?
    } else {
        let mut out: Box<dyn io::Write> = if let Some(output) = args.output {
            Box::new(File::create(output)?)
        } else {
            Box::new(io::stdout())
        };

        lati_scanner::run(
            root,
            calculator_config,
            vec!["loc", "git", "indentation"],
            &mut out,
        )?;
    }

    Ok(())
}

fn main() {
    failure_tools::ok_or_exit(real_main());
}
