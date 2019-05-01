#![warn(clippy::all)]
#![allow(dead_code)]
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::path::Path;
use std::path::PathBuf;
use tokei::Stats;

use tokei::{Config, LanguageType, Languages};

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
    println!("language: {:?}", language);
    let stats = language.parse(PathBuf::from(filename), &config).unwrap(); // TODO - errors again
    LanguageLocData {
        blanks: stats.blanks,
        code: stats.code,
        comments: stats.comments,
        lines: stats.lines,
        language: language.name().to_string(),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use regex::Regex;
    use serde_json::json;
    use serde_json::Value;

    #[test]
    fn can_get_loc_data_for_a_file() {
        let stats = parse_file(&Path::new("./tests/data/simple/parent.clj"));
        println!("stats: {:?}", stats);
        assert_eq!(stats.code, 3);
    }
    // need to get the language itself, and all the other stats
}
