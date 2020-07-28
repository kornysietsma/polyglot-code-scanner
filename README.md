# Polyglot Code Scanner

This is part of my Polyglot Code tools - for the main documentation, see <https://polyglot.korny.info>

## Intro

This application scans source code directories, identifying a range of code metrics and other data, and storing the results in a JSON file for later visualisation by the [Polyglot Code Explorer](https://polyglot.korny.info/tools/explorer/description/)

## Installation and running

I haven't distributed binary files yet - you'll need [to install rust and cargo](https://www.rust-lang.org/tools/install) and then compile and install `polyglot_code_scanner`.

If you have the code cloned locally you can install it with:

~~~sh
$ cargo build --release
~~~

The binary will be built in the `target/release` directory.

### Running from source

You can also just run it from the source directory with `cargo run polyglot_code_scanner -- (other command line arguments)`

### Getting help

This readme might be out of date - the help might be more accurate:

`polyglot_code_scanner -h`

Currently the following indicators are implemented:

- loc - lines of code - uses the [tokei](https://github.com/XAMPPRocky/tokei) library to produce lines of code and other stats for many programming languages. Unsupported languages it will try to just count lines of text.
- git - git stats - for now, this produces very basic stats:
  - the age in days since the last commit for this file
  - the timestamp (seconds since the epoch) of the last commit for this file
  - the number of unique users who have touched this file (taken from authors, committers, and "Co-authored-by" comments)
    - uniqueness is a combination of name + email - this might show excess numbers, consumers should de-duplicate this!
  - indentation - indentation is a good proxy for complexity! The output includes medians and quantiles for indentation (in spaces, with tabs assumed to be 4 spaces) for complexity display

I aim to add more indicators as I go.

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

## Why rust?

1. I wanted to play with rust - I havent used a compiled language since the '90s, and I haven't used a strongly typed language I liked for a long time
2. [Tokei](https://github.com/XAMPPRocky/tokei) is awesome - and removes a key dependenency on `cloc` of my old code
3. It's nice to ditch the JVM dependency for building a generally useful tool

## Why did you fork tokei?

I want to generate indentation ignoring comments. Comments distort metrics. Tokei will recognise comments but it just gives stats, not code with the comments removed. So for now, I've forked tokei
to let me do this. I'm not sure if this will remain a fork, or if
it is something that could be merged back into tokei.

## License

Copyright © 2019 Kornelis Sietsma

Licensed under the Apache License, Version 2.0 - see LICENSE.txt for details
