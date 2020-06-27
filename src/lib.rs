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
extern crate serde;

use failure::Error;
use std::io;
use std::path::PathBuf;

mod code_line_data;
mod coupling;
mod file_walker;
mod flare;
mod git;
mod git_user_dictionary;
mod indentation;
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
use indentation::IndentationCalculator;
use loc::LocCalculator;
use toxicity_indicator_calculator::ToxicityIndicatorCalculator;

// simple structure for config for any calculators -
pub struct CalculatorConfig {
    pub git_years: u64,
    pub detailed: bool,
}

impl CalculatorConfig {
    pub fn default() -> Self {
        CalculatorConfig {
            git_years: 3,
            detailed: false,
        }
    }
}

pub fn named_toxicity_indicator_calculator(
    name: &str,
    config: &CalculatorConfig,
) -> Option<Box<dyn ToxicityIndicatorCalculator>> {
    match name {
        "loc" => Some(Box::new(LocCalculator {})),
        "git" => Some(Box::new(GitCalculator::new(
            GitLogConfig::default().since_years(config.git_years),
            config.detailed,
        ))),
        "indentation" => Some(Box::new(IndentationCalculator {})),
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

    let mut tree = file_walker::walk_directory(&root, &mut tics)?;

    for tic in tics {
        if let Some(metadata) = tic.metadata()? {
            tree.add_data(tic.name() + "_meta", metadata);
        }
    }

    coupling::gather_coupling(&mut tree, coupling::CouplingConfig::default())?; // roughly 3 months

    // TODO: add pretty / non-pretty option to commandline?
    // for big trees, pretty is a lot bigger. You can always use `jq` to view as pretty file.
    serde_json::to_writer(out, &tree)?;
    Ok(())
}
