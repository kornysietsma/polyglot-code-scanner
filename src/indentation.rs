#![warn(clippy::all)]
#![allow(clippy::cast_lossless)]
use super::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use failure::Error;
use serde::Serialize;

use content_inspector::{inspect, ContentType};

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use tokei::{Config, LanguageType};

use super::code_line_data::CodeLines;

use histogram::Histogram;

/// a struct representing file indentation data
#[derive(Debug, PartialEq, Serialize)]
struct IndentationData {
    pub lines: u64,
    pub minimum: u64,
    pub maximum: u64,
    pub median: u64,
    pub stddev: u64,
    pub p75: u64,
    pub p90: u64,
    pub p99: u64,
}

impl IndentationData {
    fn new(codeLines: CodeLines) -> Option<Self> {
        let mut histogram = Histogram::new();
        for line in codeLines.lines {
            if line.text > 0 {
                let indentation = line.spaces + line.tabs * 4;
                histogram.increment(indentation as u64).unwrap();
            }
        }
        if histogram.entries() == 0 {
            None
        } else {
            Some(IndentationData {
                lines: histogram.entries(),
                minimum: histogram.minimum().unwrap(),
                maximum: histogram.maximum().unwrap(),
                median: histogram.percentile(50.0).unwrap(),
                stddev: histogram.stddev().unwrap(),
                p75: histogram.percentile(75.0).unwrap(),
                p90: histogram.percentile(90.0).unwrap(),
                p99: histogram.percentile(99.0).unwrap(),
            })
        }
    }
}

fn safe_extension(filename: &Path) -> String {
    match filename.extension() {
        Some(ext) => ext.to_string_lossy().to_string(),
        None => "no_extension".to_owned(),
    }
}

// TODO: remove duplication with loc.rs
const MAX_PEEK_SIZE: usize = 1024;

fn file_content_type(filename: &Path) -> Result<ContentType, Error> {
    let file = File::open(&filename)?;
    let mut buffer: Vec<u8> = vec![];

    file.take(MAX_PEEK_SIZE as u64).read_to_end(&mut buffer)?;
    Ok(inspect(&buffer))
}

fn parse_file(filename: &Path) -> Result<Option<IndentationData>, Error> {
    let config = Config::default();
    let mut language_name = None;
    let language = match LanguageType::from_path(filename, &config) {
        Some(language) => language,
        None => {
            language_name = Some(safe_extension(filename));
            if file_content_type(filename)? == ContentType::BINARY {
                return Ok(None);
            }
            LanguageType::Text
        }
    };

    let code_lines = language.parse::<CodeLines>(PathBuf::from(filename), &config);

    match code_lines {
        Ok(code_lines) => Ok(IndentationData::new(code_lines)),
        Err((error, _pathbuf)) => Err(Error::from(error)),
    }
}

#[derive(Debug)]
pub struct IndentationCalculator {}

impl ToxicityIndicatorCalculator for IndentationCalculator {
    fn name(&self) -> String {
        "indentation".to_string()
    }

    fn calculate(&mut self, path: &Path) -> Result<Option<serde_json::Value>, Error> {
        if path.is_file() {
            let indentation = parse_file(path)?;
            Ok(Some(serde_json::value::to_value(indentation).expect(
                "Serializable object couldn't be serialized to JSON",
            ))) // TODO: maybe explicit error? Though this should be fatal
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn can_get_indentation_data_for_a_file() {
        let indentation = parse_file(&Path::new("./tests/data/simple/parent.clj"))
            .unwrap()
            .unwrap();
        assert_eq!(indentation.lines, 3);
        assert_eq!(indentation.p99, 2);
    }
}
