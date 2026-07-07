#!/bin/bash

set -euo pipefail

# Common helpers for creating git fixtures under tests/data/zipped.

fixture_reset_dir() {
    local dir_name="$1"
    if [[ -d "$dir_name" ]]; then
        rm -rf "$dir_name"
    fi
    mkdir -p "$dir_name"
}

fixture_reset_zip() {
    local zip_name="$1"
    if [[ -f "$zip_name" ]]; then
        rm -f "$zip_name"
    fi
}

fixture_zip_dir() {
    local dir_name="$1"
    local zip_name="$2"
    fixture_reset_zip "$zip_name"
    zip -r "$zip_name" "$dir_name"
}

fixture_git_init_repo() {
    local repo_dir="$1"
    mkdir -p "$repo_dir"
    (
        cd "$repo_dir"
        git init
    )
}

fixture_git_set_identity() {
    local name="$1"
    local email="$2"
    export GIT_AUTHOR_NAME="$name"
    export GIT_AUTHOR_EMAIL="$email"
    export GIT_COMMITTER_NAME="$name"
    export GIT_COMMITTER_EMAIL="$email"
}

fixture_git_set_dates() {
    local iso_datetime="$1"
    export GIT_AUTHOR_DATE="$iso_datetime"
    export GIT_COMMITTER_DATE="$iso_datetime"
}

fixture_git_commit_all() {
    local repo_dir="$1"
    local message="$2"
    (
        cd "$repo_dir"
        git add .
        git commit -m "$message"
    )
}
