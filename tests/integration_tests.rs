use failure::Error;
use serde_json::Value;
use std::io::Cursor;
use std::path::PathBuf;
use tempfile::tempdir;
use test_shared::*;

#[test]
fn it_calculates_lines_of_code() -> Result<(), Error> {
    let root = PathBuf::from("./tests/data/simple/");

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result = lati_scanner::run(
        root,
        lati_scanner::CalculatorConfig::default(),
        vec!["loc"],
        out,
    );

    assert!(!result.is_err());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/loc_flare_test.json",
    );

    Ok(())
}

#[test]
fn it_calculates_git_stats() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_git_sample(gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result = lati_scanner::run(
        git_root,
        lati_scanner::CalculatorConfig::default(),
        vec!["git"],
        out,
    );

    assert!(!result.is_err());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/git_flare_test.json",
    );

    Ok(())
}

#[test]
fn it_calculates_detailed_git_stats() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_git_sample(gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let mut config = lati_scanner::CalculatorConfig::default();
    config.detailed = true;

    let result = lati_scanner::run(git_root, config, vec!["git"], out);

    assert!(!result.is_err());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/git_detailed_flare_test.json",
    );

    Ok(())
}
