#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/bump-version.sh [patch|minor|major]
#   patch: 0.1.0 → 0.1.1 (default)
#   minor: 0.1.0 → 0.2.0
#   major: 0.1.0 → 1.0.0

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

CURRENT="$(cat "$ROOT/VERSION" | tr -d ' \n')"
echo "Current version: $CURRENT"

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "${1:-patch}" in
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    ;;
  patch|*)
    PATCH=$((PATCH + 1))
    ;;
esac

NEW="$MAJOR.$MINOR.$PATCH"
echo "New version:     $NEW"

# Update VERSION file
echo "$NEW" > "$ROOT/VERSION"

# Update Cargo.toml
sed -i "s/^version = \"$CURRENT\"/version = \"$NEW\"/" "$ROOT/Cargo.toml"

echo ""
echo "Updated VERSION and Cargo.toml to $NEW"
echo "Next step: git add -A && git commit -m \"chore: bump version to $NEW\""
