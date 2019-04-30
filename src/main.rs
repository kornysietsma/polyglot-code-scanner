#![warn(clippy::all)]
#![allow(dead_code)]

extern crate ignore;
extern crate tokei;

mod file_walker;
mod flare;

use ignore::WalkBuilder;

use std::path::Path;

fn main() {
    let root = Path::new("./tests/data/simple/");
    let walker = WalkBuilder::new(root).build();

    let tree = file_walker::parse_tree(walker, root).unwrap();
    let json = serde_json::to_string_pretty(&tree).unwrap();
    println!("{}", json);
}
