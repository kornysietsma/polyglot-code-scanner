#![warn(clippy::all)]

use failure::Error;
use serde_json::Value;
use std::path::Path;

/// Wrapper for the logic that calculates toxicity indicators
pub trait ToxicityIndicatorCalculator: std::fmt::Debug {
    fn name(&self) -> String;
    fn calculate(&mut self, path: &Path) -> Result<Option<Value>, Error>;
    /// root-level metadata - output after all files added
    fn metadata(&self) -> Result<Option<Value>, Error>;
}
