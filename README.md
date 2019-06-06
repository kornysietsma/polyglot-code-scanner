# Language-Agnostic Toxicity Indicators

Version 0.0.1 - early in-dev version

## Work-in-progress warning!

This is a work in progress - it's still being fiddled with, major changes might come at any time.  Or I might abandon it as my free time evaporates.  You have been warned!

### Major issues at the moment

- There is no limit to git history scanning - this will scan the whole git history, which could be slow!
- No progress indicators - you can add `-vv` to see logs, but there's no real indication of what it's doing on a slow git scan
- You can't turn git scanning off, so if code isn't in git, the scanner will waste time looking for a git repo over and over and over

## Intro

This application scans source code directories, looking for measures that can be
useful for identifying toxic code.

I prefer to call these "indicators" rather than "metrics" as many of them are not precise enough
to really warrant the name "metrics" - they are ways of identifying bad code, but not a metric
you'd want to use in any scientific way.

There are two ways to run this - as a simple CLI tool, producing a data file as it's output; or as a local web server
in conjunction with the [lati-explorer](https://github.com/kornysietsma/lati-explorer) D3 visualisation.

## Installation and running
I haven't distributed binary files yet - you'll need rust and cargo to compile and build `lati-scanner`:

`cargo install --path .`

Or just run it with `cargo run lati-scanner -- (other command line arguments)`

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
        }
      }
    }
  ]
}
```

Currently the following indicators are implemented:

- loc - lines of code - uses the [tokei](https://github.com/XAMPPRocky/tokei) library to produce lines of code and other stats for many programming languages
- git - git stats - for now, this produces very basic stats:
  - the age in days since the last commit for this file
  - the timestamp (seconds since the epoch) of the last commit for this file
  - the number of unique users who have touched this file (taken from authors, committers, and "Co-authored-by" comments)

There are more still coming as I port functionality from my older clojure tools!

### Web Server mode

This is operated by the `--server` option (see below for command-line options, or run `lati-scanner --help`)

You also need to have downloaded the [lati-explorer](https://github.com/kornysietsma/lati-explorer) source code to your local machine - `lati-scanner` doesn't embed all the HTML, CSS and JavaScript for the server, you need to download it yourself. (for now, there might be an embedded version one day, though it'll make the binary bigger!)

The basic process is:
* Download lati-explorer from https://github.com/kornysietsma/lati-explorer
* Run `lati-scanner` specifying the location of this project directory:
`lati-scanner --server -e ~/my_stuff/lati-explorer`
* this will fail if it can't see the `docs` directory where resources actually live.
* Once the files are scanned, open a web browser to http://localhost:3000 (or you can specify a different port on the commandline)

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


ARGS:
    <root>
            Root directory, current dir if not present
```

Note you can't currently choose which indicators to output - it runs both `loc` and `git` for now.

## Why rust?

1. I wanted to play with rust - I havent used a compiled language since the '90s, and I haven't used a strongly typed language I liked for a long time
2. [Tokei](https://github.com/XAMPPRocky/tokei) is awesome - and removes a key dependenency on `cloc` of my old code
3. It's nice to ditch the JVM dependency for building a generally useful tool

## License

Copyright © 2019 Kornelis Sietsma

Licensed under the Apache License, Version 2.0 - see LICENSE.txt for details
