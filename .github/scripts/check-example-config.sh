#!/usr/bin/env bash
#
# Asserts that the two hand-maintained lists describing the examples have not drifted
# from the tree:
#
#   1. Every dependency manifest under examples/ is claimed by an entry in
#      .github/dependabot.yml, for its own ecosystem.
#   2. Every name in .github/example-matrix.json is a real directory under examples/.
#
# Why this is a check and not a note in a comment. An unclaimed manifest does not merely
# lose grouping: it silently reverts to Dependabot's default behavior — one pull request
# per advisory per manifest, which is the thirteen-against-one-lockfile pattern the
# config exists to fix — with a default commit header that fails commitlint's type-enum.
# That regression is invisible until the pull requests appear, weeks later, and with 47
# entries covering 53 manifest directories the drift is a matter of when, not if.
# examples/fastmcp and examples/sveltekit-ssr-zip are recent evidence that examples get
# added regularly.
#
# A stale matrix name is quieter: the selector would keep choosing an example that no
# longer exists, and its job would fail on a missing working directory rather than on
# anything to do with the change under review.
set -euo pipefail

cd "$(dirname "$0")/../.."

DEPENDABOT=.github/dependabot.yml
MATRIX=.github/example-matrix.json

python3 - "$DEPENDABOT" "$MATRIX" <<'PY'
import fnmatch
import json
import re
import subprocess
import sys

dependabot_path, matrix_path = sys.argv[1], sys.argv[2]

try:
    import yaml
except ImportError:  # pragma: no cover - only on a runner without PyYAML
    sys.exit("PyYAML is required: pip install pyyaml")

# Ecosystem -> every manifest filename Dependabot keys off for it. One name per
# ecosystem is not enough: an example shipping only a pyproject.toml is still pip, and
# missing that name would break the guard in both directions — the drift would pass
# unnoticed, and a maintainer who added the correct entry for it would be told the entry
# is stale.
#
# Ecosystems with no example today are listed anyway, so the next example that
# introduces one is caught rather than silently unguarded.
#
# Deliberately absent: docker. There are 24 Dockerfiles under examples/, but Dependabot
# alerts do not cover base images, so there is nothing for a security-updates entry to
# group — including it here would fail this check for entries that should not exist.
MANIFESTS = {
    "npm": ["package.json"],
    "pip": ["requirements.txt", "pyproject.toml", "Pipfile", "setup.py"],
    "gomod": ["go.mod"],
    "maven": ["pom.xml"],
    "gradle": ["build.gradle", "build.gradle.kts"],
    "nuget": ["*.csproj", "*.fsproj", "*.vbproj", "packages.config"],
    "cargo": ["Cargo.toml"],
    "bundler": ["Gemfile", "*.gemspec"],
    "composer": ["composer.json"],
}

