#![warn(clippy::all)]

extern crate ignore;
extern crate tokei;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate failure_tools;

use failure::Error;
use std::io;
use std::path::PathBuf;

mod file_walker;
mod flare;
mod loc;

use file_walker::NamedFileMetricCalculator;
use loc::LocMetricCalculator;

// TODO: would love to somehow calculate this from the types (via macro?) but for now this is manual:
#[allow(dead_code)]
const FILE_METRIC_CALCULATOR_NAMES: &[&str] = &["loc"];

pub fn named_file_metric_calculator(name: &str) -> Option<Box<dyn NamedFileMetricCalculator>> {
    match name {
        "loc" => Some(Box::new(LocMetricCalculator {})),
        _ => None,
    }
}

pub fn run<W>(root: PathBuf, file_metric_calculator_names: Vec<String>, out: W) -> Result<(), Error>
where
    W: io::Write,
{
    let maybe_fmcs: Option<Vec<_>> = file_metric_calculator_names
        .iter()
        .map(|name| named_file_metric_calculator(name))
        .collect();

    let mut fmcs = maybe_fmcs.expect("Some file metric calculator names don't exist!");

    let tree = file_walker::walk_directory(&root, &mut fmcs)?;

    serde_json::to_writer_pretty(out, &tree)?;
    Ok(())
}
