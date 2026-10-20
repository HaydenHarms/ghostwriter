#!/usr/bin/env bash
# Bring the latest WordCraft into Ghostwriter (docs/companion.md, "Keeping up with upstream").
#
#   scripts/update-from-wordcraft.sh            merge storytold/wordcraft main, then check
#   scripts/update-from-wordcraft.sh --no-check merge only
#
# Ghostwriter's own conflicts are settled automatically: its README, roadmap and agent files are
# kept; Cargo.toml takes upstream's with Ghostwriter's version and companion line put back;
# Cargo.lock is regenerated. Anything else that conflicts is a hook line: the script stops and
# lists it. On Windows, run it from Git Bash.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

check=1
[ "${1:-}" = "--no-check" ] && check=0

if [ -n "$(git status --porcelain)" ]; then
  echo "Commit or stash your changes first." >&2
  exit 1
fi

version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml)"
companion_line="$(grep -n '^default-members = .*# companion hook$' Cargo.toml | cut -d: -f2-)"

git fetch --no-tags https://github.com/storytold/wordcraft.git main:refs/remotes/upstream/main
behind="$(git rev-list --count HEAD..upstream/main)"
if [ "$behind" = 0 ]; then
  echo "Already up to date with WordCraft."
  exit 0
fi
echo "Merging $behind WordCraft commits…"

conflicted() { git diff --name-only --diff-filter=U; }

if ! git merge --no-edit upstream/main; then
  for f in README.md ROADMAP.md docs/roadmap.md AGENTS.md CLAUDE.md; do
    if conflicted | grep -qx "$f"; then
      git checkout --ours -- "$f" && git add -- "$f"
    fi
  done
  if conflicted | grep -qx Cargo.toml; then
    git checkout --theirs -- Cargo.toml
    git add Cargo.toml
  fi
  if conflicted | grep -qx Cargo.lock; then
    git checkout --theirs -- Cargo.lock
    git add Cargo.lock
  fi
fi

# Cargo.toml: Ghostwriter's version and the companion's line, whatever upstream did there.
if [ -n "$companion_line" ]; then
  grep -q '# companion hook' Cargo.toml || sed -i.bak "s|^default-members = .*|$companion_line|" Cargo.toml
  rm -f Cargo.toml.bak
fi
cargo xtask version set "$version" >/dev/null
git add Cargo.toml Cargo.lock

left="$(conflicted)"
if [ -n "$left" ]; then
  echo
  echo "These still conflict. Each is a hook line: keep WordCraft's version of the line and put the"
  echo "marked Ghostwriter line back (git grep -n \"companion hook\"), then git add and git commit:"
  echo "$left"
  exit 1
fi
if [ -f "$(git rev-parse --git-path MERGE_HEAD)" ]; then
  git commit --no-edit -q
elif ! git diff --cached --quiet; then
  git commit -q -m "Ghostwriter version and companion line after merging WordCraft"
fi

hooks="$(git grep -n "companion hook" -- '*.rs' '*.toml' | wc -l | tr -d ' ')"
echo "Merged. $hooks companion hook lines (docs/companion.md says how many there should be)."

if [ "$check" = 1 ]; then
  cargo build --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test -p wordcraft-engine -p wordcraft-mcp -p wordcraft-companion -p wordcraft-ui-egui
  echo "Built and tested. Push when ready: git push"
fi
