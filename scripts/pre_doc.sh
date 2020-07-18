#!/bin/bash
set -euxo pipefail

cd "$(git rev-parse --show-toplevel)"

COMMIT_HASH=$(git log --pretty=format:'%h' -n 1)
sed -i -e "s|\${COMMIT_HASH}|${COMMIT_HASH}|g" src/macro_use.md
