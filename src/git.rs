#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(unused_imports)]

use crate::git_file_history::{FileHistoryEntry, FileHistoryEntryBuilder, GitFileHistory};
use crate::git_logger::{GitLog, GitLogConfig, User};
use crate::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use failure::Error;
use git2::Status;
use serde::Serialize;
use std::collections::HashSet;
use std::iter::once;
use std::iter::FromIterator;
use std::path::Path;
use std::path::PathBuf;

use git2::Repository;

/// a struct representing git data for a file
#[derive(Debug, PartialEq, Serialize)]
struct GitData {
    last_update: u64,
    age_in_days: u64,
    user_count: usize,
    users: Vec<User>,
}

#[derive(Debug)]
pub struct GitCalculator {
    git_histories: Vec<GitFileHistory>,
    git_log_config: GitLogConfig, // TODO - probably should have own config struct
    detailed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GitInfo {
    pub remote_url: Option<String>,
    pub head: Option<String>,
}

fn repository_head(repository: &Repository) -> Result<String, Error> {
    let head = repository.head()?;
    let head_ref = head.resolve()?;
    Ok(head_ref.peel_to_commit()?.id().to_string())
}

impl GitInfo {
    pub fn new(path: &Path, repository: Repository) -> Self {
        let remote = repository.find_remote("origin");
        let remote_url = match remote {
            Err(e) => {
                warn!("Error fetching origin for {:?}: {}", path, e);
                None
            }
            Ok(remote) => remote.url().map(str::to_owned),
        };
        let head = match repository_head(&repository) {
            Err(e) => {
                warn!("Error fetching head for {:?}: {}", path, e);
                None
            }
            Ok(head) => Some(head),
        };
        GitInfo { remote_url, head }
    }
}

impl GitCalculator {
    pub fn new(config: GitLogConfig, detailed: bool) -> Self {
        GitCalculator {
            git_histories: Vec::new(),
            git_log_config: config,
            detailed,
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

    fn unique_changers(history: &FileHistoryEntry) -> HashSet<&User> {
        history
            .co_authors
            .iter()
            .chain(once(&history.author))
            .chain(once(&history.committer))
            .collect()
    }

    fn stats_from_history(
        &self,
        last_commit: u64,
        history: &[FileHistoryEntry],
    ) -> Option<GitData> {
        // for now, just get latest change - maybe non-trivial change? (i.e. ignore rename/copy) - or this could be configurable
        // and get set of all authors - maybe deduplicate by email.
        if history.is_empty() {
            return None;
        }
        let last_update = history.iter().map(|h| h.commit_time).max()?;

        let age_in_days = (last_commit - last_update) / (60 * 60 * 24);

        let changers: HashSet<&User> = history
            .iter()
            .flat_map(|h| GitCalculator::unique_changers(h))
            .collect();

        let mut changer_list: Vec<User> = changers.into_iter().cloned().collect();
        changer_list.sort();

        Some(GitData {
            last_update,
            age_in_days,
            user_count: changer_list.len(),
            users: changer_list,
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
                    warn!("Loading git history for {}", path.display());
                    self.add_history_for(path)?;
                    warn!("history loaded.");
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
                Ok(None)
            }
        } else {
            let git_path = path.join(".git");
            if git_path.is_dir() {
                match Repository::discover(path) {
                    Ok(repository) => {
                        let info = GitInfo::new(path, repository);
                        Ok(Some(serde_json::value::to_value(info).expect(
                            "Serializable object couldn't be serialized to JSON",
                        )))
                    }
                    Err(e) => {
                        warn!("Can't find git repository at {:?}, {}", path, e);
                        Ok(None)
                    }
                }
            } else {
                Ok(None)
            }
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
                .author(User::new(Some("Why"), Some("y@smith.com")))
                .id("2222")
                .build()
                .map_err(failure::err_msg)?,
        ];
        let calculator = GitCalculator {
            git_histories: Vec::new(),
            git_log_config: GitLogConfig::default(),
            detailed: false,
        };

        let today = first_day + 5 * one_day_in_secs;

        let stats = calculator.stats_from_history(today, &events);

        assert_eq!(
            stats,
            Some(GitData {
                last_update: first_day + 3 * one_day_in_secs,
                age_in_days: 2,
                user_count: 3,
                users: vec![
                    User::new(None, Some("jo@smith.com")),
                    User::new(None, Some("x@smith.com")),
                    User::new(Some("Why"), Some("y@smith.com"))
                ],
            })
        );
        Ok(())
    }

    #[test]
    fn gets_detailed_stats_from_git_events() -> Result<(), Error> {
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
                .author(User::new(Some("Why"), Some("y@smith.com")))
                .id("2222")
                .build()
                .map_err(failure::err_msg)?,
        ];
        let calculator = GitCalculator {
            git_histories: Vec::new(),
            git_log_config: GitLogConfig::default(),
            detailed: true,
        };

        let today = first_day + 5 * one_day_in_secs;

        let stats = calculator.stats_from_history(today, &events);

        assert_eq!(
            stats,
            Some(GitData {
                last_update: first_day + 3 * one_day_in_secs,
                age_in_days: 2,
                user_count: 3,
                users: vec![
                    User::new(None, Some("jo@smith.com")),
                    User::new(None, Some("x@smith.com")),
                    User::new(Some("Why"), Some("y@smith.com"))
                ],
            })
        );
        Ok(())
    }
}
