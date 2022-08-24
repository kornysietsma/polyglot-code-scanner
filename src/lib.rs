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
use polyglot_data::PolyglotData;
use postprocessing::postprocess_tree;
use std::io;
use std::path::PathBuf;

mod code_line_data;
// pub mod coupling;
mod file_walker;
// public so main.rs can access structures TODO: can this be done better? expose here just what main needs?
pub mod coupling;
mod flare;
mod git;
mod git_file_future;
mod git_user_dictionary;
mod indentation;
mod loc;
mod polyglot_data;
mod postprocessing;
mod toxicity_indicator_calculator;

#[cfg(test)]
extern crate tempfile;
#[cfg(test)]
extern crate test_shared;
#[cfg(test)]
extern crate zip;

mod git_file_history;
mod git_logger;

use crate::coupling::CouplingConfig;
use git::GitCalculator;
use git_logger::GitLogConfig;
use indentation::IndentationCalculator;
use loc::LocCalculator;
use toxicity_indicator_calculator::ToxicityIndicatorCalculator;

// general config for the scanner and calculators - could be split if it grows too far
pub struct ScannerConfig {
    pub git_years: Option<u64>,
    pub detailed: bool,
    pub follow_symlinks: bool,
    pub name: String,
    pub data_id: Option<String>,
}

impl ScannerConfig {
    pub fn default(name: &str) -> Self {
        ScannerConfig {
            git_years: None,
            detailed: false,
            follow_symlinks: false,
            name: name.to_owned(),
            data_id: None,
        }
    }
}

pub fn named_toxicity_indicator_calculator(
    name: &str,
    config: &ScannerConfig,
) -> Option<Box<dyn ToxicityIndicatorCalculator>> {
    match name {
        "loc" => Some(Box::new(LocCalculator {})),
        "git" => Some(Box::new(GitCalculator::new(
            GitLogConfig::default()
                .include_merges(true)
                .since_years(config.git_years),
        ))),
        "indentation" => Some(Box::new(IndentationCalculator {})),
        _ => None,
    }
}

pub fn run<W>(
    root: PathBuf,
    config: ScannerConfig,
    coupling_config: Option<CouplingConfig>,
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

    let mut polyglot_data = file_walker::walk_directory(
        &root,
        &config.name,
        config.data_id.as_deref(),
        config.follow_symlinks,
        &mut tics,
    )?;

    for tic in tics {
        if let Some(metadata) = tic.metadata()? {
            polyglot_data.add_metadata(tic.name(), metadata);
        }
    }

    if let Some(cc) = coupling_config {
        // TODO: fix this to take the data
        coupling::gather_coupling(&mut polyglot_data, cc)?;
    }

    // TODO: fix this to take the data
    postprocess_tree(polyglot_data.tree_mut(), config)?;

    serde_json::to_writer(out, &polyglot_data)?;
    Ok(())
}
