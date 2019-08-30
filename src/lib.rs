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
#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate derive_getters;

use failure::Error;
use std::io;
use std::path::PathBuf;

mod file_walker;
mod comment_filter;
mod flare;
mod git;
mod loc;
mod toxicity_indicator_calculator;


#[cfg(test)]
extern crate tempfile;
#[cfg(test)]
extern crate test_shared;
#[cfg(test)]
extern crate zip;

mod git_file_history;
mod git_logger;

use git::GitCalculator;
use git_logger::GitLogConfig;
use loc::LocCalculator;
use toxicity_indicator_calculator::ToxicityIndicatorCalculator;

// simple structure for config for any calculators -
pub struct CalculatorConfig {
    pub git_years: u64,
}

impl CalculatorConfig {
    pub fn default() -> Self {
        CalculatorConfig { git_years: 3 }
    }
}

// TODO: would love to somehow calculate this from the types (via macro?) but for now this is manual:
#[allow(dead_code)]
const TOXICITY_INDICATOR_CALCULATOR_NAMES: &[&str] = &["loc"];

pub fn named_toxicity_indicator_calculator(
    name: &str,
    config: &CalculatorConfig,
) -> Option<Box<dyn ToxicityIndicatorCalculator>> {
    match name {
        "loc" => Some(Box::new(LocCalculator {})),
        "git" => Some(Box::new(GitCalculator::new(
            GitLogConfig::default().since_years(config.git_years),
        ))),
        _ => None,
    }
}

pub fn run<W>(
    root: PathBuf,
    config: CalculatorConfig,
    toxicity_indicator_calculator_names: Vec<&str>,
    out: W,
) -> Result<(), Error>
where
    W: io::Write,
{
    let maybe_tics: Option<Vec<_>> = toxicity_indicator_calculator_names
        .iter()
        .map(|name| named_toxicity_indicator_calculator(name, &config))
        .collect();

    let mut tics = maybe_tics.expect("Some toxicity indicator calculator names don't exist!");

    let tree = file_walker::walk_directory(&root, &mut tics)?;

    serde_json::to_writer_pretty(out, &tree)?;
    Ok(())
}
