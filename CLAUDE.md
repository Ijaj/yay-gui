# CLAUDE.md

Guidance for AI agents working in this repo. Human contributors: see CONTRIBUTING.md.

## What this is

- aurdeck: pre-alpha unofficial GUI for the yay AUR helper on Arch Linux.
- Tauri 2 desktop app: Rust backend, React + TypeScript UI. Reads come from libalpm and the AUR RPC; every change goes through yay in a PTY.

## Layout

- `crates/core`: domain types, transaction planner, safety rules (no Tauri dependency)
- `crates/alpm-reader`: read-only libalpm queries
- `crates/aur-client`: AUR RPC v5 client, batching, caching
- `crates/yay-runner`: PTY spawning, output handling, prompt detection, transaction runner
- `crates/review`: AUR git fetch, PKGBUILD diffs, review history
- `apps/desktop`: Tauri 2 app root (Vite, package.json, biome, tests)
- `apps/desktop/src`: React UI. Dirs: `api/`, `components/`, `features/`, `i18n/`, `state/`, `styles/`, `test/`
- `apps/desktop/src-tauri/src`: Tauri backend. Files: `installed.rs`, `lib.rs`, `main.rs`, `plan.rs`, `review.rs`, `search.rs`, `state.rs`, `tx.rs`
- `docs/adr`: architecture decision records

## Commands

- `scripts/check.sh` runs everything. Jobs: `rust`, `ui`, `deny`.
- `scripts/check.sh rust`: `cargo fmt --all --check`, clippy `-D warnings` (workspace and the alpm docs-rs feature), `cargo test --workspace`.
- `scripts/check.sh ui`: `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`.
- `cargo test --workspace` alone is fine for quick crate work.
- Tauri crate: building it needs the WebKitGTK 4.1 dev packages (Arch: `webkit2gtk-4.1 gtk3 librsvg libayatana-appindicator libsoup3 base-devel`; Debian/Ubuntu list is in README).
- Tauri crate: its build script expects `apps/desktop/dist` to exist. Run `mkdir -p apps/desktop/dist` or `pnpm build` first.
- `scripts/check.sh deny` needs `cargo-deny`, which may not be installed. If `command -v cargo-deny` fails, do not install it silently. Check new dependency licences by hand against `deny.toml` and say so in your report.
- Debug builds run the program named in `AURDECK_YAY` instead of `yay`. Use `crates/yay-runner/tests/fixtures/fake-tx.sh` for run-flow tests.

## Conventions

- Rust: edition 2024, rust-version 1.88. `cargo fmt --all` before finishing.
- Clippy: `cargo clippy --workspace --all-targets -- -D warnings`. Warnings are errors.
- `unsafe` is forbidden (`unsafe_code = "forbid"` in the workspace lints). Do not add it.
- Commands: argument arrays only. No `sh -c`, no string concatenation.
- Validate package names against the pacman/AUR name pattern before use.
- TypeScript: Biome for format and lint (`pnpm lint`). `pnpm typecheck`, `pnpm test`, `pnpm build` must pass.
- Feature code calls the backend only through the typed wrappers in `apps/desktop/src/api/`. No direct Tauri `invoke` calls from features.
- Commits: Conventional Commits, e.g. `feat(aur-client): ...`, `fix(yay-runner): ...`. Types: feat, fix, docs, test, refactor, perf, build, ci, chore. Use `!` or a `BREAKING CHANGE:` footer for breaking changes.
- Behaviour changes need tests. Pull requests target `main`.

## Safety rules (non-negotiable)

Copied from README "Safety principles" and CONTRIBUTING "Safety checklist". Every change must respect them.

- Never run as root. The app refuses to start as UID 0. Nothing you add may run as root.
- No partial upgrades. Repo installs go through `-S --needed` after a full `-Syu`, or the app asks for an upgrade first. Never use `pacman -Sy` for update checks; repo update checks use `checkupdates`.
- No silent AUR builds. Every new AUR package and every AUR update shows its PKGBUILD or a diff since the last approved version before anything is built. The review cannot be skipped silently.
- No shell strings. Argument arrays only. Validate package names.
- Credentials are never stored: not on disk, in logs, or in the database. Sudo passwords go straight to the terminal and are wiped from memory. AUR account credentials, if ever used, go to the system keyring.

Agent rules on top of those:

- Never skip, weaken or delete a test to get green. Fix the code, or report the failure.
- Fixtures of yay/pacman output are recorded from real yay in the Arch container. Never invent them.
- Never run install, upgrade or remove paths anywhere except a disposable Arch container (`archlinux:base-devel`, non-root user). Not on the host.
- No agent reviews its own work. A different reviewer must check it.

## Two-approval paths

From CONTRIBUTING "Two-approval rule". Changes to these need two approving maintainer reviews before merge. An agent may write the change but must flag it as two-approval in the PR and must not merge it:

- `crates/yay-runner`
- review logic: `crates/review`, and PKGBUILD or diff handling
- privilege escalation: sudo, polkit, or any code that asks for a password
- anything that changes what is built or installed, including the PKGBUILD for the app itself

Review tiers for agent work (docs/COMPLETION_PLAN.md 3.3):

- **Safety**: `crates/yay-runner`, `crates/review`, the planner and safety rules in `crates/core`, anything that builds a command line, privilege escalation, user config files, `packaging/`. Two independent reviews (`/code-review` high + `/security-review`) on a top-tier model, then the owner merges.
- **Standard**: UI screens, read-only backend commands, docs, i18n. One `/code-review` at medium effort; the orchestrator may merge.
- Safety tests in `crates/core` are off-limits unless a brief owns them.

## Arch container (only place write paths may run)

- Verified Nov 15, 2026 in the cloud agent container (P0.2 owns the harness). Docker works there: start it with `dockerd &` if `docker info` fails.
- Outbound traffic goes through the agent proxy, so run containers with `--network host -e HTTPS_PROXY -e https_proxy` and add `/root/.ccr/ca-bundle.crt` as a trust anchor (`trust anchor --store`, then `update-ca-trust`).
- Allowed Arch hosts: `aur.archlinux.org`, `fastly.mirror.pkgbuild.com`, `geo.mirror.pkgbuild.com`. Anything else is refused; ask the orchestrator, do not route around it.
- Build and test as a non-root user inside the container; `makepkg` and the app both refuse root.
- `tests/arch/run.sh [repo-install|aur-install|remove|all]` builds the image (`tests/arch/Dockerfile`: non-root `dev` with a sudo password, yay) and runs `crates/arch-harness` inside it. It adds the proxy variables and CA by itself when `HTTPS_PROXY` is set. See `tests/arch/README.md`.

## Hot files (parallel-worker collisions)

These files collide when workers run in parallel. Only the package that owns one in its brief may edit it:

- `apps/desktop/src/i18n/en.json`
- `apps/desktop/src/api/types.ts`
- `apps/desktop/src/api/mock.ts`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/state.rs`

If your brief does not own one of these and you need a change in it, stop and report it.

## Worker rules

- Agent worktrees can start from a stale commit. First check `git log -1` matches the base commit named in your brief; if not, `git reset --hard <base>`.
- Stay inside the files your brief owns. Read the "files to read first" list before coding.
- Do not change the contract section of a brief (types, command signatures). Report a conflict instead.
- If the same check fails three times in a row, stop and report. Do not keep retrying.
- Do not commit unless the brief says to.
- Report in 15 lines or fewer: what changed, tests added, what you could not do, check results.
