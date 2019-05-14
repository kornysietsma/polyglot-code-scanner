#![warn(clippy::all)]
use failure::Error;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

use super::file_walker::NamedFileMetricCalculator;

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

pub struct LocMetricCalculator {}

impl NamedFileMetricCalculator for LocMetricCalculator {
    fn name(&self) -> String {
        "loc".to_string()
    }
    fn calculate_metrics(&self, path: &Path) -> Result<serde_json::Value, Error> {
        let stats = parse_file(path)?;
        Ok(serde_json::value::to_value(stats)
            .expect("Serializable object couldn't be serialized to JSON")) // TODO: maybe explicit error? Though this should be fatal
    }
}

#[cfg(test)]
mod test {
    use super::super::file_walker;
    use super::*;
    use serde_json::Value;

    #[test]
    fn can_get_loc_data_for_a_file() {
        let stats = parse_file(&Path::new("./tests/data/simple/parent.clj")).unwrap();
        assert_eq!(stats.code, 3);
        assert_eq!(stats.language, "Clojure");
    }

    #[test]
    fn can_walk_tree_and_extract_loc_data() {
        // this could really be an integration test
        let root = Path::new("./tests/data/simple/");

        let tree =
            file_walker::walk_directory(root, vec![Box::new(LocMetricCalculator {})]).unwrap();
        let json = serde_json::to_string_pretty(&tree).unwrap();
        let parsed_result: Value = serde_json::from_str(&json).unwrap();

        let expected =
            std::fs::read_to_string(Path::new("./tests/expected/simple_files_with_loc.json"))
                .unwrap();
        let parsed_expected: Value = serde_json::from_str(&expected).unwrap();

        assert_eq!(parsed_result, parsed_expected);
    }
}
