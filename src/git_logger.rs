#![warn(clippy::all)]
use crate::git_file_future::{FileNameChange, GitFileFutureRegistry};
use failure::Error;
use git2::Revwalk;
use git2::{Commit, Delta, DiffDelta, ObjectType, Odb, Oid, Patch, Repository, Tree};
use regex::Regex;
use serde::Serialize;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy)]
pub struct GitLogConfig {
    /// include merge commits in file stats - usually excluded by `git log` - see https://stackoverflow.com/questions/37801342/using-git-log-to-display-files-changed-during-merge
    include_merges: bool,
    /// earliest commmit for filtering - secs since the epoch - could use Option but this is pretty cheap to check
    earliest_time: u64,
}

impl GitLogConfig {
    pub fn default() -> GitLogConfig {
        GitLogConfig {
            include_merges: false,
            earliest_time: 0,
        }
    }

    #[allow(dead_code)]
    pub fn include_merges(self, include_merges: bool) -> GitLogConfig {
        let mut config = self;
        config.include_merges = include_merges;
        config
    }
    /// filter log by unix timestamp
    pub fn since(self, earliest_time: u64) -> GitLogConfig {
        let mut config = self;
        config.earliest_time = earliest_time;
        config
    }
    /// filter log by number of years before now
    pub fn since_years(self, years: u64) -> GitLogConfig {
        let years_ago = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 365 * years);
        let years_ago_secs = years_ago
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.since(years_ago_secs)
    }
}

pub struct GitLog {
    /// repo work dir - always canonical
    workdir: PathBuf,
    repo: Repository,
    config: GitLogConfig,
}

pub struct GitLogIterator<'a> {
    git_log: &'a GitLog,
    odb: Odb<'a>,
    revwalk: Revwalk<'a>,
    // this is an RC as we need to use it after the iterator has been consumed
    git_file_future_registry: Rc<RefCell<GitFileFutureRegistry>>,
}

/// simplified user info - based on git2::Signature
/// everything is derived, seems to work OK as the structure is so simple
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord, Serialize)]
pub struct User {
    name: Option<String>,
    email: Option<String>,
}

impl User {
    pub fn new(name: Option<&str>, email: Option<&str>) -> User {
        User {
            name: name.map(|x| x.to_owned()),
            email: email.map(|x| x.to_owned()),
        }
    }
}

/// simplified commit log entry
#[derive(Debug, Serialize, Clone, Getters)]
pub struct GitLogEntry {
    id: String,
    summary: String,
    parents: Vec<String>,
    committer: User,
    commit_time: u64,
    author: User,
    author_time: u64,
    co_authors: Vec<User>,
    file_changes: Vec<FileChange>,
}

/// the various kinds of git change we care about - a serializable subset of git2::Delta
#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum CommitChange {
    Add,
    Rename,
    Delete,
    Modify,
    Copied,
}

/// Stats for file changes
#[derive(Debug, Serialize, Clone, Getters)]
pub struct FileChange {
    file: PathBuf,
    old_file: Option<PathBuf>,
    change: CommitChange,
    lines_added: u64,
    lines_deleted: u64,
}

impl GitLog {
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub fn new(start_dir: &Path, config: GitLogConfig) -> Result<GitLog, Error> {
        let repo = Repository::discover(start_dir)?;

        let workdir = repo
            .workdir()
            .ok_or_else(|| format_err!("bare repository - no workdir"))?
            .canonicalize()?;

        debug!("work dir: {:?}", workdir);

        Ok(GitLog {
            workdir,
            repo,
            config,
        })
    }

    pub fn iterator(&self) -> Result<GitLogIterator, Error> {
        let odb = self.repo.odb()?;
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
        revwalk.push_head()?;
        Ok(GitLogIterator {
            git_log: &self,
            odb,
            revwalk,
            git_file_future_registry: Rc::new(RefCell::new(GitFileFutureRegistry::new())),
        })
    }
}

impl<'a> Iterator for GitLogIterator<'a> {
    type Item = Result<GitLogEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut next_item = self.revwalk.next();
        while next_item.is_some() {
            let c = self.summarise_commit(next_item.unwrap());
            match c {
                Ok(Some(c)) => {
                    if c.commit_time >= self.git_log.config.earliest_time {
                        self.register_file_futures(&c);
                        return Some(Ok(c));
                    } else {
                        return None; // short circuit!
                    }
                }
                Ok(None) => {}
                Err(e) => return Some(Err(e)),
            };
            next_item = self.revwalk.next();
        }
        None
    }
}

