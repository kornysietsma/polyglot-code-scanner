#![warn(clippy::all)]
use failure::Error;
use git2::{Commit, Delta, DiffDelta, ObjectType, Odb, Oid, Patch, Repository, Tree};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct GitLogConfig {
    /// include merge commits in file stats - usually excluded by `git log` - see https://stackoverflow.com/questions/37801342/using-git-log-to-display-files-changed-during-merge
    include_merges: bool,
}

impl GitLogConfig {
    pub fn default() -> GitLogConfig {
        GitLogConfig {
            include_merges: false,
        }
    }
    pub fn include_merges(self, include_merges: bool) -> GitLogConfig {
        let mut config = self;
        config.include_merges = include_merges;
        config
    }
}

#[derive(Debug, Serialize)]
pub struct GitLog {
    /// repo work dir - always canonical
    workdir: PathBuf,
    entries: Vec<GitLogEntry>,
}

/// simplified user info - based on git2::Signature but using blanks not None for now.
/// TODO: consider using None - let the UI decide how to handle?
#[derive(Debug, Serialize, PartialEq, Clone)]
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

    /// used for deduping users - returns email if it exists, otherwise name, otherwise an error value
    pub fn identifier(&self) -> &str {
        if let Some(email) = &self.email {
            email
        } else if let Some(name) = &self.name {
            name
        } else {
            "[blank user]"
        }
    }
}

/// simplified commit log entry
#[derive(Debug, Serialize, Clone)]
pub struct GitLogEntry {
    id: String,
    summary: String,
    parents: Vec<String>,
    committer: User,
    commit_time: i64,
    author: User,
    author_time: i64,
    co_authors: Vec<User>,
    file_changes: Vec<FileChange>,
}

/// the various kinds of git change we care about - a serializable subset of git2::Delta
#[derive(Debug, Serialize, Clone)]
pub enum CommitChange {
    Add,
    Rename,
    Delete,
    Modify,
    Copied,
}

/// Stats for file changes
#[derive(Debug, Serialize, Clone)]
pub struct FileChange {
    file: PathBuf,
    old_file: Option<PathBuf>,
    change: CommitChange,
    lines_added: u64,
    lines_deleted: u64,
}

/// For each file we just keep a simplified history - what the changes were, by whom, and when.
#[derive(Debug, Serialize, Builder)]
#[builder(setter(into), pattern = "owned")]
pub struct FileHistoryEntry {
    pub id: String,
    pub committer: User,
    pub commit_time: i64,
    pub author: User,
    pub author_time: i64,
    pub co_authors: Vec<User>,
    pub change: CommitChange,
    pub lines_added: u64,
    pub lines_deleted: u64,
}

impl FileHistoryEntry {
    fn from(entry: &GitLogEntry, file_change: &FileChange) -> FileHistoryEntry {
        let entry = entry.clone();
        let file_change = file_change.clone();
        FileHistoryEntry {
            id: entry.id,
            committer: entry.committer,
            commit_time: entry.commit_time,
            author: entry.author,
            author_time: entry.author_time,
            co_authors: entry.co_authors,
            change: file_change.change,
            lines_added: file_change.lines_added,
            lines_deleted: file_change.lines_deleted,
        }
    }
}

#[cfg(test)]
impl FileHistoryEntryBuilder {
    pub fn test_default() -> Self {
        FileHistoryEntryBuilder::default()
            .co_authors(Vec::new())
            .change(CommitChange::Add)
            .lines_added(0u64)
            .lines_deleted(0u64)
    }
    pub fn emails(self, email: &str) -> Self {
        self.committer(User::new(None, Some(email)))
            .author(User::new(None, Some(email)))
    }

    pub fn times(self, time: i64) -> Self {
        self.commit_time(time).author_time(time)
    }
}

#[derive(Debug, Serialize)]
pub struct GitFileHistory {
    /// repo work dir - always canonical
    workdir: PathBuf,
    history_by_file: HashMap<PathBuf, Vec<FileHistoryEntry>>,
    last_commit: i64,
}

impl GitFileHistory {
    pub fn new(log: GitLog) -> Result<GitFileHistory, Error> {
        let mut last_commit: i64 = 0;
        let mut history_by_file = HashMap::<PathBuf, Vec<FileHistoryEntry>>::new();
        for entry in log.entries {
            if entry.commit_time > last_commit {
                last_commit = entry.commit_time;
            }
            for file_change in entry.clone().file_changes {
                let hash_entry = history_by_file
                    .entry(file_change.file.clone()) // TODO: how to avoid clone? and the one 2 lines earlier?
                    .or_insert_with(Vec::new);
                let new_entry = FileHistoryEntry::from(&entry, &file_change);
                hash_entry.push(new_entry);
            }
        }
        Ok(GitFileHistory {
            workdir: log.workdir,
            history_by_file,
            last_commit,
        })
    }

