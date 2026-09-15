#!/usr/bin/env bash
#
# Picks which examples the Verify Examples workflow needs to run, and writes one
# matrix per job kind (image, zip, stream) to $GITHUB_OUTPUT.
#
# A dependency bump under examples/remix/remix-app has no bearing on springboot or
# deno-zip, so verifying all 18 matrix entries for it burns runners for no signal.
# With ~70 open Dependabot PRs against the examples that cost dominates CI.
#
# Three outcomes, in order of confidence:
#
#   verify everything — no base commit (a push or a manual run), a base commit this
#     clone does not have, or a change to a shared input every example is built against.
#   verify the examples in the diff — the normal pull request case.
#   verify nothing — the diff is empty, or touches nothing under examples/. The
#     `if: ... != '[]'` guards in examples.yaml skip the test jobs, and examples-verified
#     treats a skipped job as a pass, so the workflow is green with nothing to run.
#
# The diff base comes from the merge ref's first parent, not from BASE_SHA, for the
# reason recorded below.
#
# Inputs:
#   BASE_SHA       base commit from the event payload; empty means "verify everything".
#                  Only used when HEAD is not a merge ref.
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
    # `has` rather than a bare `.$kind`: jq prints the literal `null` and exits 0 for a
    # missing key, so a renamed top-level key in the matrix file wrote `stream=null`,
    # which `!= '[]'` reads as truthy — test-stream would start and die in
    # fromJSON('null') complaining about the workflow instead of the matrix file. The
    # selection loop at the bottom already fails loudly here, because `.$kind[]` over
    # null is a jq error; this is the path every push to main takes.
    matrix="$(jq -ce --arg kind "$kind" \
      'if has($kind) then .[$kind] else error("example-matrix.json has no \"" + $kind + "\" key") end' \
      "$MATRIX")"
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

# Shared inputs every example is built against: the adapter itself, the layer wrapper,
# this workflow, and the two scripts every test job actually runs.
#
# Named individually rather than as .github/scripts/, which also holds
# check-example-config.sh — which no example is built against, so matching the whole
# directory would rebuild and boot all eighteen entries for a change to it.
if grep -qE '^(src/|layer/|Cargo\.toml$|Cargo\.lock$|\.github/workflows/examples\.yaml$|\.github/scripts/verify-http\.sh$|\.github/scripts/select-examples\.sh$|\.github/example-matrix\.json$)' <<<"$changed"; then
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
# the commit merged. "Select nothing" is the documented contract here, not "fail" — the
# `if: ... != '[]'` guards in examples.yaml skip the test jobs and the workflow is green.
if [[ -z "$example_paths" ]]; then
  echo "No example changed: nothing to verify."
  for kind in image zip stream; do
    echo "$kind=[]" >>"$GITHUB_OUTPUT"
  done
  exit 0
fi

names="$(cut -d/ -f2 <<<"$example_paths" | sort -u | jq -R . | jq -sc .)"
echo "Changed examples: $names"

# The matrix covers 18 of the ~46 examples dependabot.yml claims, so for most Dependabot
# pull requests every matrix comes out empty and examples-verified goes green having
# built and booted nothing. Failing instead would block those examples permanently, so
# say it out loud: a reviewer reading one green aggregate check cannot otherwise tell
# that the bump they are approving was never launched, because the per-example job names
# disappear when the matrix is filtered.
uncovered="$(jq -r --argjson names "$names" \
  '([.image, .zip, .stream] | flatten | map(.name)) as $covered
   | [$names[] | select(IN($covered[]) | not)] | join(", ")' "$MATRIX")"
if [[ -n "$uncovered" ]]; then
  echo "::warning::No matrix entry builds or boots: $uncovered — this run verifies templates only for them."
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "- **Not built or booted:** $uncovered (no \`.github/example-matrix.json\` entry)" \
      >>"$GITHUB_STEP_SUMMARY"
  fi
fi

for kind in image zip stream; do
  matrix="$(jq -c --argjson names "$names" "[.$kind[] | select(.name as \$n | \$names | index(\$n))]" "$MATRIX")"
  echo "$kind=$matrix"
  echo "$kind=$matrix" >>"$GITHUB_OUTPUT"
done
