#![warn(clippy::all)]
#![allow(dead_code)]

extern crate ignore;
extern crate tokei;

mod file_walker;
mod flare;
mod loc;

use std::path::Path;

fn main() {
    let root = Path::new("./src/");

    let tree =
        file_walker::walk_directory(root, vec![Box::new(loc::LocMetricCalculator {})]).unwrap();

    let json = serde_json::to_string_pretty(&tree).unwrap();
    println!("{}", json);
}
