extern crate ignore;
extern crate tokei;

use ignore::{Walk, WalkBuilder};

use std::error::Error;

#[derive(PartialEq, PartialOrd, Debug)]
enum FileOrDir {
    FlareDir { children: Vec<FlareNode> },
    FlareFile {},
}
#[derive(PartialEq, PartialOrd, Debug)]
struct FlareNode {
    name: String,
    value: FileOrDir,
}

fn parse_tree(walker: Walk) -> Result<FlareNode, Box<dyn Error>> {
    for result in walker.map(|r| r.expect("File error!")) {
        println!("{}", result.path().display())
    }
    Ok(FlareNode {
        name: String::from("fred"),
        value: FileOrDir::FlareFile {},
    })
}

fn main() {
    let walker = WalkBuilder::new("./tests/data/simple").build();

    parse_tree(walker).expect("Ow");
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn tree_has_expected_filenames() {
        let walker = WalkBuilder::new("./tests/data/simple").build();
        let result = parse_tree(walker).expect("couldn't parse!");

        assert_eq!(
            result,
            FlareNode {
                name: String::from("fred"),
                value: FileOrDir::FlareFile {},
            } // FlareNode::FlareDir {
              //     name: String::from("simple"),
              //     children: vec![FlareNode::FlareFile {
              //         name: String::from("child.txt")
              //     }]
              // }
        );
    }
}
