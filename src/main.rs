#![warn(clippy::all)]
#![allow(dead_code)]

extern crate ignore;
extern crate tokei;

mod flare;

use ignore::{Walk, WalkBuilder};

use std::error::Error;

fn parse_tree(walker: Walk) -> Result<flare::FlareTree, Box<dyn Error>> {
    for result in walker.map(|r| r.expect("File error!")) {
        let p = result.path();
        println!("{}", p.display())
    }
    Ok(flare::FlareTree::from_file(String::from("fred")))
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

        assert_eq!(result.name(), "fred");
    }
}
