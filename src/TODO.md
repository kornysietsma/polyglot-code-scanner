# TODO

## Small things:
* publish to github
* add metrics selection to the CLI

## Bigger things
* Basic git stats (owners, most recent change)
* * does this mean needing the git log thing? That's complex
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
* deep git stats