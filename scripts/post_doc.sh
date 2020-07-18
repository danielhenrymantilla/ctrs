#!/bin/bash
set -euxo pipefail

cd "$(git rev-parse --show-toplevel)"

VERSION=$(
    cargo metadata --offline --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name == "inline_proc_macros") | .version'
)
mv target/doc{,2}
mkdir target/doc
mv target/doc2 target/doc/$VERSION-dev
{
    echo '<!DOCTYPE html><html><head><title>inline_proc_macros</title><meta http-equiv = "refresh" content = "0; url = '
    echo $VERSION-dev/
    echo 'inline_proc_macros/" /></head><body></body></html>'
} > target/doc/index.html
