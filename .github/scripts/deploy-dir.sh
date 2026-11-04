#!/usr/bin/env bash
set -euo pipefail

# Required environment variables
: "${PERSONAL_TOKEN:?PERSONAL_TOKEN is required}"
: "${REPOSITORY:?REPOSITORY is required (e.g., wensimehrp/Paiagram-archive)}"
: "${SOURCE_DIR:?SOURCE_DIR is required (e.g., ./dist)}"
: "${TARGET_DIR:?TARGET_DIR is required (e.g., new-snapshot)}"

# Optional environment variables with defaults
BRANCH="${BRANCH:-main}"
GIT_USER_NAME="${GIT_USER_NAME:-github-actions[bot]}"
GIT_USER_EMAIL="${GIT_USER_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"
COMMIT_MESSAGE="${COMMIT_MESSAGE:-Deploy ${TARGET_DIR}}"

SOURCE_PATH="$(realpath "$SOURCE_DIR")"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

# Clone metadata only
git clone \
  --filter=blob:none \
  --no-checkout \
  --depth 1 \
  --branch "$BRANCH" \
  "https://x-access-token:${PERSONAL_TOKEN}@github.com/${REPOSITORY}.git" \
  "$WORK_DIR"

cd "$WORK_DIR"

git config user.name "$GIT_USER_NAME"
git config user.email "$GIT_USER_EMAIL"

# Initialize cone-mode sparse checkout targeting only the destination directory
git sparse-checkout init --cone
git sparse-checkout set "$TARGET_DIR"
git checkout "$BRANCH"

# Wipe existing contents and copy new artifacts
rm -rf "$TARGET_DIR"
mkdir -p "$TARGET_DIR"
cp -a "$SOURCE_PATH"/. "$TARGET_DIR"/

# Stage and commit only if there are actual changes
git add -A "$TARGET_DIR"
if git diff --staged --quiet; then
  echo "No changes in $TARGET_DIR to deploy."
  exit 0
fi

git commit -m "$COMMIT_MESSAGE"

for i in 1 2 3; do
  if git push origin "$BRANCH"; then
    exit 0
  fi
  echo "Push rejected, pulling latest changes and retrying ($i/3)..."
  git pull --rebase origin "$BRANCH"
done

echo "Failed to push after 3 attempts." >&2
exit 1
