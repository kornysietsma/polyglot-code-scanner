#![forbid(unsafe_code)]
#![warn(clippy::all)]
#![warn(rust_2018_idioms)]
#![warn(clippy::pedantic)]
// pedantic is just a bit too keen for me! But still useful.
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::redundant_else)]
#![allow(clippy::single_match_else)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate derive_getters;

use anyhow::Error;
use file_stats::FileStatsCalculator;
use postprocessing::postprocess_tree;
use serde::Serialize;
use std::io;
use std::path::Path;

mod code_line_data;
// pub mod coupling;
mod file_walker;
// public so main.rs can access structures TODO: can this be done better? expose here just what main needs?
pub mod coupling;
mod file_stats;
mod flare;
mod git;
mod git_file_future;
mod git_user_dictionary;
mod indentation;
mod loc;
mod polyglot_data;
mod postprocessing;
mod toxicity_indicator_calculator;

mod git_file_history;
mod git_logger;

use crate::coupling::CouplingConfig;
use git::GitCalculator;
use git_logger::GitLogConfig;
use indentation::IndentationCalculator;
use loc::LocCalculator;
use toxicity_indicator_calculator::ToxicityIndicatorCalculator;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone, Serialize)]
pub struct FeatureFlags {
    pub git: bool,
    pub coupling: bool,
    pub git_details: bool,
    pub file_stats: bool,
}

// general config for the scanner and calculators - could be split if it grows too far
pub struct ScannerConfig {
    pub git_years: Option<u64>,
    pub follow_symlinks: bool,
    pub name: String,
    pub data_id: Option<String>,
    pub features: FeatureFlags,
}

impl ScannerConfig {
    #[must_use]
    pub fn default(name: &str) -> Self {
        ScannerConfig {
            git_years: None,
            follow_symlinks: false,
            name: name.to_owned(),
            data_id: None,
            features: FeatureFlags::default(),
        }
    }
}

#[must_use]
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
        "file_stats" => Some(Box::new(FileStatsCalculator {})),
        _ => None,
    }
}

pub fn run<W>(
    root: &Path,
    config: &ScannerConfig,
    coupling_config: Option<CouplingConfig>,
    toxicity_indicator_calculator_names: &[&str],
    out: W,
) -> Result<(), Error>
where
    W: io::Write,
{
    if toxicity_indicator_calculator_names.contains(&"git") && !config.features.git {
        bail!("Logic error - using git calculator when git is disabled!");
    }
    if toxicity_indicator_calculator_names.contains(&"file_stats") && !config.features.file_stats {
        bail!("Logic error - using file_stats calculator when file_stats is disabled!");
    }
    let maybe_tics: Option<Vec<_>> = toxicity_indicator_calculator_names
        .iter()
        .map(|name| named_toxicity_indicator_calculator(name, config))
        .collect();

    let mut tics = maybe_tics.expect("Some toxicity indicator calculator names don't exist!");

    info!("Walking directory tree");
    let mut polyglot_data = file_walker::walk_directory(
        root,
        &config.name,
        config.data_id.as_deref(),
        config.follow_symlinks,
        &mut tics,
        &config.features,
    )?;

    info!("adding metadata");
    for tic in tics {
        tic.apply_metadata(polyglot_data.metadata())?;
    }

    if let Some(cc) = coupling_config {
        // TODO: fix this to take the data
        info!("gathering coupling");
        coupling::gather_coupling(&mut polyglot_data, cc)?;
    }

    info!("postprocessing tree");
    // TODO: fix this to take the data
    postprocess_tree(polyglot_data.tree_mut(), config)?;

    info!("saving as JSON");
    serde_json::to_writer(out, &polyglot_data)?;
    Ok(())
}
