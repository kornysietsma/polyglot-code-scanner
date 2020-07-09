#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(unused_imports)]

use crate::git_file_history::{FileHistoryEntry, FileHistoryEntryBuilder, GitFileHistory};
use crate::git_logger::{CommitChange, GitLog, GitLogConfig, User};
use crate::git_user_dictionary::GitUserDictionary;
use crate::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use failure::Error;
use git2::Status;
use serde::{Deserialize, Serialize, Serializer};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::once;
use std::iter::FromIterator;
use std::path::Path;
use std::path::PathBuf;

use git2::Repository;
use serde_json::{json, Value};

/// a struct representing git data for a file
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GitData {
    last_update: u64,
    age_in_days: u64,
    // we only have a creation date if there was an Add change in the dates scanned
    creation_date: Option<u64>,
    user_count: usize,
    users: Vec<usize>, // dictionary IDs
    details: Option<Vec<GitDetails>>,
}

/// Git information for a given day, summarized
/// we don't distinguish multiple changes in a day currently, so if one person changed 1 line and another changed 100 you can't tell the difference.
/// It is assumed that people work as teams to some degree!
/// This could be revisited if needed, but I'm trying to keep the log size sane
/// Also dates are summarized by "author date" - had to pick author or commit date, and
/// author dates seem more reliable.  But it's named "commit_day" as that's more understandable
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDetails {
    /// Note this is based on "author date" - commit dates can be all over the place with PRs, rebasing and the like.
    pub commit_day: u64,
    #[serde(serialize_with = "ordered_set")]
    pub users: HashSet<usize>, // dictionary IDs
    pub commits: u64,
    pub lines_added: u64,
    pub lines_deleted: u64,
}

impl Ord for GitDetails {
    fn cmp(&self, other: &Self) -> Ordering {
        self.commit_day.cmp(&other.commit_day)
    }
}

impl PartialOrd for GitDetails {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn ordered_set<S>(value: &HashSet<usize>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut ordered: Vec<&usize> = value.iter().collect();
    ordered.sort();
    ordered.serialize(serializer)
}

/// History of any git roots discovered by the calculator
///  Split from GitCalculator as we need to mutate the dictionary while borrowing the history immutably
#[derive(Debug)]
pub struct GitHistories {
    git_file_histories: Vec<GitFileHistory>,
    /// config used to initialize any git histories
    git_log_config: GitLogConfig,
}

#[derive(Debug)]
pub struct GitCalculator {
    histories: GitHistories,
    detailed: bool,
    dictionary: GitUserDictionary,
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

fn append_unique_users(users: &mut Vec<User>, new_users: HashSet<&User>) {
    let new_users_cloned = new_users.into_iter().cloned();
    let old_users: HashSet<User> = users.drain(..).chain(new_users_cloned).collect();
    let mut all_users: Vec<User> = old_users.into_iter().collect();

    users.append(&mut all_users);
}
fn start_of_day(secs_since_epoch: u64) -> u64 {
    let date_time = NaiveDateTime::from_timestamp(secs_since_epoch as i64, 0);
    date_time
        .date()
        .and_time(NaiveTime::from_num_seconds_from_midnight(0, 0))
        .timestamp() as u64
}
impl GitHistories {
    fn git_history(&self, filename: &Path) -> Option<&GitFileHistory> {
        self.git_file_histories
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
        self.git_file_histories.push(history);
        Ok(())
    }
    fn unique_changers(
        history: &FileHistoryEntry,
        dictionary: &mut GitUserDictionary,
    ) -> HashSet<usize> {
        let mut users: Vec<&User> = history
            .co_authors
            .iter()
            .chain(once(&history.author))
            .chain(once(&history.committer))
            .collect();
        users.sort();
        users.dedup();
        // this used to use a HashSet but I want deterministic ordering and so I want it in a vec anyway
        users.into_iter().map(|u| dictionary.register(u)).collect()
    }

