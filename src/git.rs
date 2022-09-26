use crate::flare::FlareTreeNode;
use crate::git_file_history::{FileHistoryEntry, GitFileHistory};
use crate::git_logger::{CommitChange, GitLog, GitLogConfig, User};
use crate::git_user_dictionary::GitUserDictionary;
use crate::polyglot_data::GitMetadata;
use crate::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use anyhow::Error;
use chrono::{NaiveDateTime, NaiveTime};

use serde::{Deserialize, Serialize};

use std::cmp::Ordering;
use std::collections::HashSet;
use std::collections::{BTreeSet, HashMap};
use std::iter::once;

use std::path::Path;

use git2::Repository;

/// a struct representing git data for a file
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct GitData {
    pub last_update: u64,
    pub age_in_days: u64,
    // we only have a creation date if there was an Add change in the dates scanned
    pub creation_date: Option<u64>,
    pub user_count: usize,
    pub users: Vec<usize>, // dictionary IDs
    pub details: Vec<GitDetails>,
    pub activity: Vec<GitActivity>,
}

/// Git information for a given day _and_ unique set of users, summarized
/// New as of 0.3.3 - we now generate new `GitDetails` per user set - the file format hasn't changed but
/// instead of a single `GitDetails` per day, there might be multiple.
/// Also dates are summarized by "author date" - had to pick author or commit date, and
/// author dates seem more reliable.  But it's named "`commit_day`" as that's more understandable
/// WIP: for better coupling data, I want individual commits, rather than summarizing per day.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct GitDetails {
    /// Note this is based on "author date" - commit dates can be all over the place with PRs, rebasing and the like.
    pub commit_day: u64,
    pub users: BTreeSet<usize>, // dictionary IDs, ordered
    pub commits: u64,
    pub lines_added: u64,
    pub lines_deleted: u64,
}

impl Ord for GitDetails {
    fn cmp(&self, other: &Self) -> Ordering {
        let day_ordering = self.commit_day.cmp(&other.commit_day);
        if day_ordering != Ordering::Equal {
            return day_ordering;
        }
        self.users.cmp(&other.users)
    }
}

impl PartialOrd for GitDetails {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// this is the key to keep details stored uniquely
#[derive(Debug, PartialEq, Eq, Hash)]
struct GitDetailsKey {
    pub commit_day: u64,
    pub users: BTreeSet<usize>,
}

/// Fine-grained git activity, for the fine-grained coupling calculations
/// this is very verbose so probably shouldn't be kept in final JSON
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct GitActivity {
    pub author_time: u64,
    pub commit_time: u64,
    pub users: BTreeSet<usize>, // dictionary IDs
    pub change: CommitChange,
    pub lines_added: u64,
    pub lines_deleted: u64,
}
impl Ord for GitActivity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.commit_time.cmp(&other.commit_time)
    }
}

impl PartialOrd for GitActivity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// History of any git roots discovered by the calculator
///  Split from `GitCalculator` as we need to mutate the dictionary while borrowing the history immutably
#[derive(Debug)]
pub struct GitHistories {
    git_file_histories: Vec<GitFileHistory>,
    /// config used to initialize any git histories
    git_log_config: GitLogConfig,
}

#[derive(Debug)]
pub struct GitCalculator {
    histories: GitHistories,
    dictionary: GitUserDictionary,
}

// Git data for a directory - just remote git info
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GitInfo {
    pub remote_url: Option<String>,
    pub head: Option<String>,
}

// Git data for a file _or_ a directory
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(untagged)]
pub enum GitNodeData {
    File {
        #[serde(flatten)]
        data: GitData,
    },
    Dir {
        #[serde(flatten)]
        data: GitInfo,
    },
}

fn repository_head(repository: &Repository) -> Result<String, Error> {
    let head = repository.head()?;
    let head_ref = head.resolve()?;
    Ok(head_ref.peel_to_commit()?.id().to_string())
}

