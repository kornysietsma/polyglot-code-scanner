#![warn(clippy::all)]
use super::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use failure::Error;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

use tokei::{Config, LanguageType};

/// a struct representing tokei language data - based on tokei::Stats and tokei::Languages::name
#[derive(Debug, PartialEq, Serialize)]
struct LanguageLocData {
    /// Canonical language name
    pub language: String,
    /// Number of blank lines within the file.
    pub blanks: usize,
    /// Number of lines of code within the file.
    pub code: usize,
    /// Number of comments within the file. (_includes both multi line, and
    /// single line comments_)
    pub comments: usize,
    /// Total number of lines within the file.
    pub lines: usize,
}

fn parse_file(filename: &Path) -> Result<LanguageLocData, Error> {
    let config = Config::default();
    let language = LanguageType::from_path(filename, &config)
        .ok_or_else(|| format_err!("No language for file {:?}", filename))?; // TODO: maybe a real error type?
    let stats = language.parse(PathBuf::from(filename), &config);

    match stats {
        Ok(stats) => Ok(LanguageLocData {
            blanks: stats.blanks,
            code: stats.code,
            comments: stats.comments,
            lines: stats.lines,
            language: language.name().to_string(),
        }),
        Err((error, _pathbuf)) => Err(Error::from(error)),
    }
}

#[derive(Debug)]
pub struct LocCalculator {}

impl ToxicityIndicatorCalculator for LocCalculator {
    fn name(&self) -> String {
        "loc".to_string()
    }

    fn description(&self) -> String {
        "Lines of Code".to_string()
    }

    fn calculate(&mut self, path: &Path) -> Result<Option<serde_json::Value>, Error> {
        if path.is_file() {
            let stats = parse_file(path)?;
            Ok(Some(serde_json::value::to_value(stats).expect(
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
    fn can_get_loc_data_for_a_file() {
        let stats = parse_file(&Path::new("./tests/data/simple/parent.clj")).unwrap();
        assert_eq!(stats.code, 3);
        assert_eq!(stats.language, "Clojure");
    }
}
