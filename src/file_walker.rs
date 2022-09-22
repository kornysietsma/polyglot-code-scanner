#![warn(clippy::all)]

use crate::polyglot_data::PolyglotData;

use super::flare;
use super::flare::FlareTreeNode;
use super::toxicity_indicator_calculator::ToxicityIndicatorCalculator;
use anyhow::Error;
use ignore::{Walk, WalkBuilder};
#[allow(unused_imports)]
use path_slash::PathExt;
use std::{path::Path, time::Instant};

fn apply_calculators_to_node(
    node: &mut FlareTreeNode,
    path: &Path,
    toxicity_indicator_calculators: &mut [Box<dyn ToxicityIndicatorCalculator>],
) {
    toxicity_indicator_calculators.iter_mut().for_each(|tic| {
        let indicators = tic.calculate(path);
        match indicators {
            Ok(Some(indicators)) => node.add_data(tic.name(), indicators),
            Ok(None) => (),
            Err(error) => {
                warn!(
                    "Can't find {} indicators for {:?} - cause: {}",
                    tic.name(),
                    node,
                    error
                );
            }
        }
    });
}

const LOG_INTERVAL_SECS: u64 = 60 * 5;

fn walk_tree_walker(
    walker: Walk,
    prefix: &Path,
    name: &str,
    id: Option<&str>,
    toxicity_indicator_calculators: &mut [Box<dyn ToxicityIndicatorCalculator>],
) -> Result<PolyglotData, Error> {
    let mut tree = FlareTreeNode::new(flare::ROOT_NAME, false);

    apply_calculators_to_node(&mut tree, prefix, toxicity_indicator_calculators);

    let mut last_log = Instant::now();
    info!("Walking file tree");

    for result in walker.map(|r| r.expect("File error!")).skip(1) {
        let p = result.path();
        let relative = p.strip_prefix(prefix)?;
        let elapsed_since_log = last_log.elapsed();
        if elapsed_since_log.as_secs() > LOG_INTERVAL_SECS {
            info!("Walking progress: {:?}", relative);
            last_log = Instant::now();
        }

        let new_child = if p.is_dir() || p.is_file() {
            let mut f = FlareTreeNode::new(p.file_name().unwrap(), p.is_file());
            apply_calculators_to_node(&mut f, p, toxicity_indicator_calculators);
            Some(f)
        } else {
            warn!("Not a file or dir: {:?} - skipping", p);
            None
        };

        if let Some(new_child) = new_child {
            match relative.parent() {
                Some(new_parent) => {
                    let parent = tree
                        .get_in_mut(&mut new_parent.components())
                        .expect("no parent found!");
                    parent.append_child(new_child);
                }
                None => {
                    tree.append_child(new_child);
                }
            }
        }
    }
    info!("finished walking file tree");
    Ok(PolyglotData::new(name, id, tree))
}

pub fn walk_directory(
    root: &Path,
    name: &str,
    id: Option<&str>,
    follow_symlinks: bool,
    toxicity_indicator_calculators: &mut [Box<dyn ToxicityIndicatorCalculator>],
) -> Result<PolyglotData, Error> {
    walk_tree_walker(
        WalkBuilder::new(root)
            .add_custom_ignore_filename(".polyglot_code_scanner_ignore")
            .follow_links(follow_symlinks)
            .sort_by_file_name(|name1, name2| name1.cmp(name2))
            .build(),
        root,
        name,
        id,
        toxicity_indicator_calculators,
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::{json, Value};
    use test_shared::*;

    #[test]
    fn scanning_a_filesystem_builds_a_tree() {
        let root = Path::new("./tests/data/simple/");
        let tree = walk_directory(root, "test", Some("test-id"), false, &mut Vec::new()).unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files.json")
    }

    #[test]
    fn scanning_a_filesystem_can_follow_symlinks() {
        let root = Path::new("./tests/data/simple_linked/");
        let tree = walk_directory(root, "test", Some("test-id"), true, &mut Vec::new()).unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files.json")
    }

    #[derive(Debug)]
    struct SimpleTIC {}

    impl ToxicityIndicatorCalculator for SimpleTIC {
        fn name(&self) -> String {
            "foo".to_string()
        }
        fn calculate(&mut self, path: &Path) -> Result<Option<serde_json::Value>, Error> {
            if path.is_file() {
                Ok(Some(json!("bar")))
            } else {
                Ok(None)
            }
        }

        fn metadata(&self) -> Result<Option<Value>, Error> {
            unimplemented!()
        }
    }

    #[derive(Debug)]
    struct SelfNamingTIC {}

    impl ToxicityIndicatorCalculator for SelfNamingTIC {
        fn name(&self) -> String {
            "filename".to_string()
        }
        fn calculate(&mut self, path: &Path) -> Result<Option<serde_json::Value>, Error> {
            if path.is_file() {
                Ok(Some(json!(path.to_slash_lossy())))
            } else {
                Ok(None)
            }
        }

        fn metadata(&self) -> Result<Option<Value>, Error> {
            unimplemented!()
        }
    }

    #[test]
    fn scanning_merges_data_from_mutators() {
        let root = Path::new("./tests/data/simple/");
        let simple_tic = SimpleTIC {};
        let self_naming_tic = SelfNamingTIC {};
        let calculators: &mut Vec<Box<dyn ToxicityIndicatorCalculator>> =
            &mut vec![Box::new(simple_tic), Box::new(self_naming_tic)];

        let tree = walk_directory(root, "test", Some("test-id"), false, calculators).unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files_with_data.json");
    }

    #[derive(Debug)]
    struct MutableTIC {
        count: i64,
    }

    impl ToxicityIndicatorCalculator for MutableTIC {
        fn name(&self) -> String {
            "count".to_string()
        }
        fn calculate(&mut self, _path: &Path) -> Result<Option<serde_json::Value>, Error> {
            let result = json!(self.count);
            self.count += 1;
            Ok(Some(result))
        }

        fn metadata(&self) -> Result<Option<Value>, Error> {
            unimplemented!()
        }
    }

    #[test]
    fn can_mutate_state_of_calculator() {
        let root = Path::new("./tests/data/simple/");
        let tic = MutableTIC { count: 0 };
        let calculators: &mut Vec<Box<dyn ToxicityIndicatorCalculator>> = &mut vec![Box::new(tic)];

        let tree = walk_directory(root, "test", Some("test-id"), false, calculators).unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files_with_counts.json");
    }
}
