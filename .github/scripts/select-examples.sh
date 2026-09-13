#!/usr/bin/env bash
#
# Picks which examples the Verify Examples workflow needs to run, and writes one
# matrix per job kind (image, zip, stream) to $GITHUB_OUTPUT.
#
# A dependency bump under examples/remix/remix-app has no bearing on springboot or
# deno-zip, so verifying all 18 matrix entries for it burns runners for no signal.
# With ~70 open Dependabot PRs against the examples that cost dominates CI.
#
# Fails safe: anything this script cannot resolve confidently — no base commit, a
# base commit not present locally, a change to shared code — verifies everything.
#
# Inputs:
#   BASE_SHA       base commit to diff against; empty means "verify everything"
#   GITHUB_OUTPUT  set by Actions
set -euo pipefail

MATRIX="$(dirname "$0")/../example-matrix.json"

emit_all() {
  local kind
  for kind in image zip stream; do
    echo "$kind=$(jq -c ".$kind" "$MATRIX")" >>"$GITHUB_OUTPUT"
  done
}

if [[ -z "${BASE_SHA:-}" ]]; then
  echo "No base commit (push or manual run): verifying every example."
  emit_all
  exit 0
fi

if ! git cat-file -e "$BASE_SHA^{commit}" 2>/dev/null; then
  echo "Base commit $BASE_SHA is not available locally: verifying every example."
  emit_all
  exit 0
fi

# HEAD is the pull request's merge commit, so diffing from the merge base yields
# exactly the changes the PR contributes.
base="$(git merge-base "$BASE_SHA" HEAD)"
changed="$(git diff --name-only "$base" HEAD)"
echo "Changed files:"
echo "$changed" | sed 's/^/  /'

# Shared inputs every example is built against: the adapter itself, the layer
# wrapper, this workflow's own machinery.
if grep -qE '^(src/|layer/|Cargo\.toml$|Cargo\.lock$|\.github/workflows/examples\.yaml$|\.github/scripts/|\.github/example-matrix\.json$)' <<<"$changed"; then
  echo "A shared path changed: verifying every example."
  emit_all
  exit 0
fi

# examples/<name>/... -> <name>
names="$(grep -oE '^examples/[^/]+' <<<"$changed" | cut -d/ -f2 | sort -u | jq -R . | jq -sc .)"
echo "Changed examples: $names"

for kind in image zip stream; do
  matrix="$(jq -c --argjson names "$names" "[.$kind[] | select(.name as \$n | \$names | index(\$n))]" "$MATRIX")"
  echo "$kind=$matrix"
  echo "$kind=$matrix" >>"$GITHUB_OUTPUT"
done