impl<'a> GitLogIterator<'a> {
    pub fn git_file_future_registry(&self) -> Rc<RefCell<GitFileFutureRegistry>> {
        self.git_file_future_registry.clone()
    }

    /// registers renames and deletes
    fn register_file_futures(&mut self, entry: &GitLogEntry) {
        // TODO: probably should be using Oid not String globally, then this would be simpler:
        let parents: Vec<Oid> = entry
            .parents
            .iter()
            .map(|id| Oid::from_str(&id).unwrap())
            .collect();
        let mut file_changes: Vec<(PathBuf, FileNameChange)> = Vec::new();
        for file_change in &entry.file_changes {
            match file_change.change {
                CommitChange::Rename => {
                    let old_name = file_change.old_file.as_ref().unwrap().clone();
                    let new_name = file_change.file.clone();
                    file_changes.push((old_name, FileNameChange::Renamed(new_name)))
                }
                CommitChange::Delete => {
                    let name = file_change.file.clone();
                    file_changes.push((name, FileNameChange::Deleted()))
                }
                _ => (),
            }
        }
        self.git_file_future_registry.borrow_mut().register(
            &Oid::from_str(&entry.id).unwrap(),
            &parents,
            &file_changes,
        );
    }

    /// Summarises a git commit
    /// returns Error if error, Result<None> if the id was not actually a commit, or Result<Some<GitLogEntry>> if valid
    fn summarise_commit(
        &self,
        oid: Result<Oid, git2::Error>,
    ) -> Result<Option<GitLogEntry>, Error> {
        let oid = oid?;
        let kind = self.odb.read(oid)?.kind();
        match kind {
            ObjectType::Commit => {
                let commit = self.git_log.repo.find_commit(oid)?;
                debug!("processing {:?}", commit);
                let author = commit.author();
                let committer = commit.committer();
                let author_time = author.when().seconds() as u64;
                let commit_time = committer.when().seconds() as u64;
                let other_time = commit.time().seconds() as u64;
                if commit_time != other_time {
                    error!(
                        "Commit {:?} time {:?} != commit time {:?}",
                        commit, other_time, commit_time
                    );
                }
                let co_authors = if let Some(message) = commit.message() {
                    find_coauthors(message)
                } else {
                    Vec::new()
                };

                let commit_tree = commit.tree()?;
                let file_changes = commit_file_changes(
                    &self.git_log.repo,
                    &commit,
                    &commit_tree,
                    self.git_log.config,
                );
                Ok(Some(GitLogEntry {
                    id: oid.to_string(),
                    summary: commit.summary().unwrap_or("[no message]").to_string(),
                    parents: commit.parent_ids().map(|p| p.to_string()).collect(),
                    committer: signature_to_user(&committer),
                    commit_time,
                    author: signature_to_user(&author),
                    author_time,
                    co_authors,
                    file_changes,
                }))
            }
            _ => {
                info!("ignoring object type: {}", kind);
                Ok(None)
            }
        }
    }
}

fn signature_to_user(signature: &git2::Signature) -> User {
    User {
        name: signature.name().map(|x| x.to_owned()),
        email: signature.email().map(|x| x.to_owned()),
    }
}

fn trim_string(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(&trimmed)
    }
}

fn find_coauthors(message: &str) -> Vec<User> {
    lazy_static! {
        static ref CO_AUTH_LINE: Regex = Regex::new(r"(?m)^\s*Co-authored-by:(.*)$").unwrap();
        static ref CO_AUTH_ANGLE_BRACKETS: Regex = Regex::new(r"^(.*)<([^>]+)>$").unwrap();
    }

    CO_AUTH_LINE
        .captures_iter(message)
        .map(|capture_group| {
            let co_author_text = &capture_group[1];
            if let Some(co_author_bits) = CO_AUTH_ANGLE_BRACKETS.captures(co_author_text) {
                User::new(
                    trim_string(&co_author_bits.get(1).unwrap().as_str()),
                    trim_string(co_author_bits.get(2).unwrap().as_str()),
                )
            } else if co_author_text.contains('@') {
                // no angle brackets, but an @
                User::new(None, trim_string(co_author_text))
            } else {
                User::new(trim_string(co_author_text), None)
            }
        })
        .collect()
}

