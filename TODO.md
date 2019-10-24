# TODO

## General readiness

- upload / fix tokei fork
- consider merging with tokei master - discuss with author
- write a proper readme, especially how to install and run
- set up CI / Travis - maybe via https://github.com/japaric/trust
- upload binaries

## loc and general

- add test that checks binary files and unknown text files (e.g. erb)
- clear out logs for unknown file type - maybe only log each type once?
- the UI might need an option "show binary files" - maybe an alternative view to loc. Tricky as we would need a complete re-draw - but for now, can just store the size and let the ui choose.
- ignore patterns e.g. for vendored javascript, generated code

## git things:

- does a rename or copy count as a change, if no lines of code change?
- add indicator selection to the CLI
- need to test special cases:
  - submodules?!
  - local checkout is not on remote origin/master (do we care?)
- follow renames! (this is complex but would be good - currently nothing is known before a rename)
- better tests - using code with more checkins and more date ranges (maybe some rebasing?)
- check names for uniqueness, not emails - noreply@github.com isn't a user

### churn:

- store all changes not just summary - let the ui decide
- "Chunking" - combine changes within a single day?
- can we avoid the json getting vast?
- store/unique by name first? Want to handle fake emails e.g. github no-reply
- UI considerations for churn calcs

## other things:

- refactoring - use Into more ? "fn new<S: Into<String>>(name: S, is_file: bool)" allows the caller to decide...
- add a progress notifier - logs are too low level - look at indicatif https://docs.rs/indicatif/0.11.0/indicatif/
- Can we get rid of test_shared's duplication in cargo.toml ?
- "-P" cli option is confusing - it's pretty printing _for logs_ !
- can we make the log default "warn"??
- config file so things like ignore pattern are "sticky" ? Nice to have a `.lati.config` file you could drop in to a local repo

## Bigger things

- indent stats (with or without comments depending on the next bit)
- tokei-based calculations that ignore comments - might need a tokei fork! Or can we pilfer bits of tokei?
  - as a start needs to somehow use or clone tokei::language::syntax::SyntaxCounter so not a small job.

## UI stuff

- consider two UI modes:
  1. dynamic mode - just host a local web server
  2. static mode - publish a static server of current dir for uploading
     (this is so I can both publish demos, but also use it easily for local work)
- look at new UI? Lots of options

## Future stuff

- method identification? Can we work out class/method size metrics from indentation?
- deep git stats - time from author to commit, moving towards CD stats
- churn?
