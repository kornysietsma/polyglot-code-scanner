#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

source "$SCRIPT_DIR/fixture_lib.sh"

cd "$REPO_ROOT/tests/data/zipped"

FIXTURE_NAME="git_dedup_sample"

fixture_reset_dir "$FIXTURE_NAME"
fixture_git_init_repo "$FIXTURE_NAME"

cat <<EOF > "$FIXTURE_NAME/sample.txt"
line 1
EOF

fixture_git_set_identity "Alice One" "alice@example.com"
fixture_git_set_dates "2020-01-01T00:00:00Z"
fixture_git_commit_all "$FIXTURE_NAME" "initial commit by Alice One"

cat <<EOF > "$FIXTURE_NAME/sample.txt"
line 1
line 2
EOF

fixture_git_set_identity "Alice Two" "alice@example.com"
fixture_git_set_dates "2020-01-02T00:00:00Z"
fixture_git_commit_all "$FIXTURE_NAME" "follow-up commit by Alice Two"

fixture_zip_dir "$FIXTURE_NAME" "$FIXTURE_NAME.zip"
