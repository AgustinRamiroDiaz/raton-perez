#!/usr/bin/env bash
# One command that says whether the repo is in a mergeable state: the same
# checks the "code quality" workflow runs, plus a TypeScript type-check.
# Usage: scripts/validate.sh   (run `bun install` first)
set -euo pipefail

cd "$(dirname "$0")/.."

step() {
  printf '\n==> %s\n' "$1"
  shift
  "$@"
}

step "Translation consistency" bun run check:translations
step "Model language coverage" bun run check:model-languages
step "Bundled model catalog" bun run check:catalog
step "Keyboard utilities" bun run test:keyboard
step "ESLint" bun run lint
step "TypeScript type-check" bunx tsc --noEmit
step "Formatting (Prettier + cargo fmt)" bun run format:check

printf '\nAll checks passed.\n'
