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

configured = set()
problems = []
config = yaml.safe_load(open(dependabot_path))
for update in config["updates"]:
    ecosystem = update["package-ecosystem"]

    # Both spellings are valid Dependabot config. This file uses the plural throughout,
    # but the singular is the canonical form for one directory and is what someone
    # adding an entry is likely to reach for — reading only the plural would report
    # their manifest as unclaimed and tell them to add an entry that is already there.
    directories = list(update.get("directories") or [])
    if "directory" in update:
        directories.append(update["directory"])
    for directory in directories:
        configured.add((ecosystem, directory))

    where = f"{ecosystem} {directories}"

    # Both keys below are load-bearing, and an entry copy-pasted without either looks
    # correct here while silently reverting that example to what this file exists to
    # prevent. Plain `groups` batches version updates only, so without
    # `applies-to: security-updates` the grouping does not apply to the advisories that
    # are the whole point.
    groups = update.get("groups") or {}
    if not any(g.get("applies-to") == "security-updates" for g in groups.values()):
        problems.append(f"{where}: needs a group with `applies-to: security-updates`, "
                        "or its security updates arrive one pull request per advisory.")

    # And a missing or non-zero limit turns routine version bumps back on for that one
    # example.
    if update.get("open-pull-requests-limit") != 0:
        problems.append(f"{where}: needs `open-pull-requests-limit: 0`, or version "
                        "updates come back on for it.")


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

matrix = json.load(open(matrix_path))
names = {entry["name"] for group in matrix.values() for entry in group}
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
