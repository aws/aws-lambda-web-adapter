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
# A stale matrix name is quieter but worse: dependabot-automerge.sh treats a changed
# example as covered when the matrix names it, so a renamed or deleted example would be
# credited with coverage it does not have.
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

# Ecosystem -> manifest filenames Dependabot keys off. Only the ones present under
# examples/; add a row when an example introduces a new ecosystem.
MANIFESTS = {
    "npm": ["package.json"],
    "pip": ["requirements.txt"],
    "gomod": ["go.mod"],
    "maven": ["pom.xml"],
    "nuget": ["*.csproj"],
    "cargo": ["Cargo.toml"],
    "bundler": ["Gemfile"],
}

tracked = subprocess.run(
    ["git", "ls-files", "examples"], capture_output=True, text=True, check=True
).stdout.split()

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
config = yaml.safe_load(open(dependabot_path))
for update in config["updates"]:
    for directory in update.get("directories", []):
        configured.add((update["package-ecosystem"], directory))

problems = []

unclaimed = sorted(found - configured)
if unclaimed:
    problems.append(
        "These manifests have no matching entry in %s, so Dependabot will open one pull\n"
        "request per advisory for them, with a commit header Commit Lint rejects:\n%s"
        % (dependabot_path, "\n".join(f"  {eco}: {d}" for eco, d in unclaimed))
    )

# The reverse direction, limited to examples/: an entry for a directory that no longer
# has a manifest is dead config, and usually means an example was renamed.
stale = sorted(
    (eco, d) for eco, d in configured - found if d.startswith("/examples/")
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
