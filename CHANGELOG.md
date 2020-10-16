# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate
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
