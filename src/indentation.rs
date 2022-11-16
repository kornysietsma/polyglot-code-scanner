use crate::flare::FlareTreeNode;
use crate::polyglot_data::IndicatorMetadata;

use super::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use anyhow::Error;
use serde::Serialize;

use content_inspector::{inspect, ContentType};

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use tokei::{Config, LanguageType};

use super::code_line_data::CodeLines;

use hdrhistogram::Histogram;

/// a struct representing file indentation data
#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct IndentationData {
    pub lines: u64,
    pub minimum: u64,
    pub maximum: u64,
    pub median: u64,
    pub stddev: f64,
    pub p75: u64,
    pub p90: u64,
    pub p99: u64,
    /// the sum of indentations - probably best measure according to [HGH08]
    pub sum: u64,
}

impl IndentationData {
    fn new(code_lines: CodeLines) -> Option<Self> {
        // we used to have this - reinstate if creating histogram for every file is too slow.  But who knows, file I/O might be much bigger.
        // lazy_static! {
        //     static ref HISTOGRAM: Mutex<Histogram<u64>> =
        //         Mutex::new(Histogram::<u64>::new(3).unwrap());
        // }
        let mut histogram = Histogram::<u64>::new(3).expect("Can't create histogram");
        let mut sum: u64 = 0;
        for line in code_lines.lines {
            if line.text > 0 {
                let indentation = line.spaces + line.tabs * 4;
                histogram
                    .record(indentation as u64)
                    .expect("Invalid histogram value!");
                sum += indentation as u64;
            }
        }
        if histogram.is_empty() {
            None
        } else {
            Some(IndentationData {
                lines: histogram.len(),
                minimum: histogram.low(),
                maximum: histogram.high(),
                median: histogram.value_at_quantile(0.5),
                stddev: histogram.stdev(),
                p75: histogram.value_at_quantile(0.75),
                p90: histogram.value_at_quantile(0.90),
                p99: histogram.value_at_quantile(0.99),
                sum,
            })
        }
    }
}

// TODO: remove duplication with loc.rs
const MAX_PEEK_SIZE: usize = 1024;

fn file_content_type(filename: &Path) -> Result<ContentType, Error> {
    let file = File::open(filename)?;
    let mut buffer: Vec<u8> = vec![];

    file.take(MAX_PEEK_SIZE as u64).read_to_end(&mut buffer)?;
    Ok(inspect(&buffer))
}

fn parse_file(filename: &Path) -> Result<Option<IndentationData>, Error> {
    let config = Config::default();
    let code_lines = match LanguageType::from_path(filename, &config) {
        Some(language) => {
            let report = language
                .parse(PathBuf::from(filename), &config)
                .map_err(|(error, _pathbuf)| error);
            CodeLines::from_stats(&report?.stats)
        }
        None => {
            if file_content_type(filename)? == ContentType::BINARY {
                return Ok(None);
            }
            debug!("Unknown language in {:?} - treating as text", filename);
            CodeLines::new(&PathBuf::from(filename))?
        }
    };
    Ok(IndentationData::new(code_lines))
}

#[derive(Debug)]
pub struct IndentationCalculator {}

impl ToxicityIndicatorCalculator for IndentationCalculator {
    fn name(&self) -> String {
        "indentation".to_string()
    }

    fn visit_node(&mut self, node: &mut FlareTreeNode, path: &Path) -> Result<(), Error> {
        if path.is_file() {
            let indentation = parse_file(path)?;
            node.indicators_mut().indentation = indentation;
        }
        Ok(())
    }

    fn apply_metadata(&self, _metadata: &mut IndicatorMetadata) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn can_get_indentation_data_for_a_file() {
        let indentation = parse_file(Path::new("./tests/data/simple/parent.clj"))
            .unwrap()
            .unwrap();
        assert_eq!(indentation.lines, 3);
        assert_eq!(indentation.p99, 2);
        assert_eq!(indentation.sum, 2);
    }

    #[test]
    fn unknown_files_are_treated_as_code() {
        let indentation = parse_file(Path::new("./tests/data/languages/foo.unknown"))
            .unwrap()
            .unwrap();
        assert_eq!(indentation.lines, 2);
        assert_eq!(indentation.p99, 2);
        assert_eq!(indentation.sum, 2);
    }

    #[test]
    fn pf_files_are_fortran_unit_tests() {
        let indentation = parse_file(Path::new("./tests/data/languages/pfunit_test.pf"))
            .unwrap()
            .unwrap();
        assert_eq!(indentation.lines, 13);
        assert_eq!(indentation.p99, 6);
        assert_eq!(indentation.sum, 39);
    }
}
