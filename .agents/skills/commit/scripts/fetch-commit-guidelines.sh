#!/usr/bin/env bash
set -euo pipefail

URL="https://develop.sentry.dev/engineering-practices/commit-messages.md"
curl -fsSL "$URL"
