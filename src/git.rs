#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(unused_imports)]

use crate::git_file_history::{FileHistoryEntry, FileHistoryEntryBuilder, GitFileHistory};
use crate::git_logger::{GitLog, GitLogConfig};
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
    last_update: u64,
    age_in_days: u64,
    user_count: usize,
}

#[derive(Debug)]
pub struct GitCalculator {
    git_histories: Vec<GitFileHistory>,
    git_log_config: GitLogConfig, // TODO - probably should have own config struct
}

impl GitCalculator {
    pub fn new(config: GitLogConfig) -> Self {
        GitCalculator {
            git_histories: Vec::new(),
            git_log_config: config,
        }
    }

    fn git_history(&self, filename: &Path) -> Option<&GitFileHistory> {
        self.git_histories
            .iter()
            .find(|h| h.is_repo_for(filename).unwrap())
        // TODO can we get rid of unwrap here?
        // it's tricky as we can't return a Result.
    }

    fn add_history_for(&mut self, filename: &Path) -> Result<(), Error> {
        info!("Adding new git log for {:?}", &filename);
        let mut git_log = GitLog::new(filename, self.git_log_config)?;
        info!("Found working dir: {:?}", git_log.workdir());
        let history = GitFileHistory::new(&mut git_log)?;
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

    fn stats_from_history(
        &self,
        last_commit: u64,
        history: &[FileHistoryEntry],
    ) -> Option<GitData> {
        // TODO!
        // for now, just get latest change - maybe non-trivial change? (i.e. ignore rename/copy) - or this could be configurable
        // and get set of all authors - maybe deduplicate by email.
        if history.is_empty() {
            return None;
        }
        let last_update = history.iter().map(|h| h.commit_time).max()?;

        let age_in_days = (last_commit - last_update) / (60 * 60 * 24);

        let changers: HashSet<&str> = history
            .iter()
            .flat_map(|h| GitCalculator::unique_changers(h))
            .collect();

        Some(GitData {
            last_update,
            age_in_days,
            user_count: changers.len(),
        })
    }
}

impl ToxicityIndicatorCalculator for GitCalculator {
    fn name(&self) -> String {
        "git".to_string()
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
            let last_commit = history.last_commit();
            let file_history = history.history_for(path)?;

            if let Some(file_history) = file_history {
                let stats = self.stats_from_history(last_commit, file_history);
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
        let one_day_in_secs: u64 = 60 * 60 * 24;

        let first_day = one_day_in_secs;

        let events: Vec<FileHistoryEntry> = vec![
            FileHistoryEntryBuilder::test_default()
                .emails("jo@smith.com")
                .times(first_day)
                .id("1111")
                .build()
                .map_err(failure::err_msg)?,
            FileHistoryEntryBuilder::test_default()
                .emails("x@smith.com")
                .times(first_day + 3 * one_day_in_secs)
                .author(User::new(None, Some("y@smith.com")))
                .id("2222")
                .build()
                .map_err(failure::err_msg)?,
        ];
        let calculator = GitCalculator {
            git_histories: Vec::new(),
            git_log_config: GitLogConfig::default(),
        };

        let today = first_day + 5 * one_day_in_secs;

        let stats = calculator.stats_from_history(today, &events);

        assert_eq!(
            stats,
            Some(GitData {
                last_update: first_day + 3 * one_day_in_secs,
                age_in_days: 2,
                user_count: 3
            })
        );
        Ok(())
    }
}
