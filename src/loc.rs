#![warn(clippy::all)]
#![allow(dead_code)]
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

fn parse_file(filename: &Path) -> LanguageLocData {
    let config = Config::default();
    let language = LanguageType::from_path(filename, &config).unwrap(); // TODO - error handle!
    let stats = language.parse(PathBuf::from(filename), &config).unwrap(); // TODO - errors again
    LanguageLocData {
        blanks: stats.blanks,
        code: stats.code,
        comments: stats.comments,
        lines: stats.lines,
        language: language.name().to_string(),
    }
}

pub struct LocMetricCalculator {}

impl NamedFileMetricCalculator for LocMetricCalculator {
    fn name(&self) -> String {
        "loc".to_string()
    }
    fn calculate_metrics(&self, path: &Path) -> serde_json::Value {
        let stats = parse_file(path);
        serde_json::value::to_value(stats)
            .expect("Serializable object couldn't be serialized to JSON")
    }
}

#[cfg(test)]
mod test {
    use super::super::file_walker;
    use super::*;
    use serde_json::Value;

    #[test]
    fn can_get_loc_data_for_a_file() {
        let stats = parse_file(&Path::new("./tests/data/simple/parent.clj"));
        println!("stats: {:?}", stats);
        assert_eq!(stats.code, 3);
        assert_eq!(stats.language, "Clojure");
    }
    // need to get the language itself, and all the other stats

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
