use anyhow::Error;
use polyglot_code_scanner::ScannerConfig;
use serde_json::Value;
use std::io::Cursor;
use std::path::PathBuf;
use tempfile::tempdir;
use test_shared::*;

fn test_scanner_config(with_git: bool) -> ScannerConfig {
    let mut config = ScannerConfig::default("test");
    config.data_id = Some("test-id".to_string());
    config.features.git = with_git;
    config
}

#[test]
fn it_calculates_lines_of_code() -> Result<(), Error> {
    let root = PathBuf::from("./tests/data/simple/");

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result =
        polyglot_code_scanner::run(&root, &test_scanner_config(false), None, &["loc"], out);

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
    let git_root = unzip_test_sample("git_sample", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let result =
        polyglot_code_scanner::run(&git_root, &test_scanner_config(true), None, &["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/git_flare_test.json",
    );

    Ok(())
}

#[test]
fn it_calculates_git_stats_with_email_dedup_flag() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_test_sample("git_dedup_sample", gitdir.path())?;

    let mut legacy_buffer: Vec<u8> = Vec::new();
    let legacy_out = Cursor::new(&mut legacy_buffer);

    let legacy_result = polyglot_code_scanner::run(
        &git_root,
        &test_scanner_config(true),
        None,
        &["git"],
        legacy_out,
    );

    assert!(legacy_result.is_ok());

    let legacy_json: Value = serde_json::from_reader(legacy_buffer.as_slice())?;

    let mut dedup_buffer: Vec<u8> = Vec::new();
    let dedup_out = Cursor::new(&mut dedup_buffer);

    let mut config = test_scanner_config(true);
    config.git_dedup_by_email = true;

    let dedup_result = polyglot_code_scanner::run(&git_root, &config, None, &["git"], dedup_out);

    assert!(dedup_result.is_ok());

    let dedup_json: Value = serde_json::from_reader(dedup_buffer.as_slice())?;

    let legacy_users = legacy_json["metadata"]["git"]["users"]
        .as_array()
        .expect("legacy users should be an array");
    let dedup_users = dedup_json["metadata"]["git"]["users"]
        .as_array()
        .expect("dedup users should be an array");

    assert_eq!(legacy_users.len(), 2);
    assert_eq!(dedup_users.len(), 1);

    assert_eq!(dedup_users[0]["user"]["email"], "alice@example.com");
    assert_eq!(dedup_users[0]["user"]["name"], "Alice Two");

    Ok(())
}

#[test]
fn it_calculates_detailed_git_stats() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_test_sample("git_sample", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let mut config = test_scanner_config(true);
    config.features.git_details = true;

    let result = polyglot_code_scanner::run(&git_root, &config, None, &["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    assert_eq_json_file(
        &parsed_result,
        "./tests/expected/integration_tests/git_detailed_flare_test.json",
    );

    Ok(())
}

#[test]
fn it_prunes_inactive_files_with_flag() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_test_sample("prune_test", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let mut config = test_scanner_config(true);
    config.prune_inactive_years = Some(1); // Prune git roots with no activity in last 1 year
    config.git_years = Some(3); // Fetch 3 years of git history (to include Dec 2024 commits)

    let result = polyglot_code_scanner::run(&git_root, &config, None, &["git"], out);

    if let Err(e) = &result {
        eprintln!("Error running scanner: {:?}", e);
    }
    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    let children = parsed_result["tree"]["children"].as_array().unwrap();
    let children_names: Vec<String> = children
        .iter()
        .map(|child| child["name"].as_str().unwrap().to_string())
        .collect();

    // repo_with_recent_activity should be KEPT (has recent commits from Feb 2026, within 1 year)
    assert!(
        children_names.contains(&"repo_with_recent_activity".to_string()),
        "repo_with_recent_activity should be kept - git root has recent activity (Feb 2026)"
    );

    // repo_all_old_activity should be PRUNED (all commits from Dec 2024, older than 1 year)
    assert!(
        !children_names.contains(&"repo_all_old_activity".to_string()),
        "repo_all_old_activity should be pruned - all commits outside the 1-year window"
    );

    // repo_no_git should be KEPT (non-git content is kept by design - no structural anchor to identify pruning units)
    assert!(
        children_names.contains(&"repo_no_git".to_string()),
        "repo_no_git should be kept - non-git content preserved (no way to determine if it's important)"
    );

    // Verify that repo_with_recent_activity contains both old and recent files
    // This tests that directory-level semantics are preserved: git root keeps all contents
    // when it passes the date cutoff
    let repo_with_recent = children
        .iter()
        .find(|child| child["name"] == "repo_with_recent_activity")
        .unwrap();
    let repo_files: Vec<String> = repo_with_recent["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|child| child["name"].as_str().unwrap().to_string())
        .collect();

    assert!(
        repo_files.contains(&"old_file.txt".to_string()),
        "old_file.txt should be kept in repo_with_recent_activity (directory-level semantics)"
    );
    assert!(
        repo_files.contains(&"recent_file.txt".to_string()),
        "recent_file.txt should be kept in repo_with_recent_activity"
    );

    Ok(())
}

#[test]
fn it_keeps_all_files_without_prune_flag() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_test_sample("prune_test", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let config = test_scanner_config(true);
    // prune_inactive_years is None by default (no pruning)

    let result = polyglot_code_scanner::run(&git_root, &config, None, &["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    // Verify that all three directories are present without pruning
    let children_names: Vec<String> = parsed_result["tree"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|child| child["name"].as_str().unwrap().to_string())
        .collect();

    // When prune_inactive_years is None (default), all directories should be present
    assert!(
        children_names.contains(&"repo_with_recent_activity".to_string()),
        "repo_with_recent_activity should be present without prune flag"
    );
    assert!(
        children_names.contains(&"repo_all_old_activity".to_string()),
        "repo_all_old_activity should be present without prune flag"
    );
    assert!(
        children_names.contains(&"repo_no_git".to_string()),
        "repo_no_git should be present without prune flag"
    );

    Ok(())
}

#[test]
fn it_prunes_based_on_separate_inactivity_window() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_test_sample("prune_test", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let mut config = test_scanner_config(true);
    // Fetch 20 years of git history for metrics, but prune based on 1 year of inactivity
    // The test data contains multiple git repositories at different inactivity levels
    config.git_years = Some(20); // Fetch long history (would normally be just 1 year)
    config.prune_inactive_years = Some(1); // Prune based on 1-year inactivity

    let result = polyglot_code_scanner::run(&git_root, &config, None, &["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    // The root is a parent directory containing two git repos:
    // - repo_with_recent_activity (has recent commits, should be KEPT)
    // - repo_all_old_activity (all commits are old, should be PRUNED)

    let children = parsed_result["tree"]["children"].as_array().unwrap();
    let child_names: Vec<String> = children
        .iter()
        .filter_map(|child| child["name"].as_str().map(|s| s.to_string()))
        .collect();

    // Should keep the recent repo
    assert!(
        child_names.contains(&"repo_with_recent_activity".to_string()),
        "repo_with_recent_activity should be kept (has recent commits)"
    );

    // Should prune the old repo
    assert!(
        !child_names.contains(&"repo_all_old_activity".to_string()),
        "repo_all_old_activity should be pruned (all commits older than 1 year)"
    );

    // Should keep the non-git repo (conservative: no way to know if it's important)
    assert!(
        child_names.contains(&"repo_no_git".to_string()),
        "repo_no_git should be kept (non-git content preserved by design)"
    );

    // Verify the kept repo has timestamps
    let recent_repo = children
        .iter()
        .find(|child| child["name"].as_str() == Some("repo_with_recent_activity"))
        .expect("repo_with_recent_activity should exist");

    let git_data = &recent_repo["data"]["git"];
    assert!(
        git_data.get("earliest_commit_date").is_some(),
        "Git root should have earliest_commit_date field"
    );
    assert!(
        git_data.get("latest_commit_date").is_some(),
        "Git root should have latest_commit_date field"
    );

    // Verify that timestamps cover the full 20-year range we fetched
    let latest = git_data["latest_commit_date"].as_u64().unwrap();
    let feb_2026 = 1770700000u64; // roughly Feb 2026
    assert!(
        latest >= feb_2026 - 86400,
        "Should have recent commit data from Feb 2026"
    );

    // The key point: we fetched 20 years of git history instead of 1 year,
    // so we have more complete git data available for analysis
    Ok(())
}

#[test]
fn it_includes_git_root_timestamps() -> Result<(), Error> {
    let gitdir = tempdir()?;
    let git_root = unzip_test_sample("prune_test", gitdir.path())?;

    let mut buffer: Vec<u8> = Vec::new();
    let out = Cursor::new(&mut buffer);

    let config = test_scanner_config(true);

    let result = polyglot_code_scanner::run(&git_root, &config, None, &["git"], out);

    assert!(result.is_ok());

    let parsed_result: Value = serde_json::from_reader(buffer.as_slice())?;

    // The root is a parent directory, not itself a git root.
    // Check the git roots in the children: repo_with_recent_activity and repo_all_old_activity
    let children = parsed_result["tree"]["children"].as_array().unwrap();

    // Find the repo_with_recent_activity child
    let recent_repo = children
        .iter()
        .find(|child| child["name"].as_str() == Some("repo_with_recent_activity"))
        .expect("repo_with_recent_activity should exist");

    let git_data = &recent_repo["data"]["git"];

    assert!(
        git_data.get("earliest_commit_date").is_some(),
        "Git root should have earliest_commit_date field"
    );
    assert!(
        git_data.get("latest_commit_date").is_some(),
        "Git root should have latest_commit_date field"
    );

    // Verify the timestamps are reasonable (Unix timestamps, not None/null)
    let earliest = git_data["earliest_commit_date"].as_u64();
    let latest = git_data["latest_commit_date"].as_u64();

    assert!(
        earliest.is_some(),
        "earliest_commit_date should be a valid u64 timestamp"
    );
    assert!(
        latest.is_some(),
        "latest_commit_date should be a valid u64 timestamp"
    );

    // Verify that earliest <= latest
    if let (Some(e), Some(l)) = (earliest, latest) {
        assert!(
            e <= l,
            "earliest_commit_date should be <= latest_commit_date"
        );
    }

    Ok(())
}

// TODO: add a coupling e2e test!  Needs a lot of setup
