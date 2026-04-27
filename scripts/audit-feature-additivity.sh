#!/usr/bin/env bash
# audit-feature-additivity.sh
#
# Audits every non-default Cargo feature in every workspace crate for public-API
# additivity violations: enabling a feature must not remove or break any public API
# item that was present without it.
#
# Method: off-label use of cargo-semver-checks with --baseline-rustdoc /
# --current-rustdoc to diff two rustdoc JSON files built from the same source
# at different feature sets.
#
# Requirements: cargo +nightly, cargo-semver-checks, jq
#
# Usage: run from the workspace root, or pass the workspace root as first argument.
#   ./scripts/audit-feature-additivity.sh [workspace-root]

set -euo pipefail

###############################################################################
# Helpers
###############################################################################

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

info()    { echo -e "${BOLD}[INFO]${RESET} $*"; }
ok()      { echo -e "${GREEN}[PASS]${RESET} $*"; }
warn()    { echo -e "${YELLOW}[WARN]${RESET} $*"; }
fail()    { echo -e "${RED}[FAIL]${RESET} $*"; }

###############################################################################
# Dependency checks
###############################################################################

check_deps() {
    local missing=0
    if ! cargo +nightly --version >/dev/null 2>&1; then
        echo "ERROR: cargo +nightly not found. Install with: rustup toolchain install nightly" >&2
        missing=1
    fi
    if ! cargo semver-checks --version >/dev/null 2>&1; then
        echo "ERROR: cargo-semver-checks not found. Install with: cargo install cargo-semver-checks" >&2
        missing=1
    fi
    if ! command -v jq >/dev/null 2>&1; then
        echo "ERROR: jq not found. Install with your system package manager." >&2
        missing=1
    fi
    if [[ $missing -ne 0 ]]; then exit 1; fi
}

###############################################################################
# Main
###############################################################################

check_deps

WORKSPACE_ROOT="${1:-$(pwd)}"
cd "$WORKSPACE_ROOT"

# Validate we're at a workspace root
if [[ ! -f Cargo.toml ]]; then
    echo "ERROR: No Cargo.toml found in $WORKSPACE_ROOT" >&2
    exit 1
fi

TIMESTAMP=$(date -u +"%Y-%m-%dT%H-%M-%SZ")
TMPDIR_AUDIT=$(mktemp -d)
trap 'rm -rf "$TMPDIR_AUDIT"' EXIT

RESULTS_DIR="docs/results"
mkdir -p "$RESULTS_DIR"
RESULTS_FILE="$RESULTS_DIR/feature-additivity-audit-${TIMESTAMP}.md"

# Counters
TOTAL=0
PASSES=0
VIOLATIONS=0
BUILD_ERRORS=0

# Track rows for the results table
declare -a RESULT_ROWS=()

info "Collecting workspace metadata..."
METADATA=$(cargo metadata --no-deps --format-version 1)
WORKSPACE_ROOT_META=$(echo "$METADATA" | jq -r '.workspace_root')
TARGET_DIR=$(echo "$METADATA" | jq -r '.target_directory')

info "Starting feature additivity audit"
info "Workspace: $WORKSPACE_ROOT_META"
info "Nightly: $(cargo +nightly --version)"
info "cargo-semver-checks: $(cargo semver-checks --version)"
echo ""

###############################################################################
# Per-package loop
###############################################################################

