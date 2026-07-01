#!/usr/bin/env bash
# release.sh — bump versions, tag, and push to trigger the release CI.
#
# Usage:
#   ./release.sh <version>          # e.g. ./release.sh 3.0.0
#   ./release.sh <version> --dry-run  # preview without making any changes
#
# What it does:
#   1. Validates the version is semver X.Y.Z
#   2. Checks the working tree is clean
#   3. Bumps version in webcore-compiler/Cargo.toml and Cargo.lock
#   4. Bumps version in editors/vscode/package.json
#   5. Renames ## [Unreleased …] → ## [X.Y.Z] in CHANGELOG.md
#      and inserts a fresh ## [Unreleased] section above it
#   6. Commits, tags, and pushes — the CI release workflow fires on the tag

set -euo pipefail

# ── helpers ──────────────────────────────────────────────────────────────────

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n'  "$*"; }
step()  { printf '\n\033[1;34m▸ %s\033[0m\n' "$*"; }
die()   { red "error: $*" >&2; exit 1; }

DRY=0
for arg in "$@"; do [[ "$arg" == "--dry-run" ]] && DRY=1; done

run() {
    if [[ $DRY -eq 1 ]]; then
        printf '  \033[2m(dry) %s\033[0m\n' "$*"
    else
        "$@"
    fi
}

# ── arguments ────────────────────────────────────────────────────────────────

VERSION="${1:-}"
[[ -z "$VERSION" ]] && die "usage: $0 <version> [--dry-run]  (e.g. $0 3.0.0)"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || die "version must be X.Y.Z semver, got: $VERSION"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

[[ $DRY -eq 1 ]] && bold "Dry-run mode — nothing will be written or pushed"

bold "Preparing release $VERSION"

# ── pre-flight checks ────────────────────────────────────────────────────────

step "Checking git state"

if ! git rev-parse --git-dir &>/dev/null; then
    die "Not inside a git repository."
fi

BRANCH=$(git rev-parse --abbrev-ref HEAD)

# Must be on main
if [[ "$BRANCH" != "main" ]]; then
    die "Releases must be made from 'main', you are on '$BRANCH'." \
        $'\n  Merge develop → main first, then re-run.'
fi

# Dirty working tree check
if [[ -n "$(git status --porcelain)" ]]; then
    die "Working tree is dirty. Commit or stash your changes first."
fi

# Duplicate tag check
if git rev-parse "$VERSION" &>/dev/null; then
    die "Tag '$VERSION' already exists locally."
fi

printf '  branch : %s\n' "$BRANCH"
printf '  tag    : %s (new)\n' "$VERSION"

# ── doc version check ────────────────────────────────────────────────────────

step "Checking doc versions match $VERSION"

doc_errors=0

check_doc() {
    local label="$1" found="$2"
    if [[ "$found" == "$VERSION" ]]; then
        printf '  ✓ %-28s %s\n' "$label" "$found"
    else
        printf '  ✗ %-28s found "%s", expected "%s"\n' "$label" "$found" "$VERSION"
        doc_errors=$((doc_errors + 1))
    fi
}

# README.md — "| **Version** | X.Y.Z |"
readme_ver=$(awk -F'|' '/\*\*Version\*\*/ { gsub(/ /,"",$3); print $3; exit }' README.md)
check_doc "README.md"        "$readme_ver"

# README_EN.md — same pattern
readme_en_ver=$(awk -F'|' '/\*\*Version\*\*/ { gsub(/ /,"",$3); print $3; exit }' README_EN.md)
check_doc "README_EN.md"     "$readme_en_ver"

# docs/spec.md — "> Version : X.Y.Z — …"
spec_ver=$(awk '/^> Version :/ { sub(/^> Version : /, ""); sub(/ —.*/, ""); gsub(/ /,""); print; exit }' docs/spec.md)
check_doc "docs/spec.md"     "$spec_ver"

if [[ $doc_errors -gt 0 ]]; then
    die "$doc_errors doc file(s) are not up to date." \
        $'\n  Update the Version field(s) above to '"$VERSION"' and re-run.'
fi

# ── bump Cargo.toml ──────────────────────────────────────────────────────────

step "Bumping webcore-compiler/Cargo.toml"

