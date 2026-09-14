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

# Assign before echoing, so a jq failure is the command's status rather than an
# argument to echo: `echo "x=$(jq ...)"` returns echo's 0 even when jq dies, and
# set -e never fires. That wrote `image=` to $GITHUB_OUTPUT and reported success — and
# an empty value is worse than a failure, because `!= '[]'` is true for it, so the test
# jobs would run and die in fromJSON('') with an error unrelated to the real cause.
emit_all() {
  local kind matrix
  for kind in image zip stream; do
    matrix="$(jq -c ".$kind" "$MATRIX")"
    echo "$kind=$matrix" >>"$GITHUB_OUTPUT"
  done
}

if [[ -z "${BASE_SHA:-}" ]]; then
  echo "No base commit (push or manual run): verifying every example."
  emit_all
  exit 0
fi

# HEAD is refs/pull/N/merge, so its first parent is the base tip the merge was computed
# against and its second is the pull request head. HEAD^1..HEAD is therefore exactly the
# pull request's contribution.
#
# Not merge-base with BASE_SHA: that comes from the event payload and can be older than
# the tip the merge ref was recomputed against, in which case merge-base returns
# BASE_SHA itself and the diff also picks up everything that landed on main in between.
# One intervening commit under src/ then trips the shared-path rule below and verifies
# all eighteen examples — measured on a real merge commit here, one file becomes five.
# It over-selects rather than under-selects, so it is a cost rather than a hole, but
# re-running an older pull request is routine enough to be worth avoiding.
if git rev-parse --verify --quiet HEAD^2 >/dev/null; then
  base="$(git rev-parse HEAD^1)"
elif git cat-file -e "$BASE_SHA^{commit}" 2>/dev/null; then
  base="$(git merge-base "$BASE_SHA" HEAD)"
else
  # Only reachable when HEAD is not a merge ref and the payload's base is not in this
  # clone — a shallow fetch, or a fork whose base was never fetched.
  echo "Base commit $BASE_SHA is not available locally: verifying every example."
  emit_all
  exit 0
fi
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

# examples/<name>/... -> <name>. grep exits 1 when nothing matches, which pipefail
# would turn into an unexplained failure of this script — so tolerate that one status,
# and only that one, by keeping grep out of the pipeline below.
example_paths="$(grep -oE '^examples/[^/]+' <<<"$changed" || true)"

# Reachable with an empty diff: a stale pull request whose change already landed
# through a duplicate (#804 and #811 carry an identical update set), or a re-run after
# the commit merged. "Select nothing" is the documented contract here, not "fail" —
# the `if: ... != '[]'` guards in examples.yaml skip the test jobs, and the auto-merge
# workflow refuses a run with no successful test job.
if [[ -z "$example_paths" ]]; then
  echo "No example changed: nothing to verify."
  for kind in image zip stream; do
    echo "$kind=[]" >>"$GITHUB_OUTPUT"
  done
  exit 0
fi

names="$(cut -d/ -f2 <<<"$example_paths" | sort -u | jq -R . | jq -sc .)"
echo "Changed examples: $names"

for kind in image zip stream; do
  matrix="$(jq -c --argjson names "$names" "[.$kind[] | select(.name as \$n | \$names | index(\$n))]" "$MATRIX")"
  echo "$kind=$matrix"
  echo "$kind=$matrix" >>"$GITHUB_OUTPUT"
done
