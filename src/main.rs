#![warn(clippy::all)]
#![allow(dead_code)]

extern crate ignore;
extern crate tokei;
#[macro_use]
extern crate failure;

mod file_walker;
mod flare;
mod loc;

use std::path::Path;

fn main() {
    let root = Path::new(".");

    let tree =
        file_walker::walk_directory(root, vec![Box::new(loc::LocMetricCalculator {})]).unwrap();

    let json = serde_json::to_string_pretty(&tree).unwrap();
    println!("{}", json);
}
