#![warn(clippy::all)]
#![allow(dead_code)]

extern crate ignore;
extern crate tokei;

mod file_walker;
mod flare;
mod loc;

use std::path::Path;

fn main() {
    let root = Path::new("./tests/data/simple/");

    let empty_calculators: Vec<Box<dyn file_walker::NamedFileMetricCalculator>> = Vec::new();
    let tree = file_walker::walk_directory(root, empty_calculators).unwrap();
    let json = serde_json::to_string_pretty(&tree).unwrap();
    println!("{}", json);
}
