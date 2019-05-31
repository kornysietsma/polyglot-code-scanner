#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(unused_imports)]

use crate::git_logger::{FileHistoryEntry, GitFileHistory, GitLog, GitLogConfig};
use crate::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use failure::Error;
use git2::Status;
use serde::Serialize;
use std::collections::HashSet;
use std::iter::once;
use std::path::Path;
use std::path::PathBuf;

use git2::Repository;

/// a struct representing git data for a file
#[derive(Debug, PartialEq, Serialize)]
struct GitData {
    last_update: i64,
    user_count: usize,
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
        let git_log = GitLog::new(filename, self.git_log_config)?;
        info!("Found working dir: {:?}", git_log.workdir());
        let history = GitFileHistory::new(git_log)?;
        self.git_histories.push(history);
        Ok(())
    }

    fn unique_changers(history: &FileHistoryEntry) -> HashSet<&str> {
        // TODO: test me!
        history
            .co_authors
            .iter()
            .chain(once(&history.author))
            .chain(once(&history.committer))
            .map(|u| u.identifier())
            .collect()
    }

    fn stats_from_history(&self, history: &[FileHistoryEntry]) -> Option<GitData> {
        // TODO!
        // for now, just get latest change - maybe non-trivial change? (i.e. ignore rename/copy) - or this could be configurable
        // and get set of all authors - maybe dedupe by email.
        if history.is_empty() {
            return None;
        }
        let last_update = history.iter().map(|h| h.commit_time).max()?;
        let changers: HashSet<&str> = history
            .iter()
            .flat_map(|h| GitCalculator::unique_changers(h))
            .collect();

        Some(GitData {
            last_update,
            user_count: changers.len(),
        })
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
    use crate::git_logger::{CommitChange, User};
    use pretty_assertions::assert_eq;

    #[test]
    fn gets_basic_stats_from_git_events() -> Result<(), Error> {
        let events: Vec<FileHistoryEntry> = vec![
            FileHistoryEntry::build(
                "1111",
                "jo@smith.com",
                1,
                "sam@smith.com",
                CommitChange::Add,
                3,
            ),
            FileHistoryEntry::build(
                "2222",
                "x@smith.com",
                3,
                "sam@smith.com",
                CommitChange::Add,
                7,
            ),
        ];
        let calculator = GitCalculator {
            git_histories: Vec::new(),
            git_log_config: GitLogConfig::default(),
        };

        let stats = calculator.stats_from_history(&events);

        assert_eq!(
            stats,
            Some(GitData {
                last_update: 3,
                user_count: 3
            })
        );
        Ok(())
    }
}
