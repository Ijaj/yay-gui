# Work package brief: <PKG-ID>

<!-- Copy this file per work package. Replace every <...> and delete these comments. Keep it short. -->

## Package ID and title

<!-- e.g. P0.3 "Generate TypeScript types from Rust". One line. -->

`<PKG-ID>`: <title>

## Base

<!-- Commit the worktree must start from. Worker: if `git log -1` differs, run `git reset --hard <sha>` before anything else. -->

- Base commit: `<sha>`

## Tier

<!-- Tiers are defined in docs/COMPLETION_PLAN.md 3.3 and CLAUDE.md. Safe = say which path makes it safety tier. -->

- Tier: <Std | Safe>
- Arch container needed? <yes | no>. <!-- yes if any install/upgrade/remove path or real yay/pacman output is involved -->

## Goal

<!-- 2-4 lines: what this package delivers and why. -->

<goal>

## Files owned (may edit)

<!-- Exact paths. Hot files (i18n/en.json, api/types.ts, api/mock.ts, tauri lib.rs, tauri state.rs) only if listed here. -->

- `<path>`

## Files to read first

<!-- 8 at most. -->

1. `<path>`

## Contract (already written, do not change)

<!-- Types and command signatures this package must satisfy. Paste them; do not edit them. -->

```
<types / signatures>
```

## Acceptance tests

<!-- Test names that must exist and pass. Do not weaken or delete them. -->

- `<test_name>` in `<path>`

## Commands to run (gate)

<!-- The checks that must pass before you report. -->

```sh
scripts/check.sh rust
scripts/check.sh ui
```

## Non-goals

<!-- What this package must not do. -->

- <non-goal>

## Stop rules

- Same check fails 3 times in a row: stop and report. Do not keep retrying.
- Stay inside the files listed under "Files owned". Report any need outside them.
- Never skip, weaken or delete a test to get green.
- Fixtures of yay/pacman output come from real yay in the Arch container, never invented.
- Never run install, upgrade or remove paths outside a disposable Arch container.
- Do not review your own work.

## Report format

<!-- 15 lines max. -->

1. What changed (files)
2. Tests added (names)
3. What you could not do, and why
4. Check results (each gate command: pass or fail)
