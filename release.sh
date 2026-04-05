#!/usr/bin/env bash
# Release script for drivemon
# Usage: ./release.sh <version>

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.0"
    exit 1
fi

VERSION=$1
TAG="v$VERSION"
BRANCH=$(git branch --show-current)

echo "Creating release $TAG"

if [ -z "$BRANCH" ]; then
    echo "Error: could not determine the current git branch."
    exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Error: working tree is not clean. Commit or stash your changes first."
    exit 1
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Error: tag $TAG already exists locally."
    exit 1
fi

# Update version in Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Commit version bump only if the version changed
if git diff --quiet -- Cargo.toml; then
    echo "Cargo.toml is already at version $VERSION; skipping version bump commit."
else
    git commit Cargo.toml -m "Bump version to $VERSION"
fi

# Create and push tag
git tag "$TAG"
git push origin "$BRANCH"
git push origin "$TAG"

echo "Release $TAG created!"
echo "GitHub Actions will build the binaries and create the release automatically."
echo "Check https://github.com/zHappySnake/drivemon/actions for build status."
