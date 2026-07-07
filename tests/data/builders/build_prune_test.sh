#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

source "$SCRIPT_DIR/fixture_lib.sh"

cd "$REPO_ROOT/tests/data/zipped"

# This script creates test data for the --prune-inactive-years flag.
# It creates a parent directory with 3 separate git repositories:
#
# 1. repo_with_recent_activity/: A git repo with recent commits
#    - Should be KEPT when --prune-inactive-years 1 (has activity in last 1 year)
#    - Contains both old and recent files (tests directory-level semantics within a repo)
#
# 2. repo_all_old_activity/: A git repo with only old commits
#    - Should be PRUNED when --prune-inactive-years 1 (no activity in last 1 year)
#    - All commits are from Dec 2024 (14 months ago)
#
# 3. repo_no_git/: A directory with no git repo
#    - Should be KEPT (non-git content is preserved)

FIXTURE_NAME="prune_test"

fixture_reset_dir "$FIXTURE_NAME"
cd "$FIXTURE_NAME"

git_dates() {
    # Sets both author and committer dates.
    if [ -z "$1" ]; then
        echo "needs a param (ISO 8601 datetime)"
        exit 1
    fi
    fixture_git_set_dates "$1"
}

fixture_git_set_identity "Test Author" "test@example.com"

# Repository 1: Has recent activity (should be KEPT)
mkdir repo_with_recent_activity
cd repo_with_recent_activity
git init

# Add old file (14 months ago from Feb 2026 = Dec 2024)
git_dates "2024-12-15T10:00:00"
cat <<EOF > old_file.txt
This file was last updated 14 months ago.
But the repository should be kept because it has recent activity elsewhere.
EOF
git add .
git commit -m "Old commit from Dec 2024"

# Add recent file (1 week ago from Feb 2026 = Feb 9 2026)
git_dates "2026-02-09T10:00:00"
cat <<EOF > recent_file.txt
This file was last updated 1 week ago.
Because this repository has recent git activity, it should be kept when
using --prune-inactive-years 1, and all its contents are preserved.
EOF
git add .
git commit -m "Recent commit from Feb 2026"

cd ..

# Repository 2: All activity is old (should be PRUNED)
mkdir repo_all_old_activity
cd repo_all_old_activity
git init

# All commits are old (14 months ago from Feb 2026 = Dec 2024)
git_dates "2024-12-15T10:00:00"
cat <<EOF > very_old_file.txt
This file was last updated 14 months ago.
All commits in this repository are outside the 1-year window,
so this entire repository should be pruned with --prune-inactive-years 1.
EOF
git add .
git commit -m "Old commit from Dec 2024"

# Add another old file to show this is a real repo
git_dates "2024-12-20T10:00:00"
cat <<EOF > another_old_file.txt
Also 14 months old. All activity in this repo is outside the cutoff.
EOF
git add .
git commit -m "Another old commit from Dec 2024"

cd ..

# Directory 3: No git repo (should be KEPT)
mkdir repo_no_git
cat <<EOF > repo_no_git/file_without_git.txt
This file and directory have no git history.
The directory should be kept with --prune-inactive-years since non-git content is preserved.
EOF

cd ..
fixture_zip_dir "$FIXTURE_NAME" "$FIXTURE_NAME.zip"
