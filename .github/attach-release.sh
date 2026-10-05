#!/bin/sh
# Attach assets to this tag's release, creating the release if this build got
# there first. Both jobs run this, and either may win the race — hence the
# `|| true` on the create (the upload below is what reports a real failure).
#
# Every call names the repository explicitly. `gh` otherwise discovers it by
# shelling out to `git`, which fails inside the Arch container ("not a git
# repository") even with git installed.
set -eu

tag="${GITHUB_REF_NAME:?GITHUB_REF_NAME is not set}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set}"

if ! gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
    gh release create "$tag" --repo "$repo" --title "$tag" --generate-notes || true
fi

gh release upload "$tag" "$@" --repo "$repo" --clobber
