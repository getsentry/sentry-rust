#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR/..

NEW_VERSION="${1}"

find sentry* -name Cargo.toml -execdir ../scripts/generate-readme.sh $NEW_VERSION \;
