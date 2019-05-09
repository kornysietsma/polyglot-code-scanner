#![warn(clippy::all)]
#![allow(dead_code)]

extern crate ignore;
extern crate tokei;
#[macro_use]
extern crate failure;

mod file_walker;
mod flare;
mod loc;
use failure::Error;

use std::path::Path;

fn main() -> Result<(), Error> {
    let root = Path::new(".");

    let tree = file_walker::walk_directory(root, vec![Box::new(loc::LocMetricCalculator {})])?;

    let json = serde_json::to_string_pretty(&tree)?;
    println!("{}", json);
    Ok(())
}