impl GitInfo {
    pub fn new(path: &Path, repository: &Repository) -> Self {
        let remote = repository.find_remote("origin");
        let remote_url = match remote {
            Err(e) => {
                warn!("Error fetching origin for {:?}: {}", path, e);
                None
            }
            Ok(remote) => remote.url().map(str::to_owned),
        };
        let head = match repository_head(repository) {
            Err(e) => {
                warn!("Error fetching head for {:?}: {}", path, e);
                None
            }
            Ok(head) => Some(head),
        };
        GitInfo { remote_url, head }
    }
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
    ) -> BTreeSet<usize> {
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
        dictionary: &mut GitUserDictionary,
        last_commit: u64,
        history: &[FileHistoryEntry],
    ) -> Option<GitData> {
        // for now, just get latest change - maybe non-trivial change? (i.e. ignore rename/copy) - or this could be configurable
        // and get set of all authors - maybe deduplicate by email.
        if history.is_empty() {
            return None;
        }
        let mut details: HashMap<GitDetailsKey, GitDetails> = HashMap::new();

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

        let mut activity_vec: Vec<GitActivity> = Vec::new();

        for entry in history {
            let author_day = start_of_day(entry.author_time);
            let unique_changers = GitHistories::unique_changers(entry, dictionary);
            let key = GitDetailsKey {
                commit_day: author_day,
                users: unique_changers.clone(),
            };
            let daily_details = details.entry(key).or_insert(GitDetails {
                commit_day: author_day,
                users: unique_changers.clone(),
                commits: 0,
                lines_added: 0,
                lines_deleted: 0,
            });
            daily_details.commits += 1;
            daily_details
                .users
                .extend(unique_changers.clone().into_iter());
            daily_details.lines_added += entry.lines_added;
            daily_details.lines_deleted += entry.lines_deleted;

            let activity: GitActivity = GitActivity {
                commit_time: entry.commit_time,
                author_time: entry.author_time,
                users: unique_changers,
                change: entry.change,
                lines_added: entry.lines_added,
                lines_deleted: entry.lines_deleted,
            };
            activity_vec.push(activity);
        }

        let mut changer_list: Vec<usize> = changers.into_iter().collect();
        changer_list.sort_unstable();

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
            details: details_vec,
            activity: activity_vec,
        })
    }
}

impl GitCalculator {
    pub fn new(config: GitLogConfig) -> Self {
        GitCalculator {
            histories: GitHistories {
                git_file_histories: Vec::new(),
                git_log_config: config,
            },
            dictionary: GitUserDictionary::default(),
        }
    }
}

