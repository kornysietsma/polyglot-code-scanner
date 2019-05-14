#![warn(clippy::all)]

use super::flare;
use super::flare::FlareTreeNode;
use failure::Error;
use ignore::{Walk, WalkBuilder};
use std::path::Path;

/// File Metrics callback - note this only runs on files not directories - if there's a need for directory data, this will need to change.
pub trait NamedFileMetricCalculator {
    fn name(&self) -> String;
    fn calculate_metrics(&self, path: &Path) -> Result<serde_json::Value, Error>;
}

fn walk_tree_walker(
    walker: Walk,
    prefix: &Path,
    file_metric_calculators: Vec<Box<dyn NamedFileMetricCalculator>>,
) -> Result<flare::FlareTreeNode, Error> {
    let mut tree = FlareTreeNode::from_dir("flare");

    for result in walker.map(|r| r.expect("File error!")).skip(1) {
        // note we skip the root directory!
        let p = result.path();
        let relative = p.strip_prefix(prefix)?;
        let new_child = if p.is_file() {
            let mut f = FlareTreeNode::from_file(p.file_name().unwrap());
            file_metric_calculators.iter().for_each(|fmc| {
                let metrics = fmc.calculate_metrics(p);
                match metrics {
                    Ok(metrics) => f.add_data(fmc.name().to_string(), metrics),
                    Err(error) => {
                        warn!(
                            "Can't find {} metrics for {:?} - cause: {}",
                            fmc.name(),
                            p,
                            error
                        );
                    }
                }
            });
            Some(f)
        } else if p.is_dir() {
            Some(FlareTreeNode::from_dir(p.file_name().unwrap()))
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
    Ok(tree)
}

pub fn walk_directory(
    root: &Path,
    file_metric_calculators: Vec<Box<dyn NamedFileMetricCalculator>>,
) -> Result<flare::FlareTreeNode, Error> {
    walk_tree_walker(
        WalkBuilder::new(root).build(),
        root,
        file_metric_calculators,
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;
    use serde_json::Value;

    #[test]
    fn scanning_a_filesystem_builds_a_tree() {
        let root = Path::new("./tests/data/simple/");
        let tree = walk_directory(root, Vec::new()).unwrap();
        let json = serde_json::to_string_pretty(&tree).unwrap();
        let parsed_result: Value = serde_json::from_str(&json).unwrap();

        let expected =
            std::fs::read_to_string(Path::new("./tests/expected/simple_files.json")).unwrap();
        let parsed_expected: Value = serde_json::from_str(&expected).unwrap();

        assert_eq!(parsed_result, parsed_expected);
    }

    struct SimpleFMC {}

    impl NamedFileMetricCalculator for SimpleFMC {
        fn name(&self) -> String {
            "foo".to_string()
        }
        fn calculate_metrics(&self, _path: &Path) -> Result<serde_json::Value, Error> {
            Ok(json!("bar"))
        }
    }

    struct SelfNamingFMC {}

    impl NamedFileMetricCalculator for SelfNamingFMC {
        fn name(&self) -> String {
            "filename".to_string()
        }
        fn calculate_metrics(&self, path: &Path) -> Result<serde_json::Value, Error> {
            Ok(json!(path.to_str()))
        }
    }

    #[test]
    fn scanning_merges_data_from_mutators() {
        let root = Path::new("./tests/data/simple/");
        let calculators: Vec<Box<dyn NamedFileMetricCalculator>> =
            vec![Box::new(SimpleFMC {}), Box::new(SelfNamingFMC {})];

        let tree = walk_directory(root, calculators).unwrap();
        let json = serde_json::to_string_pretty(&tree).unwrap();
        let parsed_result: Value = serde_json::from_str(&json).unwrap();

        let expected =
            std::fs::read_to_string(Path::new("./tests/expected/simple_files_with_data.json"))
                .unwrap();
        let parsed_expected: Value = serde_json::from_str(&expected).unwrap();

        assert_eq!(parsed_result, parsed_expected);
    }

    // TODO: test error handling
}
