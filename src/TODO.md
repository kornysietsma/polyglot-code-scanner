# TODO

## Small things:
* Error handling throughout
* fix `main` to properly report errors
* Re-enable dead code check
* command-line parsing (probably use clap)
* Rename so we can add more metrics than `cloc`
* publish to github
* integrate with JS UI

## Bigger things
* Basic git stats (owners, most recent change)
* * does this mean needing the git log thing? That's complex
* * could for now look at doing what `code-maat` does.
* indent stats (with or without comments depending on the next bit)
* tokei-based calculations that ignore comments - might need a tokei fork! Or can we pilfer bits of tokei?

## Future stuff