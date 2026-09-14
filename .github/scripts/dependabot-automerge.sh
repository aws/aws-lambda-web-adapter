#!/usr/bin/env bash
#
# Merges one Dependabot pull request, if it is an example-only update that Verify
# Examples has verified at the pull request's current head.
#
# Usage: REPO=<owner/repo> dependabot-automerge.sh <pr-number>
#
# Two triggers call this (see ../workflows/dependabot-automerge.yaml): a completed
# Verify Examples run, and an hourly sweep. The sweep exists because the checks on a
# pull request settle in arbitrary order — this gate used to be evaluated exactly once,
# when Verify Examples finished, so a pull request whose other checks were still running
# at that instant was skipped and never looked at again. Nothing else would have
# re-triggered it: Dependabot only pushes a branch when it rebases or recreates it, so
# an idle pull request could wait indefinitely.
#
# The run is resolved here rather than taken from an event payload, so both triggers
# behave identically and the verified commit is always the head that would be merged.
#
# Every not-yet or unresolvable condition exits 0 with a reason, and records it in the
# job summary so a skipped merge is visible instead of buried in a log. Refusing is the
# safe outcome; a red run here would be indistinguishable from a real fault.
set -euo pipefail

PR="${1:?usage: dependabot-automerge.sh <pr-number>}"
: "${REPO:?REPO must be set}"

# Joins a multi-line list onto one line for a message, without a trailing separator.
join_list() {
  tr '\n' ' ' <<<"$1" | sed 's/  *$//'
}

skip() {
  echo "PR #$PR: $*"
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "- **PR #$PR not merged** — $*" >>"$GITHUB_STEP_SUMMARY"
  fi
  exit 0
}

author=$(gh pr view "$PR" --repo "$REPO" --json author -q .author.login)
if [[ "$author" != "app/dependabot" && "$author" != "dependabot[bot]" ]]; then
  skip "authored by $author, not Dependabot."
fi

head_sha=$(gh pr view "$PR" --repo "$REPO" --json headRefOid -q .headRefOid)

# A run for a superseded commit proves nothing about what would be merged, and
# Dependabot force-pushes these branches whenever it rebases.
run_id=$(gh api \
  "repos/$REPO/actions/workflows/examples.yaml/runs?head_sha=$head_sha&status=success&per_page=1" \
  -q '.workflow_runs[0].id // empty')
if [[ -z "$run_id" ]]; then
  skip "no successful Verify Examples run for $head_sha (yet)."
fi

# Verify Examples is not the only check. Commit Lint runs on every pull request with no
# path filter and does go red on Dependabot pull requests (#799), and any check added
# later would otherwise be ignored here too. Anything not green — including still
# running — means leave it alone; the sweep will look again.
#
# This workflow's own run is excluded defensively: workflow_run runs do not appear in a
# pull request's check rollup today, but if that changed, its in-progress state would
# deadlock every merge.
not_green=$(gh pr view "$PR" --repo "$REPO" --json statusCheckRollup -q '
  .statusCheckRollup[]
  | select((.workflowName // "") != "Dependabot Auto-merge")
  | select([((.conclusion // .state // "PENDING") | ascii_upcase)]
           - ["SUCCESS", "SKIPPED", "NEUTRAL"] | length > 0)
  | ((.name // .context) + " = " + (.conclusion // .state // "PENDING"))')
if [[ -n "$not_green" ]]; then
  skip "checks are not all green: $(join_list "$not_green")"
fi

# Fail closed: an empty file list must never read as "nothing outside examples/". The
# API call is kept out of the pipeline so only grep's no-match status is tolerated.
files=$(gh api --paginate "repos/$REPO/pulls/$PR/files" -q '.[].filename')
if [[ -z "$files" ]]; then
  skip "could not list its files."
fi

# Scope by what the pull request changes rather than by its branch name: grouped updates
# do not reliably encode the directory in the ref. Examples are demo apps, so a bad bump
# costs a broken sample; the adapter's own dependencies, the workflows and the templates
# stay manual.
outside=$(grep -v '^examples/' <<<"$files" || true)
if [[ -n "$outside" ]]; then
  skip "changes files outside examples/: $(join_list "$outside")"
fi

changed_examples=$(cut -d/ -f2 <<<"$files" | sort -u)

# Refuse to merge an example that was not built and booted. Proof comes from the run's
# own job names, so a run that predates a matrix change cannot be credited with
# verifying an example it never launched. If GitHub ever changes how matrix jobs are
# named, this stops finding matches and merges stop — fail-closed, and visible in the
# summary rather than silent.
verified_examples=$(gh api "repos/$REPO/actions/runs/$run_id/jobs" --paginate \
  -q '.jobs[] | select(.name | startswith("test-")) | select(.conclusion == "success") | .name' \
  | sed -E 's/^test-[a-z]+ \(([^,)]+).*/\1/' | sort -u)
unverified=$(comm -23 <(printf '%s\n' "$changed_examples") <(printf '%s\n' "$verified_examples") || true)
if [[ -n "$unverified" ]]; then
  # Distinguish "not covered by the matrix at all" from "covered but not run", because
  # the fixes differ: add a matrix entry, versus re-run the verification.
  if ! matrix_json=$(gh api "repos/$REPO/contents/.github/example-matrix.json" \
      -H 'Accept: application/vnd.github.raw'); then
    skip "could not read .github/example-matrix.json."
  fi
  if ! covered=$(jq -r '[.[][].name] | unique | .[]' <<<"$matrix_json" | sort -u); then
    skip "could not parse .github/example-matrix.json."
  fi
  uncovered=$(comm -23 <(printf '%s\n' "$unverified") <(printf '%s\n' "$covered") || true)
  if [[ -n "$uncovered" ]]; then
    skip "no build-and-boot coverage for: $(join_list "$uncovered") — add them to .github/example-matrix.json, or review by hand."
  fi
  skip "these examples are in the matrix but were not verified by run $run_id: $(join_list "$unverified")"
fi

echo "PR #$PR is example-only and verified at $head_sha by run $run_id. Merging."

# --match-head-commit closes the remaining window: if the branch moves between the
# lookups above and this call, the API rejects the merge rather than applying it to an
# unverified commit.
#
# A rejection exits non-zero, which would turn this run red like a real failure. The
# common cause is benign: sibling pull requests touching one lockfile finish minutes
# apart, the first merge conflicts the rest, and GitHub has not necessarily recomputed
# mergeability yet. Distinguish that from a genuine problem — squash merges disabled, a
# missing permission — so this workflow's red/green state still means something.
if gh pr merge "$PR" --repo "$REPO" --squash --delete-branch \
    --match-head-commit "$head_sha"; then
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "- **PR #$PR merged** — verified at \`${head_sha:0:8}\` by run $run_id" >>"$GITHUB_STEP_SUMMARY"
  fi
  exit 0
fi

head_after=$(gh pr view "$PR" --repo "$REPO" --json headRefOid -q .headRefOid || echo unknown)
state=$(gh pr view "$PR" --repo "$REPO" --json mergeStateStatus -q .mergeStateStatus || echo UNKNOWN)
if [[ "$head_after" != "$head_sha" ]]; then
  skip "merge rejected, head moved to $head_after."
fi
case "$state" in
  DIRTY | BLOCKED | BEHIND | DRAFT | UNKNOWN)
    skip "merge rejected, not mergeable (mergeStateStatus=$state) — most likely a sibling update landed first."
    ;;
esac
echo "PR #$PR: unexpected merge failure (mergeStateStatus=$state)."
echo "Nothing explains it, so failing loudly rather than hiding it."
exit 1
