#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/seed-project-agents.sh projects/<name>.dxdoc" >&2
  exit 2
fi

project_dir="$1"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
template="$repo_root/templates/project/AGENTS.md"

if [[ ! -f "$template" ]]; then
  echo "missing template: $template" >&2
  exit 1
fi

mkdir -p "$project_dir"
if [[ -f "$project_dir/AGENTS.md" ]]; then
  echo "exists $project_dir/AGENTS.md"
  exit 0
fi

cp "$template" "$project_dir/AGENTS.md"
echo "seeded $project_dir/AGENTS.md"