while IFS= read -r pkg_json; do
    PKG_NAME=$(echo "$pkg_json" | jq -r '.name')

    # Find the lib target name (hyphens → underscores in Cargo, but the target
    # name field already uses underscores).
    LIB_NAME=$(echo "$pkg_json" | jq -r '
        .targets[]
        | select(.kind[] | contains("lib"))
        | .name
    ' | head -1)

    if [[ -z "$LIB_NAME" ]]; then
        info "[$PKG_NAME] No lib target, skipping."
        continue
    fi

    # All feature keys except "default"
    mapfile -t FEATURES < <(echo "$pkg_json" | jq -r '
        .features | keys[] | select(. != "default")
    ')

    if [[ ${#FEATURES[@]} -eq 0 ]]; then
        info "[$PKG_NAME] No non-default features, skipping."
        continue
    fi

    echo -e "${BOLD}=== $PKG_NAME (${#FEATURES[@]} features) ===${RESET}"

    JSON_PATH="${TARGET_DIR}/doc/${LIB_NAME}.json"

    for FEAT in "${FEATURES[@]}"; do
        TOTAL=$((TOTAL + 1))
        BASELINE="$TMPDIR_AUDIT/${PKG_NAME}_${FEAT}_baseline.json"
        CURRENT="$TMPDIR_AUDIT/${PKG_NAME}_${FEAT}_current.json"

        info "[$PKG_NAME] Auditing feature: $FEAT"

        # ── Build baseline (no features) ─────────────────────────────────────
        rm -f "$JSON_PATH"
        if ! cargo +nightly -Z unstable-options rustdoc \
                -p "$PKG_NAME" \
                --lib \
                --no-default-features \
                --output-format json \
                2>"$TMPDIR_AUDIT/build_baseline_${PKG_NAME}_${FEAT}.log"; then
            warn "[$PKG_NAME:$FEAT] Baseline build failed (see log). Skipping."
            BUILD_ERRORS=$((BUILD_ERRORS + 1))
            RESULT_ROWS+=("| $PKG_NAME | $FEAT | BUILD-ERROR (baseline) |")
            continue
        fi
        if [[ ! -f "$JSON_PATH" ]]; then
            warn "[$PKG_NAME:$FEAT] Expected $JSON_PATH not found after baseline build. Skipping."
            BUILD_ERRORS=$((BUILD_ERRORS + 1))
            RESULT_ROWS+=("| $PKG_NAME | $FEAT | BUILD-ERROR (baseline json missing) |")
            continue
        fi
        cp "$JSON_PATH" "$BASELINE"

        # ── Build with feature ────────────────────────────────────────────────
        rm -f "$JSON_PATH"
        if ! cargo +nightly -Z unstable-options rustdoc \
                -p "$PKG_NAME" \
                --lib \
                --no-default-features \
                --features "$FEAT" \
                --output-format json \
                2>"$TMPDIR_AUDIT/build_current_${PKG_NAME}_${FEAT}.log"; then
            warn "[$PKG_NAME:$FEAT] Feature build failed (see log). Skipping."
            BUILD_ERRORS=$((BUILD_ERRORS + 1))
            RESULT_ROWS+=("| $PKG_NAME | $FEAT | BUILD-ERROR (feature build) |")
            continue
        fi
        if [[ ! -f "$JSON_PATH" ]]; then
            warn "[$PKG_NAME:$FEAT] Expected $JSON_PATH not found after feature build. Skipping."
            BUILD_ERRORS=$((BUILD_ERRORS + 1))
            RESULT_ROWS+=("| $PKG_NAME | $FEAT | BUILD-ERROR (feature json missing) |")
            continue
        fi
        cp "$JSON_PATH" "$CURRENT"

        # ── Run semver-checks ─────────────────────────────────────────────────
        SEMVER_LOG="$TMPDIR_AUDIT/semver_${PKG_NAME}_${FEAT}.log"
        if cargo semver-checks \
                --baseline-rustdoc "$BASELINE" \
                --current-rustdoc  "$CURRENT" \
                >"$SEMVER_LOG" 2>&1; then
            ok "[$PKG_NAME:$FEAT] No additivity violations."
            PASSES=$((PASSES + 1))
            RESULT_ROWS+=("| $PKG_NAME | $FEAT | PASS |")
        else
            fail "[$PKG_NAME:$FEAT] Additivity violation(s) detected!"
            # Print the semver-checks output so it's visible
            cat "$SEMVER_LOG"
            VIOLATIONS=$((VIOLATIONS + 1))
            # Capture first lint line for the results table
            LINT_SUMMARY=$(grep -E '^\s*(error|warning)\[' "$SEMVER_LOG" | head -3 | tr '\n' '; ' || echo "see log")
            RESULT_ROWS+=("| $PKG_NAME | $FEAT | **VIOLATION** — $LINT_SUMMARY |")
        fi
    done
    echo ""

done < <(echo "$METADATA" | jq -c '.packages[]')

###############################################################################
# Summary
###############################################################################

echo ""
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${BOLD}Feature Additivity Audit — Summary${RESET}"
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo "  Total features audited : $TOTAL"
echo -e "  ${GREEN}Passed${RESET}                 : $PASSES"
echo -e "  ${RED}Violations${RESET}             : $VIOLATIONS"
echo -e "  ${YELLOW}Build errors (skipped)${RESET} : $BUILD_ERRORS"
echo ""

###############################################################################
# Write results markdown
###############################################################################

cat > "$RESULTS_FILE" <<EOF
# Feature Additivity Audit Results

**Date:** $TIMESTAMP
**Workspace:** $WORKSPACE_ROOT_META
**Tool:** cargo-semver-checks $(cargo semver-checks --version 2>/dev/null)
**Nightly:** $(cargo +nightly --version 2>/dev/null)

## Summary

| Metric | Count |
|--------|-------|
| Total features audited | $TOTAL |
| Passed | $PASSES |
| Violations | $VIOLATIONS |
| Build errors (skipped) | $BUILD_ERRORS |

## Per-feature results

| Crate | Feature | Result |
|-------|---------|--------|
$(printf '%s\n' "${RESULT_ROWS[@]}")

## Method

Each non-default feature \`f\` in each workspace crate was audited by:
1. Building rustdoc JSON with \`--no-default-features\` (baseline)
2. Building rustdoc JSON with \`--no-default-features --features f\` (current)
3. Running \`cargo semver-checks --baseline-rustdoc ... --current-rustdoc ...\`

A violation means feature \`f\` removes or breaks a public API item that is present without it,
which violates the Cargo SemVer additivity rule (Cargo Book § features/semver-compatibility).

## Decisions

- Baseline is always \`--no-default-features\` (most conservative; checks pure additivity of \`f\`).
- \`default\` feature key is excluded (it is an alias list, not an auditable API surface).
- Build errors are reported but do not abort the script; remaining features are still audited.
- Platform-conditional features (e.g. \`embedded-svc-http\` on non-ESP-IDF) may produce build errors; this is expected.
EOF

info "Results written to $RESULTS_FILE"

###############################################################################
# Exit code
###############################################################################

if [[ $VIOLATIONS -gt 0 ]]; then
    fail "Audit complete: $VIOLATIONS violation(s) found."
    exit 1
else
    ok "Audit complete: no violations found."
    exit 0
fi
