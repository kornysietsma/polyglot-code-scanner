#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

source "$SCRIPT_DIR/../fixture_lib.sh"

cd "$REPO_ROOT/tests/data/zipped"

FIXTURE_NAME="rename_complex"

fixture_reset_dir "$FIXTURE_NAME"
fixture_git_init_repo "$FIXTURE_NAME"
cd "$FIXTURE_NAME"

BASE_BRANCH="$(git symbolic-ref --short HEAD)"

author_kate() {
    export GIT_AUTHOR_NAME="Kate Smith"
    export GIT_AUTHOR_EMAIL="kate@smith.com"
}

author_jay() {
    export GIT_AUTHOR_NAME="Jay"
    export GIT_AUTHOR_EMAIL="Jay@smith.com"
}

committer_jay() {
    export GIT_COMMITTER_NAME="Jay"
    export GIT_COMMITTER_EMAIL="Jay@smith.com"
}

author_dave() {
    export GIT_AUTHOR_NAME="Dave Smith"
    export GIT_AUTHOR_EMAIL="dave@smith.com"
}

git_dates() {
    # really simple - sets the hour only, so dates are ordered
    if [ -z "$1" ]; then
        echo "needs a param"
        exit 1
    fi
    fixture_git_set_dates "2020-09-13T$1:00:00"
}

author_kate
committer_jay

git_dates "01"

cat <<EOF >a.txt
a
a
a
a
EOF

cat <<EOF >z.txt
z
z
z
z
EOF

git add .
git commit -am "initial commit"

git_dates "02"

git mv a.txt a1.txt
git commit -am "rename a to a1"

git_dates "03"

author_dave
committer_jay

git checkout -b "dave_work"
git mv a1.txt a2.txt
echo "junk" >>a2.txt

cat <<EOF >bb.txt
b
b
b
b
EOF

git rm z.txt

git add .
git commit -am "rename a1 to a2, add bb, kill z"

git_dates "05"

git mv bb.txt b.txt
git mv a2.txt a.txt

git commit -am "rename bb to b, a2 back to a"

git checkout "$BASE_BRANCH"

git_dates "04"
author_jay
committer_jay

git checkout -b "jay_work"

git mv a1.txt aa.txt
echo "junk!" >>aa.txt

cat <<EOF >bee.txt
B
B
B
EOF

git add .
git commit -am "rename a1 to aa, add bee"

git_dates "06"

git mv bee.txt b.txt
git mv aa.txt a.txt
git add .
git commit -m "rename bee to b, aa back to a"

git checkout "$BASE_BRANCH"

git_dates "07"
author_kate
committer_jay

git mv a1.txt a.txt
git commit -m "rename a1 back to a prep merging"

git merge jay_work -m "merging jay work"

git_dates "08"

git merge dave_work -m "merging dave work" || true # will fail!

echo "fixing"

git_dates "09"

cat <<EOF >a.txt
a
a
a
a
fixed
EOF

cat <<EOF >b.txt
b
b
b
b
fixed
EOF

git commit -am "merging dave work with fixes"

git_dates "10"

cat <<EOF >z.txt
z
z
z
z
fixed
EOF

git add z.txt

git commit -m "restoring deleted z"

cd ..
fixture_zip_dir "$FIXTURE_NAME" "$FIXTURE_NAME.zip"
