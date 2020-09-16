#![warn(clippy::all)]
use crate::git_logger::{CommitChange, FileChange, GitLog, GitLogEntry, User};
use chrono::offset::TimeZone;
use chrono::Utc;
use failure::Error;
use git2::Oid;
use indicatif::{ProgressBar, ProgressStyle};
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

impl GitFileHistory {
    pub fn new(log: &mut GitLog) -> Result<GitFileHistory, Error> {
        let mut last_commit: u64 = 0;
        let mut history_by_file = HashMap::<PathBuf, Vec<FileHistoryEntry>>::new();
        let progress_bar = ProgressBar::new_spinner()
            .with_style(ProgressStyle::default_spinner().template("[{elapsed}] {msg}"));

        // for handling renames, this needs to be a 2-pass process

        // This is ugly! I need to think of cleaning up, probably in one of two ways:
        // 1. ditch the whole "expose an iterator" interface - if we're loading it all into memory anyway, there's no point, could make the code cleaner and maybe get rid of the ugly use of Rc<RefCell<>>
        // 2. fully split the parsing into two passes, one to get parent/child info and one to get file summary.  This would use less memory - but might be slower?  YAGNI I think.

        let log_iterator = log.iterator()?;
        // I can't find a cleaner way for an iterator to have side effects
        let git_file_future_registry = log_iterator.git_file_future_registry();
        let log_entries: Vec<Result<GitLogEntry, Error>> = log_iterator.collect();

        // safe to borrow this now as the iterator has gone and can't mutate any more
        let git_file_future_registry = git_file_future_registry.borrow();

        for entry in log_entries {
            progress_bar.tick();
            match entry {
                Ok(entry) => {
                    let commit_time = *entry.commit_time();
                    let fmt_time = Utc.timestamp(commit_time as i64, 0).to_string();
                    progress_bar.set_message(&fmt_time);
                    if commit_time > last_commit {
                        last_commit = commit_time;
                    }
                    for file_change in entry.clone().file_changes() {
                        // TODO: use Oids so we don't need ugly conversion.
                        let final_filename = git_file_future_registry
                            .final_name(&Oid::from_str(entry.id()).unwrap(), file_change.file());
                        if let Some(filename) = final_filename {
                            let hash_entry =
                                history_by_file.entry(filename).or_insert_with(Vec::new);
                            let new_entry = FileHistoryEntry::from(&entry, &file_change);
                            hash_entry.push(new_entry);
                        } else {
                            debug!(
                                "Not storing history for deleted file {:?}",
                                file_change.file()
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!("Ignoring invalid git log entry: {:?}", e);
                }
            }
        }

        Ok(GitFileHistory {
            workdir: log.workdir().to_owned(),
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
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;

        let mut git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(&mut git_log)?;

        assert_eq!(history.workdir.canonicalize()?, git_root.canonicalize()?);

        // assert_eq_json_str(&history.history_by_file, "{}");
        assert_eq_json_file(
            &history.history_by_file,
            "./tests/expected/git/git_sample_by_filename.json",
        );

        Ok(())
    }

    #[test]
    fn can_tell_if_file_is_in_git_repo() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;

        let mut git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(&mut git_log)?;

        assert_eq!(
            history.is_repo_for(&git_root.join("simple/parent.clj"))?,
            true
        );

        Ok(())
    }

    #[test]
    fn can_get_history_for_file() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;

        let mut git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(&mut git_log)?;

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
        let git_root = unzip_git_sample("git_sample", gitdir.path())?;

        let mut git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(&mut git_log)?;

        let new_file = git_root.join("simple/nonesuch.clj");
        std::fs::File::create(&new_file)?;

        let file_history = history.history_for(&new_file)?;

        assert_eq!(file_history.is_none(), true);

        Ok(())
    }

    #[test]
    fn can_get_history_for_complex_renamed_files() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("rename_complex", gitdir.path())?;
        /*
        This is generated by the script in tests/data/builders/renaming/rename_complex.sh

        log is:

        * 3629e5a (HEAD -> master) restoring deleted z
        *   261e027 merging dave work with fixes
        |\
        | * c3b47c3 (dave_work) rename bb to b, a2 back to a
        | * 500a621 rename a1 to a2, add bb, kill z
        * |   fac9419 merging jay work
        |\ \
        | * | 34b904b (jay_work) rename bee to b, aa back to a
        | * | 3bd2d90 rename a1 to aa, add bee
        | |/
        * | 8be47df rename a1 back to a prep merging
        |/
        * 388e644 rename a to a1
        * bd6d7df initial commit
        */

        let mut git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(&mut git_log)?;

        let file_history = history.history_for(&git_root.join("a.txt"))?;

        let ids: Vec<_> = file_history.unwrap().iter().map(|h| &h.id).collect();
        assert_eq!(
            ids,
            // all of these refs have a file that ends up being "a.txt" via renames and merges:
            vec![
                "c3b47c335ebd9dbb9b0c9922bc258555a2cf71c9",
                "500a621e9e83612f51dbce15202cd7bef3c88f00",
                "34b904b010abf316167bba7a7ce2b4a5996cc0d1",
                "3bd2d9088ee5b051ada1bd30f07e7bcd390f6327",
                "8be47dfc0a25ec27941413619f632a1fa66e5ba5",
                "388e644e9240aa333fe669069bb00d418ffca500",
                "bd6d7dfa063ec95ebc3bad7bffd4262e3702b77c",
            ]
        );

        Ok(())
    }

    #[test]
    fn deleted_files_dont_have_history() -> Result<(), Error> {
        let gitdir = tempdir()?;
        let git_root = unzip_git_sample("rename_complex", gitdir.path())?;

        let mut git_log = GitLog::new(&git_root, GitLogConfig::default())?;

        let history = GitFileHistory::new(&mut git_log)?;

        let file_history = history.history_for(&git_root.join("z.txt"))?;

        assert_eq!(file_history.is_some(), true);

        let ids: Vec<_> = file_history.unwrap().iter().map(|h| &h.id).collect();
        assert_eq!(
            ids,
            // z.txt is only using the final commit, not the earlier file that was deleted.
            vec!["3629e5a8d8d7547bac749530eb540d0f61535cd1",]
        );

        Ok(())
    }
}
