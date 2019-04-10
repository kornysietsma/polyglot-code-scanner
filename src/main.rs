extern crate ignore;
extern crate tokei;

mod flare;

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

impl FlareNode {
    fn name(&self) -> &String {
        &self.name
    }
    fn from_file(name: String) -> FlareNode {
        FlareNode {
            name: name,
            value: FileOrDir::FlareFile {},
        }
    }
    fn from_dir(name: String) -> FlareNode {
        FlareNode {
            name: name,
            value: FileOrDir::FlareDir {
                children: Vec::new(),
            },
        }
    }
    fn append_child(&mut self, child:FlareNode) {
        match self.value {
            FileOrDir::FlareDir { ref mut children } => {
                children.push(child);
            }
            _ => {}
        }
    }
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

        assert_eq!(result.name, "fred");

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
