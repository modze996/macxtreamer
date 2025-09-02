#!/bin/bash

# Exit immediately if a command exits with a non-zero status
set -e

# Check if a version argument is provided
if [ -z "$1" ]; then
  echo "Usage: $0 <new-version>"
  exit 1
fi

NEW_VERSION=$1

# Update the version in Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

echo "Version updated to $NEW_VERSION in Cargo.toml."

# Commit the changes
git add Cargo.toml
git commit -m "Release version $NEW_VERSION"

# Create a new Git tag with a "v" prefix
git tag -a "v$NEW_VERSION" -m "Release version $NEW_VERSION"

echo "Git tag v$NEW_VERSION created."

# Push the changes and the tag to GitHub
git push origin main
git push origin "v$NEW_VERSION"

echo "Changes and tag pushed to GitHub. Release process complete."