fn commit_file_changes(
    repo: &Repository,
    commit: &Commit,
    commit_tree: &Tree,
    config: GitLogConfig,
) -> Vec<FileChange> {
    if commit.parent_count() == 0 {
        info!("Commit {} has no parent", commit.id());

        scan_diffs(&repo, &commit_tree, None, &commit, None).expect("Can't scan for diffs")
    } else if commit.parent_count() > 1 && !config.include_merges {
        debug!(
            "Not showing file changes for merge commit {:?}",
            commit.id()
        );
        Vec::new()
    } else {
        commit
            .parents()
            .flat_map(|parent| {
                debug!("Getting changes for parent {:?}:", parent);
                let parent_tree = parent.tree().expect("can't get parent tree");
                scan_diffs(
                    &repo,
                    &commit_tree,
                    Some(&parent_tree),
                    &commit,
                    Some(&parent),
                )
                .expect("Can't scan for diffs")
            })
            .collect()
    }
}

fn scan_diffs(
    repo: &Repository,
    commit_tree: &Tree,
    parent_tree: Option<&Tree>,
    commit: &Commit,
    parent: Option<&Commit>,
) -> Result<Vec<FileChange>, Error> {
    let mut diff = repo.diff_tree_to_tree(parent_tree, Some(&commit_tree), None)?;
    // Identify renames, None means default settings - see https://libgit2.org/libgit2/#HEAD/group/diff/git_diff_find_similar
    diff.find_similar(None)?;
    let file_changes = diff
        .deltas()
        .enumerate()
        .filter_map(|(delta_index, delta)| {
            // can we / should we get bytes for binary changes?  Adds show as 0 lines.
            let patch =
                Patch::from_diff(&diff, delta_index).expect("can't get a patch from a diff");
            let (_, lines_added, lines_deleted) = if let Some(patch) = patch {
                patch
                    .line_stats()
                    .expect("Couldn't get line stats from a patch")
            } else {
                warn!("No patch possible diffing {:?} -> {:?}", commit, parent);
                (0, 0, 0)
            };
            summarise_delta(delta, lines_added as u64, lines_deleted as u64)
        });
    Ok(file_changes.collect())
}