CARGO_TOML="webcore-compiler/Cargo.toml"
OLD_CRATE=$(awk '
    /^\[package\]/      { in_pkg = 1; next }
    /^\[/               { in_pkg = 0 }
    in_pkg && /^[[:space:]]*version[[:space:]]*=/ {
        gsub(/.*=[[:space:]]*"/, ""); gsub(/".*/, ""); print; exit
    }
' "$CARGO_TOML")

[[ -z "$OLD_CRATE" ]] && die "Could not read [package] version from $CARGO_TOML"
printf '  %s → %s\n' "$OLD_CRATE" "$VERSION"

run sed -i "0,/^version = \"${OLD_CRATE}\"/{s/^version = \"${OLD_CRATE}\"/version = \"${VERSION}\"/}" \
    "$CARGO_TOML"

# ── bump Cargo.lock ──────────────────────────────────────────────────────────

step "Updating Cargo.lock"

CARGO_LOCK="webcore-compiler/Cargo.lock"

# The lock file contains an exact block:
#   name = "webcore-compiler"
#   version = "OLD"
# Replace only that occurrence (the package's own entry, not a dep of a dep).
run sed -i "/^name = \"webcore-compiler\"/{n;s/^version = \"${OLD_CRATE}\"/version = \"${VERSION}\"/}" \
    "$CARGO_LOCK"

# ── bump editors/vscode/package.json ─────────────────────────────────────────

step "Bumping editors/vscode/package.json"

PKG_JSON="editors/vscode/package.json"
OLD_PKG=$(python3 -c "import json,sys; d=json.load(open('$PKG_JSON')); print(d['version'])")
printf '  %s → %s\n' "$OLD_PKG" "$VERSION"

run python3 - "$PKG_JSON" "$VERSION" <<'PYEOF'
import json, sys
path, ver = sys.argv[1], sys.argv[2]
d = json.load(open(path))
d['version'] = ver
open(path, 'w').write(json.dumps(d, indent=4, ensure_ascii=False) + '\n')
PYEOF

# ── update CHANGELOG.md ──────────────────────────────────────────────────────

step "Updating CHANGELOG.md"

CHANGELOG="CHANGELOG.md"

# Check there's an Unreleased section to promote
if ! grep -q "^## \[Unreleased" "$CHANGELOG"; then
    die "No '## [Unreleased' section found in $CHANGELOG — add your release notes there first."
fi

# 1. Rename ## [Unreleased …] → ## [VERSION]
# 2. Insert a blank ## [Unreleased] block just above it
run python3 - "$CHANGELOG" "$VERSION" <<'PYEOF'
import sys, re

path, ver = sys.argv[1], sys.argv[2]
text = open(path).read()

# Replace the first ## [Unreleased ...] line with ## [VERSION]
new_text = re.sub(
    r'^## \[Unreleased[^\]]*\]',
    f'## [{ver}]',
    text,
    count=1,
    flags=re.MULTILINE,
)

# Insert fresh ## [Unreleased] section before ## [VERSION]
new_text = new_text.replace(
    f'## [{ver}]',
    f'## [Unreleased]\n\n---\n\n## [{ver}]',
    1,
)

open(path, 'w').write(new_text)
PYEOF

printf '  ## [Unreleased …] → ## [%s]\n' "$VERSION"
printf '  Fresh ## [Unreleased] section added above\n'

# ── commit ───────────────────────────────────────────────────────────────────

step "Committing"

run git add \
    "$CARGO_TOML" \
    "$CARGO_LOCK" \
    "$PKG_JSON" \
    "$CHANGELOG"

run git commit -m "chore: release $VERSION"

# ── tag ──────────────────────────────────────────────────────────────────────

step "Tagging"

run git tag "$VERSION"
printf '  created tag %s\n' "$VERSION"

# ── push ─────────────────────────────────────────────────────────────────────

step "Pushing"

run git push origin "$BRANCH"
run git push origin "$VERSION"

# ── done ─────────────────────────────────────────────────────────────────────

printf '\n'
if [[ $DRY -eq 1 ]]; then
    bold "Dry-run complete — no changes were made."
else
    green "Release $VERSION dispatched! CI will build and publish to GitHub Releases."
    printf '  branch : %s pushed\n' "$BRANCH"
    printf '  tag    : %s pushed → release workflow triggered\n' "$VERSION"
fi
