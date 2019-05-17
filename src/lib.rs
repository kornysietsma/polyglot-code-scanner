#![warn(clippy::all)]

extern crate ignore;
extern crate tokei;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate failure_tools;
#[macro_use]
extern crate lazy_static;

use failure::Error;
use std::io;
use std::path::PathBuf;

mod file_walker;
mod flare;
mod loc;

use file_walker::NamedFileMetricCalculator;

lazy_static! {
    static ref FILE_METRIC_CALCULATORS: Vec<&'static NamedFileMetricCalculator> =
        { vec![&loc::LocMetricCalculator {}] };
}

pub fn file_metric_calculator_names() -> Vec<(String, String)> {
    FILE_METRIC_CALCULATORS
        .iter()
        .map(|fmc| (fmc.name(), fmc.description()))
        .collect()
}
fn named_file_metric_calculator(name: &str) -> Option<&'static NamedFileMetricCalculator> {
    let fmc = FILE_METRIC_CALCULATORS
        .iter()
        .find(|fmc| fmc.name() == name)?;
    Some(*fmc)
}

pub fn run<W>(root: PathBuf, file_metric_calculator_names: Vec<String>, out: W) -> Result<(), Error>
where
    W: io::Write,
{
    let maybe_fmcs: Option<Vec<_>> = file_metric_calculator_names
        .iter()
        .map(|name| named_file_metric_calculator(name))
        .collect();

    let fmcs = maybe_fmcs.expect("Some file metric calculator names don't exist!");

    let tree = file_walker::walk_directory(&root, fmcs)?;

    serde_json::to_writer_pretty(out, &tree)?;
    Ok(())
}
