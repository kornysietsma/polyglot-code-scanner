# Language-Agnostic Toxicity Indicators

## Work-in-progress warning!

This is a work in progress - it's still being fiddled with, major changes might come at any time.  Or I might abandon it as my free time evaporates.  You have been warned!

## Intro

This application scans source code directories, looking for measures that can be
useful for identifying toxic code.

I prefer to call these "indicators" rather than "metrics" as many of them are not precise enough
to really warrant the name "metrics" - they are ways of identifying bad code, but not a metric
you'd want to use in any scientific way.

The output is a "[flare](https://github.com/d3/d3-hierarchy#hierarchy)" JSON file - a not-very-precisely documented format used by [d3](https://d3.org) hierarchical visualisations, especially my own [toxic code explorer](https://github.com/kornysietsma/toxic-code-explorer-demo/) (which is due for a refresh soon!)

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

## Usage

```
lati_scanner [FLAGS] [OPTIONS] [root]

FLAGS:
    -h, --help         Prints help information
    -V, --version      Prints version information
    -v, --verbosity    Pass many times for more log output

OPTIONS:
    -o, --output <output>    Output JSON file, stdout if not present

ARGS:
    <root>    Root directory, current dir if not present
```

Note you can't currently choose which indicators to output.  There is also a sneaky `--git` option to do a git log, to help me with debugging (this will go soon!)

## License

Copyright © 2019 Kornelis Sietsma

Licensed under the Apache License, Version 2.0 - see LICENSE.txt for details
