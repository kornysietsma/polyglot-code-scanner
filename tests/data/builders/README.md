# Test data builders

This directory contains scripts used to build test repositories.

You shouldn't need to run anything here unless you are changing tests - the generated tests will all be checked in to git.

## Reusable Helper

Use `fixture_lib.sh` for common fixture operations:

- Reset fixture directories and zip files
- Initialize git repositories
- Set deterministic git identity and dates
- Commit staged changes
- Zip fixture directories

## Example

To regenerate the git dedup fixture zip used by integration tests:

```sh
./tests/data/builders/build_git_dedup_sample.sh
```
