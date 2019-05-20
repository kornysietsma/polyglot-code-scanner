# TODO

## Soon:
* ToxicityIndicatorCalculator might want to also look at directories - may not generate numbers, but it might need to update state.
* add indicator selection to the CLI

## Bigger things
* publish to github
* Basic git stats (owners, most recent change)
* * include co-authored-by from commit log!
* * could for now look at doing what `code-maat` does.
* indent stats (with or without comments depending on the next bit)
* tokei-based calculations that ignore comments - might need a tokei fork! Or can we pilfer bits of tokei?

## UI stuff
* integrate new formats into existing UI
* look at new UI?
* consider two UI modes:
  1. dynamic mode - just host a local web server
  2. static mode - publish a static server of current dir for uploading
  (this is so I can both publish demos, but also use it easily for local work)

## Future stuff
* method identification?
* deep git stats - time from author to commit, moving towards CD stats