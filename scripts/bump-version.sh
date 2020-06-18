#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR/..

OLD_VERSION="${1}"
NEW_VERSION="${2}"

echo "Current version: ${OLD_VERSION}"
echo "Bumping version: ${NEW_VERSION}"

perl -pi -e "s/^version = \".*?\"/version = \"$NEW_VERSION\"/" sentry*/Cargo.toml
perl -pi -e "s/^(sentry.*)?version = \".*?\"/\$1version = \"$NEW_VERSION\"/" sentry*/Cargo.toml
