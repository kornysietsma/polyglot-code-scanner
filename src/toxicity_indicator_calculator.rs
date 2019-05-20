#![warn(clippy::all)]

use failure::Error;
use std::path::Path;
use serde_json::Value;

/// Wrapper for the logic that calculates toxicity indicators
pub trait ToxicityIndicatorCalculator: Sync + std::fmt::Debug {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn calculate(&mut self, path: &Path) -> Result<Value, Error>;
}