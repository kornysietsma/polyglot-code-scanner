# Polyglot Code Scanner

This is part of my Polyglot Code tools - for the main documentation, see <https://polyglot.korny.info>

## A note about releases

Binary releases are working again - see <https://github.com/kornysietsma/polyglot-code-scanner/releases> for binary releases.

However, for M1 macs this won't work - github actions doesn't yet support M1 macs for free, so you'll have to build binaries yourself for now.

For Macs you also need to run `xattr -d com.apple.quarantine polyglot-code-scanner-x86_64-macos` to remove the quarantine that OSX adds to all downloaded binaries.

## Intro

This application scans source code directories, identifying a range of code metrics and other data, and storing the results in a JSON file for later visualisation by the [Polyglot Code Explorer](https://polyglot.korny.info/tools/explorer/description/)

## Installation and running

See also <https://polyglot.korny.info/tools/scanner/howto> for more detailed instructions for building binary releases, and running the scanner.

To compile and run from source, you'll need [to install rust and cargo](https://www.rust-lang.org/tools/install) and then from a copy of this project, you can build a binary package with:

~~~sh
cargo build --release
~~~

The binary will be built in the `target/release` directory.

### Running from source

You can also just run it from the source directory with `cargo run polyglot_code_scanner -- (other command line arguments)` - this will be slower as it runs un-optimised code with more debug information.  But it's a lot faster for development.

### Getting help

See <https://polyglot.korny.info> for the main documentation for this project.

You can get up-to-date command-line help by running

~~~sh
polyglot_code_scanner -h
~~~

## Known Limitations

### SHA-256 Git Repositories

The scanner does not currently support scanning git repositories that use SHA-256 object hashes (created with `git init --object-format=sha256`). This is due to libgit2 not yet supporting SHA-256 repositories. When libgit2 adds this support, the scanner can be updated to use it.

## Ignoring files

Git ignored files in `.gitignore` are not scanned.

You can also manually add `.polyglot_code_scanner_ignore` files anywhere in the codebase, to list extra files to be ignored - the syntax is [the same as .gitignore's](https://git-scm.com/docs/gitignore)

## Usage

Run `polyglot_code_scanner -h` for full options, this is just the main options:

~~~text
USAGE:
    polyglot_code_scanner [OPTIONS] --name <NAME> [ROOT]

ARGS:
    <ROOT>    Root directory, current dir if not present

OPTIONS:
    -h, --help
            Print help information

    -n, --name <NAME>
            project name - identifies the selected data for display and state storage

        --id <ID>
            data file ID - used to identify unique data files for browser storage, generates a UUID
            if not specified

    -o, --output <OUTPUT>
            Output file, stdout if not present, or not used if sending to web server

        --no-git
            Do not scan for git repositories

        --years <GIT_YEARS>
            how many years of git history to parse - default only scan the last 3 years (from now,
            not git head) [default: 3]

        --git-dedup-by-email
            De-duplicate git users by case-insensitive email when email is valid (contains '@').
            Falls back to legacy name+email matching when email is blank or invalid.

        --prune-inactive-years <PRUNE_YEARS>
            Prune (remove) git repositories that haven't been active in the specified number of
            years. For example, `--prune-inactive-years 1` will remove all git repositories with
            no commits in the last year. Non-git content is always preserved.

    -c, --coupling
            include temporal coupling data

    -V, --version
            Print version information

~~~

## Pruning Inactive Repositories

The `--prune-inactive-years` flag lets you filter out dormant repositories from your analysis. This is useful for understanding active vs stale code in large workspaces.

### Usage Example

```sh
# Fetch 5 years of history, but only show repos active in the last year
polyglot_code_scanner --name myproject --years 5 --prune-inactive-years 1 /path/to/workspace
```

### How It Works

- **Git roots are atomic**: If a repository has any commits within the window, the entire repository is kept, including old files
- **Non-git content preserved**: Directories without git history are always kept (even if `--prune-inactive-years` is specified)
- **Date calculation**: The cutoff is based on the latest commit date in each repository, compared against the current date

### Separate Windows

The `--years` and `--prune-inactive-years` parameters are independent:

- `--years N`: Controls how far back to fetch commit history for metrics and analysis
- `--prune-inactive-years N`: Controls what to keep in the final output

This separation lets you do things like:
```sh
# Analyze 10 years of history in inactive repos, but don't show them
polyglot_code_scanner --years 10 --prune-inactive-years 1 /path/to/workspace
```

## Git User De-duplication

By default, user IDs are generated using legacy case-insensitive name+email matching.

If you enable `--git-dedup-by-email`, then users are matched by case-insensitive email whenever the email is valid (non-blank and contains `@`).

- If multiple names are seen for the same email, the most recent non-blank name is retained.
- If email is blank or invalid, matching falls back to legacy behavior.



### Running tests

To run a single named test from the command-line:

~~~sh
cargo test -- --nocapture renames_and_deletes_applied_across_history
~~~

The `--nocapture` tells rust not to capture stdout/stderr - so you can add `println!` and `eprintln!` statements to help you.

To remove some extra noise and blank lines, pipe the output through grep:

~~~sh
cargo test -- --nocapture renames_and_deletes_applied_across_history | grep -v "running 0 tests" | grep -v "0 passed" | grep -v -e '^\s*$'
~~~

### showing logs

Rust tests don't install a logger - normally you explicitly install loggers in your `main` which tests don't use.

To install a logger using the `fern` crate, add the following to tests:

~~~rust
use test_shared::*;
~~~

then

~~~rust
install_test_logger();
~~~

This sets up a simple logger which sends logs to stdout - make sure you also use the `--nocapture` parameter mentioned earlier.

### Pretty test output

If you want better assertions, your tests need to explicitly use the `pretty_assertions` crate:

~~~rust
use pretty_assertions::assert_eq;
~~~

## Releasing new versions

Releasing uses [cargo-release](https://crates.io/crates/cargo-release)

The basic process is:

* update the top CHANGELOG.md entry (under 'unreleased')
* commit and push changes
* release

~~~sh
cargo release --dry-run
~~~

or for a minor change 0.1.3 to 0.2.0 :

~~~sh
cargo release minor --dry-run
~~~

## License

Copyright © 2019-2022 Kornelis Sietsma

Licensed under the Apache License, Version 2.0 - see LICENSE.txt for details
