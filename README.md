# Polyglot Code Scanner

This is part of my Polyglot Code tools - for the main documentation, see <https://polyglot.korny.info>

## Intro

This application scans source code directories, identifying a range of code metrics and other data, and storing the results in a JSON file for later visualisation by the [Polyglot Code Explorer](https://polyglot.korny.info/tools/explorer/description/)

## Installation and running

See also <https://polyglot.korny.info/tools/scanner/howto> for more detailed instructions for fetching binary releases, and running the scanner.

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

## Ignoring files

Git ignored files in `.gitignore` are not scanned.

You can also manually add `.polyglot_code_scanner_ignore` files anywhere in the codebase, to list extra files to be ignored - the syntax is [the same as .gitignore's](https://git-scm.com/docs/gitignore)

## Usage

~~~text
polyglot_code_scanner [FLAGS] [OPTIONS] [root]

USAGE:
    polyglot_code_scanner [FLAGS] [OPTIONS] [root]

FLAGS:
    -c, --coupling           include temporal coupling data
    -h, --help               Prints help information
        --no-detailed-git    Don't include detailed git information - output may be big!
    -V, --version            Prints version information
    -v, --verbose            Logging verbosity, v = error, vv = warn, vvv = info (default), vvvv = debug, vvvvv = trace

OPTIONS:
        --coupling-bucket-days <bucket_days>
            how many days are reviewed for one "bucket" of temporal coupling [default: 91]

        --years <git_years>
            how many years of git history to parse - default only scan the last 3 years (from now, not git head)
            [default: 3]
        --coupling-min-ratio <min_coupling_ratio>
            what is the minimum ratio of (other file changes)/(this file changes) to include a file in coupling stats
            [default: 0.25]
        --coupling-source-days <min_source_days>
            how many days should a file change in a bucket for it to generate coupling stats [default: 10]

    -o, --output <output>
            Output file, stdout if not present, or not used if sending to web server


ARGS:
    <root>    Root directory, current dir if not present
~~~

## Development hints

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

## License

Copyright © 2019 Kornelis Sietsma

Licensed under the Apache License, Version 2.0 - see LICENSE.txt for details