    fn stats_from_history(
        &self,
        dictionary: &mut GitUserDictionary,
        last_commit: u64,
        history: &[FileHistoryEntry],
        detailed: bool,
    ) -> Option<GitData> {
        // for now, just get latest change - maybe non-trivial change? (i.e. ignore rename/copy) - or this could be configurable
        // and get set of all authors - maybe deduplicate by email.
        if history.is_empty() {
            return None;
        }
        let mut details: HashMap<u64, GitDetails> = HashMap::new();

        let first_date = history.iter().map(|h| h.author_time).min();

        let mut creation_date = history
            .iter()
            .filter(|h| h.change == CommitChange::Add)
            .map(|h| h.author_time)
            .min();

        if let Some(creation) = creation_date {
            // TODO: test this!
            if first_date.unwrap() < creation {
                debug!(
                    "File has a git date {:?} before the first Add operation {:?}",
                    first_date.unwrap(),
                    creation
                );
                creation_date = None;
            }
        }

        let last_update = history.iter().map(|h| h.commit_time).max()?;

        let age_in_days = (last_commit - last_update) / (60 * 60 * 24);

        let changers: HashSet<usize> = history
            .iter()
            .flat_map(|h| GitHistories::unique_changers(h, dictionary))
            .collect();

        for entry in history {
            let author_day = start_of_day(entry.author_time);

            let daily_details = details.entry(author_day).or_insert(GitDetails {
                commit_day: author_day,
                users: HashSet::new(),
                commits: 0,
                lines_added: 0,
                lines_deleted: 0,
            });
            daily_details.commits += 1;
            daily_details
                .users
                .extend(GitHistories::unique_changers(entry, dictionary).into_iter());
            daily_details.lines_added += entry.lines_added;
            daily_details.lines_deleted += entry.lines_deleted;
        }

        let mut changer_list: Vec<usize> = changers.into_iter().collect();
        changer_list.sort();

        let mut details_vec: Vec<GitDetails> = details
            .into_iter()
            .map(|(_k, v)| v)
            .collect::<Vec<GitDetails>>();
        details_vec.sort();

        Some(GitData {
            last_update,
            age_in_days,
            creation_date,
            user_count: changer_list.len(),
            users: changer_list,
            details: if detailed {
                Some(details_vec)
            } else {
                None // TODO: don't waste time processing details if we don't want them!
            },
        })
    }
}

impl GitCalculator {
    pub fn new(config: GitLogConfig, detailed: bool) -> Self {
        GitCalculator {
            histories: GitHistories {
                git_file_histories: Vec::new(),
                git_log_config: config,
            },
            detailed,
            dictionary: GitUserDictionary::new(),
        }
    }
}

impl ToxicityIndicatorCalculator for GitCalculator {
    fn name(&self) -> String {
        "git".to_string()
    }
    fn calculate(&mut self, path: &Path) -> Result<Option<serde_json::Value>, Error> {
        if path.is_file() {
            // TODO: refactor this into a method on histories (I tried this but got into a mess with mutable and immutable refs to self!)
            let history = match self.histories.git_history(path) {
                Some(history) => history,
                None => {
                    info!("Loading git history for {}", path.display());
                    self.histories.add_history_for(path)?;
                    info!("history loaded.");
                    self.histories.git_history(path).unwrap()
                }
            };
            let last_commit = history.last_commit();
            let file_history = history.history_for(path)?;

            if let Some(file_history) = file_history {
                let stats = self.histories.stats_from_history(
                    &mut self.dictionary,
                    last_commit,
                    file_history,
                    self.detailed,
                );
                Ok(Some(serde_json::value::to_value(stats).expect(
                    "Serializable object couldn't be serialized to JSON",
                ))) // TODO: maybe explicit error? Though this should be fatal
            } else {
                // probably outside date range
                debug!("No git history found for file: {:?}", path);
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

    fn metadata(&self) -> Result<Option<Value>, Error> {
        let dictionary = serde_json::value::to_value(&self.dictionary)
            .expect("Serializable object couldn't be serialized to JSON");
        Ok(Some(json!({ "users": dictionary })))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::git_logger::{CommitChange, User};
    use pretty_assertions::assert_eq;

    lazy_static! {
        static ref USER_JO: User = User::new(None, Some("jo@smith.com"));
        static ref USER_X: User = User::new(None, Some("x@smith.com"));
        static ref USER_Y: User = User::new(Some("Why"), Some("y@smith.com"));
    }

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
        let histories = GitHistories {
            git_file_histories: Vec::new(),
            git_log_config: GitLogConfig::default(),
        };
        let mut dictionary = GitUserDictionary::new();

        let today = first_day + 5 * one_day_in_secs;

        let stats = histories.stats_from_history(&mut dictionary, today, &events, false);

        assert_eq!(
            stats,
            Some(GitData {
                last_update: first_day + 3 * one_day_in_secs,
                age_in_days: 2,
                creation_date: Some(86400),
                user_count: 3,
                users: vec![0, 1, 2],
                details: None
            })
        );

        assert_eq!(dictionary.user_count(), 3);
        assert_eq!(dictionary.user_id(&USER_JO), Some(&0));
        assert_eq!(dictionary.user_id(&USER_X), Some(&1));
        assert_eq!(dictionary.user_id(&USER_Y), Some(&2));

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
        let histories = GitHistories {
            git_file_histories: Vec::new(),
            git_log_config: GitLogConfig::default(),
        };
        let mut dictionary = GitUserDictionary::new();

        let today = first_day + 5 * one_day_in_secs;

        let stats = histories.stats_from_history(&mut dictionary, today, &events, true);

        let jo_set: HashSet<usize> = vec![0].into_iter().collect();
        let xy_set: HashSet<usize> = vec![1, 2].into_iter().collect();

        let expected_details: Option<Vec<GitDetails>> = Some(vec![
            GitDetails {
                commit_day: 86400,
                users: jo_set,
                commits: 1,
                lines_added: 0,
                lines_deleted: 0,
            },
            GitDetails {
                commit_day: 345600,
                users: xy_set,
                commits: 1,
                lines_added: 0,
                lines_deleted: 0,
            },
        ]);

        assert_eq!(
            stats,
            Some(GitData {
                last_update: first_day + 3 * one_day_in_secs,
                age_in_days: 2,
                creation_date: Some(86400),
                user_count: 3,
                users: vec![0, 1, 2],
                details: expected_details
            })
        );

        assert_eq!(dictionary.user_count(), 3);
        assert_eq!(dictionary.user_id(&USER_JO), Some(&0));
        assert_eq!(dictionary.user_id(&USER_X), Some(&1));
        assert_eq!(dictionary.user_id(&USER_Y), Some(&2));

        Ok(())
    }
}
