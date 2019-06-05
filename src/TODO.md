# TODO

## server:
- specify port to use
- specify directory for lati code
- validate directory!
- update CLI docs and readme

## git things:
- does a rename or copy count as a change?
- filter by dates! Otherwise this is going to be enormous for big repos
- advanced usage might want the full history so the UI could calculate things like churn.
- add indicator selection to the CLI
- need to test special cases:
  - the repo might not be in git
  - there might be multiple git roots (i.e. I used to scan all the repos for an org into one JSON file)
  - submodules?!
  - local checkout is not on remote origin/master (do we care?)

## other things:
- Can we get rid of test_shared's duplication in cargo.toml ?
- "loc" should fall back to text file processing for unknown files (e.g. cargo.lock!) and store extension (or something for e.g. "Gemfile") as language
- "loc" could also store size for binary files? some repos are full of e.g. pngs
 - the UI might need an option "show binary files" - maybe an alternative view to loc.  Tricky as we would need a complete re-draw
- "-P" cli option is confusing - it's pretty printing _for logs_ !
- can we make the log default "warn"??
- decrease log verbosity for unknown file types

## Bigger things
- publish to github
- indent stats (with or without comments depending on the next bit)
- tokei-based calculations that ignore comments - might need a tokei fork! Or can we pilfer bits of tokei?

## UI stuff
- integrate new formats into existing UI
- minimal UI - no project selector
- consider two UI modes:
  1. dynamic mode - just host a local web server
  2. static mode - publish a static server of current dir for uploading
  (this is so I can both publish demos, but also use it easily for local work)
- look at new UI? Lots of options

## Future stuff
- method identification? Can we work out class/method size metrics from indentation?
- deep git stats - time from author to commit, moving towards CD stats
- churn?
