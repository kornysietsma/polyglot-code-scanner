# Language-Agnostic Toxicity Indicators

## Work-in-progress warning!

This is a work in progress - it's still being fiddled with, major changes might come at any time.  Or I might abandon it as my free time evaporates.  You have been warned!

*Current status*

The basic "loc" stuff works - I'm currently adding git stats, which has lots of curly edges, so needs quite a few interim commits.
For now, you can run `cargo run --git` to play with git logs, as well as the more stable commands below.

## Intro

This application scans source code directories, looking for measures that can be
useful for identifying toxic code.

I prefer to call these "indicators" rather than "metrics" as many of them are not precise enough
to really warrant the name "metrics" - they are ways of identifying bad code, but not a metric
you'd want to use in any scientific way.

(This is a work-in-progress port of my previous clojure projects `cloc2flare` and soon some others)

Current indicators produced:

- loc - lines of code - uses the [tokei](https://github.com/XAMPPRocky/tokei) library to produce lines of code and other stats for many programming languages

more to come!

## Usage

```
lati_scanner [FLAGS] [OPTIONS] [root]

FLAGS:
    -h, --help         Prints help information
    -P, --pretty       Enable pretty printing
    -V, --version      Prints version information
    -v, --verbosity    Pass many times for more log output

OPTIONS:
    -o, --output <output>    Output file, stdout if not present

ARGS:
    <root>    Root directory, current dir if not present
```
## License

Copyright © 2019 Kornelis Sietsma

Licensed under the Apache License, Version 2.0 - see LICENSE.txt for details
