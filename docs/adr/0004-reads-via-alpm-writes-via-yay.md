# 0004: Read data through libalpm and the AUR RPC; make every change through yay

- Status: Accepted
- Date: 2026-10-27
- Deciders: project maintainer

## Context and problem statement

YayGUI must show package data quickly and perform installs, upgrades and removals. yay's human-readable output changes between releases and is hard to parse reliably. Re-implementing dependency solving or AUR building would duplicate yay and risk new bugs.

## Decision drivers

- Search results for repository packages should return in under 300 ms.
- Avoid depending on yay's output format for anything except progress.
- Do not re-implement dependency solving or AUR building.
- Keep a single, auditable path that changes the system.

## Considered options

- **Parse yay's output** for every read, including search and info.
- **Re-implement package management** in the app, using libalpm for writes and its own AUR builder.
- **Split:** reads from libalpm (read-only) and the AUR RPC v5, and all writes through yay in a PTY.

## Decision outcome

Chosen option: **the split**. Search, details, installed lists and dependency trees come from libalpm, read-only and without root, and from the AUR RPC. Update checks use `checkupdates` on a temporary database copy, never `pacman -Sy`. Every system change is a single yay invocation built from an argument array, for example `yay -S --needed --answerdiff None --answeredit None pkg1 pkg2` after the review step, run in a real PTY. The backend watches pacman's lock file and explains when another tool holds it.

### Consequences

- Good: fast reads, and no dependence on yay's text output except for progress.
- Good: yay remains the only thing that solves dependencies and builds AUR packages.
- Good: one serial write path that is easy to test with recorded fixtures.
- Bad: two data sources (libalpm and the AUR RPC) must be kept consistent.
- Bad: yay output still needs prompt detection, which is the riskiest parser (see PLAN.md risks).

## More information

- PLAN.md, "Architecture" and "Key design decisions".
- Related: ADR 0002 (no root daemon), since yay runs in the user's session and escalates only for pacman's install step.
