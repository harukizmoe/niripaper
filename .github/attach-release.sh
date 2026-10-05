#!/bin/sh
# Attach assets to this tag's release, creating the release if this build got
# there first. Both jobs run this, and either may win the race — hence the
# `|| true` on the create (the upload below is what reports a real failure).
set -eu

tag="${GITHUB_REF_NAME:?GITHUB_REF_NAME is not set}"

if ! gh release view "$tag" >/dev/null 2>&1; then
    gh release create "$tag" --title "$tag" --generate-notes || true
fi

gh release upload "$tag" "$@" --clobber