    /// true if this repo is valid for this file - file must exist (as we canonicalize it)
    pub fn is_repo_for(&self, file: &Path) -> Result<bool, Error> {
        let canonical_file = file.canonicalize()?;
        Ok(canonical_file.starts_with(&self.workdir))
    }

    /// get git history for this file - file must exist (as we canonicalize it)
    pub fn history_for(&self, file: &Path) -> Result<Option<&Vec<FileHistoryEntry>>, Error> {
        let canonical_file = file.canonicalize()?;
        let relative_file = canonical_file.strip_prefix(&self.workdir)?;
        Ok(self.history_by_file.get(relative_file))
    }

    pub fn last_commit(&self) -> i64 {
        self.last_commit
    }
}

impl GitLog {
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    // TODO: move this into GitLog impl
    pub fn new(start_dir: &Path, config: GitLogConfig) -> Result<GitLog, Error> {
        let repo = Repository::discover(start_dir)?;

        let workdir = repo
            .workdir()
            .ok_or_else(|| format_err!("bare repository - no workdir"))?
            .canonicalize()?;

        debug!("work dir: {:?}", workdir);

        let odb = repo.odb()?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;

        // TODO: filter by dates! This will get mad on a big history

        let entries: Result<Vec<_>, _> = revwalk
            .map(|oid| summarise_commit(&repo, &odb, oid, config))
            .collect();

        let entries = entries?.into_iter().flat_map(|e| e).collect();

        Ok(GitLog {
            workdir: workdir.to_owned(),
            entries,
        })
    }
}

fn summarise_commit(
    repo: &Repository,
    odb: &Odb,
    oid: Result<Oid, git2::Error>,
    config: GitLogConfig,
) -> Result<Option<GitLogEntry>, Error> {
    let oid = oid?;
    let kind = odb.read(oid)?.kind();
    match kind {
        ObjectType::Commit => {
            let commit = repo.find_commit(oid)?;
            debug!("processing {:?}", commit);
            let author = commit.author();
            let committer = commit.committer();
            let author_time = author.when().seconds();
            let commit_time = committer.when().seconds();
            let other_time = commit.time().seconds();
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
            let file_changes = commit_file_changes(&repo, &commit, &commit_tree, config);
            Ok(Some(GitLogEntry {
                id: oid.to_string(),
                summary: commit.summary().unwrap_or("[no message]").to_string(),
                parents: commit.parent_ids().map({ |p| p.to_string() }).collect(),
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
    use crate::test_helpers::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn unzip_git_sample(workdir: &Path) -> Result<PathBuf, Error> {
        unzip_to_dir(workdir, "tests/data/git/git_sample.zip")?;
        Ok(PathBuf::from(workdir).join("git_sample"))
    }

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
        let git_root = unzip_git_sample(gitdir.path())?;
        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        assert_eq!(git_log.workdir.canonicalize()?, git_root.canonicalize()?);

        assert_eq_json_file(&git_log.entries, "./tests/expected/git/git_sample.json");

        Ok(())
    }

    #[test]
    fn git_log_can_include_merge_changes() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample(gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default().include_merges(true))?;

        assert_eq_json_file(
            &git_log.entries,
            "./tests/expected/git/git_sample_with_merges.json",
        );

        Ok(())
    }

    #[test]
    fn can_get_log_by_filename() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample(gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(git_log)?;

        assert_eq!(history.workdir.canonicalize()?, git_root.canonicalize()?);

        assert_eq_json_file(
            &history.history_by_file,
            "./tests/expected/git/git_sample_by_filename.json",
        );

        Ok(())
    }

    #[test]
    fn can_tell_if_file_is_in_git_repo() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample(gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(git_log)?;

        assert_eq!(
            history.is_repo_for(&git_root.join("simple/parent.clj"))?,
            true
        );

        Ok(())
    }

    #[test]
    fn can_get_history_for_file() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample(gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(git_log)?;

        let file_history = history.history_for(&git_root.join("simple/parent.clj"))?;

        assert_eq!(file_history.is_some(), true);

        let ids: Vec<_> = file_history.unwrap().iter().map(|h| &h.id).collect();
        assert_eq!(
            ids,
            vec![
                "0dbd54d4c524ecc776f381e660cce9b2dd92162c",
                "a0ae9997cfdf49fd0cbf54dacc72c778af337519",
                "ca239efb9b26db57ac9e2ec3e2df1c42578a46f8"
            ]
        );

        assert_eq!(history.last_commit(), 1_558_533_240);

        Ok(())
    }

    #[test]
    fn no_history_for_files_not_known() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample(gitdir.path())?;

        let git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(git_log)?;

        let new_file = git_root.join("simple/nonesuch.clj");
        std::fs::File::create(&new_file)?;

        let file_history = history.history_for(&new_file)?;

        assert_eq!(file_history.is_none(), true);

        Ok(())
    }

}

// run a single test with:
// cargo test -- --nocapture can_extract_basic_git_log | grep -v "running 0 tests" | grep -v "0 passed" | grep -v -e '^\s*$'
