#![warn(clippy::all)]
use crate::git_logger::{CommitChange, FileChange, GitLog, GitLogEntry, GitLogVisitor, User};
use failure::Error;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// For each file we just keep a simplified history - what the changes were, by whom, and when.
#[derive(Debug, Serialize, Builder)]
#[builder(setter(into), pattern = "owned")]
pub struct FileHistoryEntry {
    pub id: String,
    pub committer: User,
    pub commit_time: u64,
    pub author: User,
    pub author_time: u64,
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
            id: entry.id().to_owned(),
            committer: entry.committer().clone(),
            commit_time: *entry.commit_time(),
            author: entry.author().clone(),
            author_time: *entry.author_time(),
            co_authors: entry.co_authors().clone(),
            change: file_change.change().clone(),
            lines_added: *file_change.lines_added(),
            lines_deleted: *file_change.lines_deleted(),
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

    pub fn times(self, time: u64) -> Self {
        self.commit_time(time).author_time(time)
    }
}

#[derive(Debug, Serialize)]
pub struct GitFileHistory {
    /// repo work dir - always canonical
    workdir: PathBuf,
    history_by_file: HashMap<PathBuf, Vec<FileHistoryEntry>>,
    last_commit: u64,
}

impl GitLogVisitor for GitFileHistory {
    fn visit_entry(&mut self, entry: &GitLogEntry) -> Result<(), Error> {
        if *entry.commit_time() > self.last_commit {
            self.last_commit = *entry.commit_time();
        }
        for file_change in entry.clone().file_changes() {
            let hash_entry = self
                .history_by_file
                .entry(file_change.file().clone()) // TODO: how to avoid clone? and the one 2 lines earlier?
                .or_insert_with(Vec::new);
            let new_entry = FileHistoryEntry::from(&entry, &file_change);
            hash_entry.push(new_entry);
        }

        Ok(())
    }
}

impl GitFileHistory {
    pub fn new(log: GitLog) -> Result<GitFileHistory, Error> {
        let mut history = GitFileHistory {
            workdir: log.workdir().to_owned(),
            history_by_file: HashMap::<PathBuf, Vec<FileHistoryEntry>>::new(),
            last_commit: 0,
        };
        log.log(&mut history)?;
        Ok(history)
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

    pub fn last_commit(&self) -> u64 {
        self.last_commit
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::git_logger::GitLogConfig;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;
    use test_shared::*;

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
