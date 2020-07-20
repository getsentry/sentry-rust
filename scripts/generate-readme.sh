#!/bin/bash
set -euo pipefail

NEW_VERSION="${1}"
CRATE=$(basename $PWD)
CRATE_UNDERSCORE=$(echo $CRATE | sed s/-/_/)

cargo readme --template ../README.tpl --output README.md

# rewrite any relative markdown links and point them to docs.rs.
# this will break at some point with intra-rustdoc links :-(
# See https://github.com/livioribeiro/cargo-readme/issues/18
perl -pi -e "s/\](\(|: )(?!http)(\w+)/\]\$1https:\/\/docs.rs\/${CRATE}\/${NEW_VERSION}\/$CRATE_UNDERSCORE\/\$2/g" README.md
