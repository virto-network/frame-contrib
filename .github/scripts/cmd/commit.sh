#!/usr/bin/env bash
# Commits the changes of a `/cmd` command, and pushes them to the pull request's branch.
#
# Runs from the root of the pull request's checkout. Expects `CMD`, `PR_BRANCH`, `GH_ACTOR`,
# `TOKEN`, `GITHUB_REPOSITORY` and, when pushing with a GitHub App token, `APP_SLUG`.
set -euo pipefail

if [ -z "$(git status --porcelain)" ]; then
  echo "Nothing to commit"
  exit 0
fi

if [ -n "${APP_SLUG:-}" ]; then
  name="${APP_SLUG}[bot]"
  # `-g`: the `[bot]` suffix is part of the login, not a curl URL range.
  id="$(curl -gfsSL -H "Authorization: Bearer $TOKEN" "https://api.github.com/users/$name" | jq -r .id)"
  email="$id+$name@users.noreply.github.com"
else
  name="github-actions[bot]"
  email="41898282+github-actions[bot]@users.noreply.github.com"
fi
git config user.name "$name"
git config user.email "$email"

git add .
git restore --staged Cargo.lock 2>/dev/null || true # ignore changes in Cargo.lock
if git diff --cached --quiet; then
  echo "Nothing to commit"
  exit 0
fi
git diff --cached --stat

git commit -m "chore: run \`/cmd $CMD\`" -m "Requested by @$GH_ACTOR."
# Fails (rather than overwriting) if the branch moved while the command ran.
git push "https://x-access-token:$TOKEN@github.com/$GITHUB_REPOSITORY.git" "HEAD:refs/heads/$PR_BRANCH"
