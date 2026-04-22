#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

source "$SCRIPT_DIR/../fixture_lib.sh"

cd "$REPO_ROOT/tests/data/zipped"

FIXTURE_NAME="rename_simple"

fixture_reset_dir "$FIXTURE_NAME"
fixture_git_init_repo "$FIXTURE_NAME"
cd "$FIXTURE_NAME"

git_dates() {
    # really simple - sets the hour only, so dates are ordered
    if [ -z "$1" ]; then
        echo "needs a param"
        exit 1
    fi
    fixture_git_set_dates "2020-09-13T$1:00:00"
}

export GIT_AUTHOR_NAME="Kate Smith"
export GIT_AUTHOR_EMAIL="kate@smith.com"
export GIT_COMMITTER_NAME="Jay"
export GIT_COMMITTER_EMAIL="Jay@smith.com"
git_dates "01"

cat <<EOF >a.txt
a
a
a
a
EOF

git add .

git commit -m "initial commit"

git_dates "02"

cat <<EOF >b.txt
b
EOF

git add .

git commit -m "unrelated commit"

git_dates "03"

git mv a.txt c.txt

git add .

git commit -m "moving a to c"

git_dates "04"

git mv c.txt d.txt

echo "d" >>d.txt

git add .

git commit -m "moving and renaming"

cd ..
fixture_zip_dir "$FIXTURE_NAME" "$FIXTURE_NAME.zip"
