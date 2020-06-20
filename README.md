# Language-Agnostic Toxicity Indicators

## WARNING - readme is out of date

I've been making changes, need to clean up lots of docs.

I only really pushed because I realised that I foolishly pushed code using a client's email address!  (I wasn't using this at a client site, but I globally changed my git email - foolish move, never do this)

Thankfully following <https://help.github.com/en/github/using-git/changing-author-info> let me remove these from history.

If you cloned/forked this repo in the last year you might have some history glitches!  I hope not. I don't think people are cloning this much :)

More docs coming soon!

--

Version 0.0.1 - early in-dev version

This is part of a growing set of tools to help us identify toxic code in large codebases,
using language-agnostic approaches. Or at least, lightweight tools that work in a very wide range
of languages - this code might not work for your [befunge](https://esolangs.org/wiki/Befunge) project!

## Work-in-progress warning!

This is a work in progress - it's still being fiddled with, major changes might come at any time. Or I might abandon it as my free time evaporates. You have been warned!

### Major issues at the moment

- There are no progress indicators - you can add `-vv` to see logs, but there's no real indication of what it's doing on a slow git scan
- You can't turn git scanning off, so if code isn't in git, the scanner will waste time looking for a git repo over and over and over
- You can't pick and choose which indicators to scan for - it runs them all

## Intro

This application scans source code directories, looking for measures that can be
useful for identifying toxic code.

I prefer to call these "indicators" rather than "metrics" as many of them are not precise enough
to really warrant the name "metrics" - they are ways of identifying bad code, but not a metric
you'd want to use in any scientific way.

There are two ways to run this - as a simple CLI tool, producing a JSON data file as its output; or as a local web server
in conjunction with the [lati-explorer](https://github.com/kornysietsma/lati-explorer) D3 visualisation.

## Installation and running

I haven't distributed binary files yet - you'll need [to install rust and cargo](https://www.rust-lang.org/tools/install) and then compile and install `lati-scanner`.

If you have the code cloned locally you can install it with:

`cargo install --path . --force`

or if you want to install from github without downloading:

`cargo install --git https://github.com/kornysietsma/lati-scanner`

These will install to `~/.cargo/bin/lati_scanner` on a \*nix style machine. You can put this in your PATH, or just run it from this directory. As a binary it has no other dependencies.

### Running from source

You can also just run it from the source directory with `cargo run lati-scanner -- (other command line arguments)`

### Getting help

This readme might be out of date - the help might be more accurate:

`lati_scanner -h`

### Simple file output

This is the default mode - the `lati-scanner` command produces a "[flare](https://github.com/d3/d3-hierarchy#hierarchy)" JSON file - a not-very-precisely documented format used by [d3](https://d3.org) hierarchical visualisations, especially my own [toxic code explorer](https://github.com/kornysietsma/toxic-code-explorer-demo/) (which is due for a refresh soon!)

A basic output sample looks something like:

```
{
  "name": "flare",
  "children": [
    {
      "name": "foo.clj",
      "data": {
        "loc": {
          "blanks": 1,
          "code": 3,
          "comments": 0,
          "language": "Clojure",
          "lines": 4
        },
        "git": {
          "age_in_days": 56,
          "last_update": 1554826216,
          "user_count": 1
        },
        "indentation": {
          "lines": 8,
          "maximum": 6,
          "median": 4,
          "minimum": 0,
          "p75": 6,
          "p90": 6,
          "p99": 6,
          "stddev": 3
        }
      }
    }
  ]
}
```

Currently the following indicators are implemented:

- loc - lines of code - uses the [tokei](https://github.com/XAMPPRocky/tokei) library to produce lines of code and other stats for many programming languages. Unsupported languages it will try to just count lines of text.
- git - git stats - for now, this produces very basic stats:
  - the age in days since the last commit for this file
  - the timestamp (seconds since the epoch) of the last commit for this file
  - the number of unique users who have touched this file (taken from authors, committers, and "Co-authored-by" comments)
    - uniqueness is a combination of name + email - this might show excess numbers, consumers should de-duplicate this!
  - indentation - indentation is a good proxy for complexity! The output includes medians and quantiles for indentation (in spaces, with tabs assumed to be 4 spaces) for complexity display

I aim to add more indicators as I go.

### Viewing the data

You can visualise the data by cloning [lati-explorer](https://github.com/kornysietsma/lati-explorer) and then copying the output JSON file into that codebase - or use the web server mode:

### Web Server mode

This is operated by the `--server` option (see below for command-line options, or run `lati-scanner --help`)

You also need to have downloaded the [lati-explorer](https://github.com/kornysietsma/lati-explorer) source code to your local machine - `lati-scanner` doesn't embed all the HTML, CSS and JavaScript for the server, you need to download it yourself. (for now, there might be an embedded version one day, though it'll make the binary bigger!)

The basic process is:

- Download lati-explorer from https://github.com/kornysietsma/lati-explorer
- Run `lati-scanner` specifying the location of this project directory:
  `lati-scanner --server -e ~/my_stuff/lati-explorer`
- this will fail if it can't see the `docs` directory where resources actually live.
- Once the files are scanned, open a web browser to http://localhost:3000 (or you can specify a different port on the commandline)

## Ignoring files

Git ignored files in `.gitignore` are not scanned.

You can also manually add `.lati_ignore` files anywhere in the codebase, to list extra files to be ignored - the syntax is [the same as .gitignore's](https://git-scm.com/docs/gitignore)

## Usage

```
lati_scanner [FLAGS] [OPTIONS] [root]

FLAGS:
    -h, --help
            Prints help information (--help for more info)

    -s, --server
            Run a web server to display the lati-explorer visualisation Requires "-e" to indicate where to find the
            lati-explorer code Download the code from https://github.com/kornysietsma/lati-explorer if you want to see
            pretty visualisations
    -V, --version
            Prints version information

    -v, --verbosity
            Pass many times for more log output

            By default, it'll only report errors. Passing `-v` one time also prints warnings, `-vv` enables info
            logging, `-vvv` debug, and `-vvvv` trace.

OPTIONS:
    -e, --explorer <explorer_location>
            The location of the lati-explorer code, needed for server mode Download the code from
            https://github.com/kornysietsma/lati-explorer and specify the local directory name here.
    -o, --output <output>
            Output file, stdout if not present, or not used if sending to web server

    -p, --port <port>
            The web server port [default: 3000]

    --years <git_years>
            How many years of git history to parse - default is to scan 3 years of logs

ARGS:
    <root>
            Root directory, current dir if not present
```

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
