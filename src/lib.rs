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

mod flare;
// TODO: these should not be pub once main no longer uses them
pub mod file_walker;
pub mod loc;

use file_walker::NamedFileMetricCalculator;

pub fn run<W>(
    root: PathBuf,
    file_metric_calculators: Vec<Box<dyn NamedFileMetricCalculator>>,
    out: W,
) -> Result<(), Error>
where
    W: io::Write,
{
    let tree = file_walker::walk_directory(&root, file_metric_calculators)?;

    serde_json::to_writer_pretty(out, &tree)?;
    Ok(())
}
