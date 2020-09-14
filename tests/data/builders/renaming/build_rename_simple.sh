#!/bin/bash -ex

if [[ -d "rename_simple" ]]; then
    rm -r rename_simple
fi

mkdir rename_simple
cd rename_simple

git_dates() {
    # really simple - sets the hour only, so dates are ordered
    if [ -z "$1" ]; then
        echo "needs a param"
        exit 1
    fi
    export GIT_AUTHOR_DATE="2020-09-13T$1:00:00"
    export GIT_COMMITTER_DATE="2020-09-13T$1:00:00"
}

export GIT_AUTHOR_NAME="Kate Smith"
export GIT_AUTHOR_EMAIL="kate@smith.com"
export GIT_COMMITTER_NAME="Jay"
export GIT_COMMITTER_EMAIL="Jay@smith.com"
git_dates "01"

git init

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

if [[ -f "rename_simple.zip" ]]; then
    rm rename_simple.zip
fi

zip -r rename_simple.zip rename_simple
