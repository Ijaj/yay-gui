#!/usr/bin/env bash
# Runs the same checks as .github/workflows/ci.yml, locally.
#
#   scripts/check.sh          everything
#   scripts/check.sh rust     formatting, clippy, tests
#   scripts/check.sh deny     cargo deny (supply chain)
#   scripts/check.sh ui       lint, typecheck, tests, build of apps/desktop
#
# Needs: Rust (see rust-version in Cargo.toml), Node.js 22, pnpm, cargo-deny
# (`cargo install cargo-deny --locked`) and the WebKitGTK dev packages (README).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

step() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }

rust() {
  # The Tauri crate's build script needs the frontend dist folder to exist.
  mkdir -p apps/desktop/dist
  step "cargo fmt --check"
  cargo fmt --all --check
  step "cargo clippy"
  cargo clippy --workspace --all-targets -- -D warnings
  step "cargo test"
  cargo test --workspace
}

deny() {
  if ! command -v cargo-deny >/dev/null; then
    echo "cargo-deny is not installed: cargo install cargo-deny --locked" >&2
    exit 1
  fi
  step "cargo deny check"
  cargo deny check
}

ui() {
  cd "$root/apps/desktop"
  step "pnpm install"
  pnpm install --frozen-lockfile
  step "pnpm lint"
  pnpm lint
  step "pnpm typecheck"
  pnpm typecheck
  step "pnpm test"
  pnpm test
  step "pnpm build"
  pnpm build
  cd "$root"
}

jobs=("$@")
[ ${#jobs[@]} -eq 0 ] && jobs=(rust deny ui)
for job in "${jobs[@]}"; do
  case "$job" in
    rust | deny | ui) "$job" ;;
    *)
      echo "unknown job: $job (use rust, deny or ui)" >&2
      exit 2
      ;;
  esac
done
printf '\n\033[1;32mAll checks passed.\033[0m\n'
