#![warn(clippy::all)]
use super::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use failure::Error;
use serde::Serialize;

use content_inspector::{inspect, ContentType};

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tokei::{Config, LanguageType};

/// a struct representing tokei language data - based on tokei::Stats and tokei::Languages::name
#[derive(Debug, PartialEq, Serialize)]
struct LanguageLocData {
    /// Canonical language name
    pub language: String,
    /// binary files only have bytes not lines!
    pub binary: bool,
    /// Number of blank lines within the file.
    pub blanks: usize,
    /// Number of lines of code within the file.
    pub code: usize,
    /// Number of comments within the file. (_includes both multi line, and
    /// single line comments_)
    pub comments: usize,
    /// Total number of lines within the file.
    pub lines: usize,
    /// File size in bytes
    pub bytes: u64,
}

fn safe_extension(filename: &Path) -> String {
    match filename.extension() {
        Some(ext) => ext.to_string_lossy().to_string(),
        None => "no_extension".to_owned(),
    }
}

fn file_size(filename: &Path) -> Result<u64, Error> {
    Ok(filename.metadata()?.len())
}
//TODO: should binary data have 'lines:0' or should it be
// an explicit special case?
impl LanguageLocData {
    fn from_binary(language_name: String, filename: &Path) -> Result<Self, Error> {
        Ok(LanguageLocData {
            language: language_name,
            binary: true,
            blanks: 0,
            code: 0,
            comments: 0,
            lines: 0,
            bytes: file_size(filename)?,
        })
    }
}

const MAX_PEEK_SIZE: usize = 1024;

fn file_content_type(filename: &Path) -> Result<ContentType, Error> {
    let file = File::open(&filename)?;
    let mut buffer: Vec<u8> = vec![];

    file.take(MAX_PEEK_SIZE as u64).read_to_end(&mut buffer)?;
    Ok(inspect(&buffer))
}

fn parse_file(filename: &Path) -> Result<LanguageLocData, Error> {
    let config = Config::default();
    let mut language_name = None;
    let language = match LanguageType::from_path(filename, &config) {
        Some(language) => language,
        None => {
            language_name = Some(safe_extension(filename));
            if file_content_type(filename)? == ContentType::BINARY {
                return LanguageLocData::from_binary(language_name.unwrap(), filename);
            }
            LanguageType::Text
        }
    };
    let language_name = language_name.unwrap_or_else(|| language.name().to_string());
    let report = language.parse(PathBuf::from(filename), &config);

    match report {
        Ok(report) => Ok(LanguageLocData {
            binary: false,
            blanks: report.stats.blanks,
            code: report.stats.code,
            comments: report.stats.comments,
            lines: report.stats.lines(),
            language: language_name,
            bytes: file_size(filename)?,
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

    fn metadata(&self) -> Result<Option<Value>, Error> {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn can_get_loc_data_for_a_file() {
        let stats = parse_file(Path::new("./tests/data/simple/parent.clj")).unwrap();
        assert_eq!(stats.code, 3);
        assert_eq!(stats.language, "Clojure");
    }
}