fn summarise_delta(delta: DiffDelta, lines_added: u64, lines_deleted: u64) -> Option<FileChange> {
    match delta.status() {
        Delta::Added => {
            let name = delta.new_file().path().unwrap();
            Some(FileChange {
                file: name.to_path_buf(),
                old_file: None,
                change: CommitChange::Add,
                lines_added,
                lines_deleted,
            })
        }
        Delta::Renamed => {
            let old_name = delta.old_file().path().unwrap();
            let new_name = delta.new_file().path().unwrap();
            Some(FileChange {
                file: new_name.to_path_buf(),
                old_file: Some(old_name.to_path_buf()),
                change: CommitChange::Rename,
                lines_added,
                lines_deleted,
            })
        }
        Delta::Deleted => {
            let name = delta.old_file().path().unwrap();
            Some(FileChange {
                file: name.to_path_buf(),
                old_file: None,
                change: CommitChange::Delete,
                lines_added,
                lines_deleted,
            })
        }
        Delta::Modified => {
            let name = delta.new_file().path().unwrap();
            Some(FileChange {
                file: name.to_path_buf(),
                old_file: None,
                change: CommitChange::Modify,
                lines_added,
                lines_deleted,
            })
        }
        Delta::Copied => {
            let old_name = delta.old_file().path().unwrap();
            let new_name = delta.new_file().path().unwrap();
            Some(FileChange {
                file: new_name.to_path_buf(),
                old_file: Some(old_name.to_path_buf()),
                change: CommitChange::Copied,
                lines_added,
                lines_deleted,
            })
        }
        _ => {
            error!("Not able to handle delta of status {:?}", delta.status());
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::tempdir;
    use test_shared::*;

    #[test]
    fn authorless_message_has_no_coauthors() {
        assert_eq!(find_coauthors("do be do be do"), Vec::<User>::new());
    }

    #[test]
    fn can_get_coauthors_from_message() {
        let message = r#"This is a commit message
        not valid: Co-authored-by: fred jones
        Co-authored-by: valid user <valid@thing.com>
        Co-authored-by: <be.lenient@any-domain.com>
        Co-authored-by: bad@user <this isn't really trying to be clever>
        ignore random lines
        Co-authored-by: if there's no at it's a name
        Co-authored-by: if there's an @ it's email@thing.com
        ignore trailing lines
        "#;

        let expected = vec![
            User::new(Some("valid user"), Some("valid@thing.com")),
            User::new(None, Some("be.lenient@any-domain.com")),
            User::new(
                Some("bad@user"),
                Some("this isn't really trying to be clever"),
            ),
            User::new(Some("if there's no at it's a name"), None),
            User::new(None, Some("if there's an @ it's email@thing.com")),
        ];

        assert_eq!(find_coauthors(message), expected);
    }

    #[test]
    fn can_extract_basic_git_log() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;
        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        assert_eq!(git_log.workdir.canonicalize()?, git_root.canonicalize()?);

        let err_count = git_log.iterator()?.filter(|x| Result::is_err(x)).count();
        assert_eq!(err_count, 0);

        let entries: Vec<_> = git_log.iterator()?.filter_map(Result::ok).collect();

        assert_eq_json_file(&entries, "./tests/expected/git/git_sample.json");

        Ok(())
    }

    #[test]
    fn git_log_can_include_merge_changes() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default().include_merges(true))?;

        let err_count = git_log.iterator()?.filter(Result::is_err).count();
        assert_eq!(err_count, 0);

        let entries: Vec<_> = git_log.iterator()?.filter_map(Result::ok).collect();

        assert_eq_json_file(&entries, "./tests/expected/git/git_sample_with_merges.json");

        Ok(())
    }

    #[allow(clippy::unreadable_literal)]
    #[test]
    fn git_log_can_limit_to_recent_history() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default().since(1558521694))?;

        let err_count = git_log.iterator()?.filter(Result::is_err).count();
        assert_eq!(err_count, 0);

        let ids: Vec<_> = git_log
            .iterator()?
            .filter_map(Result::ok)
            .map(|h| (h.summary.clone(), h.commit_time))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("renaming".to_owned(), 1558533240u64),
                ("just changed parent.clj".to_owned(), 1558524371u64),
                ("Merge branch \'fiddling\'".to_owned(), 1558521695u64)
            ]
        );

        Ok(())
    }

    #[test]
    fn git_log_tracks_renames() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("rename_simple", gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let err_count = git_log.iterator()?.filter(Result::is_err).count();
        assert_eq!(err_count, 0);

        let mut entries: Vec<_> = git_log.iterator()?.filter_map(Result::ok).collect();
        entries.sort_by(|a, b| a.author_time.cmp(&b.author_time));

        let changes: Vec<String> = entries
            .iter()
            .map(|entry| entry.summary.to_owned())
            .collect();

        assert_eq!(
            changes,
            vec![
                "initial commit",
                "unrelated commit",
                "moving a to c",
                "moving and renaming"
            ]
        );

        let file_changes: Vec<Vec<FileChange>> = entries
            .iter()
            .map(|entry| {
                let mut entries = entry.file_changes.clone();
                entries.sort_by(|a, b| a.file.cmp(&b.file));
                entries
            })
            .collect();

        assert_eq_json_value(
            &file_changes,
            &json!([
                [{"change":"Add",
                  "file":"a.txt",
                  "lines_added": 4,
                  "lines_deleted": 0,
                  "old_file": null}
                ],
                [{"change":"Add",
                  "file":"b.txt",
                  "lines_added": 1,
                  "lines_deleted": 0,
                  "old_file": null}
                ],
                [{"change":"Rename",
                  "file":"c.txt",
                  "lines_added": 0,
                  "lines_deleted": 0,
                  "old_file": "a.txt"}
                ],
                [{"change":"Rename",
                  "file":"d.txt",
                  "lines_added": 1,
                  "lines_deleted": 0,
                  "old_file": "c.txt"}
                ]
               ]
            ),
        );

        Ok(())
    }
}
/*
<Array([
<    Array([
<        Object({
<            "change": String(
<                "Add",
<            ),
<            "file": String(
<                "a.txt",
<            ),
<            "lines_added": Number(
<                4,
<            ),
<            "lines_deleted": Number(
<                0,
<            ),
<            "old_file": Null,
<        }),
<    ]),
<    Array([
<        Object({
<            "change": String(
<                "Add",
<            ),
<            "file": String(
<                "b.txt",
<            ),
<            "lines_added": Number(
<                1,
<            ),
<            "lines_deleted": Number(
<                0,
<            ),
<            "old_file": Null,
<        }),
<    ]),
<    Array([
<        Object({
<            "change": String(
<                "Rename",
<            ),
<            "file": String(
<                "c.txt",
<            ),
<            "lines_added": Number(
<                0,
<            ),
<            "lines_deleted": Number(
<                0,
<            ),
<            "old_file": String(
<                "a.txt",
<            ),
<        }),
<    ]),
<    Array([
<        Object({
<            "change": String(
<                "Rename",
<            ),
<            "file": String(
<                "d.txt",
<            ),
<            "lines_added": Number(
<                1,
<            ),
<            "lines_deleted": Number(
<                0,
<            ),
<            "old_file": String(
<                "c.txt",
<            ),
<        }),
<    ]),
<])
*/
// run a single test with:
// cargo test -- --nocapture can_extract_basic_git_log | grep -v "running 0 tests" | grep -v "0 passed" | grep -v -e '^\s*$'
