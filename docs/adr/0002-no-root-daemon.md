# 0002: No root daemon or setuid helper

- Status: Accepted
- Date: 2026-10-27
- Deciders: project maintainer

## Context and problem statement

Installing and upgrading packages needs root for pacman's install step. A GUI could get that privilege in several ways: a root daemon that accepts requests over D-Bus, a setuid helper binary, or a short-lived escalation through sudo or polkit for each transaction. Each choice changes the attack surface of the whole system.

## Decision drivers

- Minimise the code that runs as root.
- Make the app's security story simple enough to audit.
- Work with any sudo or doas setup, not only one desktop environment.
- Avoid the class of bugs seen in privileged helpers, where an unprivileged caller can drive root actions.

## Considered options

- **Root daemon** with a D-Bus or socket API, as Pamac does.
- **Setuid helper binary** that wraps pacman.
- **Per-transaction escalation** through sudo, where yay runs in a PTY and the user types the password, or through polkit using yay's `--sudo` settings.

## Decision outcome

Chosen option: **per-transaction escalation only**. The app never runs as root, never ships a setuid helper and never ships a root daemon in v1. yay runs in a pseudo-terminal, and the sudo password prompt is answered by the user in a native field that writes into the PTY. A polkit mode may be added later, after validation.

### Consequences

- Good: no privileged long-running process to attack, and no privileged code in the app.
- Good: works with sudo and doas alike.
- Bad: each transaction may prompt for a password, and the PTY prompt parser must be reliable.
- Bad: if a serious need for a daemon emerges, this record must be revisited and the change reviewed as a security decision.

## More information

- PLAN.md, "Security model", "Privilege escalation". Pamac's root daemon is cited there as the attack surface to avoid.
- Hard rule 1: the app never runs as root.
