# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate
## [0.4.4] - 2022-11-21

* fixed error with non-UTF8 files in the recent unknown languages logic
* added error context in a few places to make diagnosing errors easier

## [0.4.3] - 2022-11-16

* tidying up

## [0.4.2] - 2022-11-16

* Unknown languages now treated as pure code - previously they were 'Text' which was treated by tokei as all comments, so ignored in stats!
* Added support for 'pf' fortran unit test files (via changes to my tokei fork)

## [0.4.1] - 2022-11-09

* Fixed the [tokei fork](https://github.com/kornysietsma/tokei/tree/accumulate-lines) to properly get indentation for COBOL and other languages with simple per-line comment checks
* Updated `Cargo.toml` to check for a particular tag in the Tokei fork, not just use the branch name

## [0.4.0] - 2022-10-13

* New release - it feels that a lot has changed, really 0.3.2 should have been 0.4.0! Anyway better late than never.
* Major changes since 0.3.1:
  * Output file format reworked
  * Project files have names and IDs
  * Disabling git scanning
  * Feature flags
  * various bug fixes

## [0.3.15] - 2022-10-06

* Bug fix for some co-authored-by lines
  * sometimes commit messages are terminated by `\r\n` characters, but rust assumed they were `\n` terminated - and my co-author regular expression didn't ignore the `\r` whitespace!
* fix for linux binaries finally
  * deleted releases 0.3.5 to 0.3.14 - debugging Github Actions took a lot of work, and a lot of release-fix-release cycles!

## [0.3.4] - 2022-10-06

* Point release mostly to test fixes to the release process!

## [0.3.3] - 2022-09-28

* Somewhat breaking release - the output file schema won't change, but the logic is changing - so now data format is 1.0.1 as this is sort-of compatible:
  * Previously all changes for a day were combined into a single GitDetails entry, now however I am generating a new GitDetails for each unique user set.
  * This is because previously if Jane made 1 change and Joe made 100, they would both show up as changes by Jane and Joe
  * This will make output files a bit more verbose, but hopefully not too much, but new user and team info was being distorted by this - now the UI shows you Jane and Joe separately, we need to track them separately, unless they are co-authors on a commit.
* Added a DesignDecisons.md document to discuss the next change:
* Removed the way the code used to use generic `Value` types for indicator data - everything is concrete types now.  See `DesignDecisions.md` for discussion
* Added feature flags, including new 'disable git' option, and flags in JSON output (data format v1.0.2)
* Added file creation and modification times, so the explorer can use them when git is disabled

## [0.3.2] - 2022-09-20

* Backward-incompatible release - changing output file format for a few reasons:
  * I want a unique ID that the front end can use by default for BrowserStorage - this can be specified or random
    * actually now split into 'name' which is descriptive, and 'id' for storage etc.
  * I want a semantic version number in the data file, so the front-end can tell if it knows the data format
  * I'm moving the front-end to Typescript which means I'd like to keep types a bit cleaner, rather than just dumping data in the 'root' directory node
  * Really the old 'flare' file format hasn't been meaningful for a while, so I might as well dump irrelevant bits (like the 'value' on each node - redundant and confusing)
* username / emails are now de-duplicated by case - so if you have "Jane smith" and "Jane Smith" as git user names, they will get the same user id (and the case of whichever example was seen first) - this was needed as, especially with `co-authored-by` tags, the same user could show up several times with only case differences.

## [0.3.1] - 2022-07-13

### Changed

* Added an option to follow symlinks to fix issue #1

## [0.3.0] - 2021-04-05

### Changed

* Major change - new coupling logic, fine-grained based on timestamps instead of aggregating into daily buckets.  This will need a lot of documenting, which will probably be on the main website not here.
* updating tokei to latest code - this was ugly as tokei is now multithreaded and more complex. Modified tokei fork at <https://github.com/kornysietsma/tokei/tree/accumulate-lines> to accumulate lines of code - but note they aren't ordered so this works for stats but not much else
* Updated all other dependencies to latest stable bits

## [0.2.1] - 2020-10-16

### Changed

* fixing build on Windows

## [0.2.0] - 2020-09-16

### Added

* git log logic follows renames - a fair bit of work, as it requires splitting the git log processing into two passes, one to aggregate all rename/deletes and parent/child relationships, and one that uses that data to find what files end up being named in the final revision.

### Changed

* Git logging may be slower and use more memory, as interim git log data is stored in memory.

## [0.1.2] - 2020-08-25

## [0.1.1] - 2020-08-24

### Changed

* Trying to get Travis to publish binaries

## [0.1.0] - 2020-08-24

### Added

* Tagged with version 0.1.0
* Added this changelog, following [cargo-release suggestions](https://github.com/sunng87/cargo-release/blob/master/docs/faq.md#maintaining-changelog) and <https://keepachangelog.com>
* First release with binary files
