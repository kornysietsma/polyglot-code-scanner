# TODO

See Trello for anything not short term (sorry for people looking at github, but
I had to look at cross-repo plans and goals)

Small / immediate things:

- add test that checks binary files and unknown text files (e.g. erb)
- refactoring - use Into more ? "fn new<S: Into<String>>(name: S, is_file: bool)" allows the caller to decide...
- Can we get rid of test_shared's duplication in cargo.toml ?
- "-P" cli option is confusing - it's pretty printing _for logs_ !
- can we make the log default "warn"??
