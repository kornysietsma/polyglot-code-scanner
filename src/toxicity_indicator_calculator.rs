#![warn(clippy::all)]

use anyhow::Error;
use std::path::Path;

use crate::{flare::FlareTreeNode, polyglot_data::IndicatorMetadata};

/// Wrapper for the logic that calculates toxicity indicators
pub trait ToxicityIndicatorCalculator: std::fmt::Debug {
    fn name(&self) -> String;
    fn visit_node(&mut self, node: &mut FlareTreeNode, path: &Path) -> Result<(), Error>;
    /// root-level metadata - output after all files added
    fn apply_metadata(&self, metadata: &mut IndicatorMetadata) -> Result<(), Error>;
}
