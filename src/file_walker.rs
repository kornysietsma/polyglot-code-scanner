#![warn(clippy::all)]

use crate::{polyglot_data::PolyglotData, FeatureFlags};

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
) -> Result<(), Error> {
    for tic in toxicity_indicator_calculators.iter_mut() {
        tic.visit_node(node, path)?;
    }
    Ok(())
}

const LOG_INTERVAL_SECS: u64 = 60 * 5;

fn walk_tree_walker(
    walker: Walk,
    prefix: &Path,
    name: &str,
    id: Option<&str>,
    toxicity_indicator_calculators: &mut [Box<dyn ToxicityIndicatorCalculator>],
    features: &FeatureFlags, // features just for JSON output
) -> Result<PolyglotData, Error> {
    let mut tree = FlareTreeNode::new(flare::ROOT_NAME, false);

    apply_calculators_to_node(&mut tree, prefix, toxicity_indicator_calculators)?;

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
            apply_calculators_to_node(&mut f, p, toxicity_indicator_calculators)?;
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
    Ok(PolyglotData::new(name, id, tree, features.clone()))
}

pub fn walk_directory(
    root: &Path,
    name: &str,
    id: Option<&str>,
    follow_symlinks: bool,
    toxicity_indicator_calculators: &mut [Box<dyn ToxicityIndicatorCalculator>],
    features: &FeatureFlags, // features just for JSON output
) -> Result<PolyglotData, Error> {
    walk_tree_walker(
        WalkBuilder::new(root)
            .add_custom_ignore_filename(".polyglot_code_scanner_ignore")
            .follow_links(follow_symlinks)
            .sort_by_file_name(std::cmp::Ord::cmp)
            .build(),
        root,
        name,
        id,
        toxicity_indicator_calculators,
        features,
    )
}

#[cfg(test)]
mod test {
    use crate::polyglot_data::IndicatorMetadata;

    use super::*;
    use test_shared::assert_eq_json_file;

    #[test]
    fn scanning_a_filesystem_builds_a_tree() {
        let root = Path::new("./tests/data/simple/");
        let tree = walk_directory(
            root,
            "test",
            Some("test-id"),
            false,
            &mut Vec::new(),
            &FeatureFlags::default(),
        )
        .unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files.json");
    }

    #[test]
    fn scanning_a_filesystem_can_follow_symlinks() {
        let root = Path::new("./tests/data/simple_linked/");
        let tree = walk_directory(
            root,
            "test",
            Some("test-id"),
            true,
            &mut Vec::new(),
            &FeatureFlags::default(),
        )
        .unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files.json");
    }

    #[derive(Debug)]
    struct FirstTIC {}

    impl ToxicityIndicatorCalculator for FirstTIC {
        fn name(&self) -> String {
            "foo".to_string()
        }
        fn visit_node(&mut self, node: &mut FlareTreeNode, path: &Path) -> Result<(), Error> {
            if path.is_file() {
                // only mutate files!  If we rename dirs, the parent relationship breaks
                let mut name = node.name().clone();
                name.push("!");
                node.set_name(&name);
            }
            Ok(())
        }
        fn apply_metadata(&self, _metadata: &mut IndicatorMetadata) -> Result<(), Error> {
            unimplemented!()
        }
    }

    #[derive(Debug)]
    struct SecondTIC {}

    impl ToxicityIndicatorCalculator for SecondTIC {
        fn name(&self) -> String {
            "filename".to_string()
        }
        fn visit_node(&mut self, node: &mut FlareTreeNode, path: &Path) -> Result<(), Error> {
            if path.is_file() {
                // only mutate files!  If we rename dirs, the parent relationship breaks
                let mut name = node.name().clone();
                name.push("?");
                node.set_name(&name);
            }
            Ok(())
        }

        fn apply_metadata(&self, _metadata: &mut IndicatorMetadata) -> Result<(), Error> {
            unimplemented!()
        }
    }

    #[test]
    fn scanning_merges_data_from_mutators() {
        let root = Path::new("./tests/data/simple/");
        let first = FirstTIC {};
        let second = SecondTIC {};
        let calculators: &mut Vec<Box<dyn ToxicityIndicatorCalculator>> =
            &mut vec![Box::new(first), Box::new(second)];

        let tree = walk_directory(
            root,
            "test",
            Some("test-id"),
            false,
            calculators,
            &FeatureFlags::default(),
        )
        .unwrap();

        assert_eq_json_file(&tree, "./tests/expected/simple_files_with_indicators.json");
    }

    // TODO: we have no unit test for new metadata - should we?
}
