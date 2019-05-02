#![warn(clippy::all)]
#![allow(dead_code)]

use super::flare;
use super::flare::FlareTree;
use ignore::{Walk, WalkBuilder};
use std::error::Error;
use std::path::Path;

pub struct FileMetricCalculator {
    pub name: String,
    pub calculator: Box<Fn(&Path) -> serde_json::Value>,
}

fn walk_tree_walker(
    walker: Walk,
    prefix: &Path,
    file_metric_calculators: Vec<FileMetricCalculator>,
) -> Result<flare::FlareTree, Box<dyn Error>> {
    let mut tree = FlareTree::from_dir("flare");

    for result in walker.map(|r| r.expect("File error!")) {
        let p = result.path();
        let relative = p.strip_prefix(prefix)?;
        let new_child = if p.is_file() {
            let mut f = FlareTree::from_file(p.file_name().unwrap());
            file_metric_calculators.iter().for_each(|fmc| {
                f.add_file_data_as_value(fmc.name.to_string(), (fmc.calculator)(p))
            });
            f
        } else {
            FlareTree::from_dir(p.file_name().unwrap())
        }; // TODO handle if not a dir either!

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
    Ok(tree)
}

pub fn walk_directory(
    root: &Path,
    file_metric_calculators: Vec<FileMetricCalculator>,
) -> Result<flare::FlareTree, Box<dyn Error>> {
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

    #[test]
    fn scanning_merges_data_from_mutators() {
        let root = Path::new("./tests/data/simple/");
        let fmc1 = FileMetricCalculator {
            name: "foo".to_string(),
            calculator: Box::new(|_path| json!("bar")),
        };
        let fmc2 = FileMetricCalculator {
            name: "filename".to_string(),
            calculator: Box::new(|path| json!(path.to_str())),
        };
        let calculators = vec![fmc1, fmc2];

        let tree = walk_directory(root, calculators).unwrap();
        let json = serde_json::to_string_pretty(&tree).unwrap();
        let parsed_result: Value = serde_json::from_str(&json).unwrap();

        let expected =
            std::fs::read_to_string(Path::new("./tests/expected/simple_files_with_data.json"))
                .unwrap();
        let parsed_expected: Value = serde_json::from_str(&expected).unwrap();

        assert_eq!(parsed_result, parsed_expected);
    }
}