impl ToxicityIndicatorCalculator for GitCalculator {
    fn name(&self) -> String {
        "git".to_string()
    }
    fn visit_node(&mut self, node: &mut FlareTreeNode, path: &Path) -> Result<(), Error> {
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
                let stats = GitHistories::stats_from_history(
                    &mut self.dictionary,
                    last_commit,
                    file_history,
                );
                node.indicators_mut().git = stats.map(|stats| GitNodeData::File { data: stats });
            } else {
                // probably outside date range
                debug!("No git history found for file: {:?}", path);
            }
        } else {
            let git_path = path.join(".git");
            if git_path.is_dir() {
                match Repository::discover(path) {
                    Ok(repository) => {
                        let info = GitInfo::new(path, &repository);
                        node.indicators_mut().git = Some(GitNodeData::Dir { data: info });
                    }
                    Err(e) => {
                        warn!("Can't find git repository at {:?}, {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    fn apply_metadata(
        &self,
        metadata: &mut crate::polyglot_data::IndicatorMetadata,
    ) -> Result<(), Error> {
        metadata.git = Some(GitMetadata {
            users: self.dictionary.clone(),
        });
        Ok(())
    }
}

// Hacky - I need this constructor for coupling tests, until I build better integration tests
#[cfg(test)]
impl GitData {
    pub fn fake_with_activity(activity: Vec<GitActivity>) -> Self {
        Self {
            last_update: 0,
            age_in_days: 0,
            creation_date: None,
            user_count: 0,
            users: Vec::new(),
            details: Vec::new(),
            activity,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::git_file_history::FileHistoryEntryBuilder;
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
                .map_err(Error::msg)?,
            FileHistoryEntryBuilder::test_default()
                .emails("x@smith.com")
                .times(first_day + 3 * one_day_in_secs)
                .author(User::new(Some("Why"), Some("y@smith.com")))
                .id("2222")
                .build()
                .map_err(Error::msg)?,
        ];
        let mut dictionary = GitUserDictionary::default();

        let today = first_day + 5 * one_day_in_secs;

        let stats = GitHistories::stats_from_history(&mut dictionary, today, &events).unwrap();

        assert_eq!(stats.last_update, first_day + 3 * one_day_in_secs);
        assert_eq!(stats.age_in_days, 2);
        assert_eq!(stats.creation_date, Some(86400));
        assert_eq!(stats.user_count, 3);
        assert_eq!(stats.users, vec![0, 1, 2]);
        // don't assert details - details used to be optional, so it is tested in next test.

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
                .map_err(Error::msg)?,
            FileHistoryEntryBuilder::test_default()
                .emails("jo@smith.com")
                .times(first_day)
                .author(User::new(Some("Why"), Some("y@smith.com"))) // second author so new stats
                .id("1111")
                .build()
                .map_err(Error::msg)?,
            FileHistoryEntryBuilder::test_default()
                .emails("x@smith.com")
                .times(first_day + 3 * one_day_in_secs)
                .author(User::new(Some("Why"), Some("y@smith.com")))
                .id("2222")
                .build()
                .map_err(Error::msg)?,
        ];

        let mut dictionary = GitUserDictionary::default();

        let today = first_day + 5 * one_day_in_secs;

        let stats = GitHistories::stats_from_history(&mut dictionary, today, &events);

        let jo_set: BTreeSet<usize> = vec![0].into_iter().collect();
        let xy_set: BTreeSet<usize> = vec![1, 2].into_iter().collect();
        let jo_y_set: BTreeSet<usize> = vec![0, 1].into_iter().collect();

        let expected_details: Vec<GitDetails> = vec![
            GitDetails {
                commit_day: 86400,
                users: jo_set.clone(),
                commits: 1,
                lines_added: 0,
                lines_deleted: 0,
            },
            GitDetails {
                commit_day: 86400,
                users: jo_y_set.clone(),
                commits: 1,
                lines_added: 0,
                lines_deleted: 0,
            },
            GitDetails {
                commit_day: 345_600,
                users: xy_set.clone(),
                commits: 1,
                lines_added: 0,
                lines_deleted: 0,
            },
        ];

        let expected_activity: Vec<GitActivity> = vec![
            GitActivity {
                author_time: 86400,
                commit_time: 86400,
                users: jo_set,
                change: CommitChange::Add,
                lines_added: 0,
                lines_deleted: 0,
            },
            GitActivity {
                author_time: 86400,
                commit_time: 86400,
                users: jo_y_set,
                change: CommitChange::Add,
                lines_added: 0,
                lines_deleted: 0,
            },
            GitActivity {
                author_time: 345_600,
                commit_time: 345_600,
                users: xy_set,
                change: CommitChange::Add,
                lines_added: 0,
                lines_deleted: 0,
            },
        ];

        assert_eq!(
            stats,
            Some(GitData {
                last_update: first_day + 3 * one_day_in_secs,
                age_in_days: 2,
                creation_date: Some(86400),
                user_count: 3,
                users: vec![0, 1, 2],
                details: expected_details,
                activity: expected_activity,
            })
        );

        assert_eq!(dictionary.user_count(), 3);
        assert_eq!(dictionary.user_id(&USER_JO), Some(&0));
        assert_eq!(dictionary.user_id(&USER_Y), Some(&1));
        assert_eq!(dictionary.user_id(&USER_X), Some(&2));

        Ok(())
    }
}
