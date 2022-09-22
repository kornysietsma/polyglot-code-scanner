use anyhow::Error;
use polyglot_code_scanner::ScannerConfig;
use serde_json::Value;
use std::io::Cursor;
use std::path::PathBuf;
use tempfile::tempdir;
use test_shared::*;

fn test_scanner_config() -> ScannerConfig {
    let mut config = ScannerConfig::default("test");
    config.data_id = Some("test-id".to_string());
    config
}

#[test]
fn it_calculates_lines_of_code() -> Result<(), Error> {
    let root = PathBuf::from("./tests/data/simple/");

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result = polyglot_code_scanner::run(root, test_scanner_config(), None, vec!["loc"], out);

    assert!(result.is_ok());

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
    let git_root = unzip_git_sample("git_sample", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result =
        polyglot_code_scanner::run(git_root, test_scanner_config(), None, vec!["git"], out);

    assert!(result.is_ok());

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
    let git_root = unzip_git_sample("git_sample", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let mut config = test_scanner_config();
    config.detailed = true;

    let result = polyglot_code_scanner::run(git_root, config, None, vec!["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/git_detailed_flare_test.json",
    );

    Ok(())
}

// TODO: THIS TEST IS JUNK! We need some commits that actually generate coupling data :)

#[test]
fn it_calculates_detailed_git_stats_with_coupling() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_git_sample("git_sample", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let mut config = test_scanner_config();
    config.detailed = true;
    let coupling_config =
        polyglot_code_scanner::coupling::CouplingConfig::new(3, 1, 0.1, 120 * 60, 60 * 60, 0, None);

    let result =
        polyglot_code_scanner::run(git_root, config, Some(coupling_config), vec!["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/git_detailed_flare_test.json",
    );

    Ok(())
}
