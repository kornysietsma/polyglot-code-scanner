#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(unused_imports)]

use crate::git_logger::{log, FileHistoryEntry, GitFileHistory, GitLogConfig};
use crate::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use failure::Error;
use git2::Status;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

use git2::Repository;

/// a struct representing git data for a file
#[derive(Debug, PartialEq, Serialize)]
struct GitData {}

fn parse_file(_filename: &Path) -> Result<GitData, Error> {
    Ok(GitData {})
}

#[derive(Debug)]
pub struct GitCalculator {
    git_histories: Vec<GitFileHistory>,
    git_log_config: GitLogConfig,
}

impl GitCalculator {
    fn git_history(&self, filename: &Path) -> Option<&GitFileHistory> {
        self.git_histories
            .iter()
            .find(|h| h.is_repo_for(filename).unwrap())
        // TODO can we get rid of unwrap here?
        // it's tricky as we can't return a Result.
    }

    fn add_history_for(&mut self, filename: &Path) -> Result<(), Error> {
        info!("Adding new git log for {:?}", &filename);
        let git_log = log(filename, self.git_log_config)?;
        info!("Found working dir: {:?}", git_log.workdir());
        let history = GitFileHistory::new(git_log)?;
        self.git_histories.push(history);
        Ok(())
    }

    fn stats_from_history(&self, history: &[FileHistoryEntry]) -> GitData {
        GitData {}
    }
}

impl ToxicityIndicatorCalculator for GitCalculator {
    fn name(&self) -> String {
        "git".to_string()
    }

    fn description(&self) -> String {
        "Git statistics".to_string()
    }

    fn calculate(&mut self, path: &Path) -> Result<Option<serde_json::Value>, Error> {
        if path.is_file() {
            let history = match self.git_history(path) {
                Some(history) => history,
                None => {
                    self.add_history_for(path)?;
                    self.git_history(path).unwrap()
                }
            };
            let file_history = history.history_for(path)?;

            if let Some(file_history) = file_history {
                let stats = self.stats_from_history(file_history);
                Ok(Some(serde_json::value::to_value(stats).expect(
                    "Serializable object couldn't be serialized to JSON",
                ))) // TODO: maybe explicit error? Though this should be fatal
            } else {
                info!("No git history found for file: {:?}", path);
                return Ok(None);
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

}
