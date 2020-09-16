#![warn(clippy::all)]
use git2::Oid;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Track file changes for a file - renames and deletes
#[derive(Debug, Clone)]
pub struct GitFileFutureRegistry {
    rev_changes: HashMap<Oid, RevChange>,
}

#[derive(Debug, Clone)]
struct RevChange {
    files: HashMap<PathBuf, FileNameChange>,
    /// first child is generally used only, it is the main branch - don't divert into other branches!
    children: Vec<Oid>,
}

#[derive(Debug, Clone)]
pub enum FileNameChange {
    Renamed(PathBuf),
    Deleted(),
}

impl RevChange {
    pub fn new() -> Self {
        RevChange {
            files: HashMap::new(),
            children: Vec::new(),
        }
    }
}

impl GitFileFutureRegistry {
    pub fn new() -> Self {
        GitFileFutureRegistry {
            rev_changes: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        id: &Oid,
        parent_ids: &[Oid],
        file_changes: &[(PathBuf, FileNameChange)],
    ) {
        let entry = self.rev_changes.entry(*id).or_insert_with(RevChange::new);
        (*entry).files.extend(file_changes.iter().cloned());
        for parent_id in parent_ids {
            let pentry = self
                .rev_changes
                .entry(*parent_id)
                .or_insert_with(RevChange::new);
            (*pentry).children.push(*id);
        }
    }

    /// what is this called in the final revision?
    /// returns None if it is deleted, or Some(final name)
    pub fn final_name(&self, ref_id: &Oid, file: &Path) -> Option<PathBuf> {
        let mut current_name: &PathBuf = &file.to_path_buf();
        let mut current_ref: Oid = *ref_id;
        loop {
            let current_change = self.rev_changes.get(&current_ref).unwrap();
            match current_change.files.get(current_name) {
                Some(FileNameChange::Renamed(new_name)) => {
                    current_name = new_name;
                }
                Some(FileNameChange::Deleted()) => return None,
                None => (),
            }
            if let Some(first_child) = current_change.children.get(0) {
                current_ref = *first_child;
            // and loop will continue
            } else {
                // no children, so finished looking into the future
                return Some(current_name.to_path_buf());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use failure::Error;
    use pretty_assertions::assert_eq;

    fn pb(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn trivial_repo_returns_original_name() -> Result<(), Error> {
        let mut registry = GitFileFutureRegistry::new();
        let my_id = Oid::from_str("01")?;
        registry.register(&my_id, &[], &[]);
        assert_eq!(
            registry.final_name(&my_id, &pb("foo.txt")),
            Some(pb("foo.txt"))
        );
        Ok(())
    }

    #[test]
    fn simple_rename_returns_old_name() -> Result<(), Error> {
        let mut registry = GitFileFutureRegistry::new();
        let my_id = Oid::from_str("01")?;

        registry.register(
            &my_id,
            &[],
            &[(pb("foo.txt"), FileNameChange::Renamed(pb("bar.txt")))],
        );
        assert_eq!(
            registry.final_name(&my_id, &pb("foo.txt")),
            Some(pb("bar.txt"))
        );
        Ok(())
    }

    #[test]
    fn renames_and_deletes_applied_across_history() -> Result<(), Error> {
        // my bad - this should be a few isolated tests not one big test-all test.
        // classic how my standards slip for side projects!
        let mut registry = GitFileFutureRegistry::new();
        /*
                   +-----+
                   |01   |
                   |add a|
                   |add z|
                   +--+--+
                      |
               +------v------+
               |02           |
               |rename a to b|
               |delete z     |
               +-------------+
               |             |
        +------v------+ +----v--------+
        |04           | |05           |
        |rename b to c| |rename b to d|
        +--------------+--------------+
                       |
              +--------v---------+
              |06 merge          |
              |rename c to afinal|
              |create new z      |
              +------------------+
                */
        let id_1 = Oid::from_str("01")?;
        let id_2 = Oid::from_str("02")?;
        let id_4 = Oid::from_str("04")?;
        let id_5 = Oid::from_str("05")?;
        let id_6 = Oid::from_str("06")?;

        registry.register(
            &id_6,
            &[id_4, id_5],
            &[(pb("c"), FileNameChange::Renamed(pb("afinal")))],
        );
        // NOTE: topological order should (I think?) register rev 4 before rev 5 as it's first
        registry.register(
            &id_4,
            &[id_2],
            &[(pb("b"), FileNameChange::Renamed(pb("c")))],
        );
        registry.register(
            &id_5,
            &[id_2],
            &[(pb("b"), FileNameChange::Renamed(pb("d")))],
        );
        registry.register(
            &id_2,
            &[id_1],
            &[
                (pb("a"), FileNameChange::Renamed(pb("b"))),
                (pb("z"), FileNameChange::Deleted()),
            ],
        );
        registry.register(&id_1, &[], &[]);

        // original a is afinal
        // original z is gone
        assert_eq!(registry.final_name(&id_1, &pb("a")), Some(pb("afinal")));
        assert_eq!(registry.final_name(&id_1, &pb("z")), None);
        // from the perspective of the filesystem after node 2, we know nothing of a any more, only b
        assert_eq!(registry.final_name(&id_2, &pb("b")), Some(pb("afinal")));

        Ok(())
    }
}
