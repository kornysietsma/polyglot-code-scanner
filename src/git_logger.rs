#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(unused_imports)]
use failure::Error;
use git2::DiffDelta;
use git2::Odb;
use git2::Oid;
use git2::{Commit, Delta, ObjectType, Patch, Repository, Status, Tree};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct GitLog {
    entries: Vec<GitLogEntry>,
}

#[derive(Debug, Serialize)]
pub struct GitLogEntry {
    id: String,
    summary: String,
    parents: Vec<String>,
    // commit_time_secs: i64,
    // author_time_secs: i64,
    // author: String,
    // committer: String,
    // co_authors: Vec<String>,
    file_changes: Vec<FileChange>,
}

fn commit_summary(commit: &Commit) -> String {
    format!(
        "Commit: {} : {}",
        commit.id(),
        commit.summary().unwrap_or("no message")
    )
}

pub fn log(start_dir: &Path) -> Result<GitLog, Error> {
    let repo = Repository::discover(start_dir)?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| format_err!("bare repository - no workdir"));

    debug!("work dir: {:?}", workdir);

    let odb = repo.odb()?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    // TODO: filter by dates! This will get mad on a big history

    let entries: Result<Vec<_>, _> = revwalk
        .map(|oid| summarise_commit(&repo, &odb, oid))
        .collect();

    let entries = entries?.into_iter().flat_map(|e| e).collect();

    Ok(GitLog { entries })
}

fn summarise_commit(
    repo: &Repository,
    odb: &Odb,
    oid: Result<Oid, git2::Error>,
) -> Result<Option<GitLogEntry>, Error> {
    let oid = oid?;
    let kind = odb.read(oid)?.kind();
    match kind {
        ObjectType::Commit => {
            let commit = repo.find_commit(oid)?;
            info!("processing {}", commit_summary(&commit));
            let commit_tree = commit.tree()?;
            if commit.parent_count() > 1 {
                info!("Commit has multiple parents!");
                // TODO: are we handling multiple parents right?
            }
            let file_changes = commit_file_changes(&repo, &commit, &commit_tree);
            Ok(Some(GitLogEntry {
                id: oid.to_string(),
                summary: commit.summary().unwrap_or("[no message]").to_string(),
                parents: commit.parent_ids().map({ |p| p.to_string() }).collect(),
                file_changes,
            }))
        }
        _ => {
            debug!("ignoring object type: {}", kind);
            Ok(None)
        }
    }
}

fn commit_file_changes(repo: &Repository, commit: &Commit, commit_tree: &Tree) -> Vec<FileChange> {
    commit
        .parents()
        .flat_map(|parent| {
            info!("Parent {:?}:", parent);
            let parent_tree = parent.tree().expect("can't get parent tree");
            let changes = scan_diffs(&repo, &commit_tree, &parent_tree, &commit, &parent)
                .expect("Can't scan for diffs");
            info!("Changes: {:?}", changes);
            changes
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub enum CommitEntryEvent {
    Add,
    Rename,
    Delete,
    Modify,
    Copied,
}
#[derive(Debug, Serialize)]
pub struct FileChange {
    file: PathBuf,
    old_file: Option<PathBuf>,
    change: CommitEntryEvent,
    lines_added: usize,
    lines_deleted: usize,
}

fn scan_diffs(
    repo: &Repository,
    commit_tree: &Tree,
    parent_tree: &Tree,
    commit: &Commit,
    parent: &Commit,
) -> Result<Vec<FileChange>, Error> {
    let mut diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)?;
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
            summarise_delta(delta, lines_added, lines_deleted)
        });
    Ok(file_changes.collect())
}

fn summarise_delta(
    delta: DiffDelta,
    lines_added: usize,
    lines_deleted: usize,
) -> Option<FileChange> {
    match delta.status() {
        Delta::Added => {
            let name = delta.new_file().path().unwrap();
            Some(FileChange {
                file: name.to_path_buf(),
                old_file: None,
                change: CommitEntryEvent::Add,
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
                change: CommitEntryEvent::Rename,
                lines_added,
                lines_deleted,
            })
        }
        Delta::Deleted => {
            let name = delta.old_file().path().unwrap();
            Some(FileChange {
                file: name.to_path_buf(),
                old_file: None,
                change: CommitEntryEvent::Delete,
                lines_added,
                lines_deleted,
            })
        }
        Delta::Modified => {
            let name = delta.new_file().path().unwrap();
            Some(FileChange {
                file: name.to_path_buf(),
                old_file: None,
                change: CommitEntryEvent::Modify,
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
                change: CommitEntryEvent::Copied,
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

#[derive(Debug, PartialEq, Serialize)]
struct GitData {
    authors: Vec<String>,
    last_change: i64,
}

fn parse_file(filename: &Path) -> Result<GitData, Error> {
    let repo = Repository::discover(filename)?;
    let odb = repo.odb()?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    let mut authors = HashSet::new();
    for oid in revwalk {
        let oid = oid?;
        let kind = odb.read(oid)?.kind();
        if kind == ObjectType::Commit {
            let commit = repo.find_commit(oid)?;
            let author = commit.author();
            let message = commit.message().unwrap_or("no message");
            println!("scanning: {:?}", message);
            let name: String = author.name().unwrap_or("UNKNOWN AUTHOR").to_string();
            authors.insert(name);
        } else {
            println!("Unexpected Kind {:?}", kind);
        }
    }

    Ok(GitData {
        authors: authors.into_iter().collect(),
        last_change: 0,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::Value;

    extern crate tempfile;
    extern crate zip;
    use std::fs::File;
    use std::path::Path;
    use tempfile::tempdir;
    use zip::ZipArchive;

    /// adapted from https://github.com/mvdnes/zip-rs/blob/master/examples/extract.rs
    pub fn unzip_to_dir(dest: &Path, zipname: &str) -> Result<(), Error> {
        let fname = std::path::Path::new(zipname);
        let file = File::open(&fname)?;

        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = PathBuf::from(dest).join(file.sanitized_name());

            if (&*file.name()).ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(&p)?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    }

    #[test]
    fn can_extract_basic_git_log() -> Result<(), Error> {
        let gitdir = tempdir()?;
        unzip_to_dir(gitdir.path(), "tests/data/git/git_sample.zip")?;
        let git_root = PathBuf::from(gitdir.path()).join("git_sample");

        let git_log = log(&git_root)?;

        let json = serde_json::to_string_pretty(&git_log).unwrap();
        let parsed_result: Value = serde_json::from_str(&json).unwrap();

        let expected =
            std::fs::read_to_string(Path::new("./tests/expected/git/git_sample.json")).unwrap();
        let parsed_expected: Value = serde_json::from_str(&expected).unwrap();

        assert_eq!(parsed_result, parsed_expected);

        Ok(())
    }
}

// run a single test with:
// cargo test -- --nocapture can_extract_basic_git_log | grep -v "running 0 tests" | grep -v "0 passed" | grep -v -e '^\s*$'