# -z with a NUL split, not .split(): git ls-files prints a path containing a space
# verbatim, so whitespace splitting tears "examples/x/my app/package.json" into fragments
# and the tail one yields a directory ("/app") that no entry can ever claim — validate
# failing on a correct config. -z also turns off git's C-style quoting of non-ASCII
# paths, which would corrupt the derived directory the same way.
tracked = [
    path
    for path in subprocess.run(
        ["git", "ls-files", "-z", "examples"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split("\0")
    if path
]

# (ecosystem, directory) pairs the tree actually contains.
found = set()
for path in tracked:
    parts = path.split("/")
    if len(parts) < 2:
        continue
    directory = "/" + "/".join(parts[:-1])
    for ecosystem, patterns in MANIFESTS.items():
        if any(fnmatch.fnmatch(parts[-1], pattern) for pattern in patterns):
            found.add((ecosystem, directory))

# commitlint.config.js is the source of truth for the accepted types; parsed rather than
# duplicated so this check cannot drift from the linter. An unparseable file yields an
# empty set, which downgrades the assertion to "a prefix is set".
COMMITLINT_HEADER_MAX = 120
COMMITLINT_TYPES = set()
try:
    config_js = open("commitlint.config.js").read()
    enum = re.search(r"['\"]type-enum['\"]\s*:\s*\[[^\[]*\[(.*?)\]", config_js, re.S)
    if enum:
        COMMITLINT_TYPES = set(re.findall(r"['\"]([a-z]+)['\"]", enum.group(1)))
    # The last number in the rule, not the first: the array is [severity, applicability,
    # value], so a lazy match returns the severity (2) and every header looks over budget.
    header_max = re.search(
        r"['\"]header-max-length['\"]\s*:\s*\[\s*\d+\s*,\s*['\"]\w+['\"]\s*,\s*(\d+)",
        config_js,
    )
    # A sanity floor for the same reason: a parse that yields a severity rather than a
    # length would fail every entry, and a guard that cries wolf is worse than no guard.
    if header_max and int(header_max.group(1)) >= 40:
        COMMITLINT_HEADER_MAX = int(header_max.group(1))
except OSError:
    pass

configured = set()
problems = []
config = yaml.safe_load(open(dependabot_path))
for update in config["updates"]:
    ecosystem = update["package-ecosystem"]

    # Both spellings are valid Dependabot config. This file uses the plural throughout,
    # but the singular is the canonical form for one directory and is what someone
    # adding an entry is likely to reach for — reading only the plural would report
    # their manifest as unclaimed and tell them to add an entry that is already there.
    # Normalized on collection: Dependabot resolves "/examples/fastapi/app/" and
    # "/examples/fastapi/app" to the same manifest, but comparing the strings verbatim
    # reported the first as an entry with no manifest *and* the manifest as having no
    # entry — two contradictory problems for a config that works, which is the failure
    # already fixed here twice, for the singular `directory` key and for globs. Stripping
    # only slashes leaves glob patterns alone. It also lets the duplicate check see
    # `/examples/x` and `/examples/x/` as the collision Dependabot rejects the file over.
    def normalize(value):
        stripped = value.strip("/")
        return "/" + stripped if stripped else "/"

    directories = [normalize(d) for d in (update.get("directories") or [])]
    if "directory" in update:
        directories.append(normalize(update["directory"]))

    where = f"{ecosystem} {directories}"

    # `configured` is a set, so a directory claimed twice for one ecosystem would collapse
    # into one member and every assertion built on it would still pass — the one drift mode
    # this guard could not see, and the likeliest one in a file of 47 near-identical
    # entries where the copy that forgets to change the directory is as plausible as the
    # copy that forgets a key. Dependabot itself rejects it ("Update configs must have a
    # unique combination of 'package-ecosystem', 'directory', and 'target-branch'"), so
    # GitHub's own config check would go red — but it names neither entry, while this one
    # can.
    for directory in directories:
        key = (ecosystem, directory)
        if key in configured:
            problems.append(
                f"{where}: {directory} is claimed more than once for {ecosystem}. "
                "Dependabot requires a unique ecosystem/directory pair and rejects the "
                "whole file otherwise, at which point none of it applies."
            )
        configured.add(key)

    # schedule.interval is required for an updates entry, and its absence is the worst of
    # the copy-paste failures: Dependabot rejects the whole file, so all 47 groups stop
    # applying at once and every example reverts to one pull request per advisory. Asserted
    # for every entry, not just examples — an invalid root cargo or github-actions entry
    # invalidates the file just the same.
    if not (update.get("schedule") or {}).get("interval"):
        problems.append(f"{where}: needs `schedule.interval`; without it Dependabot "
                        "rejects the whole config and none of the grouping applies.")

    # The two grouping assertions below apply to example entries only. Everything else in
    # this script is scoped to examples/ — `found` comes from `git ls-files examples`, the
    # stale check filters on the prefix — and applying them to the root cargo and
    # github-actions entries would turn an examples-drift guard into a repository-wide
    # policy lock: enabling version updates for the adapter's own crates is a normal thing
    # to want, has nothing to do with example grouping, and would fail validate with a
    # message that does not hint at editing this script.
    is_example = any(directory.startswith("/examples/") for directory in directories)

    # Load-bearing for an example: an entry copy-pasted without either key looks correct
    # here while silently reverting that example to what this file exists to prevent.
    # Plain `groups` batches version updates only, so without
    # `applies-to: security-updates` the grouping does not apply to the advisories that
    # are the whole point.
    groups = update.get("groups") or {}
    if is_example and not any(
        g.get("applies-to") == "security-updates" for g in groups.values()
    ):
        problems.append(f"{where}: needs a group with `applies-to: security-updates`, "
                        "or its security updates arrive one pull request per advisory.")

    # And a missing or non-zero limit turns routine version bumps back on for that one
    # example.
    if is_example and update.get("open-pull-requests-limit") != 0:
        problems.append(f"{where}: needs `open-pull-requests-limit: 0`, or version "
                        "updates come back on for it.")

    prefix = (update.get("commit-message") or {}).get("prefix")

    # `patterns` is the other half of the grouping claim: a group with
    # patterns: ["lodash"] satisfies the applies-to assertion above while leaving every
    # other advisory for that example ungrouped, which is the state this file exists to
    # prevent.
    if is_example:
        for group_name, group in groups.items():
            if group.get("applies-to") != "security-updates":
                continue
            if group.get("patterns") != ["*"]:
                problems.append(
                    f"{where}: group {group_name!r} needs `patterns: [\"*\"]`, or "
                    "advisories outside the pattern arrive one pull request each."
                )

            # dependabot.yml records why every group is named `security`: the commit
            # header is built from the group name and the directory, and commitlint caps
            # it at 120. A group named after its example produced 137. Nothing checked
            # that, so the next example could reintroduce it — headroom is 11 characters
            # at the longest directory configured today.
            for directory in directories:
                # Built from this entry's own prefix, and deliberately pessimistic in two
                # ways: it assumes the `(deps)` scope and a three-digit update count.
                #
                # The scope is why this reads 6 characters longer than the 137 the comment
                # above cites, which was measured without it. Dependabot infers a
                # conventional-commit scope from history — every existing Dependabot pull
                # request here is titled `chore(deps): ...` — and an explicit prefix
                # without `include: scope` should drop it, but being wrong in the strict
                # direction costs a shortened group name, while being wrong in the lax
                # direction costs a red Commit Lint on a pull request nobody wrote, weeks
                # later. That is the whole point of this assertion. It still leaves 9
                # characters of headroom at the longest directory configured today.
                header = (f"{prefix or 'chore'}(deps): bump the {group_name} group in "
                          f"{directory} with 100 updates")
                if len(header) > COMMITLINT_HEADER_MAX:
                    problems.append(
                        f"{where}: group {group_name!r} makes a "
                        f"{len(header)}-character commit header for {directory}, over "
                        f"commitlint's {COMMITLINT_HEADER_MAX}; use a shorter group name."
                    )

    # The third load-bearing key, and the one this repository has already paid for:
    # without a prefix Dependabot writes "bump <dep> from x to y", which has no
    # conventional type, and Commit Lint runs on every pull request with no path filter.
    # #799 is the evidence. Same shape of regression as the two above — the entry looks
    # fine here and the bill arrives weeks later on a pull request nobody wrote.
    #
    # The accepted types are read from commitlint.config.js rather than copied, so this
    # cannot disagree with the linter that actually runs. If that file's shape changes the
    # list comes back empty and the assertion falls back to "a prefix is set", which is
    # the part that matters; a wrong-but-present prefix would then be caught by Commit
    # Lint on the pull request that adds the entry.
    if not prefix:
        problems.append(f"{where}: needs `commit-message.prefix`, or Commit Lint rejects "
                        "the header Dependabot generates.")
    elif COMMITLINT_TYPES and prefix not in COMMITLINT_TYPES:
        problems.append(f"{where}: `commit-message.prefix: {prefix}` is not one of "
                        f"commitlint's types ({', '.join(sorted(COMMITLINT_TYPES))}).")


def directory_matches(pattern, directory):
    """Whether a `directories` value covers a directory.

    Globs are a supported form of the key. This config avoids them only because
    Dependabot refuses several entries for one ecosystem when it cannot prove they do not
    overlap — which does not apply to a single-entry ecosystem, so a maintainer may well
    write one. Comparing the values as literal strings reported the covered manifests as
    unclaimed *and* the pattern as stale, both wrong at once.

    `*` and `?` stay within one path segment and `**` spans several, matching Dependabot's
    globbing rather than fnmatch's, which would let `*` cross a `/` and so pass over
    exactly the drift this check exists to catch.
    """
    if not any(character in pattern for character in "*?["):
        return pattern == directory

    parts = pattern.strip("/").split("/")
    segments = directory.strip("/").split("/")

    def walk(p, s):
        while p < len(parts):
            if parts[p] == "**":
                if p + 1 == len(parts):
                    return True
                return any(walk(p + 1, k) for k in range(s, len(segments) + 1))
            if s >= len(segments) or not fnmatch.fnmatch(segments[s], parts[p]):
                return False
            p, s = p + 1, s + 1
        return s == len(segments)

    return walk(0, 0)


unclaimed = sorted(
    (ecosystem, directory)
    for ecosystem, directory in found
    if not any(
        eco == ecosystem and directory_matches(pattern, directory)
        for eco, pattern in configured
    )
)
if unclaimed:
    problems.append(
        "These manifests have no matching entry in %s, so Dependabot will open one pull\n"
        "request per advisory for them, with a commit header Commit Lint rejects:\n%s"
        % (dependabot_path, "\n".join(f"  {eco}: {d}" for eco, d in unclaimed))
    )

# The reverse direction, limited to examples/: an entry for a directory that no longer
# has a manifest is dead config, and usually means an example was renamed.
stale = sorted(
    (ecosystem, pattern)
    for ecosystem, pattern in configured
    if pattern.startswith("/examples/")
    and not any(
        eco == ecosystem and directory_matches(pattern, directory)
        for eco, directory in found
    )
)
if stale:
    problems.append(
        "These %s entries point at directories with no matching manifest:\n%s"
        % (dependabot_path, "\n".join(f"  {eco}: {d}" for eco, d in stale))
    )

# The keys each job kind interpolates. Exact sets, not minimums, because an unexpected
# key is almost always a typo of an expected one — and a typo is worse than an omission
# here: verify-http.sh skips the body assertion entirely when the expectation is empty
# (`[ -z "$EXPECT_BODY" ] ||`), so `expect_bodY` would leave the job green while checking
# only the status code. A missing `port` is merely noisy by comparison: test-zip
# interpolates it into PORT= and the app does not listen where the verify step looks, so
# the run burns its 90-second deadline and fails with "expectation not met", which reads
# as a broken example. Only `stream` fails clearly today, via the `*)` arm on its `kind`.
MATRIX_KEYS = {
    "image": {"name", "path", "expect_body"},
    "zip": {"name", "path", "expect_body", "port"},
    "stream": {"name", "kind", "path", "expect_body"},
}

matrix = json.load(open(matrix_path))

# Both directions. A key no job reads is dead weight; a kind a job reads that the file
# does not have is worse — select-examples.sh derives its loops from this file, so that
# job would get no output line, and `!= '[]'` is true for the empty string.
unknown_kinds = sorted(set(matrix) - set(MATRIX_KEYS))
if unknown_kinds:
    problems.append(
        "%s has kinds no job consumes: %s" % (matrix_path, ", ".join(unknown_kinds))
    )

missing_kinds = sorted(set(MATRIX_KEYS) - set(matrix))
if missing_kinds:
    problems.append(
        "%s is missing kinds a job reads: %s" % (matrix_path, ", ".join(missing_kinds))
    )

for kind, entries in matrix.items():
    expected = MATRIX_KEYS.get(kind)
    if expected is None:
        continue
    for entry in entries:
        absent = expected - entry.keys()
        extra = entry.keys() - expected
        label = entry.get("name", "<unnamed>")
        if absent:
            problems.append(
                f"{matrix_path} {kind} entry {label!r} is missing "
                f"{', '.join(sorted(absent))}."
            )
        if extra:
            problems.append(
                f"{matrix_path} {kind} entry {label!r} has keys no {kind} job reads: "
                f"{', '.join(sorted(extra))} — a typo of an expected key would leave the "
                "assertion silently unset."
            )

names = {entry["name"] for group in matrix.values() for entry in group if "name" in entry}
import os

missing = sorted(n for n in names if not os.path.isdir(os.path.join("examples", n)))
if missing:
    problems.append(
        "These %s names are not directories under examples/:\n%s"
        % (matrix_path, "\n".join(f"  {n}" for n in missing))
    )

if problems:
    print("\n\n".join(problems))
    sys.exit(1)

print(
    f"{len(found)} manifest directories under examples/ are all claimed by "
    f"{dependabot_path}, and every {matrix_path} name exists."
)
PY
